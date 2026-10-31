//! Exact N66 3.1.32 integration, isolated-runtime validation in progress.
//! EVID-ASTROLINK-N66-ABI-001 records every usable entry and incompatible shape.
//! Zero entries are unresolved contracts, not call targets.

use canopus_target_generated as fw;

pub const TARGET_ID: &str = "xiaomi-band-9-3.1.32";
/// Exact entry bytes for legacy advertising/scan, external queue and CoC owner.
pub const SIGNATURES: &[(usize, [u8; 12])] = &[
    (
        0x0c3d8614,
        [
            0x2d, 0xe9, 0xf0, 0x47, 0x90, 0xb0, 0x06, 0x46, 0x0c, 0x46, 0x02, 0xaf,
        ],
    ),
    (
        0x0c3d7598,
        [
            0x2d, 0xe9, 0xf0, 0x47, 0x8c, 0xb0, 0x05, 0x46, 0x04, 0xaf, 0x07, 0x20,
        ],
    ),
    (
        0x0c3ae8e4,
        [
            0x2d, 0xe9, 0xf0, 0x4f, 0x83, 0xb0, 0xdf, 0xf8, 0xc4, 0xa0, 0x81, 0x46,
        ],
    ),
    (
        0x0c3b219c,
        [
            0x2d, 0xe9, 0xf0, 0x47, 0x82, 0xb0, 0x16, 0x7a, 0x14, 0x46, 0x06, 0xf0,
        ],
    ),
];
pub const SUPPORTED: bool = true;
pub const F_ALLOC: usize = fw::CANOPUS_FW_HEAP_MALLOC_NATIVE_CALLABLE;
pub const F_FREE: usize = fw::CANOPUS_FW_HEAP_FREE_NATIVE_CALLABLE;
pub const F_UV_MUTEX_INIT: usize = fw::CANOPUS_FW_UV_MUTEX_INIT_CALLABLE;
pub const F_UV_MUTEX_LOCK: usize = fw::CANOPUS_FW_UV_MUTEX_LOCK_CALLABLE;
pub const F_UV_MUTEX_UNLOCK: usize = fw::CANOPUS_FW_UV_MUTEX_UNLOCK_CALLABLE;
pub const F_UV_ASYNC_INIT: usize = fw::CANOPUS_FW_UV_ASYNC_INIT_CALLABLE;
pub const F_UV_ASYNC_SEND: usize = fw::CANOPUS_FW_UV_ASYNC_SEND_CALLABLE;
pub const FEATURE_ASYNC_WORDS: usize = 16;
pub const F_UV_CLOSE: usize = fw::CANOPUS_FW_UV_CLOSE_CALLABLE;
pub const F_FILES_GETFILEP: usize = 0;
pub const F_FS_GETFILEP: usize = fw::CANOPUS_FW_FS_GETFILEP_BORROWED_CALLABLE;
pub const F_FS_PUTFILEP: usize = 0;
pub const F_FILE_DUP3: usize = 0;
pub const F_FILE_DUP2_NATIVE: usize = fw::CANOPUS_FW_FILE_DUP2_NATIVE_CALLABLE;
pub const F_FILE_WRITE: usize = fw::CANOPUS_FW_FILE_WRITE_CALLABLE;
pub const F_FILE_CLOSE: usize = fw::CANOPUS_FW_FILE_CLOSE_CALLABLE;
pub const CURRENT_TASK: usize = 0x2007da18;
pub const F_NXMUTEX_LOCK: usize = fw::CANOPUS_FW_NXMUTEX_LOCK_CALLABLE;
pub const F_NXMUTEX_UNLOCK: usize = fw::CANOPUS_FW_NXMUTEX_UNLOCK_CALLABLE;
/// EVID-N66-FILE-CAPTURE-002: scheduler-locked task identity walker.
pub const F_NXSCHED_FOREACH: usize = 0x0c13bdd4;
pub const D_UV_KEY: usize = fw::canopus_fw_uv_loop_key;
pub const LOOP_QUEUE_HEAD: usize = 64;
pub const LOOP_MUTEX: usize = 72;
pub const LOOP_ASYNC: usize = 96;
pub const LOOP_READ_FD: usize = 296;
pub const LOOP_WRITE_FD: usize = 300;
pub const ASYNC_BUSY_WORD: usize = 0;
pub const ASYNC_PENDING_WORD: usize = 15;
pub const TCB_TLS: usize = 68;
pub const TCB_GROUP: usize = 8;
pub const GROUP_FILELIST: usize = 84;
pub const FILELIST_ROW: usize = 8;
pub const TASKINFO_UVSTATE: usize = 20;
pub const TASKINFO_UVSTATE_STRIDE: usize = 4;
pub const UVSTATE_LOOP: usize = 584;
pub const FILE_INODE: usize = 16;
pub const INODE_OPS_PIPE: usize = fw::canopus_fw_pipe_file_operations;
pub const UV_KEY_LIMIT: u32 = 3;
pub const F_ADAPTER_REGISTER: usize = 0; // Legacy uses gap_register().
pub const F_ADAPTER_GET_STATE: usize = 0; // Legacy readiness uses gap_ready().
pub const F_ADAPTER_ENABLE: usize = 0; // Legacy btservice owns adapter lifecycle.
pub const F_SCAN_CREATE: usize = 0; // Legacy uses create_scan().
pub const F_SCAN_STOP: usize = 0; // Legacy uses stop_scan().
pub const F_ADVERTISING_CREATE: usize = 0; // Legacy uses create_advertising().
pub const F_ADVERTISING_STOP: usize = 0; // Legacy uses stop_advertising().
pub const D_ADAPTER_INSTANCE: usize = 0; // Legacy owner/client index0, not an IPC pointer.
pub const ADAPTER_STATE_WORD: usize = 0;
pub const ADAPTER_LOOP: usize = 0;
pub const ADAPTER_READY_STATE: i32 = 0;
pub const SCAN_PARAM_SIZE: usize = 5;
pub const SCAN_CALLBACKS_SIZE: u32 = 16;
pub const ADV_CALLBACKS_SIZE: usize = 12;
pub const SCAN_REPORT_ADDRESS: usize = 0;
pub const SCAN_REPORT_RSSI: usize = 7;
pub const SCAN_REPORT_ADDRESS_TYPE: usize = 8;
pub const SCAN_REPORT_PACKET_TYPE: usize = 9;
pub const SCAN_REPORT_LENGTH: usize = 10;
pub const SCAN_REPORT_DATA: usize = 11;
pub const ADV_PARAM_SIZE: usize = 28;
pub const ADV_PARAM_MODE: usize = 0;
pub const ADV_PARAM_TX_POWER: usize = 0;
pub const ADV_PARAM_INTERVAL: usize = 0;
pub const D_L2CAP_OWNER: usize = fw::canopus_fw_bt_l2cap_owner;
pub const F_L2CAP_FIND_LE_PSM: usize = fw::CANOPUS_FW_L2CAP_FIND_LE_PSM_CALLABLE;
pub const F_L2CAP_REGISTER_PSM: usize = fw::CANOPUS_FW_L2CAP_REGISTER_PSM_HANDLER_CALLABLE;
pub const F_L2CAP_RELEASE_PSM: usize = fw::CANOPUS_FW_L2CAP_UNREGISTER_PSM_HANDLER_CALLABLE;
pub const F_L2CAP_CONNECT: usize = fw::CANOPUS_FW_L2CAP_CONNECT_HANDLER_CALLABLE;
pub const F_L2CAP_ACCEPT: usize = fw::CANOPUS_FW_L2CAP_ACCEPT_HANDLER_CALLABLE;
pub const F_L2CAP_SEND: usize = fw::CANOPUS_FW_L2CAP_SEND_HANDLER_CALLABLE;
pub const F_L2CAP_FIND_CHANNEL: usize = fw::CANOPUS_FW_L2CAP_FIND_CHANNEL_CALLABLE;
pub const F_L2CAP_DISCONNECT_CID: usize = fw::CANOPUS_FW_L2CAP_DISCONNECT_CHANNEL_CALLABLE;
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
pub const F_QJS_GET_EXCEPTION: usize = 0;
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
pub const F_QJS_GET_TYPED_ARRAY_BUFFER: usize = 0;
pub const F_FEATURE_REGISTER: usize = 0;
pub const D_FRAMEWORK: usize = fw::canopus_fw_ferry_framework_instance;
pub const D_FRAMEWORK_VTABLE: usize = fw::canopus_fw_ferry_framework_vtable;
pub const D_APPLICATION_VTABLE: usize = fw::canopus_fw_ferry_application_vtable;
pub const VTABLE_PREFIX_WORDS: usize = 2;
pub const FRAMEWORK_FACTORY_SLOT: usize = 0;
pub const FRAMEWORK_COPY_WORDS: usize = 0;
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

