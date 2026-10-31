//! Bounded, allocation-free Session/Link state owned by the module runtime.
//!
//! Callers serialize access. Bluetooth callbacks copy borrowed payloads before
//! returning; QuickJS callbacks consume these queues only on their owner task.

use crate::protocol::{self, Advertisement, AdvertisingData, ServiceId};

pub const MAX_SESSIONS: usize = 4;
pub const MAX_PEERS: usize = 8;
pub const MAX_MESSAGE_BYTES: usize = 512;
pub const QUEUE_DEPTH: usize = 4;
/// Shared TX/RX message budget across all Links, each still bounded by QUEUE_DEPTH.
pub const POOL: usize = 40;
pub const CONNECT_TIMEOUT_MS: u64 = 15_000;
pub const DEVICE_MAX_LINKS: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Unsupported,
    InvalidArgument,
    Busy,
    Timeout,
    Disconnected,
    ReceiveOverflow,
    Backend(i32),
    NotReady,
}

impl Error {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unsupported => "UNSUPPORTED",
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::Busy => "BUSY",
            Self::Timeout => "TIMEOUT",
            Self::Disconnected => "DISCONNECTED",
            Self::ReceiveOverflow => "RECEIVE_OVERFLOW",
            Self::Backend(_) => "BACKEND_ERROR",
            Self::NotReady => "BUSY",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageId(pub u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Peer {
    pub address: [u8; 6],
    pub address_type: u8,
}

impl Peer {
    /// Opaque printable identity includes the address type; no allocation.
    pub fn encode(self) -> [u8; 14] {
        let mut output = [0; 14];
        let bytes = [
            self.address_type,
            self.address[0],
            self.address[1],
            self.address[2],
            self.address[3],
            self.address[4],
            self.address[5],
        ];
        for (i, byte) in bytes.iter().enumerate() {
            output[i * 2] = b"0123456789abcdef"[(byte >> 4) as usize];
            output[i * 2 + 1] = b"0123456789abcdef"[(byte & 15) as usize];
        }
        output
    }

    pub fn decode(value: &str) -> Result<Self, Error> {
        fn nibble(value: u8) -> Result<u8, Error> {
            match value {
                b'0'..=b'9' => Ok(value - b'0'),
                b'a'..=b'f' => Ok(value - b'a' + 10),
                b'A'..=b'F' => Ok(value - b'A' + 10),
                _ => Err(Error::InvalidArgument),
            }
        }
        if value.len() != 14 {
            return Err(Error::InvalidArgument);
        }
        let mut bytes = [0; 7];
        for (i, part) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            bytes[i] = (nibble(part[0])? << 4) | nibble(part[1])?;
        }
        if bytes[0] > 3 {
            return Err(Error::InvalidArgument);
        }
        Ok(Self {
            address_type: bytes[0],
            address: bytes[1..].try_into().unwrap(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceState {
    Idle,
    Starting,
    Active,
    Stopping,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkState {
    Connecting,
    Opening,
    CheckingService,
    Connected,
    Closed,
}

#[derive(Clone, Copy, Debug)]
pub struct PeerRecord {
    pub peer: Peer,
    pub spsm: u8,
    pub data: AdvertisingData,
    pub rssi: i8,
}

#[derive(Clone, Copy, Debug)]
pub struct Session {
    pub id: SessionId,
    pub owner: u32,
    pub service: ServiceId,
    pub scan: ResourceState,
    pub advertising: ResourceState,
    pub allow_duplicates: bool,
    // Connectable advertising can terminate before the peer opens CoC on its
    // established ACL. Only explicit stop/close revokes listener admission.
    accepting: bool,
    peers: [Option<PeerRecord>; MAX_PEERS],
    next_peer: usize,
}

impl Session {
    pub fn peer(&self, peer: Peer) -> Option<PeerRecord> {
        self.peers
            .iter()
            .flatten()
            .find(|record| record.peer == peer)
            .copied()
    }
}

#[derive(Clone, Copy)]
pub struct Message {
    bytes: [u8; MAX_MESSAGE_BYTES],
    len: u16,
}

impl Message {
    pub const EMPTY: Self = Self {
        bytes: [0; MAX_MESSAGE_BYTES],
        len: 0,
    };

    pub fn new(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(Error::InvalidArgument);
        }
        let mut message = Self::EMPTY;
        message.bytes[..bytes.len()].copy_from_slice(bytes);
        message.len = bytes.len() as u16;
        Ok(message)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

#[derive(Clone, Copy)]
struct Queue<T: Copy, const N: usize> {
    entries: [Option<T>; N],
    head: usize,
    len: usize,
}

impl<T: Copy, const N: usize> Queue<T, N> {
    const fn new() -> Self {
        Self {
            entries: [None; N],
            head: 0,
            len: 0,
        }
    }
    fn push(&mut self, value: T) -> Result<(), Error> {
        if self.len == N {
            return Err(Error::Busy);
        }
        self.entries[(self.head + self.len) % N] = Some(value);
        self.len += 1;
        Ok(())
    }
    fn front(&self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.entries[self.head]
        }
    }
    fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let value = self.entries[self.head].take();
        self.head = (self.head + 1) % N;
        self.len -= 1;
        value
    }
}

#[derive(Clone, Copy)]
struct PendingSend {
    id: SendId,
    message: MessageId,
}

#[derive(Clone, Copy)]
pub struct Link {
    pub id: LinkId,
    pub session: SessionId,
    pub owner: u32,
    pub peer: Peer,
    pub inbound: bool,
    pub state: LinkState,
    pub channel: Option<u16>,
    pub max_message_bytes: usize,
    pub close_reason: Error,
    service: ServiceId,
    started_ms: u64,
    hello_sent: bool,
    hello_received: bool,
    ready_pending: bool,
    close_pending: bool,
    disconnect_pending: bool,
    tx: Queue<PendingSend, QUEUE_DEPTH>,
    rx: Queue<MessageId, QUEUE_DEPTH>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    Connected {
        link: LinkId,
        session: SessionId,
        inbound: bool,
    },
    Disconnected {
        link: LinkId,
        session: SessionId,
        reason: Error,
    },
}

#[derive(Clone, Copy)]
pub struct Transmission {
    pub link: LinkId,
    pub channel: u16,
    /// None denotes the initial HELLO; it never completes an application send.
    pub send: Option<SendId>,
    pub message: Message,
}

#[derive(Clone, Copy, Debug)]
pub struct Cleanup {
    pub scan: ResourceState,
    pub advertising: ResourceState,
}

pub struct Engine<const LINKS: usize> {
    sessions: [Option<Session>; MAX_SESSIONS],
    links: [Option<Link>; LINKS],
    pool: [Option<Message>; POOL],
    free_messages: u64,
    next_id: u32,
    round_robin: usize,
}

impl<const LINKS: usize> Default for Engine<LINKS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const LINKS: usize> Engine<LINKS> {
    pub const fn new() -> Self {
        Self {
            sessions: [None; MAX_SESSIONS],
            links: [None; LINKS],
            pool: [None; POOL],
            free_messages: (1u64 << POOL) - 1,
            next_id: 1,
            round_robin: 0,
        }
    }

    fn allocate_message(&mut self, message: Message) -> Result<MessageId, Error> {
        if self.free_messages == 0 {
            return Err(Error::Busy);
        }
        let slot = self.free_messages.trailing_zeros() as usize;
        self.free_messages &= !(1u64 << slot);
        self.pool[slot] = Some(message);
        Ok(MessageId(slot as u16))
    }

    fn release_message(&mut self, id: MessageId) -> Message {
        let slot = usize::from(id.0);
        let message = self.pool[slot].take().expect("queued message owns a slot");
        self.free_messages |= 1u64 << slot;
        message
    }

    fn allocate_id(&mut self) -> Result<u32, Error> {
        let id = self.next_id;
        // Exhaustion is explicit. Handles never wrap and resurrect old objects.
        self.next_id = id.checked_add(1).ok_or(Error::Busy)?;
        Ok(id)
    }

    pub fn create_session(&mut self, owner: u32, service: &str) -> Result<SessionId, Error> {
        let service = ServiceId::new(service)?;
        if self
            .sessions
            .iter()
            .flatten()
            .any(|session| session.service == service)
        {
            return Err(Error::Busy);
        }
        let slot = self
            .sessions
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Busy)?;
        let id = SessionId(self.allocate_id()?);
        self.sessions[slot] = Some(Session {
            id,
            owner,
            service,
            scan: ResourceState::Idle,
            advertising: ResourceState::Idle,
            allow_duplicates: false,
            accepting: false,
            peers: [None; MAX_PEERS],
            next_peer: 0,
        });
        Ok(id)
    }

    pub fn session(&self, id: SessionId) -> Result<&Session, Error> {
        self.sessions
            .iter()
            .flatten()
            .find(|session| session.id == id)
            .ok_or(Error::Disconnected)
    }

    fn session_mut(&mut self, id: SessionId) -> Result<&mut Session, Error> {
        self.sessions
            .iter_mut()
            .flatten()
            .find(|session| session.id == id)
            .ok_or(Error::Disconnected)
    }

    pub fn sessions(&self) -> impl Iterator<Item = &Session> {
        self.sessions.iter().flatten()
    }
    pub fn links(&self) -> impl Iterator<Item = &Link> {
        self.links.iter().flatten()
    }

    pub fn begin_scan(&mut self, id: SessionId, allow_duplicates: bool) -> Result<(), Error> {
        let session = self.session_mut(id)?;
        if session.scan != ResourceState::Idle {
            return Err(Error::Busy);
        }
        session.scan = ResourceState::Starting;
        session.allow_duplicates = allow_duplicates;
        session.peers = [None; MAX_PEERS];
        session.next_peer = 0;
        Ok(())
    }

    pub fn scan_started(&mut self, id: SessionId, result: Result<(), Error>) -> Result<(), Error> {
        let session = self.session_mut(id)?;
        if session.scan != ResourceState::Starting {
            return Err(Error::Disconnected);
        }
        session.scan = if result.is_ok() {
            ResourceState::Active
        } else {
            ResourceState::Idle
        };
        result
    }

    pub fn begin_stop_scan(&mut self, id: SessionId) -> Result<bool, Error> {
        let session = self.session_mut(id)?;
        match session.scan {
            ResourceState::Idle | ResourceState::Stopping => Ok(false),
            _ => {
                session.scan = ResourceState::Stopping;
                Ok(true)
            }
        }
    }

    pub fn scan_stopped(&mut self, id: SessionId) -> Result<(), Error> {
        self.session_mut(id)?.scan = ResourceState::Idle;
        Ok(())
    }

    pub fn begin_advertising(&mut self, id: SessionId) -> Result<(), Error> {
        let session = self.session_mut(id)?;
        if session.advertising != ResourceState::Idle {
            return Err(Error::Busy);
        }
        session.advertising = ResourceState::Starting;
        Ok(())
    }

    pub fn advertising_started(
        &mut self,
        id: SessionId,
        result: Result<(), Error>,
    ) -> Result<(), Error> {
        let session = self.session_mut(id)?;
        if session.advertising != ResourceState::Starting {
            return Err(Error::Disconnected);
        }
        session.advertising = if result.is_ok() {
            ResourceState::Active
        } else {
            ResourceState::Idle
        };
        session.accepting = result.is_ok();
        result
    }

    pub fn begin_stop_advertising(&mut self, id: SessionId) -> Result<bool, Error> {
        let session = self.session_mut(id)?;
        session.accepting = false;
        match session.advertising {
            ResourceState::Idle | ResourceState::Stopping => Ok(false),
            _ => {
                session.advertising = ResourceState::Stopping;
                Ok(true)
            }
        }
    }

    pub fn advertising_stopped(&mut self, id: SessionId) -> Result<(), Error> {
        self.session_mut(id)?.advertising = ResourceState::Idle;
        Ok(())
    }

    /// Returns a complete matching advertisement only when it should be delivered.
    pub fn advertisement(
        &mut self,
        id: SessionId,
        peer: Peer,
        rssi: i8,
        bytes: &[u8],
    ) -> Option<PeerRecord> {
        let session = self.session_mut(id).ok()?;
        if session.scan != ResourceState::Active {
            return None;
        }
        let advertisement = Advertisement::decode(bytes)?;
        if advertisement.service != session.service {
            return None;
        }
        let record = PeerRecord {
            peer,
            spsm: advertisement.spsm,
            data: advertisement.data,
            rssi,
        };
        let existing = session
            .peers
            .iter()
            .position(|entry| entry.is_some_and(|old| old.peer == peer));
        let duplicate = existing.is_some_and(|slot| {
            session.peers[slot]
                .is_some_and(|old| old.data == record.data && old.spsm == record.spsm)
        });
        let slot = existing.unwrap_or_else(|| {
            let slot = session.next_peer;
            session.next_peer = (slot + 1) % MAX_PEERS;
            slot
        });
        session.peers[slot] = Some(record);
        if duplicate && !session.allow_duplicates {
            None
        } else {
            Some(record)
        }
    }

    pub fn connect(
        &mut self,
        session: SessionId,
        peer: Peer,
        now_ms: u64,
    ) -> Result<(LinkId, bool), Error> {
        self.session(session)?
            .peer(peer)
            .ok_or(Error::InvalidArgument)?;
        if let Some(link) = self.links().find(|link| {
            link.session == session && link.peer == peer && link.state != LinkState::Closed
        }) {
            return Ok((link.id, false));
        }
        Ok((self.allocate_link(session, peer, false, now_ms)?, true))
    }

    pub fn accept(&mut self, session: SessionId, peer: Peer, now_ms: u64) -> Result<LinkId, Error> {
        if !self.session(session)?.accepting {
            return Err(Error::Busy);
        }
        if self.links().any(|link| {
            link.session == session && link.peer == peer && link.state != LinkState::Closed
        }) {
            return Err(Error::Busy);
        }
        self.allocate_link(session, peer, true, now_ms)
    }

    fn allocate_link(
        &mut self,
        session: SessionId,
        peer: Peer,
        inbound: bool,
        now_ms: u64,
    ) -> Result<LinkId, Error> {
        let owner = self.session(session)?.owner;
        let service = self.session(session)?.service;
        let slot = self
            .links
            .iter()
            .position(|entry| {
                entry.is_none_or(|link| {
                    link.state == LinkState::Closed
                        && !link.close_pending
                        && !link.disconnect_pending
                })
            })
            .ok_or(Error::Busy)?;
        let id = LinkId(self.allocate_id()?);
        self.links[slot] = Some(Link {
            id,
            session,
            owner,
            peer,
            inbound,
            state: LinkState::Connecting,
            channel: None,
            max_message_bytes: 0,
            close_reason: Error::Disconnected,
            service,
            started_ms: now_ms,
            hello_sent: false,
            hello_received: false,
            ready_pending: false,
            close_pending: false,
            disconnect_pending: false,
            tx: Queue::new(),
            rx: Queue::new(),
        });
        Ok(id)
    }

    pub fn link(&self, id: LinkId) -> Result<&Link, Error> {
        self.links()
            .find(|link| link.id == id)
            .ok_or(Error::Disconnected)
    }

    fn link_mut(&mut self, id: LinkId) -> Result<&mut Link, Error> {
        self.links
            .iter_mut()
            .flatten()
            .find(|link| link.id == id)
            .ok_or(Error::Disconnected)
    }

    pub fn acl_connected(&mut self, id: LinkId) -> Result<(), Error> {
        let link = self.link_mut(id)?;
        if link.state != LinkState::Connecting {
            return Err(Error::Disconnected);
        }
        link.state = LinkState::Opening;
        Ok(())
    }

    /// peer_receive_mtu is the remote receive limit, never our local receive MTU.
    pub fn channel_opened(
        &mut self,
        id: LinkId,
        channel: u16,
        peer_receive_mtu: u16,
    ) -> Result<(), Error> {
        let link = self.link_mut(id)?;
        if !matches!(link.state, LinkState::Connecting | LinkState::Opening) {
            return Err(Error::Disconnected);
        }
        link.channel = Some(channel);
        if peer_receive_mtu < (2 + link.service.as_bytes().len()) as u16 {
            self.close_link(id, Error::InvalidArgument)?;
            return Err(Error::InvalidArgument);
        }
        link.max_message_bytes = usize::from(peer_receive_mtu).min(MAX_MESSAGE_BYTES);
        link.state = LinkState::CheckingService;
        Ok(())
    }

    pub fn send(&mut self, id: LinkId, bytes: &[u8]) -> Result<SendId, Error> {
        let link = self.link(id)?;
        if link.state != LinkState::Connected {
            return Err(Error::Disconnected);
        }
        if bytes.len() > link.max_message_bytes {
            return Err(Error::InvalidArgument);
        }
        if link.tx.len == QUEUE_DEPTH || self.free_messages == 0 {
            return Err(Error::Busy);
        }
        let message = Message::new(bytes)?;
        let send = SendId(self.allocate_id()?);
        let message = self.allocate_message(message)?;
        if let Err(error) = self
            .link_mut(id)?
            .tx
            .push(PendingSend { id: send, message })
        {
            self.release_message(message);
            return Err(error);
        }
        Ok(send)
    }

    /// Inspect at most one SDU per Link per pass. A BUSY backend simply leaves
    /// that transmission queued while the caller continues to the next Link.
    /// The returned copy owns its bytes; the pool slot remains queued for retries
    /// until transmission_submitted() confirms host acceptance.
    pub fn next_transmission(&mut self) -> Option<Transmission> {
        for _ in 0..LINKS {
            let slot = self.round_robin;
            self.round_robin = (slot + 1) % LINKS;
            let Some(link) = &self.links[slot] else {
                continue;
            };
            let Some(channel) = link.channel else {
                continue;
            };
            if link.state == LinkState::CheckingService && !link.hello_sent {
                let mut bytes = [0; protocol::MAX_HELLO_BYTES];
                let len = protocol::encode_hello(link.service, &mut bytes);
                return Some(Transmission {
                    link: link.id,
                    channel,
                    send: None,
                    message: Message::new(&bytes[..len]).ok()?,
                });
            }
            if link.state == LinkState::Connected {
                if let Some(pending) = link.tx.front() {
                    return Some(Transmission {
                        link: link.id,
                        channel,
                        send: Some(pending.id),
                        message: self.pool[usize::from(pending.message.0)]
                            .expect("queued message owns a slot"),
                    });
                }
            }
        }
        None
    }

    /// Called only after the host has accepted responsibility for the SDU.
    pub fn transmission_submitted(
        &mut self,
        id: LinkId,
        send: Option<SendId>,
    ) -> Result<(), Error> {
        let link = self.link_mut(id)?;
        match send {
            None if link.state == LinkState::CheckingService && !link.hello_sent => {
                link.hello_sent = true;
                Self::finish_handshake(link);
            }
            Some(send)
                if link.state == LinkState::Connected
                    && link.tx.front().is_some_and(|p| p.id == send) =>
            {
                let pending = link.tx.pop().unwrap();
                self.release_message(pending.message);
            }
            _ => return Err(Error::Disconnected),
        }
        Ok(())
    }

    fn finish_handshake(link: &mut Link) {
        if link.hello_sent && link.hello_received {
            link.state = LinkState::Connected;
            link.ready_pending = true;
        }
    }

    pub fn receive(&mut self, id: LinkId, bytes: &[u8]) -> Result<(), Error> {
        let link = self.link_mut(id)?;
        match link.state {
            LinkState::CheckingService if !link.hello_received => {
                if let Err(error) = protocol::validate_hello(link.service, bytes) {
                    self.close_link(id, error)?;
                    return Err(error);
                }
                link.hello_received = true;
                Self::finish_handshake(link);
                Ok(())
            }
            // Our HELLO can still be waiting for host credits after the peer
            // has sent its HELLO. Its subsequent SDUs must remain queued.
            LinkState::Connected | LinkState::CheckingService => {
                let message = match Message::new(bytes) {
                    Ok(message) => message,
                    Err(_) => {
                        self.close_link(id, Error::ReceiveOverflow)?;
                        return Err(Error::ReceiveOverflow);
                    }
                };
                let message = match self.allocate_message(message) {
                    Ok(message) => message,
                    Err(_) => {
                        self.close_link(id, Error::ReceiveOverflow)?;
                        return Err(Error::ReceiveOverflow);
                    }
                };
                if self.link_mut(id)?.rx.push(message).is_err() {
                    self.release_message(message);
                    self.close_link(id, Error::ReceiveOverflow)?;
                    return Err(Error::ReceiveOverflow);
                }
                Ok(())
            }
            _ => Err(Error::Disconnected),
        }
    }

    pub fn has_received(&self, id: LinkId) -> bool {
        self.link(id)
            .is_ok_and(|link| link.state == LinkState::Connected && link.rx.len != 0)
    }

    pub fn take_received(&mut self, id: LinkId) -> Result<Option<Message>, Error> {
        let link = self.link_mut(id)?;
        if link.state != LinkState::Connected {
            return Err(Error::Disconnected);
        }
        let message = link.rx.pop();
        Ok(message.map(|message| self.release_message(message)))
    }

    pub fn close_link(&mut self, id: LinkId, reason: Error) -> Result<(), Error> {
        let link = self.link_mut(id)?;
        if link.state == LinkState::Closed {
            return Ok(());
        }
        link.state = LinkState::Closed;
        link.close_reason = reason;
        link.ready_pending = false;
        link.close_pending = true;
        // Also cancel an in-flight ACL/CoC request that does not have a CID yet.
        link.disconnect_pending = true;
        let mut rx = ::core::mem::replace(&mut link.rx, Queue::new());
        let mut tx = ::core::mem::replace(&mut link.tx, Queue::new());
        while let Some(message) = rx.pop() {
            self.release_message(message);
        }
        while let Some(pending) = tx.pop() {
            self.release_message(pending.message);
        }
        Ok(())
    }

    pub fn channel_closed(&mut self, id: LinkId, reason: Error) -> Result<(), Error> {
        self.close_link(id, reason)?;
        let link = self.link_mut(id)?;
        link.disconnect_pending = false;
        link.channel = None;
        Ok(())
    }

    pub fn next_disconnect(&self) -> Option<(LinkId, Option<u16>)> {
        self.links()
            .find(|link| link.disconnect_pending)
            .map(|link| (link.id, link.channel))
    }

    pub fn disconnect_submitted(&mut self, id: LinkId) -> Result<(), Error> {
        self.link_mut(id)?.disconnect_pending = false;
        Ok(())
    }

    pub fn take_event(&mut self) -> Option<Event> {
        for link in self.links.iter_mut().flatten() {
            if link.close_pending {
                link.close_pending = false;
                return Some(Event::Disconnected {
                    link: link.id,
                    session: link.session,
                    reason: link.close_reason,
                });
            }
            if link.ready_pending {
                link.ready_pending = false;
                return Some(Event::Connected {
                    link: link.id,
                    session: link.session,
                    inbound: link.inbound,
                });
            }
        }
        None
    }

    pub fn tick(&mut self, now_ms: u64) {
        for slot in 0..LINKS {
            let Some(link) = &self.links[slot] else {
                continue;
            };
            if !matches!(link.state, LinkState::Connected | LinkState::Closed)
                && now_ms.saturating_sub(link.started_ms) >= CONNECT_TIMEOUT_MS
            {
                let id = link.id;
                let _ = self.close_link(id, Error::Timeout);
            }
        }
    }

    pub fn close_session(&mut self, id: SessionId) -> Option<Cleanup> {
        let slot = self
            .sessions
            .iter()
            .position(|session| session.is_some_and(|session| session.id == id))?;
        let session = self.sessions[slot].take()?;
        for link in 0..LINKS {
            if self.links[link].is_some_and(|link| link.session == id) {
                let id = self.links[link].as_ref().unwrap().id;
                let _ = self.close_link(id, Error::Disconnected);
            }
        }
        Some(Cleanup {
            scan: session.scan,
            advertising: session.advertising,
        })
    }
}
