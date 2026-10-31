//! O66 3.1.28 target under construction. SUPPORTED remains false.
//! Exact evidence: target pack EVID-O66-ASTROLINK-ABI-001.
//! Zero address/layout fields below are unresolved and must not be called.
//! Factory virtual slot does not exist on this older FrameworkExt host.
use canopus_target_generated as fw;

pub const SUPPORTED: bool = false;
pub const TARGET_ID: &str = "xiaomi-band-10-3.1.28";
pub const SIGNATURES: &[(usize, [u8; 12])] = &[
    (
        0xc4c6ddc,
        [
            0x2d, 0xe9, 0xf0, 0x43, 0xad, 0xf5, 0x1d, 0x7d, 0x0, 0xaf, 0x91, 0x46,
        ],
    ),
    (
        0xc4cc828,
        [
            0x2d, 0xe9, 0xf0, 0x43, 0x5, 0x46, 0xad, 0xf5, 0x1b, 0x7d, 0x0, 0xaf,
        ],
    ),
    (
        0xc3acdb0,
        [
            0x2d, 0xe9, 0xf0, 0x4f, 0x83, 0xb0, 0xdf, 0xf8, 0xc4, 0xa0, 0x81, 0x46,
        ],
    ),
    (
        0xc3c1e54,
        [
            0x2d, 0xe9, 0xf0, 0x47, 0x82, 0xb0, 0x16, 0x7a, 0x14, 0x46, 0x6, 0xf0,
        ],
    ),
];

pub const F_ALLOC: usize = fw::CANOPUS_FW_ASTROLINK_ALLOC_CALLABLE;
pub const F_FREE: usize = fw::CANOPUS_FW_ASTROLINK_FREE_CALLABLE;
pub const F_UV_MUTEX_INIT: usize = fw::CANOPUS_FW_UV_MUTEX_INIT_CALLABLE;
pub const F_UV_MUTEX_LOCK: usize = fw::CANOPUS_FW_UV_MUTEX_LOCK_CALLABLE;
pub const F_UV_MUTEX_UNLOCK: usize = fw::CANOPUS_FW_UV_MUTEX_UNLOCK_CALLABLE;
pub const F_UV_ASYNC_INIT: usize = fw::CANOPUS_FW_UV_ASYNC_INIT_CALLABLE;
pub const F_UV_CLOSE: usize = fw::CANOPUS_FW_UV_CLOSE_CALLABLE;
pub const F_FILES_GETFILEP: usize = fw::CANOPUS_FW_FILES_GETFILEP_CALLABLE;
pub const F_FS_GETFILEP: usize = fw::CANOPUS_FW_FS_GETFILEP_CALLABLE;
pub const F_FS_PUTFILEP: usize = fw::CANOPUS_FW_FS_PUTFILEP_CALLABLE;
pub const F_FILE_DUP3: usize = fw::CANOPUS_FW_FILE_DUP3_CALLABLE;
pub const F_FILE_WRITE: usize = fw::CANOPUS_FW_FILE_WRITE_CALLABLE;
pub const F_FILE_CLOSE: usize = fw::CANOPUS_FW_FILE_CLOSE_CALLABLE;
pub const F_NXSCHED_FOREACH: usize = fw::CANOPUS_FW_NXSCHED_FOREACH_CALLABLE;
pub const D_UV_KEY: usize = fw::canopus_fw_uv_task_key;

pub const LOOP_QUEUE_HEAD: usize = 64;
pub const LOOP_MUTEX: usize = 72;
pub const LOOP_ASYNC: usize = 96;
pub const LOOP_READ_FD: usize = 296;
pub const LOOP_WRITE_FD: usize = 300;
pub const ASYNC_BUSY_WORD: usize = 6;
pub const ASYNC_PENDING_WORD: usize = 15;
pub const TCB_TLS: usize = 68;
pub const TCB_GROUP: usize = 8;
pub const GROUP_FILELIST: usize = 88;
pub const FILELIST_ROW: usize = 8;
pub const TASKINFO_UVSTATE: usize = 20;
pub const TASKINFO_UVSTATE_STRIDE: usize = 4;
pub const UVSTATE_LOOP: usize = 640;
pub const FILE_INODE: usize = 16;
pub const INODE_OPS_PIPE: usize = fw::canopus_fw_pipe_file_operations;
pub const UV_KEY_LIMIT: u32 = 3;