pub const LEGACY_EXTENSION_LIST: usize = fw::canopus_fw_legacy_extension_list;
pub const LEGACY_CONTEXT_RUNTIME: usize = 248;
pub const LEGACY_RUNTIME_LOOP: usize = 24;
pub const LEGACY_RUNTIME_CONTEXT: usize = 20;
pub const LEGACY_APPLICATION_RUNTIME: usize = 296;
pub const LEGACY_APPLICATION_DESTROY_SLOT: usize = 19;
pub const LEGACY_FRAMEWORK_ACTIVE_APP: usize = 256;
pub const LEGACY_CONTEXT_JS_RUNTIME: usize = 24;
pub const ASYNC_HAS_BUSY: bool = false;
pub const FILE_RECORD_SIZE: usize = 24;
pub const FILELIST_COUNT: usize = 16;
pub const FILELIST_ARRAY: usize = 20;
pub const FILE_GETFILEP_RETAINS: bool = false;
pub const F_QJS_GET_GLOBAL_OBJECT: usize = fw::CANOPUS_FW_QJS_GET_GLOBAL_OBJECT_CALLABLE;
pub const F_QJS_GET_EXCEPTION_RT: usize = fw::CANOPUS_FW_QJS_GET_EXCEPTION_RT_CALLABLE;

