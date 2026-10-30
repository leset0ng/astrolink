//! Native Feature lifecycle shared by the measured Band 11 and ferry hosts.
use super::js::{
    self, clear_exception, define, duplicate, free, integer, Context, Shared, Value, UNDEFINED,
};
use super::*;
use crate::abi;
use core::{
    cell::UnsafeCell,
    ffi::{c_char, c_void},
    mem, ptr,
    sync::atomic::{AtomicBool, Ordering},
};

const OWNER_COUNT: usize = 4;
const NAME: &[u8] = b"canopus.astrolink\0";

macro_rules! fw {
    ($address:expr, $ty:ty) => {
        mem::transmute::<usize, $ty>($address | 1)
    };
}
#[cfg(not(any(
    feature = "target-xiaomi-band-10-pro-3-101-036",
    feature = "target-xiaomi-band-10-pro-3-101-043"
)))]
#[path = "framework.rs"]
mod host;
#[cfg(any(
    feature = "target-xiaomi-band-10-pro-3-101-036",
    feature = "target-xiaomi-band-10-pro-3-101-043"
))]
#[path = "ferry.rs"]
mod host;
pub use host::install;

#[repr(C)]
struct Lifecycle {
    registered: Option<unsafe extern "C" fn(*const c_char)>,
    create: unsafe extern "C" fn(Context, usize),
    required: unsafe extern "C" fn(Context, usize),
    released: Option<unsafe extern "C" fn(Context, usize)>,
    destroy: unsafe extern "C" fn(Context, usize),
    final_hook: Option<unsafe extern "C" fn(Context, usize)>,
}
#[repr(C)]
struct Description {
    version: u32,
    name: *const c_char,
    interface_name: *const c_char,
    reserved: u32,
    lifecycle: *const Lifecycle,
    member_count: u32,
    members: *const c_void,
}
unsafe impl Sync for Description {}
static LIFECYCLE: Lifecycle = Lifecycle {
    registered: None,
    create: on_create,
    required: on_required,
    released: None,
    destroy: on_destroy,
    final_hook: None,
};
static DESCRIPTION: Description = Description {
    version: 1,
    name: NAME.as_ptr().cast(),
    interface_name: NAME.as_ptr().cast(),
    reserved: 0,
    lifecycle: &LIFECYCLE,
    member_count: 0,
    members: ptr::null(),
};

#[derive(Clone, Copy)]
struct Owner {
    id: u32,
    context: Context,
    prototype: usize,
    endpoint: *mut Notification,
    callback: Value,
    queued: bool,
}
impl Owner {
    const EMPTY: Self = Self {
        id: 0,
        context: ptr::null_mut(),
        prototype: 0,
        endpoint: ptr::null_mut(),
        callback: UNDEFINED,
        queued: false,
    };
}
// The async handle is initialized and closed only by its QJS loop. The
// duplicated kernel file is independent of every producer's descriptor table.
#[repr(C, align(8))]
struct Notification {
    async_handle: [u32; abi::FEATURE_ASYNC_WORDS],
    wake: crate::loop_dispatch::PipeWake,
}
unsafe fn create_notification(manager: usize, owner: u32) -> *mut Notification {
    let stock_async = ptr::read_unaligned((manager + abi::MANAGER_ASYNC) as *const usize);
    if stock_async == 0 {
        return ptr::null_mut();
    }
    let loop_pointer = ptr::read_unaligned((stock_async + abi::HANDLE_LOOP) as *const usize);
    if loop_pointer == 0 {
        return ptr::null_mut();
    }
    let endpoint = fw!(
        abi::F_ALLOC,
        unsafe extern "C" fn(usize) -> *mut Notification
    )(mem::size_of::<Notification>());
    if endpoint.is_null() {
        return endpoint;
    }
    ptr::write(
        endpoint,
        Notification {
            async_handle: [0; abi::FEATURE_ASYNC_WORDS],
            wake: crate::loop_dispatch::PipeWake::EMPTY,
        },
    );
    (*endpoint).async_handle[0] = owner;
    let result = fw!(
        abi::F_UV_ASYNC_INIT,
        unsafe extern "C" fn(
            usize,
            *mut Notification,
            unsafe extern "C" fn(*mut Notification),
        ) -> i32
    )(loop_pointer, endpoint, notification_ready);
    if result != 0 {
        fw!(abi::F_FREE, unsafe extern "C" fn(*mut Notification))(endpoint);
        return ptr::null_mut();
    }
    if (*endpoint).wake.capture_current(loop_pointer).is_err() {
        close_notification(endpoint);
        return ptr::null_mut();
    }
    endpoint
}
unsafe extern "C" fn notification_ready(endpoint: *mut Notification) {
    drain(1, (*endpoint).async_handle[0] as usize);
}
unsafe fn close_notification(endpoint: *mut Notification) {
    fw!(
        abi::F_UV_CLOSE,
        unsafe extern "C" fn(*mut Notification, unsafe extern "C" fn(*mut Notification))
    )(endpoint, notification_closed);
}
unsafe extern "C" fn notification_closed(endpoint: *mut Notification) {
    (*endpoint).wake.close();
    fw!(abi::F_FREE, unsafe extern "C" fn(*mut Notification))(endpoint);
}

