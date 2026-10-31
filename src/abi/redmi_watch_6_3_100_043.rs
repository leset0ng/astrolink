//! Redmi Watch 6, exact firmware 3.100.043 (P65).
//! Legacy native-extension integration. Static recovery is incomplete: zero
//! entries below are deliberately unresolved and SUPPORTED stays false.

pub const SUPPORTED: bool = false;
pub const TARGET_ID: &str = "redmi-watch-6-3.100.043";

/// Entry bytes of `ble_advertising_start`, `ble_scan_start`, `bt_queue_external`
/// and the CoC connect owner handler. `backend::supported` compares them before
/// any firmware call, so a mismatched image is refused instead of called.
pub const SIGNATURES: &[(usize, [u8; 12])] = &[
    (
        0x1c00a908,
        [
            0x2d, 0xe9, 0xf0, 0x47, 0xad, 0xf5, 0x1a, 0x7d, 0x00, 0xaf, 0xb7, 0xf8,
        ],
    ),
    (
        0x1c00aaa4,
        [
            0x2d, 0xe9, 0xf0, 0x43, 0x05, 0x46, 0xad, 0xf5, 0x1b, 0x7d, 0x00, 0xaf,
        ],
    ),
    (
        0x1c1676c8,
        [
            0x5f, 0xf8, 0x00, 0xf0, 0x8d, 0xfd, 0x98, 0x0c, 0x5f, 0xf8, 0x00, 0xf0,
        ],
    ),
    (
        0x1c067018,
        [
            0x2d, 0xe9, 0xf0, 0x47, 0x82, 0xb0, 0x16, 0x7a, 0x14, 0x46, 0x16, 0xf0,
        ],
    ),
];

use canopus_target_generated as fw;

// ---- kernel and libuv -------------------------------------------------------
pub const F_ALLOC: usize = 0x0c50b2a0;
pub const F_FREE: usize = 0x0c50b25c;
pub const F_UV_MUTEX_INIT: usize = 0x0c629280;
pub const F_UV_MUTEX_LOCK: usize = 0x0c62935c;
pub const F_UV_MUTEX_UNLOCK: usize = 0x0c629378;
pub const F_UV_ASYNC_INIT: usize = 0x0c621bdc;
pub const F_UV_CLOSE: usize = 0x0c621f98;
pub const F_FILES_GETFILEP: usize = 0x1c1542b4;
pub const F_FS_GETFILEP: usize = 0x1c154700;
pub const F_FS_PUTFILEP: usize = 0x1c15479c;
pub const F_FILE_DUP3: usize = 0x1c156380;
pub const F_FILE_WRITE: usize = 0x1c159a90;

pub const F_FILE_CLOSE: usize = 0x1c1562e4;
pub const F_NXSCHED_FOREACH: usize = 0x0c4c6078;
/// libuv loop key used by the QuickJS task's `uv_loop_t` slot vector.
pub const D_UV_KEY: usize = 0x3c44c388;

// ---- default loop and task layout ------------------------------------------
/// Intrusive ready-work queue head, mutex and async handle in `uv_loop_t`.
pub const LOOP_QUEUE_HEAD: usize = 64;
pub const LOOP_MUTEX: usize = 72;
pub const LOOP_ASYNC: usize = 96;
/// Default loop's pipe endpoints as integer descriptors.
pub const LOOP_READ_FD: usize = 360;
pub const LOOP_WRITE_FD: usize = 364;
/// `uv_async_t` busy/pending words.
pub const ASYNC_BUSY_WORD: usize = 6;
pub const ASYNC_PENDING_WORD: usize = 31;
pub const ASYNC_HAS_BUSY: bool = true;
/// NuttX task descriptor: TLS pointer, task group, and the group's file list.
pub const TCB_TLS: usize = 68;
pub const TCB_GROUP: usize = 8;
pub const GROUP_FILELIST: usize = 88;
pub const FILELIST_ROW: usize = 8;
pub const FILELIST_COUNT: usize = 0;
pub const FILELIST_ARRAY: usize = 4;
/// `tls -> taskinfo -> uv state slots[key] -> loop`.
pub const TASKINFO_UVSTATE: usize = 20;
pub const TASKINFO_UVSTATE_STRIDE: usize = 4;
pub const UVSTATE_LOOP: usize = 768;
/// 32-byte retained `file` record, reference count and inode pointer.
pub const FILE_RECORD_SIZE: usize = 32;
pub const FILE_REFCOUNT: usize = 4;
pub const FILE_GETFILEP_RETAINS: bool = true;
pub const FILE_INODE: usize = 16;
/// Exact P65 pipe operations identity; both ends must share this inode.
pub const INODE_OPS_PIPE: usize = 0x3c169f7c;

