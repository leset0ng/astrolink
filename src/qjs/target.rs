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
pub(crate) unsafe fn trace(stage: u32) {
    use ::core::sync::atomic::AtomicU32;
    static NEXT: AtomicU32 = AtomicU32::new(0);
    #[no_mangle]
    static ASTROLINK_TRACE: Shared<[u32; 64]> = Shared(UnsafeCell::new([0; 64]));
    let index = NEXT.fetch_add(1, Ordering::Relaxed) as usize % 64;
    ptr::write_volatile(ASTROLINK_TRACE.0.get().cast::<u32>().add(index), stage);
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
    original_framework_vtable: usize,
    hook_id: i32,
}
struct Shared<T>(UnsafeCell<T>);
unsafe impl<T> Sync for Shared<T> {}
// pthread_mutex_t is initialized by the stock libuv mutex initializer. Storage
// exceeds its observed fields and is never moved after initialization.
static READY: AtomicBool = AtomicBool::new(false);
static MUTEX: Shared<[u32; 8]> = Shared(UnsafeCell::new([0; 8]));
// Both tables retain the two Itanium ABI prefix words. Only one primary
// dispatch slot changes in each table; secondary Application vptrs stay stock.
const FRAMEWORK_VTABLE: usize = 0x2ca8065c;
const APPLICATION_VTABLE: usize = 0x2ca7c2d4;
static FRAMEWORK_DISPATCH: Shared<[usize; 15]> = Shared(UnsafeCell::new([0; 15]));
static APPLICATION_DISPATCH: Shared<[usize; 52]> = Shared(UnsafeCell::new([0; 52]));
static STATE: Shared<State> = Shared(UnsafeCell::new(State {
    adapter: None,
    owners: [Owner::EMPTY; OWNER_COUNT],
    next_owner: 1,
    original_framework_vtable: 0,
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

/// Install the synchronous Application factory/init dispatcher on Q66 .155.
/// The framework must exist and no QuickApp launch may overlap activation.
/// The module remains resident after this succeeds.
///
/// # Safety
/// `ctx` must be a valid Supervisor context for this exact target. Call from
/// serialized module activation while QuickApps are closed. The adapter and
/// this module's code/data must remain live until reboot. No firmware code is
/// modified: the Supervisor pointer hook replaces a heap object's RAM vptr.
pub unsafe fn install(ctx: *const CanopusContext, adapter: &'static Adapter) -> Result<(), i32> {
    if ctx.is_null() {
        return Err(-22);
    }
    if (*STATE.0.get()).hook_id >= 0 {
        return Ok(());
    }
    let framework = ptr::read_volatile(0x200d972c as *const usize);
    if !ram_object(framework) {
        return Err(-16);
    }
    let implementation = ptr::read_unaligned((framework + 4) as *const usize);
    if !ram_object(implementation) {
        return Err(-16);
    }
    let current = ptr::read_volatile(framework as *const usize);
    // A distinct extension can own the same RAM dispatcher. Report BUSY rather
    // than replace a table whose full layout/ownership has not been audited.
    if current != FRAMEWORK_VTABLE {
        return Err(-16);
    }
    let rc = fw!(0x0c3f4b64, unsafe extern "C" fn(*mut c_void) -> i32)(MUTEX.0.get().cast());
    if rc != 0 {
        return Err(rc);
    }
    ptr::copy_nonoverlapping(
        (FRAMEWORK_VTABLE - 8) as *const usize,
        FRAMEWORK_DISPATCH.0.get().cast::<usize>(),
        15,
    );
    ptr::copy_nonoverlapping(
        (APPLICATION_VTABLE - 8) as *const usize,
        APPLICATION_DISPATCH.0.get().cast::<usize>(),
        52,
    );
    (*FRAMEWORK_DISPATCH.0.get())[2 + 8] = create_application as *const () as usize;
    (*APPLICATION_DISPATCH.0.get())[2 + 2] = initialize_application as *const () as usize;
    (*STATE.0.get()).adapter = Some(adapter);
    // The real framework's recursive render/list mutex guards its app vector.
    let framework_mutex = (implementation + 204) as *mut c_void;
    fw!(0x0c3f4bf8, unsafe extern "C" fn(*mut c_void))(framework_mutex);
    let first = ptr::read_unaligned((implementation + 320) as *const usize);
    let end = ptr::read_unaligned((implementation + 324) as *const usize);
    if first != end {
        fw!(0x0c3f4c14, unsafe extern "C" fn(*mut c_void))(framework_mutex);
        (*STATE.0.get()).adapter = None;
        return Err(-16);
    }
    READY.store(true, Ordering::Release);
    let result = Hook::install(
        ctx,
        framework,
        FRAMEWORK_DISPATCH.0.get().cast::<usize>().add(2) as usize,
        &mut (*STATE.0.get()).original_framework_vtable,
        HookType::Pointer,
    );
    fw!(0x0c3f4c14, unsafe extern "C" fn(*mut c_void))(framework_mutex);
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

fn ram_object(address: usize) -> bool {
    (0x20000000..0x40000000).contains(&address) && address & 3 == 0
}

unsafe extern "C" fn create_application(framework: usize, uri: *const c_char) -> usize {
    trace(10);
    // The stock factory returns a constructed Application and has not yet
    // started its JS task. Replace its own primary dispatch pointer now.
    let original = ptr::read_unaligned((FRAMEWORK_VTABLE + 32) as *const usize);
    let application = fw!(
        original,
        unsafe extern "C" fn(usize, *const c_char) -> usize
    )(framework, uri);
    if application != 0 && ptr::read_volatile(application as *const usize) == APPLICATION_VTABLE {
        trace(11);
        ptr::write_volatile(
            application as *mut usize,
            APPLICATION_DISPATCH.0.get().cast::<usize>().add(2) as usize,
        );
    }
    application
}

unsafe extern "C" fn initialize_application(
    application: usize,
    options: usize,
    loop_pointer: usize,
) -> i32 {
    let original = ptr::read_unaligned((APPLICATION_VTABLE + 8) as *const usize);
    let result = fw!(original, unsafe extern "C" fn(usize, usize, usize) -> i32)(
        application,
        options,
        loop_pointer,
    );
    trace(20);
    // Runs synchronously on this Application's JS task between its stock
    // initialize and run calls. Registry mutation cannot race require/eval.
    let runtime = ptr::read_unaligned((application + 100) as *const usize);
    if runtime == 0 {
        return result;
    }
    let manager = ptr::read_unaligned((runtime + 116) as *const usize);
    if manager == 0 {
        return result;
    }
    let registry = ptr::read_unaligned((manager + 4) as *const usize);
    if registry == 0 {
        return result;
    }
    trace(21);
    fw!(
        0x0c495f2c,
        unsafe extern "C" fn(usize, *const Description) -> i32
    )(registry, &DESCRIPTION)
}

unsafe extern "C" fn on_create(context: Context, prototype: usize) {
    trace(30);
    // FeaturePrototypeQjs, not the QJS class object constructed at C4929C8.
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
    trace(31);
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
        trace(94);
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
    if magic == 0 {
        trace(40);
    } else if magic == 1 {
        trace(93);
    }
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
    if magic == 2 && !args.is_empty() {
        trace(100 + (args[0] as u32).min(20));
    }
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
            trace(41);
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
    if payload == 0 {
        free(context, buffer);
        clear_exception(context);
        return None;
    }
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