pub const F_ADAPTER_REGISTER: usize = fw::CANOPUS_FW_BT_ADAPTER_REGISTER_CALLABLE;
pub const F_ADAPTER_GET_STATE: usize = fw::CANOPUS_FW_BT_ADAPTER_GET_STATE_CALLABLE;
pub const F_ADAPTER_ENABLE: usize = fw::CANOPUS_FW_BT_ADAPTER_ENABLE_CALLABLE;
pub const F_SCAN_CREATE: usize = fw::CANOPUS_FW_GAP_SCAN_START_IMPL_CALLABLE;
pub const F_SCAN_STOP: usize = fw::CANOPUS_FW_BLE_SCAN_STOP_CALLABLE;
pub const F_ADVERTISING_CREATE: usize = fw::CANOPUS_FW_BLE_ADVERTISING_START_CALLABLE;
pub const F_ADVERTISING_STOP: usize = fw::CANOPUS_FW_BLE_ADVERTISING_STOP_CALLABLE;
pub const D_ADAPTER_INSTANCE: usize = fw::canopus_fw_gap_client_instance;
/// Unresolved cached state; shared direct read must remain gated.
pub const ADAPTER_STATE_WORD: usize = 0;
pub const ADAPTER_LOOP: usize = 52;
pub const ADAPTER_READY_STATE: i32 = 4;
pub const SCAN_PARAM_SIZE: usize = 8;
pub const SCAN_CALLBACKS_SIZE: u32 = 16;
pub const ADV_CALLBACKS_SIZE: usize = 12;
pub const SCAN_REPORT_ADDRESS: usize = 0;
pub const SCAN_REPORT_RSSI: usize = 7;
pub const SCAN_REPORT_ADDRESS_TYPE: usize = 8;
pub const SCAN_REPORT_PACKET_TYPE: usize = 9;
pub const SCAN_REPORT_LENGTH: usize = 10;
pub const SCAN_REPORT_DATA: usize = 11;
pub const ADV_PARAM_SIZE: usize = 28;
pub const ADV_PARAM_MODE: usize = 0; // CLI0xC4D7EDC mode=legacy ADV_IND5
pub const ADV_PARAM_TX_POWER: usize = 20;
pub const ADV_PARAM_INTERVAL: usize = 16;