pub const UV_KEY_LIMIT: u32 = 3;

// ---- adapter and GAP -------------------------------------------------------
pub const F_ADAPTER_REGISTER: usize = 0x1c008608;
pub const F_ADAPTER_GET_STATE: usize = 0x1c008720;
pub const F_ADAPTER_ENABLE: usize = 0x1c0086d0;
pub const F_SCAN_CREATE: usize = 0x1c00aa40;

pub const F_SCAN_STOP: usize = 0x1c00ab28;

pub const F_ADVERTISING_CREATE: usize = 0x1c00a908;

pub const F_ADVERTISING_STOP: usize = 0x1c00a9ac;

/// Process-lifetime MiWear Bluetooth IPC instance created by firmware boot.
pub const D_ADAPTER_INSTANCE: usize = 0x3c4942b0;
/// Business-loop pointer inside the IPC client; +48 is the response loop.
pub const ADAPTER_STATE_WORD: usize = 0;
pub const ADAPTER_LOOP: usize = 52;
pub const ADAPTER_READY_STATE: i32 = 4;

pub const SCAN_PARAM_SIZE: usize = 8;
pub const SCAN_CALLBACKS_SIZE: u32 = 16;
pub const ADV_CALLBACKS_SIZE: usize = 12;
/// Scan result report: address, RSSI, address type, packet type, AD length, AD.
pub const SCAN_REPORT_ADDRESS: usize = 0;
pub const SCAN_REPORT_RSSI: usize = 7;
pub const SCAN_REPORT_ADDRESS_TYPE: usize = 8;
pub const SCAN_REPORT_PACKET_TYPE: usize = 9;
pub const SCAN_REPORT_LENGTH: usize = 10;
pub const SCAN_REPORT_DATA: usize = 11;
/// `bt_le_adv_param_t`.
pub const ADV_PARAM_SIZE: usize = 28;
pub const ADV_PARAM_MODE: usize = 0;
pub const ADV_PARAM_TX_POWER: usize = 20;
pub const ADV_PARAM_INTERVAL: usize = 16;

// ---- legacy LE CoC ---------------------------------------------------------
pub const D_L2CAP_OWNER: usize = fw::canopus_fw_bt_l2cap_owner;

pub const F_L2CAP_FIND_LE_PSM: usize = fw::CANOPUS_FW_L2CAP_FIND_LE_PSM_CALLABLE;
pub const F_L2CAP_REGISTER_PSM: usize = fw::CANOPUS_FW_L2CAP_REGISTER_PSM_HANDLER_CALLABLE;
/// Unregister releases the PSM registry copy and disconnects its inbound
/// channels. It is distinct from ordinary accept and channel disconnect.
pub const F_L2CAP_RELEASE_PSM: usize = fw::CANOPUS_FW_L2CAP_UNREGISTER_PSM_HANDLER_CALLABLE;
pub const F_L2CAP_CONNECT: usize = fw::CANOPUS_FW_L2CAP_CONNECT_HANDLER_CALLABLE;
pub const F_L2CAP_ACCEPT: usize = fw::CANOPUS_FW_L2CAP_ACCEPT_HANDLER_CALLABLE;

pub const F_L2CAP_SEND: usize = fw::CANOPUS_FW_L2CAP_SEND_HANDLER_CALLABLE;
pub const F_L2CAP_FIND_CHANNEL: usize = 0x1c065c18;
/// Established-channel disconnect primitive. The target shim handles all
/// other lifecycle states through the stock reject/teardown branches.
pub const F_L2CAP_DISCONNECT_CID: usize = fw::CANOPUS_FW_L2CAP_DISCONNECT_CHANNEL_CALLABLE;

