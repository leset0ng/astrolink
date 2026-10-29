//! Q66 .155 Feature/QJS bridge. Only `device` links the exact firmware ABI.
//! All Adapter callbacks must serialize their own engine access; none may call
//! JavaScript. `notify` is safe from Bluetooth tasks and schedules a JS drain.

pub const OP_START_SCAN: u32 = 1;
pub const OP_STOP_SCAN: u32 = 2;
pub const OP_START_ADVERTISING: u32 = 3;
pub const OP_STOP_ADVERTISING: u32 = 4;
pub const OP_CONNECT: u32 = 5;
pub const OP_SEND: u32 = 6;
pub const OP_DISCONNECT: u32 = 7;
pub const OP_CLOSE: u32 = 8;
pub const EVENT_COMPLETE: u32 = 1;
pub const EVENT_ADVERTISEMENT: u32 = 2;
pub const EVENT_CONNECTION: u32 = 3;
pub const EVENT_DATA: u32 = 4;
pub const EVENT_DISCONNECT: u32 = 5;

/// Errors crossing the native boundary. Positive errno-like error codes are
/// emitted in events; a failed synchronous `request` returns the negative code.
pub const ERROR_UNSUPPORTED: i32 = 1;
pub const ERROR_INVALID_ARGUMENT: i32 = 2;
pub const ERROR_BUSY: i32 = 3;
pub const ERROR_TIMEOUT: i32 = 4;
pub const ERROR_DISCONNECTED: i32 = 5;
pub const ERROR_RECEIVE_OVERFLOW: i32 = 6;
pub const ERROR_BACKEND: i32 = 7;

#[derive(Clone, Copy)]
pub struct Capabilities {
    pub supported: bool,
    pub max_links: u32,
    pub max_service_id_bytes: u32,
    pub max_advertising_data_bytes: u32,
}

/// Every event is copied before invoking JS. `data_len` and `peer_len` must be
/// within the arrays; invalid values are rejected by the binding.
pub struct Event {
    pub kind: u32,
    pub token: u32,
    pub session: u32,
    pub link: u32,
    pub peer: [u8; 14],
    pub peer_len: usize,
    pub max_message_bytes: u32,
    pub rssi: i32,
    pub error: i32,
    pub backend_error: i32,
    pub data: [u8; 512],
    pub data_len: usize,
}
impl Event {
    pub const EMPTY: Self = Self {
        kind: 0,
        token: 0,
        session: 0,
        link: 0,
        peer: [0; 14],
        peer_len: 0,
        max_message_bytes: 0,
        rssi: 0,
        error: 0,
        backend_error: 0,
        data: [0; 512],
        data_len: 0,
    };
}

pub type Request = fn(
    owner: u32,
    operation: u32,
    handle: u32,
    token: u32,
    data: &[u8],
    peer: &[u8],
    flag: bool,
) -> Result<(), i32>;

pub struct Adapter {
    pub capabilities: fn() -> Capabilities,
    pub create_session: fn(owner: u32, service: &[u8]) -> Result<u32, i32>,
    pub request: Request,
    pub close_owner: fn(owner: u32),
    pub next_event: fn(owner: u32, event: &mut Event) -> bool,
}

#[cfg(all(test, not(feature = "device")))]
pub(crate) fn notify(_: u32) {}

#[cfg(feature = "device")]
mod target;
#[cfg(feature = "device")]
pub use target::{install, notify};