pub const D_L2CAP_OWNER: usize = fw::canopus_fw_bt_l2cap_owner;
pub const F_L2CAP_FIND_LE_PSM: usize = fw::CANOPUS_FW_L2CAP_FIND_LE_PSM_CALLABLE;
pub const F_L2CAP_REGISTER_PSM: usize = fw::CANOPUS_FW_L2CAP_REGISTER_PSM_HANDLER_CALLABLE;
pub const F_L2CAP_RELEASE_PSM: usize = fw::CANOPUS_FW_L2CAP_UNREGISTER_PSM_HANDLER_CALLABLE;
pub const F_L2CAP_CONNECT: usize = fw::CANOPUS_FW_L2CAP_CONNECT_HANDLER_CALLABLE;
pub const F_L2CAP_ACCEPT: usize = fw::CANOPUS_FW_L2CAP_ACCEPT_HANDLER_CALLABLE;
pub const F_L2CAP_SEND: usize = fw::CANOPUS_FW_L2CAP_SEND_HANDLER_CALLABLE;
pub const F_L2CAP_FIND_CHANNEL: usize = fw::CANOPUS_FW_L2CAP_FIND_CHANNEL_CALLABLE;
pub const F_L2CAP_DISCONNECT_CID: usize = fw::CANOPUS_FW_L2CAP_DISCONNECT_POLICY_CALLABLE;
pub const F_L2CAP_QUEUE_PUSH: usize = fw::CANOPUS_FW_L2CAP_QUEUE_PUSH_CALLABLE;
pub const F_L2CAP_CREDIT_FLUSH: usize = fw::CANOPUS_FW_L2CAP_CREDIT_QUEUE_FLUSH_CALLABLE;
pub const F_BT_QUEUE_EXTERNAL: usize = fw::CANOPUS_FW_BT_QUEUE_EXTERNAL_CALLABLE;
pub const COC_QUEUE_EVENT: u32 = 1;
pub const COC_EVENT_INCOMING: u32 = 1;
pub const COC_EVENT_ASSIGNED: u32 = 2;
pub const COC_EVENT_OPEN: u32 = 3;
pub const COC_EVENT_CLOSED: u32 = 6;
pub const COC_EVENT_DATA: u32 = 7;
pub const COC_EVENT_LOCAL_FLOW: u32 = 8;
pub const COC_EVENT_PSM_REGISTERED: u32 = 9;
pub const COC_EVENT_CREDITS: u32 = 17;
pub const COC_PSM_MIN: u16 = 128;
pub const COC_PSM_MAX: u16 = 255;
pub const COC_PSM_TRANSPORT: u8 = 3;
pub const COC_CID_MIN: u16 = 64;
pub const COC_CID_MAX: u16 = 127;
pub const COC_DISCONNECT_POLICY: u16 = 9;
pub const COC_REQUEST_SIZE: usize = 68;
pub const COC_REQUEST_CID: usize = 0;
pub const COC_REQUEST_PSM: usize = 2;
pub const COC_REQUEST_STATUS: usize = 4;
pub const COC_REQUEST_STATUS_DETAIL: usize = 6;
pub const COC_REQUEST_FLAGS: usize = 8;
pub const COC_REQUEST_CALLBACK: usize = 12;
pub const COC_REQUEST_ADDRESS: usize = 16;
pub const COC_REQUEST_ADDRESS_TYPE: usize = 22;
pub const COC_CONFIG_MTU: usize = 52;
pub const COC_CONFIG_CREDITS: usize = 60;
pub const COC_CONFIG_MPS: usize = 62;
pub const COC_CHANNEL_QUEUE: usize = 4;
pub const COC_CREDIT_QUEUE_CREDITS: usize = 14;
pub const COC_REGISTER_NODE_SIZE: usize = 16;
pub const COC_REGISTER_NODE_PAYLOAD: usize = 8;
pub const COC_REGISTER_NODE_CALLBACK: usize = 0;
pub const COC_REGISTER_NODE_PSM: usize = 4;
pub const COC_REGISTER_NODE_TRANSPORT: usize = 6;
pub const COC_EV_REQUEST_CID: usize = 0;
pub const COC_EV_REQUEST_PSM: usize = 2;
pub const COC_EV_REQUEST_ADDRESS: usize = 16;
pub const COC_EV_REQUEST_ADDRESS_TYPE: usize = 22;
pub const COC_EV_COMPLETE_CID: usize = 108;
pub const COC_EV_COMPLETE_REMOTE_MTU: usize = 72;
pub const COC_EV_CLOSE_CID: usize = 0;
pub const COC_EV_CLOSE_REASON: usize = 2;
pub const COC_EV_DATA_END: usize = 0;
pub const COC_EV_DATA_OFFSET: usize = 2;
pub const COC_EV_DATA_CID: usize = 4;
pub const COC_EV_DATA_BASE: usize = 4;
pub const COC_EV_PSM_CID: usize = 4;
pub const COC_EV_CREDIT_CID: usize = 0;
pub const COC_EV_CREDIT_ADDED: usize = 2;
pub const SDU_BUFFER_END: usize = 0;
pub const SDU_BUFFER_OFFSET: usize = 2;
pub const SDU_BUFFER_CID: usize = 4;
pub const SDU_BUFFER_META: usize = 8;
pub const SDU_BUFFER_META_VALUE: u8 = 3;
pub const SDU_BUFFER_PAYLOAD: usize = 4;
pub const SDU_BUFFER_ALLOC_TAIL: usize = 6;
pub const FRAGMENT_NODE_SIZE: usize = 16;
pub const FRAGMENT_NODE_PAYLOAD: usize = 8;
pub const FRAGMENT_NODE_BUFFER: usize = 0;
pub const FRAGMENT_NODE_FLAGS: usize = 4;
pub const FRAGMENT_FLAGS_FIRST_LAST: u8 = 129;