pub const F_L2CAP_QUEUE_PUSH: usize = fw::CANOPUS_FW_L2CAP_QUEUE_PUSH_CALLABLE;

pub const F_L2CAP_CREDIT_FLUSH: usize = fw::CANOPUS_FW_L2CAP_CREDIT_QUEUE_FLUSH_CALLABLE;
pub const F_BT_QUEUE_EXTERNAL: usize = fw::CANOPUS_FW_BT_QUEUE_EXTERNAL_CALLABLE;

/// Second argument of every owner-handler call (`enqueue` event).
pub const COC_QUEUE_EVENT: u32 = 1;
/// Callback event identifiers of the legacy LE channel path.
pub const COC_EVENT_INCOMING: u32 = 1;
pub const COC_EVENT_ASSIGNED: u32 = 2;
pub const COC_EVENT_OPEN: u32 = 3;
pub const COC_EVENT_CLOSED: u32 = 6;
pub const COC_EVENT_DATA: u32 = 7;
pub const COC_EVENT_LOCAL_FLOW: u32 = 8;
pub const COC_EVENT_PSM_REGISTERED: u32 = 9;
pub const COC_EVENT_CREDITS: u32 = 17;
pub const COC_PSM_MIN: u16 = 0x80;
pub const COC_PSM_MAX: u16 = 0xff;
pub const COC_PSM_TRANSPORT: u8 = 3;
pub const COC_CID_MIN: u16 = 0x40;
pub const COC_CID_MAX: u16 = 0x7f;
/// `disconnect_channel` policy argument used by the audited caller.
pub const COC_DISCONNECT_POLICY: u16 = 0;

pub const COC_REQUEST_SIZE: usize = 68;
pub const COC_REQUEST_CID: usize = 0;
pub const COC_REQUEST_PSM: usize = 2;
pub const COC_REQUEST_STATUS: usize = 4;
pub const COC_REQUEST_STATUS_DETAIL: usize = 6;
pub const COC_REQUEST_FLAGS: usize = 8;
pub const COC_REQUEST_CALLBACK: usize = 12;
pub const COC_REQUEST_ADDRESS: usize = 16;
pub const COC_REQUEST_ADDRESS_TYPE: usize = 22;
/// Local configuration area of the connect/accept request.
pub const COC_CONFIG_MTU: usize = 52;
pub const COC_CONFIG_CREDITS: usize = 60;
pub const COC_CONFIG_MPS: usize = 62;

pub const COC_CHANNEL_QUEUE: usize = 4;
pub const COC_CREDIT_QUEUE_CREDITS: usize = 14;

/// PSM registry node: eight-byte intrusive list header, then callback, PSM and
/// transport.
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

/// Stock send buffer: end, offset, CID word, metadata flag, payload start.
pub const SDU_BUFFER_END: usize = 0;
pub const SDU_BUFFER_OFFSET: usize = 2;
pub const SDU_BUFFER_CID: usize = 4;
pub const SDU_BUFFER_META: usize = 8;
pub const SDU_BUFFER_META_VALUE: u8 = 3;
pub const SDU_BUFFER_PAYLOAD: usize = 4;
pub const SDU_BUFFER_ALLOC_TAIL: usize = 6;

/// Fragment list node handed to the credit-controlled queue.
pub const FRAGMENT_NODE_SIZE: usize = 16;
pub const FRAGMENT_NODE_PAYLOAD: usize = 8;
pub const FRAGMENT_NODE_BUFFER: usize = 0;
pub const FRAGMENT_NODE_FLAGS: usize = 4;
pub const FRAGMENT_FLAGS_FIRST_LAST: u8 = 0x81;

// ---- QuickJS Feature host --------------------------------------------------
pub const F_QJS_GET_EXCEPTION: usize = fw::CANOPUS_FW_QJS_GET_EXCEPTION_CALLABLE;
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

pub const F_QJS_GET_TYPED_ARRAY_BUFFER: usize = fw::CANOPUS_FW_QJS_GET_TYPED_ARRAY_BUFFER_CALLABLE;

pub const F_FEATURE_REGISTER: usize = 0;
/// The single framework instance published by firmware boot.
pub const D_FRAMEWORK: usize = fw::canopus_fw_ferry_framework_instance;
pub const D_FRAMEWORK_VTABLE: usize = fw::canopus_fw_ferry_framework_vtable;
pub const D_APPLICATION_VTABLE: usize = fw::canopus_fw_ferry_application_vtable;

