//! Exact-target Bluetooth transport. Events borrow firmware memory only for the
//! duration of `EventSink`; the runtime must copy them into its bounded queue.
//! GAP operations use the firmware's independent advertiser/scanner handles;
//! CoC work is marshalled onto Bluelet's owner task.

use crate::core::{Error, LinkId, Peer, SendId, SessionId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Service {
    slot: u8,
    session: SessionId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncomingId(pub u32);

#[derive(Clone, Copy, Debug)]
pub enum Event<'a> {
    ClientReady,
    ClientFailed {
        error: Error,
    },
    Registered {
        session: SessionId,
        spsm: u8,
    },
    ScanStarted {
        session: SessionId,
        status: u8,
    },
    ScanStopped {
        session: SessionId,
    },
    Advertisement {
        session: SessionId,
        peer: Peer,
        rssi: i8,
        packet_type: u8,
        data: &'a [u8],
    },
    AdvertisingStarted {
        session: SessionId,
        status: u8,
    },
    AdvertisingStopped {
        session: SessionId,
        status: u8,
    },
    Incoming {
        session: SessionId,
        request: IncomingId,
        peer: Peer,
        cid: u16,
        spsm: u8,
    },
    Assigned {
        link: LinkId,
        cid: u16,
    },
    Opened {
        link: LinkId,
        cid: u16,
        remote_receive_mtu: u16,
    },
    Data {
        link: LinkId,
        cid: u16,
        data: &'a [u8],
    },
    Credits {
        link: LinkId,
        cid: u16,
        added: u16,
    },
    Closed {
        link: LinkId,
        cid: u16,
        reason: u16,
    },
    Submitted {
        link: LinkId,
        send: Option<SendId>,
    },
    Failed {
        session: SessionId,
        link: Option<LinkId>,
        operation: u8,
        error: Error,
    },
    ServiceClosed {
        session: SessionId,
    },
}

pub type EventSink = for<'a> fn(Event<'a>);

#[cfg(all(feature = "device", target_arch = "arm"))]
mod bluelet;
#[cfg(all(feature = "device", target_arch = "arm"))]
pub use bluelet::*;

/// Host builds expose the contract without attempting firmware calls. Host
/// tests exercise the Engine with their own explicitly supplied transport.
#[cfg(not(all(feature = "device", target_arch = "arm")))]
mod unavailable {
    use super::*;
    pub fn supported() -> bool {
        false
    }
    pub fn install(_: EventSink) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub fn acquire(_: SessionId) -> Result<Service, Error> {
        Err(Error::Unsupported)
    }
    pub fn ensure_client() -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub fn client_ready() -> bool {
        false
    }
    pub fn register(_: Service) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub fn start_scan(_: Service) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub fn stop_scan(_: Service) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub fn start_advertising(_: Service, _: &[u8]) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub fn stop_advertising(_: Service) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub fn connect(_: Service, _: LinkId, _: Peer, _: u8) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub fn accept(_: Service, _: IncomingId, _: LinkId) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub fn reject(_: Service, _: IncomingId) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub fn send(_: Service, _: LinkId, _: Option<SendId>, _: &[u8]) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub fn disconnect(_: Service, _: LinkId) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    pub fn close(_: Service) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}
#[cfg(not(all(feature = "device", target_arch = "arm")))]
pub use unavailable::*;