struct State {
    adapter: Option<&'static Adapter>,
    owners: [Owner; OWNER_COUNT],
    next_owner: u32,
    original_dispatch: usize,
    hook_id: i32,
}
// pthread_mutex_t is initialized by the stock libuv mutex initializer. Storage
// exceeds its observed fields and is never moved after initialization.
static READY: AtomicBool = AtomicBool::new(false);
static MUTEX: Shared<[u32; 8]> = Shared(UnsafeCell::new([0; 8]));
static STATE: Shared<State> = Shared(UnsafeCell::new(State {
    adapter: None,
    owners: [Owner::EMPTY; OWNER_COUNT],
    next_owner: 1,
    original_dispatch: 0,
    hook_id: -1,
}));
struct Guard;
impl Guard {
    unsafe fn lock() -> Self {
        fw!(abi::F_UV_MUTEX_LOCK, unsafe extern "C" fn(*mut c_void))(MUTEX.0.get().cast());
        Self
    }
    unsafe fn state(&mut self) -> &mut State {
        &mut *STATE.0.get()
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            fw!(abi::F_UV_MUTEX_UNLOCK, unsafe extern "C" fn(*mut c_void))(MUTEX.0.get().cast());
        }
    }
}

unsafe extern "C" fn on_create(context: Context, prototype: usize) {
    trace(30);
    // FeaturePrototypeQjs, not the QJS class object constructed at C4929C8.
    let manager = ptr::read_unaligned((prototype + abi::PROTOTYPE_MANAGER) as *const usize);
    let mut guard = Guard::lock();
    let state = guard.state();
    let Some(index) = state.owners.iter().position(|owner| owner.id == 0) else {
        return;
    };
    let Some(next) = state.next_owner.checked_add(1) else {
        return;
    };
    let id = state.next_owner;
    let endpoint = create_notification(manager, id);
    if endpoint.is_null() {
        trace(95);
        return;
    }
    state.next_owner = next;
    state.owners[index] = Owner {
        id,
        context,
        prototype,
        endpoint,
        callback: UNDEFINED,
        queued: false,
    };
}

unsafe extern "C" fn on_required(context: Context, instance: usize) {
    trace(31);
    let object = ptr::read_unaligned((instance + abi::INSTANCE_HANDLE) as *const Value);
    for (name, operation, argc) in [
        (b"getCapabilities\0".as_slice(), 0, 0),
        (b"createSession\0".as_slice(), 1, 1),
        (b"request\0".as_slice(), 2, 6),
        (b"subscribe\0".as_slice(), 3, 1),
    ] {
        let function = fw!(
            abi::F_QJS_NEW_C_FUNCTION2,
            unsafe extern "C" fn(Context, usize, *const c_char, i32, i32, i32) -> Value
        )(
            context,
            native_call as *const () as usize,
            name.as_ptr().cast(),
            argc,
            1,
            operation,
        );
        if !define(context, object, name, function) {
            clear_exception(context);
            if let Some((owner, adapter)) = current(context) {
                (adapter.close_owner)(owner.id);
            }
            return;
        }
    }
}