/// Native module declaration runs during the export pass, before module init.
/// # Safety
/// Context/module are live and `name` is NUL-terminated.
pub unsafe fn legacy_declare(
    context: *mut core::ffi::c_void,
    module: usize,
    name: *const u8,
) -> i32 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, usize, *const u8) -> i32 =
        core::mem::transmute(fw::CANOPUS_FW_LEGACY_DECLARE_EXPORT_CALLABLE | 1);
    call(context, module, name)
}

/// Set a declared module export; the engine consumes value on every exit.
/// # Safety
/// Called by the native module initializer on its JS owner task.
pub unsafe fn legacy_export(
    context: *mut core::ffi::c_void,
    module: usize,
    name: *const u8,
    value: u64,
) -> i32 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, usize, *const u8, u64) -> i32 =
        core::mem::transmute(fw::CANOPUS_FW_LEGACY_SET_EXPORT_CALLABLE | 1);
    call(context, module, name, value)
}

/// # Safety
/// Context is live on its owning JS task.
pub unsafe fn qjs_get_exception(context: *mut core::ffi::c_void) -> u64 {
    let runtime = core::ptr::read_unaligned((context as usize + 24) as *const usize);
    let call: unsafe extern "C" fn(usize) -> u64 = core::mem::transmute(F_QJS_GET_EXCEPTION_RT | 1);
    call(runtime)
}

/// # Safety
/// Values belong to the live context; free consumes one owned reference.
pub unsafe fn qjs_free(context: *mut core::ffi::c_void, value: u64) {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, u64) =
        core::mem::transmute(F_QJS_FREE_VALUE | 1);
    call(context, value)
}

/// Publish the native primitive; consumes value even if acquiring global fails.
/// # Safety
/// Context and value belong to the current application task.
pub unsafe fn legacy_publish(context: *mut core::ffi::c_void, value: u64) -> i32 {
    let global: unsafe extern "C" fn(*mut core::ffi::c_void) -> u64 =
        core::mem::transmute(F_QJS_GET_GLOBAL_OBJECT | 1);
    let object = global(context);
    if (object >> 32) as i32 != -1 {
        qjs_free(context, value);
        qjs_free(context, object);
        return -1;
    }
    let result = qjs_define_property_value_str(
        context,
        object,
        c"__canopusAstrolinkNative".as_ptr().cast(),
        value,
        7,
    );
    qjs_free(context, object);
    result
}

/// Recover the app bound by nativeproxy initialization before extensions run.
/// Application+296 is not populated until after bootstrap; that word cannot be
/// the sole identity check here. Global object's AppEffectManager+92 is set by
/// nativeproxy init from the framework's active app before extension callbacks.
/// # Safety
/// Only call during a native extension callback on its live JS task.
pub unsafe fn legacy_application(runtime: usize) -> usize {
    let context = core::ptr::read_unaligned((runtime + LEGACY_RUNTIME_CONTEXT) as *const usize);
    if context == 0
        || core::ptr::read_unaligned((context + LEGACY_CONTEXT_RUNTIME) as *const usize) != runtime
    {
        return 0;
    }
    let get_global: unsafe extern "C" fn(usize) -> u64 =
        core::mem::transmute(F_QJS_GET_GLOBAL_OBJECT | 1);
    let global = get_global(context);
    if (global >> 32) as i32 != -1 {
        qjs_free(context as *mut _, global);
        return 0;
    }
    let manager = core::ptr::read_unaligned((global as u32 as usize + 40) as *const usize);
    let application = if manager == 0 {
        0
    } else {
        core::ptr::read_unaligned((manager + 92) as *const usize)
    };
    qjs_free(context as *mut _, global);
    application
}

