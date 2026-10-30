//! JavaScript values and public methods shared by the firmware Feature hosts.
use super::*;
use crate::abi;
use core::{cell::UnsafeCell, ffi::c_void, mem, ptr, slice, sync::atomic::Ordering};

pub(super) type Context = *mut c_void;
pub(super) type Value = u64;
pub(super) const UNDEFINED: Value = 3u64 << 32;
pub(super) const EXCEPTION: Value = 6u64 << 32;
pub(super) struct Shared<T>(pub(super) UnsafeCell<T>);
unsafe impl<T> Sync for Shared<T> {}

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

pub(super) unsafe fn invoke(
    context: Context,
    owner: u32,
    adapter: &Adapter,
    subscribe: unsafe fn(Context, u32, Value) -> Result<(), i32>,
    argc: i32,
    argv: *const Value,
    magic: i32,
) -> Value {
    if magic == 0 {
        trace(40);
    } else if magic == 1 {
        trace(93);
    }
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
            match (adapter.create_session)(owner, &service[..length]) {
                Ok(handle) if handle <= i32::MAX as u32 => integer(handle as i32),
                Ok(_) => integer(-ERROR_BUSY),
                Err(error) => integer(-error.checked_abs().unwrap_or(ERROR_BACKEND)),
            }
        }
        2 => request(context, owner, adapter, args),
        3 => {
            trace(41);
            let value = args.first().copied().unwrap_or(UNDEFINED);
            if value != UNDEFINED
                && fw!(
                    abi::F_QJS_IS_FUNCTION,
                    unsafe extern "C" fn(Context, Value) -> i32
                )(context, value)
                    == 0
            {
                return integer(-ERROR_INVALID_ARGUMENT);
            }
            match subscribe(context, owner, value) {
                Ok(()) => integer(0),
                Err(error) => integer(-error),
            }
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

pub(super) fn integer(value: i32) -> Value {
    value as u32 as Value
}
fn unsigned(value: Value) -> Option<u32> {
    if value >> 32 == 0 && (value as u32) <= i32::MAX as u32 {
        Some(value as u32)
    } else {
        None
    }
}
pub(super) unsafe fn duplicate(value: Value) -> Value {
    if (value >> 32) as u32 > 0xfffffff4 {
        let count = value as u32 as *mut i32;
        *count += 1;
    }
    value
}
pub(super) unsafe fn free(context: Context, value: Value) {
    fw!(abi::F_QJS_FREE_VALUE, unsafe extern "C" fn(Context, Value))(context, value);
}
pub(super) unsafe fn clear_exception(context: Context) {
    let value = fw!(
        abi::F_QJS_GET_EXCEPTION,
        unsafe extern "C" fn(Context) -> Value
    )(context);
    free(context, value);
}
unsafe fn string(context: Context, bytes: &[u8]) -> Value {
    abi::qjs_new_string_len(context, bytes.as_ptr(), bytes.len())
}
unsafe fn string_bytes(context: Context, value: Value, out: &mut [u8]) -> Option<usize> {
    if (value >> 32) as i32 != -7 {
        return None;
    }
    let mut len = 0;
    let pointer = abi::qjs_to_cstring_len2(context, &mut len, value, 0);
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
    fw!(
        abi::F_QJS_FREE_CSTRING,
        unsafe extern "C" fn(Context, *const u8)
    )(context, pointer);
    result
}
unsafe fn binary_bytes(context: Context, value: Value, out: &mut [u8]) -> Option<usize> {
    if (value >> 32) as i32 != -1 {
        return None;
    }
    let class =
        ptr::read_unaligned((value as u32 as usize + abi::JSVALUE_CLASS_OFFSET) as *const u16);
    let mut offset = 0;
    let mut length = 0;
    let mut element_size = 0;
    let buffer = if class == 19 {
        duplicate(value)
    } else if class == 23 {
        abi::qjs_get_typed_array_buffer(context, value, &mut offset, &mut length, &mut element_size)
    } else {
        return None;
    };
    if buffer == EXCEPTION {
        clear_exception(context);
        return None;
    }
    let mut buffer_len = 0;
    let pointer = abi::qjs_get_array_buffer(context, &mut buffer_len, buffer);
    if class == 19 {
        length = buffer_len;
    }
    let payload =
        ptr::read_unaligned((buffer as u32 as usize + abi::ARRAY_BUFFER_DATA) as *const usize);
    if payload == 0 {
        free(context, buffer);
        clear_exception(context);
        return None;
    }
    let detached = ptr::read((payload + abi::ARRAY_BUFFER_DETACHED) as *const u8) != 0;
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
    fw!(
        abi::F_QJS_NEW_OBJECT,
        unsafe extern "C" fn(Context) -> Value
    )(context)
}
pub(super) unsafe fn define(context: Context, object: Value, name: &[u8], value: Value) -> bool {
    if value == EXCEPTION {
        return false;
    }
    abi::qjs_define_property_value_str(context, object, name.as_ptr(), value, 7) >= 0
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
pub(super) unsafe fn event_object(context: Context, event: &Event) -> Value {
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
    let buffer = abi::qjs_new_array_buffer_copy(context, event.data.as_ptr(), event.data_len);
    property!(context, value, b"data\0", buffer);
    value
}