/// Raw runtime primitive; call only through qjs_get_exception below.
pub const F_QJS_GET_EXCEPTION: usize = fw::CANOPUS_FW_QJS_TAKE_RUNTIME_EXCEPTION_CALLABLE;
pub const F_QJS_FREE_VALUE: usize = fw::CANOPUS_FW_QJS_FREE_VALUE_CALLABLE;
pub const F_QJS_NEW_C_FUNCTION2: usize = fw::CANOPUS_FW_QJS_NEW_C_FUNCTION2_CALLABLE;
pub const F_QJS_NEW_OBJECT: usize = fw::CANOPUS_FW_QJS_NEW_OBJECT_CALLABLE;
pub const F_QJS_NEW_STRING_LEN: usize = fw::CANOPUS_FW_QJS_NEW_STRING_LEN_CALLABLE;
pub const F_QJS_TO_CSTRING_LEN2: usize = fw::CANOPUS_FW_QJS_TO_CSTRING_LEN2_CALLABLE;
pub const F_QJS_FREE_CSTRING: usize = fw::CANOPUS_FW_QJS_FREE_CSTRING_CALLABLE;
pub const F_QJS_IS_FUNCTION: usize = fw::CANOPUS_FW_QJS_IS_FUNCTION_CALLABLE;
pub const F_QJS_CALL: usize = fw::CANOPUS_FW_QJS_CALL_CALLABLE;
pub const F_QJS_DEFINE_PROPERTY_VALUE_STR: usize =
    fw::CANOPUS_FW_QJS_DEFINE_PROPERTY_VALUE_STR_CALLABLE;
pub const F_QJS_NEW_ARRAY_BUFFER_COPY: usize = fw::CANOPUS_FW_QJS_NEW_ARRAY_BUFFER_COPY_CALLABLE;
pub const F_QJS_GET_ARRAY_BUFFER: usize = fw::CANOPUS_FW_QJS_GET_ARRAY_BUFFER_CALLABLE;
/// Composed accessor below; no direct function pointer is used.
pub const F_QJS_GET_TYPED_ARRAY_BUFFER: usize = 0;
pub const F_FEATURE_REGISTER: usize = 0;
pub const D_FRAMEWORK: usize = fw::canopus_fw_ferry_framework_instance;
pub const D_FRAMEWORK_VTABLE: usize = fw::canopus_fw_ferry_framework_vtable;
pub const D_APPLICATION_VTABLE: usize = fw::canopus_fw_ferry_application_vtable;
pub const VTABLE_PREFIX_WORDS: usize = 2;
/// Unused legacy compatibility placeholder; no factory virtual exists.
pub const FRAMEWORK_FACTORY_SLOT: usize = 0;
pub const FRAMEWORK_COPY_WORDS: usize = 9;
pub const APPLICATION_INIT_SLOT: usize = 2;
pub const APPLICATION_COPY_WORDS: usize = 30;
pub const FRAMEWORK_IMPLEMENTATION: usize = 4;
pub const FRAMEWORK_MUTEX: usize = 200;
pub const FRAMEWORK_APPS_BEGIN: usize = 260;
pub const FRAMEWORK_APPS_END: usize = 264;
pub const APPLICATION_RUNTIME: usize = 296;
pub const RUNTIME_MANAGER: usize = 0;
pub const MANAGER_REGISTRY: usize = 0;
pub const MANAGER_ASYNC: usize = 0;
pub const HANDLE_LOOP: usize = 0;
pub const PROTOTYPE_MANAGER: usize = 0;
pub const INSTANCE_HANDLE: usize = 0;
pub const JSVALUE_CLASS_OFFSET: usize = 6;
pub const ARRAY_BUFFER_DATA: usize = 40;
pub const ARRAY_BUFFER_DETACHED: usize = 4;
pub const F_QJS_GET_GLOBAL_OBJECT: usize = fw::CANOPUS_FW_QJS_GET_GLOBAL_OBJECT_CALLABLE;
pub const F_LEGACY_DECLARE: usize = fw::CANOPUS_FW_QJS_ADD_MODULE_EXPORT_CALLABLE;
pub const F_LEGACY_EXPORT: usize = fw::CANOPUS_FW_QJS_SET_MODULE_EXPORT_CALLABLE;
pub const LEGACY_EXTENSION_LIST: usize = fw::canopus_fw_legacy_extension_list;

