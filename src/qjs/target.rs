//! Exact Q66 user-4.100.155 ABI. Evidence and calling conventions are recorded
//! in docs/qjs-abi.md; addresses are Thumb entry points, not portable symbols.
use super::*;
use canopus_abi::{CanopusContext, HookType};
use canopus_runtime::hook::Hook;
use core::{
    cell::UnsafeCell,
    ffi::{c_char, c_void},
    mem, ptr, slice,
    sync::atomic::{AtomicBool, Ordering},
};

type Context = *mut c_void;
type Value = u64;
const UNDEFINED: Value = 3u64 << 32;
const EXCEPTION: Value = 6u64 << 32;
const OWNER_COUNT: usize = 4;
const NAME: &[u8] = b"canopus.astrolink\0";

macro_rules! fw {
    ($address:expr, $ty:ty) => {
        mem::transmute::<usize, $ty>($address | 1)
    };
}

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
    manager: usize,
    callback: Value,
    queued: bool,
}
impl Owner {
    const EMPTY: Self = Self {
        id: 0,
        context: ptr::null_mut(),
        prototype: 0,
        manager: 0,
        callback: UNDEFINED,
        queued: false,
    };
}
struct State {
    adapter: Option<&'static Adapter>,
    owners: [Owner; OWNER_COUNT],
    next_owner: u32,
    original_register: usize,
    hook_id: i32,
}
struct Shared<T>(UnsafeCell<T>);
unsafe impl<T> Sync for Shared<T> {}
// pthread_mutex_t is initialized by the stock libuv mutex initializer. Storage
// exceeds its observed fields and is never moved after initialization.
static READY: AtomicBool = AtomicBool::new(false);
static MUTEX: Shared<[u32; 8]> = Shared(UnsafeCell::new([0; 8]));
static STATE: Shared<State> = Shared(UnsafeCell::new(State {
    adapter: None,
    owners: [Owner::EMPTY; OWNER_COUNT],
    next_owner: 1,
    original_register: 0,
    hook_id: -1,
}));
struct Guard;
impl Guard {
    unsafe fn lock() -> Self {
        fw!(0x0c3f4bf8, unsafe extern "C" fn(*mut c_void))(MUTEX.0.get().cast());
        Self
    }
    unsafe fn state(&mut self) -> &mut State {
        &mut *STATE.0.get()
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            fw!(0x0c3f4c14, unsafe extern "C" fn(*mut c_void))(MUTEX.0.get().cast());
        }
    }
}

/// Called once, on module activation, before any Astrolink JS references exist.
/// The module must remain resident after this succeeds. Requires Q66 .155.
///
/// # Safety
/// `ctx` must be a valid Supervisor context for this exact target. Call once
/// from serialized activation, before any application can access this Feature.
/// The adapter and this module's code/data must remain live until reboot.
pub unsafe fn install(ctx: *const CanopusContext, adapter: &'static Adapter) -> Result<(), i32> {
    if ctx.is_null() {
        return Err(-22);
    }
    if (*STATE.0.get()).hook_id >= 0 {
        return Ok(());
    }
    let rc = fw!(0x0c3f4b64, unsafe extern "C" fn(*mut c_void) -> i32)(MUTEX.0.get().cast());
    if rc != 0 {
        return Err(rc);
    }
    (*STATE.0.get()).adapter = Some(adapter);
    READY.store(true, Ordering::Release);
    // Supervisor writes the trampoline before publishing the branch patch.
    let result = Hook::install(
        ctx,
        0x0c495f2d,
        register_hook as *const () as usize,
        &mut (*STATE.0.get()).original_register,
        HookType::Inline,
    );
    match result {
        Ok(id) => {
            (*STATE.0.get()).hook_id = id;
            Ok(())
        }
        Err(error) => {
            READY.store(false, Ordering::Release);
            (*STATE.0.get()).adapter = None;
            Err(error)
        }
    }
}

unsafe extern "C" fn register_hook(registry: usize, description: *const Description) -> i32 {
    let original = fw!(
        (*STATE.0.get()).original_register,
        unsafe extern "C" fn(usize, *const Description) -> i32
    );
    let result = original(registry, description);
    // locale is the first stock registration in every fresh application registry.
    // It is immutable firmware data; avoid reading arbitrary variable strings.
    if result != 0
        && !description.is_null()
        && (*description).name as usize == 0x2cb24eb4
        && original(registry, &DESCRIPTION) == 0
    {
        return 0;
    }
    result
}

