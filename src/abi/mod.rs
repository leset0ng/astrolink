//! Per-target firmware ABI description.
//!
//! `docs/ble-abi.md` and `docs/qjs-abi.md` record how every address, structure
//! offset and size below was recovered from an exact firmware image.
//! `canopus build --target <id>` enables the matching `target-<id>` cargo
//! feature, which selects one of the modules here.
//!
//! Callable addresses are taken from `canopus_target_generated`, so a target
//! pack that does not carry a record fails to compile instead of falling back
//! to a guessed value. That is the intended gate: a firmware whose GAP, LE CoC
//! or QuickJS Feature host has not been recovered must not be adapted by
//! guessing.
//!
//! Targets declared with [`unsupported!`] keep the module linkable and loadable
//! but expose the Feature as unsupported; they never touch firmware.
//!
//! Every target module must provide the items listed below. Groups:
//!
//! * identity: `TARGET_ID`, `SIGNATURES`
//! * kernel and libuv: `F_ALLOC`, `F_FREE`, `F_UV_MUTEX_INIT`,
//!   `F_UV_MUTEX_LOCK`, `F_UV_MUTEX_UNLOCK`, `F_UV_ASYNC_INIT`, `F_UV_CLOSE`,
//!   `F_FILES_GETFILEP`, `F_FS_GETFILEP`, `F_FS_PUTFILEP`, `F_FILE_DUP3`,
//!   `F_FILE_WRITE`, `F_FILE_CLOSE`, `F_NXSCHED_FOREACH`, `D_UV_KEY`
//! * loop and task layout: `LOOP_QUEUE_HEAD`, `LOOP_MUTEX`, `LOOP_ASYNC`,
//!   `LOOP_READ_FD`, `LOOP_WRITE_FD`, `ASYNC_BUSY_WORD`,
//!   `ASYNC_PENDING_WORD`, `TCB_TLS`, `TCB_GROUP`, `GROUP_FILELIST`,
//!   `FILELIST_ROW`, `TASKINFO_UVSTATE`, `TASKINFO_UVSTATE_STRIDE`,
//!   `UVSTATE_LOOP`, `FILE_INODE`, `INODE_OPS_PIPE`, `UV_KEY_LIMIT`
//! * adapter and GAP: `F_ADAPTER_REGISTER`, `F_ADAPTER_GET_STATE`,
//!   `F_ADAPTER_ENABLE`, `F_SCAN_CREATE`, `F_SCAN_STOP`,
//!   `F_ADVERTISING_CREATE`, `F_ADVERTISING_STOP`, `D_ADAPTER_INSTANCE`,
//!   `ADAPTER_STATE_WORD`, `ADAPTER_LOOP`, `ADAPTER_READY_STATE`,
//!   `SCAN_PARAM_SIZE`, `SCAN_CALLBACKS_SIZE`, `ADV_CALLBACKS_SIZE`,
//!   `SCAN_REPORT_ADDRESS`, `SCAN_REPORT_RSSI`, `SCAN_REPORT_ADDRESS_TYPE`,
//!   `SCAN_REPORT_PACKET_TYPE`, `SCAN_REPORT_LENGTH`, `SCAN_REPORT_DATA`,
//!   `ADV_PARAM_SIZE`, `ADV_PARAM_MODE`, `ADV_PARAM_TX_POWER`,
//!   `ADV_PARAM_INTERVAL`
//! * LE CoC: `D_L2CAP_OWNER`, `F_L2CAP_FIND_LE_PSM`,
//!   `F_L2CAP_REGISTER_PSM`, `F_L2CAP_RELEASE_PSM`, `F_L2CAP_CONNECT`,
//!   `F_L2CAP_ACCEPT`, `F_L2CAP_SEND`, `F_L2CAP_FIND_CHANNEL`,
//!   `F_L2CAP_DISCONNECT_CID`, `F_L2CAP_QUEUE_PUSH`, `F_L2CAP_CREDIT_FLUSH`,
//!   `F_BT_QUEUE_EXTERNAL`, `COC_QUEUE_EVENT`, `COC_EVENT_INCOMING`,
//!   `COC_EVENT_ASSIGNED`, `COC_EVENT_OPEN`, `COC_EVENT_CLOSED`,
//!   `COC_EVENT_DATA`, `COC_EVENT_LOCAL_FLOW`, `COC_EVENT_PSM_REGISTERED`,
//!   `COC_EVENT_CREDITS`, `COC_PSM_MIN`, `COC_PSM_MAX`,
//!   `COC_PSM_TRANSPORT`, `COC_CID_MIN`, `COC_CID_MAX`,
//!   `COC_DISCONNECT_POLICY`, `COC_REQUEST_SIZE`, `COC_REQUEST_CID`,
//!   `COC_REQUEST_PSM`, `COC_REQUEST_STATUS`, `COC_REQUEST_STATUS_DETAIL`,
//!   `COC_REQUEST_FLAGS`, `COC_REQUEST_CALLBACK`, `COC_REQUEST_ADDRESS`,
//!   `COC_REQUEST_ADDRESS_TYPE`, `COC_CONFIG_MTU`, `COC_CONFIG_CREDITS`,
//!   `COC_CONFIG_MPS`, `COC_CHANNEL_QUEUE`, `COC_CREDIT_QUEUE_CREDITS`,
//!   `COC_REGISTER_NODE_SIZE`, `COC_REGISTER_NODE_PAYLOAD`,
//!   `COC_REGISTER_NODE_CALLBACK`, `COC_REGISTER_NODE_PSM`,
//!   `COC_REGISTER_NODE_TRANSPORT`, `COC_EV_REQUEST_CID`,
//!   `COC_EV_REQUEST_PSM`, `COC_EV_REQUEST_ADDRESS`,
//!   `COC_EV_REQUEST_ADDRESS_TYPE`, `COC_EV_COMPLETE_CID`,
//!   `COC_EV_COMPLETE_REMOTE_MTU`, `COC_EV_CLOSE_CID`, `COC_EV_CLOSE_REASON`,
//!   `COC_EV_DATA_END`, `COC_EV_DATA_OFFSET`, `COC_EV_DATA_CID`,
//!   `COC_EV_DATA_BASE`, `COC_EV_PSM_CID`, `COC_EV_CREDIT_CID`,
//!   `COC_EV_CREDIT_ADDED`, `SDU_BUFFER_END`, `SDU_BUFFER_OFFSET`,
//!   `SDU_BUFFER_CID`, `SDU_BUFFER_META`, `SDU_BUFFER_META_VALUE`,
//!   `SDU_BUFFER_PAYLOAD`, `SDU_BUFFER_ALLOC_TAIL`,
//!   `FRAGMENT_NODE_SIZE`, `FRAGMENT_NODE_PAYLOAD`,
//!   `FRAGMENT_NODE_BUFFER`, `FRAGMENT_NODE_FLAGS`,
//!   `FRAGMENT_FLAGS_FIRST_LAST`
//! * QuickJS Feature: `F_QJS_GET_EXCEPTION`, `F_QJS_FREE_VALUE`,
//!   `F_QJS_NEW_C_FUNCTION2`, `F_QJS_NEW_OBJECT`, `F_QJS_NEW_STRING_LEN`,
//!   `F_QJS_TO_CSTRING_LEN2`, `F_QJS_FREE_CSTRING`, `F_QJS_IS_FUNCTION`,
//!   `F_QJS_CALL`, `F_QJS_DEFINE_PROPERTY_VALUE_STR`,
//!   `F_QJS_NEW_ARRAY_BUFFER_COPY`, `F_QJS_GET_ARRAY_BUFFER`,
//!   `F_QJS_GET_TYPED_ARRAY_BUFFER`, `F_FEATURE_REGISTER`, `D_FRAMEWORK`,
//!   `D_FRAMEWORK_VTABLE`, `D_APPLICATION_VTABLE`, `VTABLE_PREFIX_WORDS`,
//!   `FRAMEWORK_FACTORY_SLOT`, `FRAMEWORK_COPY_WORDS`,
//!   `APPLICATION_INIT_SLOT`, `APPLICATION_COPY_WORDS`,
//!   `FRAMEWORK_IMPLEMENTATION`, `FRAMEWORK_MUTEX`, `FRAMEWORK_APPS_BEGIN`,
//!   `FRAMEWORK_APPS_END`, `APPLICATION_RUNTIME`, `RUNTIME_MANAGER`,
//!   `MANAGER_REGISTRY`, `MANAGER_ASYNC`, `HANDLE_LOOP`, `PROTOTYPE_MANAGER`,
//!   `INSTANCE_HANDLE`, `JSVALUE_CLASS_OFFSET`, `ARRAY_BUFFER_DATA`,
//!   `ARRAY_BUFFER_DETACHED`