/// Firmware async handle storage, in words.
pub const FEATURE_ASYNC_WORDS: usize = 16;

#[inline(always)]
/// QuickJS: create a string value from `length` bytes at `bytes`.
///
/// # Safety
///
/// The context must be the live runtime's, and every pointer argument
/// must be valid for the access the firmware performs.
pub unsafe fn qjs_new_string_len(
    context: *mut ::core::ffi::c_void,
    bytes: *const u8,
    length: usize,
) -> u64 {
    let call: unsafe extern "C" fn(*mut ::core::ffi::c_void, *const u8, usize) -> u64 =
        ::core::mem::transmute(F_QJS_NEW_STRING_LEN | 1);
    call(context, bytes, length)
}

#[inline(always)]
/// QuickJS: copy a string value into a NUL-terminated C string.
///
/// Returns the allocation, which the caller releases through
/// `F_QJS_FREE_CSTRING`, and reports the length through `length`.
///
/// # Safety
///
/// The context must be the live runtime's, and every pointer argument
/// must be valid for the access the firmware performs.
pub unsafe fn qjs_to_cstring_len2(
    context: *mut ::core::ffi::c_void,
    length: *mut usize,
    value: u64,
    flags: i32,
) -> *const u8 {
    let call: unsafe extern "C" fn(*mut ::core::ffi::c_void, *mut usize, u64, i32) -> *const u8 =
        ::core::mem::transmute(F_QJS_TO_CSTRING_LEN2 | 1);
    call(context, length, value, flags)
}

#[inline(always)]
/// QuickJS: define an own property whose name is a byte string.
///
/// Returns a negative value on failure, like the firmware function.
///
/// # Safety
///
/// The context must be the live runtime's, and every pointer argument
/// must be valid for the access the firmware performs.
pub unsafe fn qjs_define_property_value_str(
    context: *mut ::core::ffi::c_void,
    object: u64,
    name: *const u8,
    value: u64,
    flags: i32,
) -> i32 {
    let call: unsafe extern "C" fn(*mut ::core::ffi::c_void, u64, *const u8, u64, i32) -> i32 =
        ::core::mem::transmute(F_QJS_DEFINE_PROPERTY_VALUE_STR | 1);
    call(context, object, name, value, flags)
}

#[inline(always)]
/// QuickJS: create an ArrayBuffer value holding a copy of `length` bytes.
///
/// # Safety
///
/// The context must be the live runtime's, and every pointer argument
/// must be valid for the access the firmware performs.
pub unsafe fn qjs_new_array_buffer_copy(
    context: *mut ::core::ffi::c_void,
    data: *const u8,
    length: usize,
) -> u64 {
    let call: unsafe extern "C" fn(*mut ::core::ffi::c_void, *const u8, usize) -> u64 =
        ::core::mem::transmute(F_QJS_NEW_ARRAY_BUFFER_COPY | 1);
    call(context, data, length)
}

#[inline(always)]
/// QuickJS: read an ArrayBuffer value's payload and length.
///
/// Reports `length` through `length`, returns the payload or null.
///
/// # Safety
///
/// The context must be the live runtime's, and every pointer argument
/// must be valid for the access the firmware performs.
pub unsafe fn qjs_get_array_buffer(
    context: *mut ::core::ffi::c_void,
    length: *mut usize,
    value: u64,
) -> *const u8 {
    let call: unsafe extern "C" fn(*mut ::core::ffi::c_void, *mut usize, u64) -> *const u8 =
        ::core::mem::transmute(F_QJS_GET_ARRAY_BUFFER | 1);
    call(context, length, value)
}