unsafe extern "C" fn on_destroy(context: Context, prototype: usize) {
    let (owner, adapter) = {
        let mut guard = Guard::lock();
        let state = guard.state();
        let Some(index) = state.owners.iter().position(|owner| {
            owner.prototype == prototype && owner.context == context && owner.id != 0
        }) else {
            return;
        };
        let owner = state.owners[index];
        state.owners[index] = Owner::EMPTY;
        (owner, state.adapter)
    };
    // onDestroy runs on the owning QJS loop before its manager closes. The
    // state withdrawal above bars producers; the guard serialized all prior
    // writes. Storage and file ownership last until the uv_close callback.
    if !owner.endpoint.is_null() {
        close_notification(owner.endpoint);
    }
    if let Some(adapter) = adapter {
        (adapter.close_owner)(owner.id);
    }
    free(context, owner.callback);
}

unsafe fn current(context: Context) -> Option<(Owner, &'static Adapter)> {
    let mut guard = Guard::lock();
    let state = guard.state();
    let owner = state
        .owners
        .iter()
        .find(|owner| owner.context == context && owner.id != 0)?;
    Some((*owner, state.adapter?))
}

/// Signal the JS owner's module async through its retained pipe file.
/// The binding guard pins the handle and file until the write completes.
pub fn notify(owner_id: u32) {
    if owner_id == 0 || !READY.load(Ordering::Acquire) {
        return;
    }
    unsafe {
        trace(90);
        let mut guard = Guard::lock();
        let state = guard.state();
        let Some(owner) = state
            .owners
            .iter_mut()
            .find(|owner| owner.id == owner_id && owner_id != 0)
        else {
            return;
        };
        if owner.queued || owner.callback == UNDEFINED {
            return;
        }
        owner.queued = true;
        let result = (*owner.endpoint)
            .wake
            .signal((*owner.endpoint).async_handle.as_mut_ptr());
        if result != 0 {
            owner.queued = false;
            trace(96);
            trace(result as u32);
        }
    }
}

unsafe extern "C" fn drain(active: i32, owner_id: usize) {
    trace(if active == 0 { 91 } else { 92 });
    let (owner, adapter) = {
        let mut guard = Guard::lock();
        let state = guard.state();
        let Some(owner) = state
            .owners
            .iter_mut()
            .find(|owner| owner.id as usize == owner_id && owner.id != 0)
        else {
            return;
        };
        owner.queued = false;
        if active == 0 {
            return;
        }
        let Some(adapter) = state.adapter else {
            return;
        };
        (*owner, adapter)
    };
    js::deliver(
        owner.context,
        owner.id,
        owner.callback,
        adapter,
        subscription_matches,
    );
}

unsafe fn subscription_matches(context: Context, owner: u32, callback: Value) -> bool {
    current(context).is_some_and(|(now, _)| now.id == owner && now.callback == callback)
}

unsafe extern "C" fn native_call(
    context: Context,
    _this: Value,
    argc: i32,
    argv: *const Value,
    magic: i32,
) -> Value {
    let Some((owner, adapter)) = current(context) else {
        return integer(-ERROR_BUSY);
    };
    js::invoke(context, owner.id, adapter, subscribe, argc, argv, magic)
}

unsafe fn subscribe(context: Context, owner: u32, value: Value) -> Result<(), i32> {
    let value = duplicate(value);
    let previous = {
        let mut guard = Guard::lock();
        let state = guard.state();
        let Some(slot) = state.owners.iter_mut().find(|slot| slot.id == owner) else {
            free(context, value);
            return Err(ERROR_DISCONNECTED);
        };
        mem::replace(&mut slot.callback, value)
    };
    free(context, previous);
    notify(owner);
    Ok(())
}