/// Both tables retain the two Itanium ABI prefix words.
pub const VTABLE_PREFIX_WORDS: usize = 2;
pub const FRAMEWORK_FACTORY_SLOT: usize = 0;
pub const FRAMEWORK_COPY_WORDS: usize = 9;
pub const APPLICATION_INIT_SLOT: usize = 2;
pub const APPLICATION_COPY_WORDS: usize = 29;

pub const FRAMEWORK_IMPLEMENTATION: usize = 4;
pub const FRAMEWORK_MUTEX: usize = 392;
pub const FRAMEWORK_APPS_BEGIN: usize = 452;
pub const FRAMEWORK_APPS_END: usize = 456;
pub const APPLICATION_RUNTIME: usize = 0;
pub const RUNTIME_MANAGER: usize = 0;
pub const MANAGER_REGISTRY: usize = 0;
pub const MANAGER_ASYNC: usize = 0;
pub const HANDLE_LOOP: usize = 0;
pub const PROTOTYPE_MANAGER: usize = 0;
pub const INSTANCE_HANDLE: usize = 0;
pub const JSVALUE_CLASS_OFFSET: usize = 6;
pub const ARRAY_BUFFER_DATA: usize = 8;
pub const ARRAY_BUFFER_DETACHED: usize = 4;

pub const FEATURE_ASYNC_WORDS: usize = 32;

/// Calls the measured p67 engine entry. See docs/p67-native-contract.md.
///
/// # Safety
/// Context and values must belong to the live owning JavaScript task.
pub unsafe fn qjs_new_string_len(
    context: *mut core::ffi::c_void,
    bytes: *const u8,
    length: usize,
) -> u64 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, *const u8, usize) -> u64 =
        core::mem::transmute(F_QJS_NEW_STRING_LEN | 1);
    call(context, bytes, length)
}

/// Calls the measured p67 engine entry. See docs/p67-native-contract.md.
///
/// # Safety
/// Context and values must belong to the live owning JavaScript task.
pub unsafe fn qjs_new_array_buffer_copy(
    context: *mut core::ffi::c_void,
    data: *const u8,
    length: usize,
) -> u64 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, *const u8, usize) -> u64 =
        core::mem::transmute(F_QJS_NEW_ARRAY_BUFFER_COPY | 1);
    call(context, data, length)
}

/// Calls the measured p67 engine entry. See docs/p67-native-contract.md.
///
/// # Safety
/// Context and values must belong to the live owning JavaScript task.
pub unsafe fn qjs_get_array_buffer(
    context: *mut core::ffi::c_void,
    length: *mut usize,
    value: u64,
) -> *const u8 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, *mut usize, u64) -> *const u8 =
        core::mem::transmute(F_QJS_GET_ARRAY_BUFFER | 1);
    call(context, length, value)
}

/// Calls the measured p67 engine entry. See docs/p67-native-contract.md.
///
/// # Safety
/// Context and values must belong to the live owning JavaScript task.
pub unsafe fn qjs_get_typed_array_buffer(
    context: *mut core::ffi::c_void,
    value: u64,
    offset: *mut usize,
    length: *mut usize,
    element_size: *mut usize,
) -> u64 {
    let call: unsafe extern "C" fn(
        *mut core::ffi::c_void,
        u64,
        *mut usize,
        *mut usize,
        *mut usize,
    ) -> u64 = core::mem::transmute(F_QJS_GET_TYPED_ARRAY_BUFFER | 1);
    call(context, value, offset, length, element_size)
}

/// # Safety
/// The pinned file must remain valid for this write.
pub unsafe fn write_byte(file: *mut core::ffi::c_void, byte: u8) -> isize {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, *const u8, usize) -> isize =
        core::mem::transmute(F_FILE_WRITE | 1);
    call(file, &byte, 1)
}