#[inline(always)]
/// QuickJS: unwrap a typed array into its buffer value.
///
/// Fills `offset`, `length` and `element_size`, returns the buffer
/// value or the exception tag.
///
/// # Safety
///
/// The context must be the live runtime's, and every pointer argument
/// must be valid for the access the firmware performs.
pub unsafe fn qjs_get_typed_array_buffer(
    context: *mut ::core::ffi::c_void,
    value: u64,
    offset: *mut usize,
    length: *mut usize,
    element_size: *mut usize,
) -> u64 {
    // Class registration uses21..29 and element shifts table0x2C659800.
    if (value >> 32) as i32 != -1 {
        return 6u64 << 32;
    }
    let object = value as u32 as usize;
    let class = ::core::ptr::read_unaligned((object + 6) as *const u16);
    if !(21..=29).contains(&class) {
        return 6u64 << 32;
    }
    let view = ::core::ptr::read_unaligned((object + 40) as *const usize);
    if view == 0 {
        return 6u64 << 32;
    }
    let buffer = ::core::ptr::read_unaligned((view + 12) as *const usize);
    if buffer == 0 {
        return 6u64 << 32;
    }
    let payload = ::core::ptr::read_unaligned((buffer + 40) as *const usize);
    if payload == 0 || ::core::ptr::read((payload + 4) as *const u8) != 0 {
        // Use native detached-array error, preserving the runtime exception.
        let detached: unsafe extern "C" fn(*mut ::core::ffi::c_void) -> u64 =
            ::core::mem::transmute(F_QJS_ARRAY_BUFFER_DETACHED_ERROR | 1);
        return detached(context);
    }
    if !offset.is_null() {
        *offset = ::core::ptr::read_unaligned((view + 16) as *const u32) as usize;
    }
    if !length.is_null() {
        *length = ::core::ptr::read_unaligned((view + 20) as *const u32) as usize;
    }
    let shift = [0u8, 0, 0, 1, 1, 2, 2, 2, 3][(class - 21) as usize];
    if !element_size.is_null() {
        *element_size = 1usize << shift;
    }
    // Stock buffer getter0xC1BAFB4 returns a duplicated object reference.
    let count = buffer as *mut i32;
    *count += 1;
    (u32::MAX as u64) << 32 | buffer as u32 as u64
}

/// Apply the target's disconnect policy on the Bluelet owner task.
///
/// # Safety
/// `channel` must be live and owned by the current Bluelet task.
pub unsafe fn disconnect_channel(channel: usize, policy: u16) {
    let call: unsafe extern "C" fn(usize, u16) = core::mem::transmute(F_L2CAP_DISCONNECT_CID | 1);
    call(channel, policy);
}

#[inline(always)]
/// Hands one outbound payload to the channel's send path.
///
/// # Safety
///
/// `channel` must be live and `data` must point at `length` readable bytes.
pub unsafe fn queue_send(
    owner: usize,
    channel: usize,
    cid: u16,
    data: *const u8,
    length: u16,
) -> isize {
    let queue = core::ptr::read_unaligned((channel + COC_CHANNEL_QUEUE) as *const usize);
    if queue == 0 {
        return -1;
    }
    let payload_offset = 12u16 + SDU_BUFFER_PAYLOAD as u16;
    // An empty SDU still has its first-fragment two-byte length field.
    let end = payload_offset.wrapping_add(if length == 0 { 2 } else { length });
    let allocate: unsafe extern "C" fn(u32) -> *mut u8 = core::mem::transmute(F_ALLOC | 1);
    let buffer = allocate(end as u32 + SDU_BUFFER_ALLOC_TAIL as u32);
    if buffer.is_null() {
        return -1;
    }
    core::ptr::write_unaligned(buffer.add(SDU_BUFFER_END).cast::<u16>(), end);
    core::ptr::write_unaligned(buffer.add(SDU_BUFFER_OFFSET).cast::<u16>(), payload_offset);
    if payload_offset > 7 {
        *buffer.add(SDU_BUFFER_META) = SDU_BUFFER_META_VALUE;
    }
    if length == 0 {
        // Stock code loops only while the payload length is non-zero, so
        // a zero-length SDU still needs its one first+last fragment.
        *buffer.add(SDU_BUFFER_META) = 1;
        core::ptr::write_unaligned(
            buffer
                .add(SDU_BUFFER_PAYLOAD + payload_offset as usize)
                .cast::<u16>(),
            0,
        );
    } else {
        core::ptr::write_unaligned(buffer.add(SDU_BUFFER_CID).cast::<u32>(), cid as u32);
        core::ptr::copy_nonoverlapping(
            data,
            buffer.add(SDU_BUFFER_PAYLOAD + payload_offset as usize),
            length as usize,
        );
        // The stock handler owns the original SDU and creates its own
        // fragment nodes before releasing that original allocation.
        let send: unsafe extern "C" fn(usize, u32, *mut u8) -> i32 =
            core::mem::transmute(F_L2CAP_SEND | 1);
        return send(owner, COC_QUEUE_EVENT, buffer) as isize;
    }
    let allocation = allocate(FRAGMENT_NODE_SIZE as u32);
    if allocation.is_null() {
        let release: unsafe extern "C" fn(*mut u8) = core::mem::transmute(F_FREE | 1);
        release(buffer);
        return -1;
    }
    // Stock queue insertion preserves the node header; initialize next/prev
    // just like the firmware node allocator before handing it to the queue.
    core::ptr::write_bytes(allocation, 0, FRAGMENT_NODE_SIZE);
    let node = allocation.add(FRAGMENT_NODE_PAYLOAD);
    core::ptr::write_unaligned(
        node.add(FRAGMENT_NODE_BUFFER).cast::<u32>(),
        buffer as usize as u32,
    );
    *node.add(FRAGMENT_NODE_FLAGS) = FRAGMENT_FLAGS_FIRST_LAST;
    let push: unsafe extern "C" fn(usize, *mut u8) -> i32 =
        core::mem::transmute(F_L2CAP_QUEUE_PUSH | 1);
    let flush: unsafe extern "C" fn(usize, usize) -> i32 =
        core::mem::transmute(F_L2CAP_CREDIT_FLUSH | 1);
    push(queue, node);
    flush(queue, channel) as isize
}

