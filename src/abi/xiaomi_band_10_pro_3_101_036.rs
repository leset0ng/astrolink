//! Xiaomi Smart Band 10 Pro, exact firmware 3.101.036 (p67).
//! Native ferry Feature registration and generic GAP/LE CoC integration.
//! Measured entry contracts are recorded in docs/p67-native-contract.md.

pub const SUPPORTED: bool = true;
pub const TARGET_ID: &str = "xiaomi-band-10-pro-3.101.036";

/// Entry bytes of `ble_advertising_start`, `ble_scan_start`, `bt_queue_external`
/// and the CoC connect owner handler. `backend::supported` compares them before
/// any firmware call, so a mismatched image is refused instead of called.
pub const SIGNATURES: &[(usize, [u8; 12])] = &[
    (
        0xc39aa88,
        [0x2d, 0xe9, 0xf0, 0x47, 0xad, 0xf5, 0x32, 0x7d, 0x0, 0xaf, 0xb7, 0xf8],
    ),
    (
        0xc39ac70,
        [0x2d, 0xe9, 0xf0, 0x43, 0x80, 0x46, 0xad, 0xf5, 0x33, 0x7d, 0x0, 0xaf],
    ),
    (
        0xc7d3404,
        [0x2d, 0xe9, 0xf0, 0x4f, 0x83, 0xb0, 0xdf, 0xf8, 0x2c, 0xa1, 0x0, 0xaf],
    ),
    (
        0xc7e94fc,
        [0x2d, 0xe9, 0xf0, 0x4f, 0x83, 0xb0, 0x16, 0x7a, 0x15, 0x46, 0x16, 0xf0],
    ),
];

use canopus_target_generated as fw;

// ---- kernel and libuv -------------------------------------------------------
pub const F_ALLOC: usize = fw::CANOPUS_FW_BT_ALLOC_CALLABLE;
pub const F_FREE: usize = fw::CANOPUS_FW_BT_FREE_CALLABLE;
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
/// libuv loop key used by the QuickJS task's `uv_loop_t` slot vector.
pub const D_UV_KEY: usize = fw::canopus_fw_uv__tls_key;

// ---- default loop and task layout ------------------------------------------
/// Intrusive ready-work queue head, mutex and async handle in `uv_loop_t`.
pub const LOOP_QUEUE_HEAD: usize = 68;
pub const LOOP_MUTEX: usize = 76;
pub const LOOP_ASYNC: usize = 104;
/// Default loop's pipe endpoints as integer descriptors.
pub const LOOP_READ_FD: usize = 360;
pub const LOOP_WRITE_FD: usize = 364;
/// `uv_async_t` busy/pending words.
pub const ASYNC_BUSY_WORD: usize = 6;
pub const ASYNC_PENDING_WORD: usize = 27;
/// NuttX task descriptor: TLS pointer, task group, and the group's file list.
pub const TCB_TLS: usize = 104;
pub const TCB_GROUP: usize = 8;
pub const GROUP_FILELIST: usize = 720;
pub const FILELIST_ROW: usize = 8;
/// `tls -> taskinfo -> uv state slots[key] -> loop`.
pub const TASKINFO_UVSTATE: usize = 16;
pub const TASKINFO_UVSTATE_STRIDE: usize = 4;
pub const UVSTATE_LOOP: usize = 760;
/// 32-byte pinned `file` record and its inode pointer.
pub const FILE_INODE: usize = 16;
/// `inode->i_ops` identity of the loop's pipe.
///
/// Exact p67 pipe-operations identity recorded in the target pack. The wake
/// bridge accepts a pair only when both ends share an inode with this table.
pub const INODE_OPS_PIPE: usize = fw::canopus_fw_pipe_file_operations;

pub const UV_KEY_LIMIT: u32 = 7;

// ---- adapter and GAP -------------------------------------------------------
pub const F_ADAPTER_REGISTER: usize = fw::CANOPUS_FW_BT_ADAPTER_REGISTER_CALLABLE;
pub const F_ADAPTER_GET_STATE: usize = fw::CANOPUS_FW_BT_ADAPTER_GET_STATE_CALLABLE;
pub const F_ADAPTER_ENABLE: usize = fw::CANOPUS_FW_BT_ADAPTER_ENABLE_CALLABLE;
pub const F_SCAN_CREATE: usize = fw::CANOPUS_FW_GAP_SCAN_START_IMPL_CALLABLE;