#[cfg(feature = "target-redmi-watch-6-3-100-043")]
#[path = "redmi_watch_6_3_100_043.rs"]
mod redmi_watch_6_3_100_043;
#[cfg(feature = "target-xiaomi-band-10-3-1-28")]
#[path = "xiaomi_band_10_3_1_28.rs"]
mod xiaomi_band_10_3_1_28;
#[cfg(feature = "target-xiaomi-band-10-pro-3-101-036")]
#[path = "xiaomi_band_10_pro_3_101_036.rs"]
mod xiaomi_band_10_pro_3_101_036;
#[cfg(feature = "target-xiaomi-band-10-pro-3-101-043")]
#[path = "xiaomi_band_10_pro_3_101_043.rs"]
mod xiaomi_band_10_pro_3_101_043;
#[cfg(feature = "target-xiaomi-band-11-4-100-139")]
#[path = "xiaomi_band_11_4_100_139.rs"]
mod xiaomi_band_11_4_100_139;
#[cfg(feature = "target-xiaomi-band-11-4-100-155")]
#[path = "xiaomi_band_11_4_100_155.rs"]
mod xiaomi_band_11_4_100_155;
#[cfg(feature = "target-xiaomi-band-11-4-100-179")]
#[path = "xiaomi_band_11_4_100_179.rs"]
mod xiaomi_band_11_4_100_179;
#[cfg(feature = "target-xiaomi-band-9-3-1-32")]
#[path = "xiaomi_band_9_3_1_32.rs"]
mod xiaomi_band_9_3_1_32;
#[cfg(feature = "target-xiaomi-band-9-pro-3-1-190")]
#[path = "xiaomi_band_9_pro_3_1_190.rs"]
mod xiaomi_band_9_pro_3_1_190;