/// Writes one byte to a pinned file, returning the firmware's result.
///
/// # Safety
///
/// The context must be the live runtime's, and every pointer argument
/// must be valid for the access the firmware performs.
pub unsafe fn write_byte(file: *mut ::core::ffi::c_void, byte: u8) -> isize {
    let call: unsafe extern "C" fn(*mut ::core::ffi::c_void, *const u8, usize) -> isize =
        ::core::mem::transmute(F_FILE_WRITE | 1);
    call(file, &byte, 1)
}

// Legacy core-module ABI, independently traced in this exact image.
pub const LEGACY_APPLICATION_RUNTIME: usize = 296;
pub const LEGACY_RUNTIME_CONTEXT: usize = 20;
pub const LEGACY_CONTEXT_RUNTIME: usize = 248;
/// Embedded uv_loop_t; add this offset, do not load it as a pointer.
pub const LEGACY_RUNTIME_LOOP: usize = 24;
pub const LEGACY_APPLICATION_DESTROY_SLOT: usize = 8;
pub const LEGACY_FRAMEWORK_ACTIVE_APPLICATION: usize = 256;

/// Declare a native core export before its initializer sets the value.
///
/// # Safety
/// Context, module and NUL-terminated name must belong to the live owner task.
pub unsafe fn legacy_declare(
    context: *mut core::ffi::c_void,
    module: usize,
    name: *const u8,
) -> i32 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, usize, *const u8) -> i32 =
        core::mem::transmute(F_LEGACY_DECLARE | 1);
    call(context, module, name)
}

/// Set an already-declared native core export; consumes value on all paths.
///
/// # Safety
/// All arguments belong to the live JS owner task; name is NUL terminated.
pub unsafe fn legacy_export(
    context: *mut core::ffi::c_void,
    module: usize,
    name: *const u8,
    value: u64,
) -> i32 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, usize, *const u8, u64) -> i32 =
        core::mem::transmute(F_LEGACY_EXPORT | 1);
    call(context, module, name, value)
}

/// Publish the primitive through the measured global-object value API.
/// Consumes value on success and failure, including global acquisition failure.
///
/// # Safety
/// Context and value belong to the live owner task.
pub unsafe fn legacy_publish(context: *mut core::ffi::c_void, value: u64) -> i32 {
    let global: unsafe extern "C" fn(*mut core::ffi::c_void) -> u64 =
        core::mem::transmute(F_QJS_GET_GLOBAL_OBJECT | 1);
    let free: unsafe extern "C" fn(*mut core::ffi::c_void, u64) =
        core::mem::transmute(F_QJS_FREE_VALUE | 1);
    let object = global(context);
    if object == 6u64 << 32 {
        free(context, value);
        return -1;
    }
    let result = qjs_define_property_value_str(
        context,
        object,
        c"__canopusAstrolinkNative".as_ptr().cast(),
        value,
        7,
    );
    free(context, object);
    result
}