/// Execute the state-dependent policy of the stock disconnect handler 0xC802F18.
/// No request allocation is needed because the module already runs on Bluelet.
///
/// # Safety
/// `channel` must be live and this call must run on the Bluelet owner task.
/// Execute the P65 stock disconnect/reject policy on the Bluelet owner task.
///
/// # Safety
/// `channel` must be a live ordinary LE CoC channel owned by this task.
pub unsafe fn disconnect_channel(channel: usize, _policy: u16) {
    let state = core::ptr::read((channel + 120) as *const u8);
    if state == 8 || state == 9 {
        let disconnect: unsafe extern "C" fn(usize) -> i32 =
            core::mem::transmute(F_L2CAP_DISCONNECT_CID | 1);
        disconnect(channel);
    } else if state == 2 || state == 131 {
        let respond: unsafe extern "C" fn(usize, usize, u16, u8, u16, u16, u8) -> i32 =
            core::mem::transmute(fw::CANOPUS_FW_L2CAP_LE_CONNECTION_RESPONSE_CALLABLE | 1);
        let peer = core::ptr::read_unaligned(channel as *const usize);
        let remote_cid = core::ptr::read_unaligned((channel + 118) as *const u16);
        let identifier = core::ptr::read((channel + 121) as *const u8);
        respond(peer, channel, remote_cid, identifier, 4, 0, 3);
    } else {
        let teardown: unsafe extern "C" fn(usize, u16) -> i32 =
            core::mem::transmute(fw::CANOPUS_FW_L2CAP_TEARDOWN_CHANNEL_CALLABLE | 1);
        teardown(channel, 1033);
    }
}

/// Stock SDU/fragment submission; the ordinary p67 sender retains this exact layout.
///
/// # Safety
/// Call on Bluelet with a live channel and a readable payload.
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
    let release: unsafe extern "C" fn(*mut u8) = core::mem::transmute(F_FREE | 1);
    let end = length as usize + 16 + if length == 0 { 2 } else { 0 };
    let buffer = allocate((end + SDU_BUFFER_ALLOC_TAIL) as u32);
    if buffer.is_null() {
        return -12;
    }
    core::ptr::write_bytes(buffer, 0, end + SDU_BUFFER_ALLOC_TAIL);
    core::ptr::write_unaligned(buffer.cast::<u16>(), end as u16);
    core::ptr::write_unaligned(buffer.add(2).cast::<u16>(), 16);
    core::ptr::write_unaligned(buffer.add(4).cast::<u32>(), cid as u32);
    *buffer.add(SDU_BUFFER_META) = SDU_BUFFER_META_VALUE;
    if length != 0 {
        core::ptr::copy_nonoverlapping(data, buffer.add(20), length as usize);
        let send: unsafe extern "C" fn(usize, u32, *mut u8) -> i32 =
            core::mem::transmute(F_L2CAP_SEND | 1);
        return send(owner, COC_QUEUE_EVENT, buffer) as isize;
    }
    // The stock sender drops empty SDUs. Preserve its two-byte zero prefix
    // with an explicit first+last fragment and the ordinary credit queue.
    core::ptr::write_unaligned(buffer.add(20).cast::<u16>(), 0);
    let queue = core::ptr::read_unaligned((channel + COC_CHANNEL_QUEUE) as *const usize);
    if queue == 0 {
        release(buffer);
        return -1;
    }
    let allocation = allocate(FRAGMENT_NODE_SIZE as u32);
    if allocation.is_null() {
        release(buffer);
        return -12;
    }
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
    // With no credit, the queue retains both allocations until event17.
    flush(queue, channel) as isize
}

/// Convert the string using the native UTF-8/CESU-8 engine accessor.
///
/// # Safety
/// The context and string value must belong to the live owning JS task;
/// release the returned pointer with F_QJS_FREE_CSTRING.
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

/// Define an own property through the direct engine string-name wrapper.
///
/// # Safety
/// Context and values must belong to the live JS task. Name is NUL terminated;
/// the engine consumes `value` on both success and failure.
pub unsafe fn qjs_define_property_value_str(
    context: *mut core::ffi::c_void,
    object: u64,
    name: *const u8,
    value: u64,
    flags: i32,
) -> i32 {
    let define: unsafe extern "C" fn(*mut core::ffi::c_void, u64, *const u8, u64, i32) -> i32 =
        core::mem::transmute(F_QJS_DEFINE_PROPERTY_VALUE_STR | 1);
    define(context, object, name, value, flags)
}