#[cfg(feature = "target-redmi-watch-6-3-100-043")]
pub use redmi_watch_6_3_100_043::*;
#[cfg(feature = "target-xiaomi-band-10-3-1-28")]
pub use xiaomi_band_10_3_1_28::*;
#[cfg(feature = "target-xiaomi-band-10-pro-3-101-036")]
pub use xiaomi_band_10_pro_3_101_036::*;
#[cfg(feature = "target-xiaomi-band-10-pro-3-101-043")]
pub use xiaomi_band_10_pro_3_101_043::*;
#[cfg(feature = "target-xiaomi-band-11-4-100-139")]
pub use xiaomi_band_11_4_100_139::*;
#[cfg(feature = "target-xiaomi-band-11-4-100-155")]
pub use xiaomi_band_11_4_100_155::*;
#[cfg(feature = "target-xiaomi-band-11-4-100-179")]
pub use xiaomi_band_11_4_100_179::*;
#[cfg(feature = "target-xiaomi-band-9-3-1-32")]
pub use xiaomi_band_9_3_1_32::*;
#[cfg(feature = "target-xiaomi-band-9-pro-3-1-190")]
pub use xiaomi_band_9_pro_3_1_190::*;

#[macro_use]
#[path = "shims.rs"]
mod shims;

/// Declares a target whose GAP/LE CoC/QuickJS host has not been recovered.
///
/// The module still compiles and loads for that firmware, but
/// [`crate::backend::supported`] reports false, the Feature is published as
/// unsupported and no firmware function or address is ever used. This is
/// deliberately not a fallback ABI: an unadapted target must fail visibly.
#[macro_export]
macro_rules! abi_unsupported {
    ($target:literal) => {
        pub const SUPPORTED: bool = false;
        pub const TARGET_ID: &str = $target;
        pub const SIGNATURES: &[(usize, [u8; 12])] = &[];

        pub const F_ALLOC: usize = 0;
        pub const F_FREE: usize = 0;
        pub const F_UV_MUTEX_INIT: usize = 0;
        pub const F_UV_MUTEX_LOCK: usize = 0;
        pub const F_UV_MUTEX_UNLOCK: usize = 0;
        pub const F_UV_ASYNC_INIT: usize = 0;
        pub const FEATURE_ASYNC_WORDS: usize = 16;
        pub const F_UV_CLOSE: usize = 0;
        pub const F_FILES_GETFILEP: usize = 0;
        pub const F_FS_GETFILEP: usize = 0;
        pub const F_FS_PUTFILEP: usize = 0;
        pub const F_FILE_DUP3: usize = 0;
        pub const F_FILE_WRITE: usize = 0;
        pub const F_FILE_CLOSE: usize = 0;
        pub const F_NXSCHED_FOREACH: usize = 0;
        pub const D_UV_KEY: usize = 0;

        pub const LOOP_QUEUE_HEAD: usize = 0;
        pub const LOOP_MUTEX: usize = 0;
        pub const LOOP_ASYNC: usize = 0;
        pub const LOOP_READ_FD: usize = 0;
        pub const LOOP_WRITE_FD: usize = 0;
        pub const ASYNC_BUSY_WORD: usize = 0;
        pub const ASYNC_PENDING_WORD: usize = 0;
        pub const TCB_TLS: usize = 0;
        pub const TCB_GROUP: usize = 0;
        pub const GROUP_FILELIST: usize = 0;
        pub const FILELIST_ROW: usize = 0;
        pub const TASKINFO_UVSTATE: usize = 0;
        pub const TASKINFO_UVSTATE_STRIDE: usize = 0;
        pub const UVSTATE_LOOP: usize = 0;
        pub const FILE_INODE: usize = 0;
        pub const INODE_OPS_PIPE: usize = 0;
        pub const UV_KEY_LIMIT: u32 = 0;

        pub const F_ADAPTER_REGISTER: usize = 0;
        pub const F_ADAPTER_GET_STATE: usize = 0;
        pub const F_ADAPTER_ENABLE: usize = 0;
        pub const F_SCAN_CREATE: usize = 0;
        pub const F_SCAN_STOP: usize = 0;
        pub const F_ADVERTISING_CREATE: usize = 0;
        pub const F_ADVERTISING_STOP: usize = 0;
        pub const D_ADAPTER_INSTANCE: usize = 0;
        pub const ADAPTER_STATE_WORD: usize = 0;
        pub const ADAPTER_LOOP: usize = 0;
        pub const ADAPTER_READY_STATE: i32 = 0;
        pub const SCAN_PARAM_SIZE: usize = 0;
        pub const SCAN_CALLBACKS_SIZE: u32 = 0;
        pub const ADV_CALLBACKS_SIZE: usize = 0;
        pub const SCAN_REPORT_ADDRESS: usize = 0;
        pub const SCAN_REPORT_RSSI: usize = 0;
        pub const SCAN_REPORT_ADDRESS_TYPE: usize = 0;
        pub const SCAN_REPORT_PACKET_TYPE: usize = 0;
        pub const SCAN_REPORT_LENGTH: usize = 0;
        pub const SCAN_REPORT_DATA: usize = 0;
        pub const ADV_PARAM_SIZE: usize = 0;
        pub const ADV_PARAM_MODE: usize = 0;
        pub const ADV_PARAM_TX_POWER: usize = 0;
        pub const ADV_PARAM_INTERVAL: usize = 0;

        pub const D_L2CAP_OWNER: usize = 0;
        pub const F_L2CAP_FIND_LE_PSM: usize = 0;
        pub const F_L2CAP_REGISTER_PSM: usize = 0;
        pub const F_L2CAP_RELEASE_PSM: usize = 0;
        pub const F_L2CAP_CONNECT: usize = 0;
        pub const F_L2CAP_ACCEPT: usize = 0;
        pub const F_L2CAP_SEND: usize = 0;
        pub const F_L2CAP_FIND_CHANNEL: usize = 0;
        pub const F_L2CAP_DISCONNECT_CID: usize = 0;
        pub const F_L2CAP_QUEUE_PUSH: usize = 0;
        pub const F_L2CAP_CREDIT_FLUSH: usize = 0;
        pub const F_BT_QUEUE_EXTERNAL: usize = 0;
        pub const COC_QUEUE_EVENT: u32 = 0;
        pub const COC_EVENT_INCOMING: u32 = 0;
        pub const COC_EVENT_ASSIGNED: u32 = 0;
        pub const COC_EVENT_OPEN: u32 = 0;
        pub const COC_EVENT_CLOSED: u32 = 0;
        pub const COC_EVENT_DATA: u32 = 0;
        pub const COC_EVENT_LOCAL_FLOW: u32 = 0;
        pub const COC_EVENT_PSM_REGISTERED: u32 = 0;
        pub const COC_EVENT_CREDITS: u32 = 0;
        pub const COC_PSM_MIN: u16 = 0;
        pub const COC_PSM_MAX: u16 = 0;
        pub const COC_PSM_TRANSPORT: u8 = 0;
        pub const COC_CID_MIN: u16 = 0;
        pub const COC_CID_MAX: u16 = 0;
        pub const COC_DISCONNECT_POLICY: u16 = 0;
        pub const COC_REQUEST_SIZE: usize = 0;
        pub const COC_REQUEST_CID: usize = 0;
        pub const COC_REQUEST_PSM: usize = 0;
        pub const COC_REQUEST_STATUS: usize = 0;
        pub const COC_REQUEST_STATUS_DETAIL: usize = 0;
        pub const COC_REQUEST_FLAGS: usize = 0;
        pub const COC_REQUEST_CALLBACK: usize = 0;
        pub const COC_REQUEST_ADDRESS: usize = 0;
        pub const COC_REQUEST_ADDRESS_TYPE: usize = 0;
        pub const COC_CONFIG_MTU: usize = 0;
        pub const COC_CONFIG_CREDITS: usize = 0;
        pub const COC_CONFIG_MPS: usize = 0;
        pub const COC_CHANNEL_QUEUE: usize = 0;
        pub const COC_CREDIT_QUEUE_CREDITS: usize = 0;
        pub const COC_REGISTER_NODE_SIZE: usize = 0;
        pub const COC_REGISTER_NODE_PAYLOAD: usize = 0;
        pub const COC_REGISTER_NODE_CALLBACK: usize = 0;
        pub const COC_REGISTER_NODE_PSM: usize = 0;
        pub const COC_REGISTER_NODE_TRANSPORT: usize = 0;
        pub const COC_EV_REQUEST_CID: usize = 0;
        pub const COC_EV_REQUEST_PSM: usize = 0;
        pub const COC_EV_REQUEST_ADDRESS: usize = 0;
        pub const COC_EV_REQUEST_ADDRESS_TYPE: usize = 0;
        pub const COC_EV_COMPLETE_CID: usize = 0;
        pub const COC_EV_COMPLETE_REMOTE_MTU: usize = 0;
        pub const COC_EV_CLOSE_CID: usize = 0;
        pub const COC_EV_CLOSE_REASON: usize = 0;
        pub const COC_EV_DATA_END: usize = 0;
        pub const COC_EV_DATA_OFFSET: usize = 0;
        pub const COC_EV_DATA_CID: usize = 0;
        pub const COC_EV_DATA_BASE: usize = 0;
        pub const COC_EV_PSM_CID: usize = 0;
        pub const COC_EV_CREDIT_CID: usize = 0;
        pub const COC_EV_CREDIT_ADDED: usize = 0;
        pub const SDU_BUFFER_END: usize = 0;
        pub const SDU_BUFFER_OFFSET: usize = 0;
        pub const SDU_BUFFER_CID: usize = 0;
        pub const SDU_BUFFER_META: usize = 0;
        pub const SDU_BUFFER_META_VALUE: u8 = 0;
        pub const SDU_BUFFER_PAYLOAD: usize = 0;
        pub const SDU_BUFFER_ALLOC_TAIL: usize = 0;
        pub const FRAGMENT_NODE_SIZE: usize = 0;
        pub const FRAGMENT_NODE_PAYLOAD: usize = 0;
        pub const FRAGMENT_NODE_BUFFER: usize = 0;
        pub const FRAGMENT_NODE_FLAGS: usize = 0;
        pub const FRAGMENT_FLAGS_FIRST_LAST: u8 = 0;

        pub const F_QJS_GET_EXCEPTION: usize = 0;
        pub const F_QJS_FREE_VALUE: usize = 0;
        pub const F_QJS_NEW_C_FUNCTION2: usize = 0;
        pub const F_QJS_NEW_OBJECT: usize = 0;
        pub const F_QJS_NEW_STRING_LEN: usize = 0;
        pub const F_QJS_TO_CSTRING_LEN2: usize = 0;
        pub const F_QJS_FREE_CSTRING: usize = 0;
        pub const F_QJS_IS_FUNCTION: usize = 0;
        pub const F_QJS_CALL: usize = 0;
        pub const F_QJS_DEFINE_PROPERTY_VALUE_STR: usize = 0;
        pub const F_QJS_NEW_ARRAY_BUFFER_COPY: usize = 0;
        pub const F_QJS_GET_ARRAY_BUFFER: usize = 0;
        pub const F_QJS_GET_TYPED_ARRAY_BUFFER: usize = 0;
        pub const F_FEATURE_REGISTER: usize = 0;
        pub const D_FRAMEWORK: usize = 0;
        pub const D_FRAMEWORK_VTABLE: usize = 0;
        pub const D_APPLICATION_VTABLE: usize = 0;
        pub const VTABLE_PREFIX_WORDS: usize = 0;
        pub const FRAMEWORK_FACTORY_SLOT: usize = 0;
        pub const FRAMEWORK_COPY_WORDS: usize = 0;
        pub const APPLICATION_INIT_SLOT: usize = 0;
        pub const APPLICATION_COPY_WORDS: usize = 0;
        pub const FRAMEWORK_IMPLEMENTATION: usize = 0;
        pub const FRAMEWORK_MUTEX: usize = 0;
        pub const FRAMEWORK_APPS_BEGIN: usize = 0;
        pub const FRAMEWORK_APPS_END: usize = 0;
        pub const APPLICATION_RUNTIME: usize = 0;
        pub const RUNTIME_MANAGER: usize = 0;
        pub const MANAGER_REGISTRY: usize = 0;
        pub const MANAGER_ASYNC: usize = 0;
        pub const HANDLE_LOOP: usize = 0;
        pub const PROTOTYPE_MANAGER: usize = 0;
        pub const INSTANCE_HANDLE: usize = 0;
        pub const JSVALUE_CLASS_OFFSET: usize = 0;
        pub const ARRAY_BUFFER_DATA: usize = 0;
        pub const ARRAY_BUFFER_DETACHED: usize = 0;
    };
}