/// # Safety
/// Input bytes readable and context is live.
pub unsafe fn qjs_new_string_len(
    context: *mut core::ffi::c_void,
    bytes: *const u8,
    length: usize,
) -> u64 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, *const u8, usize) -> u64 =
        core::mem::transmute(F_QJS_NEW_STRING_LEN | 1);
    call(context, bytes, length)
}
/// # Safety
/// Live context and borrowed JS value, returned C string must be released.
pub unsafe fn qjs_to_cstring_len2(
    context: *mut core::ffi::c_void,
    length: *mut usize,
    value: u64,
    flags: i32,
) -> *const u8 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, *mut usize, u64, i32) -> *const u8 =
        core::mem::transmute(F_QJS_TO_CSTRING_LEN2 | 1);
    call(context, length, value, flags)
}
/// # Safety
/// Live owner context, borrowed object, NUL name. Value is consumed.
pub unsafe fn qjs_define_property_value_str(
    context: *mut core::ffi::c_void,
    object: u64,
    name: *const u8,
    value: u64,
    flags: i32,
) -> i32 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, u64, *const u8, u64, i32) -> i32 =
        core::mem::transmute(F_QJS_DEFINE_PROPERTY_VALUE_STR | 1);
    call(context, object, name, value, flags)
}
/// # Safety
/// Input bytes readable and context is live; engine copies the data.
pub unsafe fn qjs_new_array_buffer_copy(
    context: *mut core::ffi::c_void,
    bytes: *const u8,
    length: usize,
) -> u64 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, *const u8, usize) -> u64 =
        core::mem::transmute(F_QJS_NEW_ARRAY_BUFFER_COPY | 1);
    call(context, bytes, length)
}
/// # Safety
/// Value is borrowed from live context.
pub unsafe fn qjs_get_array_buffer(
    context: *mut core::ffi::c_void,
    length: *mut usize,
    value: u64,
) -> *const u8 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, *mut usize, u64) -> *const u8 =
        core::mem::transmute(F_QJS_GET_ARRAY_BUFFER | 1);
    call(context, length, value)
}
/// Measured typed array layout; validation helper supplies the exception path.
/// # Safety
/// Value belongs to live context; output pointers are writable.
pub unsafe fn qjs_get_typed_array_buffer(
    context: *mut core::ffi::c_void,
    value: u64,
    offset: *mut usize,
    length: *mut usize,
    element_size: *mut usize,
) -> u64 {
    let validate: unsafe extern "C" fn(*mut core::ffi::c_void, u64, i32) -> usize =
        core::mem::transmute(fw::CANOPUS_FW_QJS_TYPED_ARRAY_VALIDATE_CALLABLE | 1);
    let object = validate(context, value, 0);
    if object == 0 {
        return 6u64 << 32;
    }
    let class = core::ptr::read_unaligned((object + 6) as *const u16) as usize;
    let shift = core::ptr::read(
        (fw::canopus_fw_legacy_typed_array_element_shifts + class - 21) as *const u8,
    );
    let typed = core::ptr::read_unaligned((object + 40) as *const usize);
    *offset = core::ptr::read_unaligned((typed + 16) as *const u32) as usize;
    *length = core::ptr::read_unaligned((typed + 20) as *const u32) as usize;
    *element_size = 1usize << shift;
    let backing = core::ptr::read_unaligned((typed + 12) as *const usize);
    *(backing as *mut i32) += 1;
    (u32::MAX as u64) << 32 | backing as u32 as u64
}
/// # Safety
/// File belongs to caller and remains alive during the write.
pub unsafe fn write_byte(file: *mut core::ffi::c_void, byte: u8) -> isize {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, *const u8, usize) -> isize =
        core::mem::transmute(F_FILE_WRITE | 1);
    call(file, &byte, 1)
}
/// # Safety
/// Live channel on Bluelet owner task.
pub unsafe fn disconnect_channel(channel: usize, policy: u16) {
    let call: unsafe extern "C" fn(usize, u16) -> i32 =
        core::mem::transmute(F_L2CAP_DISCONNECT_CID | 1);
    call(channel, policy);
}
/// Nonempty SDUs use the original-buffer owner handler. Empty SDUs use an
/// explicit first+last fragment node and credit flush, matching the same owner.
/// # Safety
/// Live owner/channel; data points to length readable bytes.
pub unsafe fn queue_send(
    owner: usize,
    channel: usize,
    cid: u16,
    data: *const u8,
    length: u16,
) -> isize {
    if length != 0 && data.is_null() {
        return -22;
    }
    let allocate: unsafe extern "C" fn(u32) -> *mut u8 = core::mem::transmute(F_ALLOC | 1);
    let end = length as usize + 16 + if length == 0 { 2 } else { 0 };
    let buffer = allocate((end + 6) as u32);
    if buffer.is_null() {
        return -12;
    }
    core::ptr::write_bytes(buffer, 0, end + 6);
    core::ptr::write_unaligned(buffer.cast::<u16>(), end as u16);
    core::ptr::write_unaligned(buffer.add(2).cast::<u16>(), 16);
    core::ptr::write_unaligned(buffer.add(4).cast::<u32>(), cid as u32);
    *buffer.add(8) = 3;
    if length != 0 {
        core::ptr::copy_nonoverlapping(data, buffer.add(20), length as usize);
        let send: unsafe extern "C" fn(usize, u32, *mut u8) -> i32 =
            core::mem::transmute(F_L2CAP_SEND | 1);
        return send(owner, COC_QUEUE_EVENT, buffer) as isize;
    }
    // Stock send loops only over nonempty payloads. Preserve its 2-byte zero
    // length prefix while bypassing that loop with an explicit fragment.
    core::ptr::write_unaligned(buffer.add(20).cast::<u16>(), 0);
    // Header-type metadata stays3 here; fragment flags live on node+4.
    let queue = core::ptr::read_unaligned((channel + COC_CHANNEL_QUEUE) as *const usize);
    if queue == 0 {
        let release: unsafe extern "C" fn(*mut u8) = core::mem::transmute(F_FREE | 1);
        release(buffer);
        return -1;
    }
    let allocation = allocate(FRAGMENT_NODE_SIZE as u32);
    if allocation.is_null() {
        let release: unsafe extern "C" fn(*mut u8) = core::mem::transmute(F_FREE | 1);
        release(buffer);
        return -12;
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
    let push: unsafe extern "C" fn(usize, *mut u8) -> usize =
        core::mem::transmute(F_L2CAP_QUEUE_PUSH | 1);
    let flush: unsafe extern "C" fn(usize, usize) -> usize =
        core::mem::transmute(F_L2CAP_CREDIT_FLUSH | 1);
    push(queue, node);
    let result = flush(queue, channel);
    // If the queue had no credit the node remains queued and owns its SDU;
    // the ordinary event17/credit flush retires it later.
    result as isize
}

// ---- Legacy exclusive GAP transport ----------------------------------------

/// BTM/BTS use caller-owned handles, not generic MiWear IPC handles. The
/// firmware owns controller settings; stopping a scan is not independent of
/// other clients' controller state.
pub const LEGACY_EXCLUSIVE_GAP: bool = true;
pub const LEGACY_SCAN_STOP_IS_GLOBAL: bool = true;
pub const D_BT_SERVICE_LOOP: usize = 0x200ac608;
pub const D_GAP_READY_FLAGS: usize = 0x200ac470;
pub const GAP_READY_MASK: u32 = 0x300;
pub const GAP_READY_VALUE: u32 = 0x300;
pub const F_LEGACY_SCAN_START: usize = fw::CANOPUS_FW_LEGACY_SCAN_START_CALLABLE;
pub const F_LEGACY_SCAN_STOP: usize = fw::CANOPUS_FW_LEGACY_SCAN_STOP_CALLABLE;
pub const F_LEGACY_ADV_START: usize = fw::CANOPUS_FW_LEGACY_ADVERTISING_START_CALLABLE;
pub const F_LEGACY_ADV_STOP: usize = fw::CANOPUS_FW_LEGACY_ADVERTISING_STOP_CALLABLE;
pub const F_GAP_REGISTER: usize = fw::CANOPUS_FW_GAP_REGISTER_CALLBACKS_CALLABLE;
static LAST_GAP_ERROR: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(3);
/// Preserve the native synchronous failure for the validation facade.
/// # Safety
/// Call only after a legacy GAP creation attempt on the same serialized task.
pub unsafe fn take_gap_error() -> crate::core::Error {
    crate::core::Error::Backend(LAST_GAP_ERROR.load(core::sync::atomic::Ordering::Acquire))
}

#[repr(C)]
struct SharedCell<T>(core::cell::UnsafeCell<T>);
unsafe impl<T> Sync for SharedCell<T> {}

type ScanFn = extern "C" fn(usize, *const u8);
type ScanStartFn = extern "C" fn(usize);
type ScanStopFn = extern "C" fn(usize);
type ScanFailureFn = extern "C" fn(usize, u8);
type CommonScanStartFn = extern "C" fn(usize, u8);

#[repr(C)]
pub struct LegacyScanState {
    native: usize,
    generation: u32,
    callbacks: LegacyScanCallbacks,
    common: *const core::ffi::c_void,
}
pub struct LegacyScan(core::cell::UnsafeCell<LegacyScanState>);
unsafe impl Sync for LegacyScan {}

const fn scan_state(index: usize) -> LegacyScan {
    let (report, failed, started, stopped): (ScanFn, ScanFailureFn, ScanStartFn, ScanStopFn) =
        match index {
            0 => (
                scan_report_legacy::<0>,
                scan_failed_legacy::<0>,
                scan_started_legacy::<0>,
                scan_stopped_legacy::<0>,
            ),
            1 => (
                scan_report_legacy::<1>,
                scan_failed_legacy::<1>,
                scan_started_legacy::<1>,
                scan_stopped_legacy::<1>,
            ),
            2 => (
                scan_report_legacy::<2>,
                scan_failed_legacy::<2>,
                scan_started_legacy::<2>,
                scan_stopped_legacy::<2>,
            ),
            3 => (
                scan_report_legacy::<3>,
                scan_failed_legacy::<3>,
                scan_started_legacy::<3>,
                scan_stopped_legacy::<3>,
            ),
            _ => unreachable!(),
        };
    LegacyScan(core::cell::UnsafeCell::new(LegacyScanState {
        native: 0,
        generation: 0,
        callbacks: LegacyScanCallbacks {
            report,
            failed,
            started,
            stopped,
        },
        common: core::ptr::null(),
    }))
}

static SCANS: [LegacyScan; 4] = [scan_state(0), scan_state(1), scan_state(2), scan_state(3)];

#[repr(C)]
struct LegacyScanCallbacks {
    report: extern "C" fn(usize, *const u8),
    failed: extern "C" fn(usize, u8),
    started: extern "C" fn(usize),
    stopped: extern "C" fn(usize),
}
#[repr(C)]
struct LegacyCommonCallbacks {
    size: u32,
    report: extern "C" fn(usize, *const u8),
    started: CommonScanStartFn,
    stopped: extern "C" fn(usize),
}

unsafe fn legacy_scan_context<const INDEX: usize>(
    handle: usize,
) -> Option<(usize, *const LegacyCommonCallbacks, u32)> {
    if INDEX >= 4 || handle == 0 {
        return None;
    }
    let state = SCANS[INDEX].0.get();
    let native_slot = core::ptr::read_unaligned(handle as *const usize);
    let generation = core::ptr::read_unaligned((handle + 4) as *const u32);
    if native_slot != state as usize || generation != (*state).generation {
        return None;
    }
    Some((state as usize, (*state).common.cast(), generation))
}

extern "C" fn scan_report_legacy<const INDEX: usize>(handle: usize, report: *const u8) {
    unsafe {
        let Some((_, callbacks, generation)) = legacy_scan_context::<INDEX>(handle) else {
            return;
        };
        if let Some(callbacks) = callbacks.as_ref() {
            (callbacks.report)(INDEX | (generation as usize) << 8, report);
        }
    }
}

extern "C" fn scan_failed_legacy<const INDEX: usize>(handle: usize, status: u8) {
    unsafe {
        let Some((_, callbacks, generation)) = legacy_scan_context::<INDEX>(handle) else {
            return;
        };
        if let Some(callbacks) = callbacks.as_ref() {
            (callbacks.started)(INDEX | (generation as usize) << 8, status);
        }
        // BTM frees the native handle and clears the caller slot only after
        // this callback returns. Do not reuse state from inside this callback.
    }
}

extern "C" fn scan_started_legacy<const INDEX: usize>(handle: usize) {
    unsafe {
        let Some((_, callbacks, generation)) = legacy_scan_context::<INDEX>(handle) else {
            return;
        };
        if let Some(callbacks) = callbacks.as_ref() {
            (callbacks.started)(INDEX | (generation as usize) << 8, 0);
        }
    }
}

extern "C" fn scan_stopped_legacy<const INDEX: usize>(handle: usize) {
    unsafe {
        let Some((_, callbacks, generation)) = legacy_scan_context::<INDEX>(handle) else {
            return;
        };
        if let Some(callbacks) = callbacks.as_ref() {
            (callbacks.stopped)(INDEX | (generation as usize) << 8);
        }
    }
}

/// Legacy BTM scan writes a caller-owned handle slot and has four callbacks,
/// unlike the newer IPC handle-returning API. Its complete parameter conversion
/// and persistent handle storage have not been proven; fail closed.
/// # Safety
/// `callbacks` would need to be a resident legacy callback adapter.
pub unsafe fn create_scan(_adapter: usize, callbacks: *const core::ffi::c_void) -> usize {
    if callbacks.is_null() {
        return 0;
    }
    let index =
        (0..4).find(|&index| core::ptr::read_unaligned(SCANS[index].0.get() as *const usize) == 0);
    let Some(index) = index else { return 0 };
    let state = SCANS[index].0.get();
    let generation = (*state).generation.wrapping_add(1).max(1);
    (*state).generation = generation;
    (*state).common = callbacks;
    (*state).native = 1;
    let settings = [
        0x90, 0x01, 0x68, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
    ];
    let filter = [0u8; 6];
    let start: unsafe extern "C" fn(
        *mut usize,
        *const u8,
        *const u8,
        *const LegacyScanCallbacks,
    ) -> i32 = core::mem::transmute(F_LEGACY_SCAN_START | 1);
    let result = start(
        &mut (*state).native,
        settings.as_ptr(),
        filter.as_ptr(),
        (&raw const (*state).callbacks).cast::<LegacyScanCallbacks>(),
    );
    LAST_GAP_ERROR.store(result, core::sync::atomic::Ordering::Release);
    if result != 0 {
        (*state).native = 0;
        (*state).common = core::ptr::null();
        return 0;
    }
    index | (generation as usize) << 8
}

/// Forward a native BTM failure through the common started callback with the
/// actual error byte. It is not a stopped notification. Return true only when
/// the token matches a live target slot; stale events are ignored.
/// # Safety
/// Called synchronously from the btservice callback before native slot clears.
pub unsafe fn scan_failed(token: usize, native_error: u8) -> bool {
    let index = token & 0xff;
    if index >= 4 {
        return false;
    }
    let state = SCANS[index].0.get();
    if (*state).generation as usize != token >> 8 || (*state).common.is_null() {
        return false;
    }
    let callbacks = &*(*state).common.cast::<LegacyCommonCallbacks>();
    (callbacks.started)(token, native_error);
    true
}

/// Normal BTM stopped notification, distinct from the failure callback.
/// Return true only for a live target slot; do not synthesize a start error.
/// # Safety
/// `token` must have been produced by this target's live scan callback table.
pub unsafe fn scan_stopped(token: usize) -> bool {
    let index = token & 0xff;
    if index >= 4 {
        return false;
    }
    let state = SCANS[index].0.get();
    if (*state).generation as usize != token >> 8 || (*state).common.is_null() {
        return false;
    }
    let callbacks = &*(*state).common.cast::<LegacyCommonCallbacks>();
    (callbacks.stopped)(token);
    true
}

/// Stop legacy scan using its stable target handle, not the native BTM id.
/// # Safety
/// Handle was returned by this target's create_scan and belongs to caller.
pub unsafe fn stop_scan(_adapter: usize, token: usize) -> i32 {
    let index = token & 0xff;
    let generation = (token >> 8) as u32;
    if index < 4 {
        let state = SCANS[index].0.get();
        if (*state).generation == generation {
            let native = core::ptr::read_unaligned((*state).native as *const usize);
            if native != 0 {
                let stop: unsafe extern "C" fn(usize) -> i32 =
                    core::mem::transmute(F_LEGACY_SCAN_STOP | 1);
                return stop(native);
            }
        }
    }
    0
}

/// # Safety
/// The btservice loop is running and the target's GAP is initialized.
pub unsafe fn gap_loop() -> usize {
    core::ptr::read_volatile(D_BT_SERVICE_LOOP as *const usize)
}
/// # Safety
/// Volatile readiness flags live in this exact firmware's BSS.
pub unsafe fn gap_ready() -> bool {
    core::ptr::read_volatile(D_GAP_READY_FLAGS as *const u32) & GAP_READY_MASK == GAP_READY_VALUE
}

/// One resident per-owner GAP registration is required before advertising.
/// Callback dispatch is at table+76 started(owner,adv_id) and +80 stopped(owner).
/// # Safety
/// Caller owns the GAP owner until unregister; the callbacks stay resident.
pub unsafe fn gap_register(owner: usize, callbacks: *const core::ffi::c_void) -> i32 {
    // 0xC3D28E4 ignores arg0, reads the owner through arg1 and stores arg2 as
    // the raw callback table. Passing (owner, callbacks, null) registers the
    // right owner with no callbacks and can never deliver advertising events.
    let call: unsafe extern "C" fn(usize, *const usize, *const core::ffi::c_void) -> i32 =
        core::mem::transmute(F_GAP_REGISTER | 1);
    call(0, &owner, callbacks)
}

/// Resident owner advertising id/generation. The caller's existing12-byte
/// common AdvCallbacks is used directly: +4 started(handle,id,status),
/// +8 stopped(handle,id). Both callbacks are passed the same token.
#[repr(C)]
struct LegacyAdvState {
    id: u8,
    generation: u32,
    active: bool,
    callbacks: *const core::ffi::c_void,
}
static ADVERTISEMENTS: [SharedCell<LegacyAdvState>; 4] = [
    SharedCell(core::cell::UnsafeCell::new(LegacyAdvState {
        id: 0,
        generation: 0,
        active: false,
        callbacks: core::ptr::null(),
    })),
    SharedCell(core::cell::UnsafeCell::new(LegacyAdvState {
        id: 0,
        generation: 0,
        active: false,
        callbacks: core::ptr::null(),
    })),
    SharedCell(core::cell::UnsafeCell::new(LegacyAdvState {
        id: 0,
        generation: 0,
        active: false,
        callbacks: core::ptr::null(),
    })),
    SharedCell(core::cell::UnsafeCell::new(LegacyAdvState {
        id: 0,
        generation: 0,
        active: false,
        callbacks: core::ptr::null(),
    })),
];
#[repr(C)]
struct LegacyAdvCallbacks {
    reserved: u32,
    started: extern "C" fn(usize, u8, u8),
    stopped: extern "C" fn(usize, u8),
}

/// The owner is an index registered by the legacy GAP helper, not a MiWear IPC
/// instance. Callback table is the caller's resident common12-byte adapter.
/// # Safety
/// Serialized on btservice loop; callbacks and payload valid during this call.
pub unsafe fn create_advertising(
    owner: usize,
    callbacks: *const core::ffi::c_void,
    data: *const u8,
    length: u32,
) -> usize {
    if callbacks.is_null() || data.is_null() || length == 0 || length > 31 {
        return 0;
    }
    let Some(index) = (0..4).find(|&index| !(*ADVERTISEMENTS[index].0.get()).active) else {
        return 0;
    };
    let state = ADVERTISEMENTS[index].0.get();
    let generation = (*state).generation.wrapping_add(1).max(1);
    let mut request = [0u8; 48];
    request[0] = 4;
    request[4] = 8;
    core::ptr::write_unaligned(request.as_mut_ptr().add(8).cast::<u32>(), 1600);
    core::ptr::write_unaligned(request.as_mut_ptr().add(12).cast::<u16>(), 10);
    core::ptr::write_unaligned(request.as_mut_ptr().add(16).cast::<u16>(), 30);
    core::ptr::write_unaligned(request.as_mut_ptr().add(20).cast::<u32>(), 320);
    request[21] = 0xf6;
    request[24] = 0;
    core::ptr::write_unaligned(request.as_mut_ptr().add(28).cast::<u16>(), length as u16);
    core::ptr::write_unaligned(
        request.as_mut_ptr().add(32).cast::<u32>(),
        data as usize as u32,
    );
    let start: unsafe extern "C" fn(usize, *mut u8) -> i32 =
        core::mem::transmute(F_LEGACY_ADV_START | 1);
    let status = start(owner, request.as_mut_ptr());
    if status != 0 {
        return 0;
    }
    (*state).id = request[0];
    (*state).generation = generation;
    (*state).callbacks = callbacks;
    (*state).active = true;
    index | (generation as usize) << 8
}

/// # Safety
/// Handle is a live token from this target's create_advertising.
pub unsafe fn stop_advertising(owner: usize, token: usize) -> i32 {
    let index = token & 0xff;
    if index >= 4 {
        return 0;
    }
    let state = ADVERTISEMENTS[index].0.get();
    if !(*state).active || (*state).generation as usize != token >> 8 {
        return 0;
    }
    let stop: unsafe extern "C" fn(usize, i32) -> i32 = core::mem::transmute(F_LEGACY_ADV_STOP | 1);
    stop(owner, (*state).id as i32)
}

fn adv_common<const INDEX: usize>(id: u8) -> Option<(usize, *const LegacyAdvCallbacks)> {
    if INDEX >= 4 {
        return None;
    }
    let state = ADVERTISEMENTS[INDEX].0.get();
    unsafe {
        if !(*state).active || (*state).id != id {
            return None;
        }
        Some((
            INDEX | ((*state).generation as usize) << 8,
            (*state).callbacks.cast(),
        ))
    }
}

extern "C" fn adv_started<const INDEX: usize>(id: u8) {
    let Some((token, callbacks)) = adv_common::<INDEX>(id) else {
        return;
    };
    unsafe {
        if let Some(callbacks) = callbacks.as_ref() {
            (callbacks.started)(token, id, 0);
        }
    }
}
extern "C" fn adv_stopped<const INDEX: usize>(id: u8) {
    let Some((token, callbacks)) = adv_common::<INDEX>(id) else {
        return;
    };
    unsafe {
        if let Some(callbacks) = callbacks.as_ref() {
            (callbacks.stopped)(token, id);
        }
        (*ADVERTISEMENTS[INDEX].0.get()).active = false;
        (*ADVERTISEMENTS[INDEX].0.get()).callbacks = core::ptr::null();
    }
}

/// Target-owned84-byte raw GAP callback function arrays. Slots19/20 (+76/+80)
/// are owner-scoped adv started/stopped. Other callback slots remain null.
/// BTS never interprets this as the common packed advertising callback table.
#[repr(C)]
struct LegacyOwnerCallbacks {
    slots: [Option<extern "C" fn(usize, u8)>; 21],
}
const EMPTY_OWNER_CALLBACKS: LegacyOwnerCallbacks = LegacyOwnerCallbacks { slots: [None; 21] };
static mut ADV_CALLBACKS: [LegacyOwnerCallbacks; 4] = [
    EMPTY_OWNER_CALLBACKS,
    EMPTY_OWNER_CALLBACKS,
    EMPTY_OWNER_CALLBACKS,
    EMPTY_OWNER_CALLBACKS,
];

extern "C" fn owner_adv_started(owner: usize, id: u8) {
    match owner {
        0 => adv_started::<0>(id),
        1 => adv_started::<1>(id),
        2 => adv_started::<2>(id),
        3 => adv_started::<3>(id),
        _ => {}
    }
}
extern "C" fn owner_adv_stopped(owner: usize, id: u8) {
    match owner {
        0 => adv_stopped::<0>(id),
        1 => adv_stopped::<1>(id),
        2 => adv_stopped::<2>(id),
        3 => adv_stopped::<3>(id),
        _ => {}
    }
}

/// # Safety
/// One serialized target installation assigns these tables once; callbacks
/// remain resident until reboot and only this target uses these owner indices.
pub unsafe fn gap_owner_callbacks(index: usize) -> *const core::ffi::c_void {
    if index >= 4 {
        return core::ptr::null();
    }
    ADV_CALLBACKS[index].slots[19] = Some(owner_adv_started);
    ADV_CALLBACKS[index].slots[20] = Some(owner_adv_stopped);
    (&raw const ADV_CALLBACKS[index]).cast()
}