// ---- Legacy native-extension integration -----------------------------------
pub const LEGACY_EXTENSION_LIST: usize = fw::canopus_fw_legacy_extension_list;
/// The stock P65 core-to-global export-map copy has not yet been verified.
pub const LEGACY_CORE_EXPORT: bool = false;
pub const LEGACY_CONTEXT_RUNTIME: usize = 248;
pub const LEGACY_RUNTIME_CONTEXT: usize = 20;
pub const LEGACY_RUNTIME_LOOP: usize = 24;
pub const LEGACY_APPLICATION_RUNTIME: usize = 296;
pub const LEGACY_APPLICATION_DESTROY_SLOT: usize = 19;
pub const F_QJS_GET_GLOBAL_OBJECT: usize = fw::CANOPUS_FW_QJS_GET_GLOBAL_OBJECT_CALLABLE;
pub const F_QJS_GET_OPAQUE: usize = fw::CANOPUS_FW_QJS_GET_OPAQUE_CALLABLE;
pub const LEGACY_NATIVEPROXY_CLASS_ID: u32 = 1;
pub const LEGACY_EFFECT_MANAGER_APPLICATION: usize = 108;
pub const F_QJS_ADD_MODULE_EXPORT: usize = fw::CANOPUS_FW_QJS_ADD_MODULE_EXPORT_CALLABLE;
pub const F_QJS_SET_MODULE_EXPORT: usize = fw::CANOPUS_FW_QJS_SET_MODULE_EXPORT_CALLABLE;

/// Application association is not yet proven during extension initialization.
/// Returning zero makes the shared host skip vptr mutation rather than guess.
///
/// # Safety
/// `runtime` must be a live legacy runtime on its owning JavaScript task.
/// Find the Application through the context-local NativeProxy effect manager.
/// P65 initialization stores the framework's current Application at
/// `AppEffectManager + 108`; this does not depend on Application.runtime,
/// which is assigned only after bootstrap.
///
/// # Safety
/// `runtime` must be a live legacy RuntimeContext on its owning JS task. The
/// returned pointer is borrowed and is checked against the stock primary
/// vtable before it is exposed to the caller.
pub unsafe fn legacy_application(runtime: usize) -> usize {
    if runtime == 0 {
        return 0;
    }
    let context = core::ptr::read_unaligned(
        (runtime + LEGACY_RUNTIME_CONTEXT) as *const *mut core::ffi::c_void,
    );
    if context.is_null() {
        return 0;
    }
    let get_global: unsafe extern "C" fn(*mut core::ffi::c_void) -> u64 =
        core::mem::transmute(F_QJS_GET_GLOBAL_OBJECT | 1);
    let get_opaque: unsafe extern "C" fn(*mut core::ffi::c_void, u64, u32) -> usize =
        core::mem::transmute(F_QJS_GET_OPAQUE | 1);
    let free: unsafe extern "C" fn(*mut core::ffi::c_void, u64) =
        core::mem::transmute(F_QJS_FREE_VALUE | 1);
    let global = get_global(context);
    let manager = get_opaque(context, global, LEGACY_NATIVEPROXY_CLASS_ID);
    free(context, global);
    if manager == 0 {
        return 0;
    }
    let application =
        core::ptr::read_unaligned((manager + LEGACY_EFFECT_MANAGER_APPLICATION) as *const usize);
    if application == 0
        || core::ptr::read_volatile(application as *const usize) != D_APPLICATION_VTABLE
    {
        return 0;
    }
    application
}

/// Declare an export in the legacy native-module namespace.
///
/// # Safety
/// `context` and `module` must be live on the owning JavaScript task; `name`
/// must point to a readable NUL-terminated string.
pub unsafe fn legacy_declare(
    context: *mut core::ffi::c_void,
    module: usize,
    name: *const u8,
) -> i32 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, usize, *const u8) -> i32 =
        core::mem::transmute(F_QJS_ADD_MODULE_EXPORT | 1);
    call(context, module, name)
}