/// Resolve the application from this JS context's own AppEffectManager.
///
/// The initializer attaches a 148-byte manager to the context global before
/// native bootstrap, saves the context at manager+112, and native core init
/// saves its owning application at manager+108 before extension init. Unlike
/// Application+296, this association already exists at the extension callback.
///
/// # Safety
/// Call on the live JS owner task with its exact RuntimeContext.
pub unsafe fn legacy_application(runtime: usize) -> usize {
    if runtime == 0 {
        return 0;
    }
    let context = core::ptr::read_unaligned((runtime + LEGACY_RUNTIME_CONTEXT) as *const usize);
    if context == 0
        || core::ptr::read_unaligned((context + LEGACY_CONTEXT_RUNTIME) as *const usize) != runtime
    {
        return 0;
    }
    let global: unsafe extern "C" fn(*mut core::ffi::c_void) -> u64 =
        core::mem::transmute(F_QJS_GET_GLOBAL_OBJECT | 1);
    let free: unsafe extern "C" fn(*mut core::ffi::c_void, u64) =
        core::mem::transmute(F_QJS_FREE_VALUE | 1);
    let value = global(context as *mut core::ffi::c_void);
    if (value >> 32) as i32 != -1 {
        if value != 6u64 << 32 {
            free(context as *mut core::ffi::c_void, value);
        }
        return 0;
    }
    let manager = core::ptr::read_unaligned((value as u32 as usize + 40) as *const usize);
    let application =
        if manager != 0 && core::ptr::read_unaligned((manager + 112) as *const usize) == context {
            core::ptr::read_unaligned((manager + 108) as *const usize)
        } else {
            0
        };
    free(context as *mut core::ffi::c_void, value);
    if application == 0 {
        return 0;
    }
    let published =
        core::ptr::read_unaligned((application + LEGACY_APPLICATION_RUNTIME) as *const usize);
    if published == 0 || published == runtime {
        application
    } else {
        0
    }
}

/// Start an unfiltered legacy 1M active scan through O66 command 213.
///
/// CLI producer 0xC4D7C20 sets mode(u32), legacy(+4), type(+5), PHY(+6).
/// Consumer 0xC4DF360 copies settings8 and filters16; zero filter byte+12
/// disables filtering. Owner 0xC4D0868 maps mode2 to interval128/window32.
///
/// # Safety
/// Adapter and callbacks must stay live on the GAP owner loop; callback table
/// starts with its measured size word (16) followed by report/start/stop slots.
pub unsafe fn create_scan(adapter: usize, callbacks: *const core::ffi::c_void) -> usize {
    #[repr(C)]
    struct ScanSettings {
        mode: u32,
        legacy: u8,
        scan_type: u8,
        phy: u8,
        padding: u8,
    }
    let settings = ScanSettings {
        mode: 2,
        legacy: 1,
        scan_type: 1,
        phy: 0,
        padding: 0,
    };
    let filters = [0u8; 16];
    let call: unsafe extern "C" fn(
        usize,
        *const ScanSettings,
        *const u8,
        *const core::ffi::c_void,
    ) -> usize = core::mem::transmute(F_SCAN_CREATE | 1);
    call(adapter, &settings, filters.as_ptr(), callbacks)
}

/// Stock O66 bootstrap feature map has not yet been bytecode-audited for custom exports.
pub const LEGACY_CORE_EXPORT: bool = false;

/// Native detached-buffer error constructor, measured on O66.
pub const F_QJS_ARRAY_BUFFER_DETACHED_ERROR: usize =
    fw::CANOPUS_FW_QJS_ARRAY_BUFFER_DETACHED_ERROR_CALLABLE;

/// Take the pending exception via the raw runtime primitive, consuming the slot.
///
/// # Safety
/// Context is live on the owning JS task.
pub unsafe fn qjs_get_exception(context: *mut ::core::ffi::c_void) -> u64 {
    let runtime = ::core::ptr::read_unaligned((context as usize + 24) as *const usize);
    let take: unsafe extern "C" fn(usize) -> u64 = ::core::mem::transmute(F_QJS_GET_EXCEPTION | 1);
    take(runtime)
}