unsafe extern "C" fn on_create(context: Context, prototype: usize) {
    let manager = ptr::read_unaligned((prototype + 12) as *const usize);
    let mut guard = Guard::lock();
    let state = guard.state();
    let Some(index) = state.owners.iter().position(|owner| owner.id == 0) else {
        return;
    };
    let Some(next) = state.next_owner.checked_add(1) else {
        return;
    };
    let id = state.next_owner;
    state.next_owner = next;
    state.owners[index] = Owner {
        id,
        context,
        prototype,
        manager,
        callback: UNDEFINED,
        queued: false,
    };
}

unsafe extern "C" fn on_required(context: Context, instance: usize) {
    let object = ptr::read_unaligned((instance + 112) as *const Value);
    for (name, operation, argc) in [
        (b"getCapabilities\0".as_slice(), 0, 0),
        (b"createSession\0".as_slice(), 1, 1),
        (b"request\0".as_slice(), 2, 6),
        (b"subscribe\0".as_slice(), 3, 1),
    ] {
        let function = fw!(
            0x0c3b98d0,
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

/// Wake a context from any ordinary firmware task. No JS values are touched.
/// Posting and onDestroy are serialized so a manager cannot be freed mid-post.
pub fn notify(owner_id: u32) {
    if owner_id == 0 || !READY.load(Ordering::Acquire) {
        return;
    }
    unsafe {
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
        #[repr(C)]
        struct Task {
            reference: usize,
            callback: usize,
            user: usize,
            reserved: usize,
            simple: u8,
            padding: [u8; 7],
        }
        let task = Task {
            reference: 0,
            callback: drain as *const () as usize,
            user: owner.id as usize,
            reserved: 0,
            simple: 0,
            padding: [0; 7],
        };
        fw!(0x0c491cd0, unsafe extern "C" fn(usize, *const Task))(owner.manager, &task);
    }
}

unsafe extern "C" fn drain(active: i32, owner_id: usize) {
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
    if owner.callback == UNDEFINED {
        return;
    }
    // Reentrant unsubscribe/close from a callback must not release the function
    // currently used by this drain. A local owned reference protects it.
    let callback = duplicate(owner.callback);
    let mut event = Event::EMPTY;
    // Bound one owner-task turn; remaining work is scheduled through libuv.
    for _ in 0..16 {
        if !(adapter.next_event)(owner.id, &mut event) {
            free(owner.context, callback);
            return;
        }
        if event.peer_len > event.peer.len() || event.data_len > event.data.len() {
            continue;
        }
        let value = event_object(owner.context, &event);
        if value == EXCEPTION {
            clear_exception(owner.context);
            (adapter.close_owner)(owner.id);
            free(owner.context, callback);
            return;
        }
        let result = fw!(
            0x0c3ccb00,
            unsafe extern "C" fn(Context, Value, Value, i32, *const Value) -> Value
        )(owner.context, callback, UNDEFINED, 1, &value);
        free(owner.context, result);
        free(owner.context, value);
        if result == EXCEPTION {
            clear_exception(owner.context);
        }
        // A callback can replace the subscription. Remaining events belong to
        // the new subscriber and should run in a later turn.
        if current(owner.context)
            .is_none_or(|(now, _)| now.id != owner.id || now.callback != owner.callback)
        {
            free(owner.context, callback);
            notify(owner.id);
            return;
        }
    }
    free(owner.context, callback);
    notify(owner.id);
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
    if argc < 0 || (argc > 0 && argv.is_null()) {
        return integer(-ERROR_INVALID_ARGUMENT);
    }
    let args = if argc == 0 {
        &[]
    } else {
        slice::from_raw_parts(argv, argc as usize)
    };
    match magic {
        0 => capabilities_object(context, (adapter.capabilities)()),
        1 => {
            let Some(value) = args.first().copied() else {
                return integer(-ERROR_INVALID_ARGUMENT);
            };
            let mut service = [0u8; 64];
            let Some(length) = string_bytes(context, value, &mut service) else {
                return integer(-ERROR_INVALID_ARGUMENT);
            };
            match (adapter.create_session)(owner.id, &service[..length]) {
                Ok(handle) if handle <= i32::MAX as u32 => integer(handle as i32),
                Ok(_) => integer(-ERROR_BUSY),
                Err(error) => integer(-error.checked_abs().unwrap_or(ERROR_BACKEND)),
            }
        }
        2 => request(context, owner.id, adapter, args),
        3 => {
            let value = args.first().copied().unwrap_or(UNDEFINED);
            if value != UNDEFINED
                && fw!(0x0c3bf280, unsafe extern "C" fn(Context, Value) -> i32)(context, value) == 0
            {
                return integer(-ERROR_INVALID_ARGUMENT);
            }
            let value = duplicate(value);
            let previous = {
                let mut guard = Guard::lock();
                let state = guard.state();
                let Some(slot) = state.owners.iter_mut().find(|slot| slot.id == owner.id) else {
                    free(context, value);
                    return integer(-ERROR_DISCONNECTED);
                };
                mem::replace(&mut slot.callback, value)
            };
            free(context, previous);
            notify(owner.id);
            integer(0)
        }
        _ => integer(-ERROR_INVALID_ARGUMENT),
    }
}

unsafe fn request(context: Context, owner: u32, adapter: &Adapter, args: &[Value]) -> Value {
    if args.len() < 3 {
        return integer(-ERROR_INVALID_ARGUMENT);
    }
    let Some(operation) = unsigned(args[0]) else {
        return integer(-ERROR_INVALID_ARGUMENT);
    };
    let Some(handle) = unsigned(args[1]) else {
        return integer(-ERROR_INVALID_ARGUMENT);
    };
    let Some(token) = unsigned(args[2]) else {
        return integer(-ERROR_INVALID_ARGUMENT);
    };
    if !(OP_START_SCAN..=OP_CLOSE).contains(&operation) || token == 0 {
        return integer(-ERROR_INVALID_ARGUMENT);
    }
    let mut data = [0u8; 512];
    let data_len = match args.get(3).copied().filter(|value| *value != UNDEFINED) {
        Some(value) => match binary_bytes(context, value, &mut data) {
            Some(len) => len,
            None => return integer(-ERROR_INVALID_ARGUMENT),
        },
        None => 0,
    };
    let mut peer = [0u8; 14];
    let peer_len = match args.get(4).copied().filter(|value| *value != UNDEFINED) {
        Some(value) => match string_bytes(context, value, &mut peer) {
            Some(len) => len,
            None => return integer(-ERROR_INVALID_ARGUMENT),
        },
        None => 0,
    };
    let flag = match args.get(5).copied().unwrap_or(UNDEFINED) {
        UNDEFINED => false,
        value if (value >> 32) as i32 == 1 => value as u32 != 0,
        _ => return integer(-ERROR_INVALID_ARGUMENT),
    };
    match (adapter.request)(
        owner,
        operation,
        handle,
        token,
        &data[..data_len],
        &peer[..peer_len],
        flag,
    ) {
        Ok(()) => {
            notify(owner);
            integer(0)
        }
        Err(error) => integer(-error.checked_abs().unwrap_or(ERROR_BACKEND)),
    }
}

fn integer(value: i32) -> Value {
    value as u32 as Value
}
fn unsigned(value: Value) -> Option<u32> {
    if value >> 32 == 0 && (value as u32) <= i32::MAX as u32 {
        Some(value as u32)
    } else {
        None
    }
}
unsafe fn duplicate(value: Value) -> Value {
    if (value >> 32) as u32 > 0xfffffff4 {
        let count = value as u32 as *mut i32;
        *count += 1;
    }
    value
}
unsafe fn free(context: Context, value: Value) {
    fw!(0x0c3b813c, unsafe extern "C" fn(Context, Value))(context, value);
}
unsafe fn clear_exception(context: Context) {
    let value = fw!(0x0c3b793e, unsafe extern "C" fn(Context) -> Value)(context);
    free(context, value);
}
unsafe fn string(context: Context, bytes: &[u8]) -> Value {
    fw!(
        0x0c3b69e8,
        unsafe extern "C" fn(Context, *const u8, usize) -> Value
    )(context, bytes.as_ptr(), bytes.len())
}
unsafe fn string_bytes(context: Context, value: Value, out: &mut [u8]) -> Option<usize> {
    if (value >> 32) as i32 != -7 {
        return None;
    }
    let mut len = 0;
    let pointer = fw!(
        0x0c3b883c,
        unsafe extern "C" fn(Context, *mut usize, Value, i32) -> *const u8
    )(context, &mut len, value, 0);
    if pointer.is_null() {
        clear_exception(context);
        return None;
    }
    let result = if len <= out.len() {
        out[..len].copy_from_slice(slice::from_raw_parts(pointer, len));
        Some(len)
    } else {
        None
    };
    fw!(0x0c3b89f2, unsafe extern "C" fn(Context, *const u8))(context, pointer);
    result
}
unsafe fn binary_bytes(context: Context, value: Value, out: &mut [u8]) -> Option<usize> {
    if (value >> 32) as i32 != -1 {
        return None;
    }
    let class = ptr::read_unaligned((value as u32 as usize + 6) as *const u16);
    let mut offset = 0;
    let mut length = 0;
    let mut element_size = 0;
    let buffer = if class == 19 {
        duplicate(value)
    } else if class == 23 {
        fw!(
            0x0c3e848c,
            unsafe extern "C" fn(Context, Value, *mut usize, *mut usize, *mut usize) -> Value
        )(context, value, &mut offset, &mut length, &mut element_size)
    } else {
        return None;
    };
    if buffer == EXCEPTION {
        clear_exception(context);
        return None;
    }
    let mut buffer_len = 0;
    let pointer = fw!(
        0x0c3e7f9c,
        unsafe extern "C" fn(Context, *mut usize, Value) -> *const u8
    )(context, &mut buffer_len, buffer);
    if class == 19 {
        length = buffer_len;
    }
    let payload = ptr::read_unaligned((buffer as u32 as usize + 32) as *const usize);
    let detached = ptr::read((payload + 4) as *const u8) != 0;
    let valid = !detached
        && (length == 0 || !pointer.is_null())
        && length <= out.len()
        && offset <= buffer_len
        && length <= buffer_len - offset;
    if valid && length != 0 {
        out[..length].copy_from_slice(slice::from_raw_parts(pointer.add(offset), length));
    }
    free(context, buffer);
    if !valid {
        clear_exception(context);
    }
    valid.then_some(length)
}
unsafe fn object(context: Context) -> Value {
    fw!(0x0c3b982c, unsafe extern "C" fn(Context) -> Value)(context)
}
unsafe fn define(context: Context, object: Value, name: &[u8], value: Value) -> bool {
    if value == EXCEPTION {
        return false;
    }
    fw!(
        0x0c3c3c46,
        unsafe extern "C" fn(Context, Value, *const u8, Value, i32) -> i32
    )(context, object, name.as_ptr(), value, 7)
        >= 0
}
macro_rules! property {
    ($context:expr, $object:expr, $name:expr, $value:expr) => {
        if !define($context, $object, $name, $value) {
            free($context, $object);
            return EXCEPTION;
        }
    };
}
unsafe fn capabilities_object(context: Context, caps: Capabilities) -> Value {
    let value = object(context);
    if value == EXCEPTION {
        return value;
    }
    property!(
        context,
        value,
        b"supported\0",
        (1u64 << 32) | caps.supported as u64
    );
    property!(
        context,
        value,
        b"maxLinks\0",
        integer(caps.max_links as i32)
    );
    property!(
        context,
        value,
        b"maxServiceIdBytes\0",
        integer(caps.max_service_id_bytes as i32)
    );
    property!(
        context,
        value,
        b"maxAdvertisingDataBytes\0",
        integer(caps.max_advertising_data_bytes as i32)
    );
    value
}
unsafe fn event_object(context: Context, event: &Event) -> Value {
    let value = object(context);
    if value == EXCEPTION {
        return value;
    }
    for (name, number) in [
        (b"kind\0".as_slice(), event.kind as i32),
        (b"token\0".as_slice(), event.token as i32),
        (b"session\0".as_slice(), event.session as i32),
        (b"link\0".as_slice(), event.link as i32),
        (
            b"maxMessageBytes\0".as_slice(),
            event.max_message_bytes as i32,
        ),
        (b"rssi\0".as_slice(), event.rssi),
        (b"error\0".as_slice(), event.error),
        (b"backendError\0".as_slice(), event.backend_error),
    ] {
        property!(context, value, name, integer(number));
    }
    property!(
        context,
        value,
        b"peerId\0",
        string(context, &event.peer[..event.peer_len])
    );
    let buffer = fw!(
        0x0c3e7674,
        unsafe extern "C" fn(Context, *const u8, usize) -> Value
    )(context, event.data.as_ptr(), event.data_len);
    property!(context, value, b"data\0", buffer);
    value
}