/// Assign a previously declared export. The engine consumes `value` on both
/// success and failure, including atom-intern and property-update failures.
///
/// # Safety
/// `context` and `module` must be live on the owning JavaScript task; `name`
/// must be NUL-terminated and `value` must be transferable to the engine.
pub unsafe fn legacy_export(
    context: *mut core::ffi::c_void,
    module: usize,
    name: *const u8,
    value: u64,
) -> i32 {
    let call: unsafe extern "C" fn(*mut core::ffi::c_void, usize, *const u8, u64) -> i32 =
        core::mem::transmute(F_QJS_SET_MODULE_EXPORT | 1);
    call(context, module, name, value)
}

/// Publish the legacy primitive on this context's global object. `value` is
/// consumed on every return path; the temporary global reference is released.
///
/// # Safety
/// `context` must be live on its owning JavaScript task and `value` must be
/// valid for exactly one ownership transfer.
pub unsafe fn legacy_publish(context: *mut core::ffi::c_void, value: u64) -> i32 {
    let get_global: unsafe extern "C" fn(*mut core::ffi::c_void) -> u64 =
        core::mem::transmute(F_QJS_GET_GLOBAL_OBJECT | 1);
    let global = get_global(context);
    if global == (6u64 << 32) {
        let free: unsafe extern "C" fn(*mut core::ffi::c_void, u64) =
            core::mem::transmute(F_QJS_FREE_VALUE | 1);
        free(context, value);
        return -1;
    }
    let result = qjs_define_property_value_str(
        context,
        global,
        c"__canopusAstrolinkNative".as_ptr().cast(),
        value,
        7,
    );
    let free: unsafe extern "C" fn(*mut core::ffi::c_void, u64) =
        core::mem::transmute(F_QJS_FREE_VALUE | 1);
    free(context, global);
    result
}

/// Create the eight-byte native scan wrapper with stock passive settings.
/// The wrapper and callbacks remain owned by the firmware until started with
/// nonzero status or stopped; the common backend retains that terminal handle.
///
/// # Safety
/// `adapter` and `callbacks` must remain live on the MiWear business loop.
pub unsafe fn create_scan(adapter: usize, callbacks: *const core::ffi::c_void) -> usize {
    if callbacks.is_null() {
        return 0;
    }
    let settings = [2, 0, 0, 0, 0, 0, 0, 0];
    let create: unsafe extern "C" fn(usize, *const u8, *const core::ffi::c_void) -> usize =
        core::mem::transmute(F_SCAN_CREATE | 1);
    create(adapter, settings.as_ptr(), callbacks)
}

/// Stop by the native wrapper, not by PHY or adapter state. P65 stop has no
/// useful return status; completion arrives only through the stopped callback.
///
/// # Safety
/// `handle` must be a live wrapper returned by `create_scan` on this loop.
pub unsafe fn stop_scan(adapter: usize, handle: usize) -> i32 {
    let stop: unsafe extern "C" fn(usize, usize) = core::mem::transmute(F_SCAN_STOP | 1);
    stop(adapter, handle);
    0
}

/// Stop by advertising wrapper+8. As with scan, the callback is authoritative.
///
/// # Safety
/// `handle` must be a live advertising wrapper on this business loop.
pub unsafe fn stop_advertising(adapter: usize, handle: usize) -> i32 {
    let stop: unsafe extern "C" fn(usize, usize) = core::mem::transmute(F_ADVERTISING_STOP | 1);
    stop(adapter, handle);
    0
}

/// Construct a native generic-magic function with the direct P65 six-argument
/// NewCFunction2 entry. The final cproto and magic arguments are passed on the
/// stack by AAPCS; no NewCFunction3 prototype-value alias is involved.
///
/// # Safety
/// `context` must be live on its JS owner task; `callback` must be a valid
/// firmware native-function pointer and `name` must be NUL-terminated.
pub unsafe fn qjs_new_c_function(
    context: *mut core::ffi::c_void,
    callback: usize,
    name: *const core::ffi::c_char,
    argc: i32,
    magic: i32,
) -> u64 {
    let call: unsafe extern "C" fn(
        *mut core::ffi::c_void,
        usize,
        *const core::ffi::c_char,
        i32,
        i32,
        i32,
    ) -> u64 = core::mem::transmute(F_QJS_NEW_C_FUNCTION2 | 1);
    call(context, callback, name, argc, 1, magic)
}