pub const F_SCAN_STOP: usize = fw::CANOPUS_FW_BLE_SCAN_STOP_CALLABLE;

pub const F_ADVERTISING_CREATE: usize = fw::CANOPUS_FW_BLE_ADVERTISING_START_CALLABLE;

pub const F_ADVERTISING_STOP: usize = fw::CANOPUS_FW_BT_SAL_LE_STOP_ADVERTISING_CALLABLE;

/// Process-lifetime MiWear Bluetooth IPC instance created by firmware boot.
pub const D_ADAPTER_INSTANCE: usize = fw::canopus_fw_gap_client_instance;
/// Adapter state word and business-loop pointer inside the IPC client.
pub const ADAPTER_STATE_WORD: usize = 48;
pub const ADAPTER_LOOP: usize = 56;
pub const ADAPTER_READY_STATE: i32 = 4;

pub const SCAN_PARAM_SIZE: usize = 5;
pub const SCAN_CALLBACKS_SIZE: u32 = 16;
pub const ADV_CALLBACKS_SIZE: usize = 12;
/// Scan result report: address, RSSI, address type, packet type, AD length, AD.
pub const SCAN_REPORT_ADDRESS: usize = 0;
pub const SCAN_REPORT_RSSI: usize = 7;
pub const SCAN_REPORT_ADDRESS_TYPE: usize = 8;
pub const SCAN_REPORT_PACKET_TYPE: usize = 9;
pub const SCAN_REPORT_LENGTH: usize = 10;
pub const SCAN_REPORT_DATA: usize = 12;
/// `bt_le_adv_param_t`.
pub const ADV_PARAM_SIZE: usize = 28;
pub const ADV_PARAM_MODE: usize = 6;
pub const ADV_PARAM_TX_POWER: usize = 15;
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
pub const F_L2CAP_FIND_CHANNEL: usize = fw::CANOPUS_FW_L2CAP_FIND_CHANNEL_CALLABLE;
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

pub const F_FEATURE_REGISTER: usize = fw::CANOPUS_FW_FEATURE_REGISTER_FEATURE_CALLABLE;
/// The single framework instance published by firmware boot.
pub const D_FRAMEWORK: usize = fw::canopus_fw_ferry_framework_instance;
pub const D_FRAMEWORK_VTABLE: usize = fw::canopus_fw_ferry_framework_vtable;
pub const D_APPLICATION_VTABLE: usize = fw::canopus_fw_ferry_application_vtable;

/// Both tables retain the two Itanium ABI prefix words.
pub const VTABLE_PREFIX_WORDS: usize = 2;
pub const FRAMEWORK_FACTORY_SLOT: usize = 8;
pub const FRAMEWORK_COPY_WORDS: usize = 15;
pub const APPLICATION_INIT_SLOT: usize = 2;
pub const APPLICATION_COPY_WORDS: usize = 52;

pub const FRAMEWORK_IMPLEMENTATION: usize = 4;
pub const FRAMEWORK_MUTEX: usize = 348;
pub const FRAMEWORK_APPS_BEGIN: usize = 460;
pub const FRAMEWORK_APPS_END: usize = 464;
pub const APPLICATION_RUNTIME: usize = 100;
pub const RUNTIME_MANAGER: usize = 164;
pub const MANAGER_REGISTRY: usize = 4;
pub const MANAGER_ASYNC: usize = 60;
pub const HANDLE_LOOP: usize = 4;
pub const PROTOTYPE_MANAGER: usize = 12;
pub const INSTANCE_HANDLE: usize = 112;
pub const JSVALUE_CLASS_OFFSET: usize = 6;
pub const ARRAY_BUFFER_DATA: usize = 32;
pub const ARRAY_BUFFER_DETACHED: usize = 4;

pub const FEATURE_ASYNC_WORDS: usize = 28;

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
    let queue = core::ptr::read_unaligned((channel + COC_CHANNEL_QUEUE) as *const usize);
    if queue == 0 {
        return -1;
    }
    let payload_offset = 12u16 + SDU_BUFFER_PAYLOAD as u16;
    // The first CoC PDU carries the two-byte SDU length even when empty.
    let wire_length = if length == 0 { 2 } else { length };
    let end = wire_length.wrapping_add(payload_offset);
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
        // The stock handler consumes the SDU buffer and creates its own
        // fragment nodes. Passing a node here interprets its fields as an SDU
        // header and eventually frees the interior node pointer.
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
    let result = flush(queue, channel);
    result as isize
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
