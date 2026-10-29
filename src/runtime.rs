//! Serialized core integration. No firmware call or JS call runs with IRQs off.
#![cfg_attr(test, allow(dead_code))]

use crate::{backend, core::*, protocol::*, qjs};
use ::core::{
    cell::UnsafeCell,
    ffi::c_void,
    sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, Ordering},
};
use canopus_abi::{CanopusContext, ContextV1};

const REQUESTS: usize = 32;
const EVENTS: usize = 40;

#[derive(Clone, Copy)]
struct Request {
    owner: u32,
    operation: u32,
    handle: u32,
    session: SessionId,
    token: u32,
    send: Option<SendId>,
    completed: Option<Notice>,
}
#[derive(Clone, Copy)]
struct Slot {
    id: SessionId,
    owner: u32,
    service: Option<backend::Service>,
    spsm: u8,
    data: AdvertisingData,
    advertising_ready: bool,
    advertising_launching: bool,
    stop_scan_ready: bool,
    stop_advertising_ready: bool,
    closing: bool,
    close_submitted: bool,
}
#[derive(Clone, Copy)]
enum Notice {
    Complete {
        token: u32,
        session: u32,
        link: u32,
        peer: Option<Peer>,
        max_message: u32,
        error: Option<Error>,
    },
    Advertisement {
        session: u32,
        peer: PeerRecord,
    },
    Connection {
        session: u32,
        link: u32,
        peer: Peer,
        max_message: u32,
    },
    Disconnect {
        session: u32,
        link: u32,
        reason: Error,
    },
}
#[derive(Clone, Copy)]
struct Envelope {
    owner: u32,
    notice: Notice,
}
struct Runtime {
    engine: Engine<DEVICE_MAX_LINKS>,
    slots: [Option<Slot>; MAX_SESSIONS],
    pending: [Option<Request>; REQUESTS],
    events: [Option<Envelope>; EVENTS],
    transmitting: [Option<LinkId>; DEVICE_MAX_LINKS],
    disconnecting: [Option<LinkId>; DEVICE_MAX_LINKS],
    now: u64,
    last_clock: u32,
}
impl Runtime {
    const fn new() -> Self {
        Self {
            engine: Engine::new(),
            slots: [None; MAX_SESSIONS],
            pending: [None; REQUESTS],
            events: [None; EVENTS],
            transmitting: [None; DEVICE_MAX_LINKS],
            disconnecting: [None; DEVICE_MAX_LINKS],
            now: 0,
            last_clock: 0,
        }
    }
    fn slot(&self, id: SessionId) -> Option<Slot> {
        self.slots
            .iter()
            .flatten()
            .find(|slot| slot.id == id)
            .copied()
    }
    fn slot_mut(&mut self, id: SessionId) -> Option<&mut Slot> {
        self.slots.iter_mut().flatten().find(|slot| slot.id == id)
    }
    fn session_owner(&self, id: SessionId) -> Option<u32> {
        self.slot(id).map(|slot| slot.owner)
    }
    fn push(&mut self, owner: u32, notice: Notice) {
        if owner == 0 {
            return;
        }
        // Advertisements are observations; reserve the last REQUESTS slots for
        // completions and disconnects so scan traffic cannot strand a Promise.
        let free = self.events.iter().filter(|event| event.is_none()).count();
        if matches!(notice, Notice::Advertisement { .. }) && free <= REQUESTS + 2 {
            return;
        }
        if let Some(slot) = self.events.iter_mut().find(|event| event.is_none()) {
            *slot = Some(Envelope { owner, notice });
        }
    }
    fn reserve(&mut self, request: Request) -> Result<usize, Error> {
        if self
            .pending
            .iter()
            .flatten()
            .any(|pending| pending.owner == request.owner && pending.token == request.token)
        {
            return Err(Error::InvalidArgument);
        }
        let used = self.pending.iter().flatten().count()
            + self
                .events
                .iter()
                .flatten()
                .filter(|event| !matches!(event.notice, Notice::Advertisement { .. }))
                .count();
        if used >= REQUESTS {
            return Err(Error::Busy);
        }
        let index = self
            .pending
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Busy)?;
        self.pending[index] = Some(request);
        Ok(index)
    }
    fn finish(&mut self, index: usize, link: Option<LinkId>, result: Result<(), Error>) {
        let Some(request) = self.pending[index] else {
            return;
        };
        if request.completed.is_some() {
            return;
        }
        let info = link
            .and_then(|id| self.engine.link(id).ok())
            .map(|link| (link.id.0, Some(link.peer), link.max_message_bytes as u32));
        let (link, peer, max_message) = info.unwrap_or((0, None, 0));
        let session = request.session.0;
        let notice = Notice::Complete {
            token: request.token,
            session,
            link,
            peer,
            max_message,
            error: result.err(),
        };
        if self.events.iter().any(Option::is_none) {
            self.pending[index] = None;
            self.push(request.owner, notice);
        } else {
            self.pending[index].as_mut().unwrap().completed = Some(notice);
        }
    }
    fn flush_completions(&mut self) {
        for index in 0..REQUESTS {
            if !self.events.iter().any(Option::is_none) {
                break;
            }
            if let Some(request) = self.pending[index] {
                if let Some(notice) = request.completed {
                    self.pending[index] = None;
                    self.push(request.owner, notice);
                }
            }
        }
    }
    fn remove_owner_events(&mut self, owner: u32) {
        let mut write = 0;
        for read in 0..EVENTS {
            if let Some(event) = self.events[read] {
                if event.owner != owner {
                    self.events[write] = Some(event);
                    write += 1;
                }
            }
        }
        self.events[write..].fill(None);
    }
    fn complete_operation(&mut self, handle: u32, operation: u32, result: Result<(), Error>) {
        for i in 0..REQUESTS {
            if self.pending[i]
                .is_some_and(|request| request.handle == handle && request.operation == operation)
            {
                self.finish(i, None, result);
            }
        }
    }
    fn collect_core_events(&mut self) {
        while let Some(event) = self.engine.take_event() {
            match event {
                Event::Connected {
                    link,
                    session,
                    inbound,
                } => {
                    let Some(owner) = self.session_owner(session) else {
                        continue;
                    };
                    if inbound {
                        let info = self.engine.link(link).unwrap();
                        self.push(
                            owner,
                            Notice::Connection {
                                session: session.0,
                                link: link.0,
                                peer: info.peer,
                                max_message: info.max_message_bytes as u32,
                            },
                        );
                    } else {
                        for i in 0..REQUESTS {
                            if self.pending[i].is_some_and(|request| {
                                request.operation == qjs::OP_CONNECT
                                    && request.send == Some(SendId(link.0))
                            }) {
                                self.finish(i, Some(link), Ok(()));
                            }
                        }
                    }
                }
                Event::Disconnected {
                    link,
                    session,
                    reason,
                } => {
                    for slot in &mut self.transmitting {
                        if *slot == Some(link) {
                            *slot = None;
                        }
                    }
                    for slot in &mut self.disconnecting {
                        if *slot == Some(link) {
                            *slot = None;
                        }
                    }
                    let owner = self.session_owner(session).unwrap_or(0);
                    for i in 0..REQUESTS {
                        if let Some(request) = self.pending[i] {
                            if request.operation == qjs::OP_CONNECT
                                && request.send == Some(SendId(link.0))
                                || request.operation == qjs::OP_SEND && request.handle == link.0
                            {
                                self.finish(i, None, Err(reason));
                            }
                        }
                    }
                    self.push(
                        owner,
                        Notice::Disconnect {
                            session: session.0,
                            link: link.0,
                            reason,
                        },
                    );
                }
            }
        }
    }
}

struct Shared(UnsafeCell<Runtime>);
unsafe impl Sync for Shared {}
static RUNTIME: Shared = Shared(UnsafeCell::new(Runtime::new()));
static CONTEXT: AtomicPtr<CanopusContext> = AtomicPtr::new(::core::ptr::null_mut());
static PUMPING: AtomicBool = AtomicBool::new(false);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static TIMER: AtomicI32 = AtomicI32::new(-1);

fn locked<T>(call: impl FnOnce(&mut Runtime) -> T) -> T {
    #[cfg(all(test, not(target_arch = "arm")))]
    {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap();
        unsafe { call(&mut *RUNTIME.0.get()) }
    }
    #[cfg(target_arch = "arm")]
    {
        // Single-core Cortex-M33: masking IRQs prevents task switches while touching
        // bounded state. Firmware allocations, IPC, callbacks and JS stay outside.
        let primask: u32;
        unsafe {
            ::core::arch::asm!("mrs {0}, PRIMASK", "cpsid i", out(reg) primask, options(nostack));
            let result = call(&mut *RUNTIME.0.get());
            if primask & 1 == 0 {
                ::core::arch::asm!("cpsie i", options(nostack));
            }
            result
        }
    }
}

fn error_code(error: Error) -> i32 {
    match error {
        Error::Unsupported => qjs::ERROR_UNSUPPORTED,
        Error::InvalidArgument => qjs::ERROR_INVALID_ARGUMENT,
        Error::Busy => qjs::ERROR_BUSY,
        Error::Timeout => qjs::ERROR_TIMEOUT,
        Error::Disconnected => qjs::ERROR_DISCONNECTED,
        Error::ReceiveOverflow => qjs::ERROR_RECEIVE_OVERFLOW,
        Error::Backend(_) => qjs::ERROR_BACKEND,
    }
}
fn wake_owners() {
    let owners = locked(|runtime| {
        let mut owners = [0; MAX_SESSIONS + EVENTS + REQUESTS];
        for (index, event) in runtime.events.iter().enumerate() {
            if let Some(event) = event {
                owners[index] = event.owner;
            }
        }
        for (index, slot) in runtime.slots.iter().enumerate() {
            if let Some(slot) = slot {
                if slot.advertising_ready
                    || runtime.engine.links().any(|link| {
                        link.owner == slot.owner
                            && (runtime.engine.has_received(link.id)
                                || link.state == LinkState::CheckingService
                                || (!runtime.transmitting.contains(&Some(link.id))
                                    && runtime.pending.iter().flatten().any(|request| {
                                        request.operation == qjs::OP_SEND
                                            && request.handle == link.id.0
                                    })))
                    })
                {
                    owners[EVENTS + index] = slot.owner;
                }
            }
        }
        for (index, request) in runtime.pending.iter().enumerate() {
            if let Some(request) = request {
                if request.completed.is_some() {
                    owners[EVENTS + MAX_SESSIONS + index] = request.owner;
                }
            }
        }
        owners
    });
    for (index, owner) in owners.iter().copied().enumerate() {
        if owner != 0 && !owners[..index].contains(&owner) {
            qjs::notify(owner);
        }
    }
}
fn capabilities() -> qjs::Capabilities {
    qjs::Capabilities {
        supported: ACTIVE.load(Ordering::Acquire),
        max_links: DEVICE_MAX_LINKS as u32,
        max_service_id_bytes: MAX_SERVICE_ID_BYTES as u32,
        max_advertising_data_bytes: MAX_ADVERTISING_DATA_BYTES as u32,
    }
}
fn create_session(owner: u32, bytes: &[u8]) -> Result<u32, i32> {
    if !ACTIVE.load(Ordering::Acquire) {
        return Err(qjs::ERROR_UNSUPPORTED);
    }
    let service = ::core::str::from_utf8(bytes).map_err(|_| qjs::ERROR_INVALID_ARGUMENT)?;
    let id = locked(|runtime| {
        let slot = runtime
            .slots
            .iter()
            .position(Option::is_none)
            .ok_or(Error::Busy)?;
        let id = runtime.engine.create_session(owner, service)?;
        runtime.slots[slot] = Some(Slot {
            id,
            owner,
            service: None,
            spsm: 0,
            data: AdvertisingData::EMPTY,
            advertising_ready: false,
            advertising_launching: false,
            stop_scan_ready: false,
            stop_advertising_ready: false,
            closing: false,
            close_submitted: false,
        });
        Ok::<_, Error>(id)
    })
    .map_err(error_code)?;
    match backend::acquire(id) {
        Ok(service) => {
            locked(|runtime| runtime.slot_mut(id).unwrap().service = Some(service));
            Ok(id.0)
        }
        Err(error) => {
            locked(|runtime| {
                runtime.engine.close_session(id);
                if let Some(index) = runtime
                    .slots
                    .iter()
                    .position(|slot| slot.is_some_and(|slot| slot.id == id))
                {
                    runtime.slots[index] = None;
                }
            });
            Err(error_code(error))
        }
    }
}

#[derive(Clone, Copy)]
#[allow(clippy::large_enum_variant)] // Bounded owned SDU crosses the lock without heap allocation.
enum Work {
    None,
    Scan(backend::Service),
    StopScan(backend::Service),
    Register(backend::Service),
    Advertise(SessionId, backend::Service, [u8; 31], usize),
    StopAdvertising(backend::Service),
    Connect(backend::Service, LinkId, Peer, u8),
    Close(backend::Service),
    Disconnect(backend::Service, LinkId),
    Send(backend::Service, Transmission),
}
fn perform(work: Work) -> Result<(), Error> {
    match work {
        Work::None => Ok(()),
        Work::Scan(service) => backend::start_scan(service),
        Work::StopScan(service) => backend::stop_scan(service),
        Work::Register(service) => backend::register(service),
        Work::Advertise(_, service, data, len) => backend::start_advertising(service, &data[..len]),
        Work::StopAdvertising(service) => backend::stop_advertising(service),
        Work::Connect(service, link, peer, spsm) => backend::connect(service, link, peer, spsm),
        Work::Close(service) => backend::close(service),
        Work::Disconnect(service, link) => backend::disconnect(service, link),
        Work::Send(service, tx) => backend::send(service, tx.link, tx.send, tx.message.as_bytes()),
    }
}
fn request(
    owner: u32,
    operation: u32,
    handle: u32,
    token: u32,
    data: &[u8],
    peer_bytes: &[u8],
    flag: bool,
) -> Result<(), i32> {
    let (index, work) = locked(|runtime| -> Result<_, Error> {
        let link_operation = operation == qjs::OP_SEND || operation == qjs::OP_DISCONNECT;
        let session = if link_operation {
            runtime.engine.link(LinkId(handle))?.session
        } else {
            SessionId(handle)
        };
        let slot = runtime
            .slot(session)
            .filter(|slot| slot.owner == owner)
            .ok_or(Error::Disconnected)?;
        if slot.closing && operation != qjs::OP_CLOSE {
            return Err(Error::Disconnected);
        }
        let service = slot.service.ok_or(Error::Busy)?;
        let index = runtime.reserve(Request {
            owner,
            operation,
            handle,
            session,
            token,
            send: None,
            completed: None,
        })?;
        let result = (|| -> Result<Work, Error> {
            Ok(match operation {
                qjs::OP_START_SCAN => {
                    runtime.engine.begin_scan(session, flag)?;
                    Work::Scan(service)
                }
                qjs::OP_STOP_SCAN => {
                    if runtime.engine.begin_stop_scan(session)? {
                        runtime.slot_mut(session).unwrap().stop_scan_ready = true;
                        runtime.complete_operation(
                            handle,
                            qjs::OP_START_SCAN,
                            Err(Error::Disconnected),
                        );
                        Work::None
                    } else if runtime.engine.session(session)?.scan == ResourceState::Idle {
                        runtime.finish(index, None, Ok(()));
                        Work::None
                    } else {
                        Work::None
                    }
                }
                qjs::OP_START_ADVERTISING => {
                    let data = AdvertisingData::new(data)?;
                    runtime.engine.begin_advertising(session)?;
                    let slot = runtime.slot_mut(session).unwrap();
                    slot.data = data;
                    if slot.spsm == 0 {
                        Work::Register(service)
                    } else {
                        slot.advertising_ready = true;
                        Work::None
                    }
                }
                qjs::OP_STOP_ADVERTISING => {
                    if runtime.engine.begin_stop_advertising(session)? {
                        runtime.slot_mut(session).unwrap().advertising_ready = false;
                        runtime.complete_operation(
                            handle,
                            qjs::OP_START_ADVERTISING,
                            Err(Error::Disconnected),
                        );
                        if slot.advertising_launching {
                            runtime.slot_mut(session).unwrap().stop_advertising_ready = true;
                        } else {
                            runtime.engine.advertising_stopped(session)?;
                            runtime.finish(index, None, Ok(()));
                        }
                        Work::None
                    } else if runtime.engine.session(session)?.advertising == ResourceState::Idle {
                        runtime.finish(index, None, Ok(()));
                        Work::None
                    } else {
                        Work::None
                    }
                }
                qjs::OP_CONNECT => {
                    let peer = Peer::decode(
                        ::core::str::from_utf8(peer_bytes).map_err(|_| Error::InvalidArgument)?,
                    )?;
                    let spsm = runtime
                        .engine
                        .session(session)?
                        .peer(peer)
                        .ok_or(Error::InvalidArgument)?
                        .spsm;
                    let (link, fresh) = runtime.engine.connect(session, peer, runtime.now)?;
                    runtime.pending[index].as_mut().unwrap().send = Some(SendId(link.0));
                    if runtime.engine.link(link)?.state == LinkState::Connected {
                        runtime.finish(index, Some(link), Ok(()));
                        Work::None
                    } else if fresh {
                        Work::Connect(service, link, peer, spsm)
                    } else {
                        Work::None
                    }
                }
                qjs::OP_SEND => {
                    let send = runtime.engine.send(LinkId(handle), data)?;
                    runtime.pending[index].as_mut().unwrap().send = Some(send);
                    Work::None
                }
                qjs::OP_DISCONNECT => {
                    runtime
                        .engine
                        .close_link(LinkId(handle), Error::Disconnected)?;
                    // Completion waits until the backend actually reports Closed.
                    if runtime.engine.link(LinkId(handle))?.channel.is_none() {
                        runtime.finish(index, None, Ok(()));
                    }
                    Work::None
                }
                qjs::OP_CLOSE => {
                    if !slot.closing {
                        runtime.engine.close_session(session);
                        let slot = runtime.slot_mut(session).unwrap();
                        slot.closing = true;
                        slot.advertising_ready = false;
                        for i in 0..REQUESTS {
                            if i != index
                                && runtime.pending[i].is_some_and(|request| {
                                    request.handle == handle && request.operation != qjs::OP_CLOSE
                                })
                            {
                                runtime.finish(i, None, Err(Error::Disconnected));
                            }
                        }
                    }
                    Work::None
                }
                _ => return Err(Error::InvalidArgument),
            })
        })();
        match result {
            Ok(work) => {
                runtime.collect_core_events();
                Ok((index, work))
            }
            Err(error) => {
                runtime.pending[index] = None;
                Err(error)
            }
        }
    })
    .map_err(error_code)?;
    if let Err(error) = perform(work) {
        locked(|runtime| {
            if let Some(request) = runtime.pending[index] {
                match request.operation {
                    qjs::OP_START_SCAN => {
                        let _ = runtime.engine.scan_started(SessionId(handle), Err(error));
                    }
                    qjs::OP_START_ADVERTISING => {
                        let _ = runtime
                            .engine
                            .advertising_started(SessionId(handle), Err(error));
                    }
                    qjs::OP_CONNECT => {
                        if let Some(link) = request.send {
                            let _ = runtime.engine.close_link(LinkId(link.0), error);
                        }
                    }
                    _ => (),
                }
            }
            runtime.finish(index, None, Err(error));
            runtime.collect_core_events();
        });
    }
    pump();
    wake_owners();
    Ok(())
}

fn apply_event(
    runtime: &mut Runtime,
    event: backend::Event<'_>,
) -> Option<(backend::Service, backend::IncomingId, Result<LinkId, Error>)> {
    let mut incoming = None;
    match event {
        backend::Event::Registered { session, spsm } => {
            if let Some(slot) = runtime.slot_mut(session) {
                slot.spsm = spsm;
                if !slot.closing {
                    slot.advertising_ready = true;
                }
            }
        }
        backend::Event::ScanStarted { session, status } => {
            let result = if status == 0 {
                Ok(())
            } else {
                Err(Error::Backend(status as i32))
            };
            let previous = runtime.engine.session(session).map(|session| session.scan);
            if previous == Ok(ResourceState::Starting) {
                let _ = runtime.engine.scan_started(session, result);
            } else if previous == Ok(ResourceState::Stopping) && status != 0 {
                let _ = runtime.engine.scan_stopped(session);
                if let Some(slot) = runtime.slot_mut(session) {
                    slot.stop_scan_ready = false;
                }
                runtime.complete_operation(session.0, qjs::OP_STOP_SCAN, Ok(()));
            }
            runtime.complete_operation(
                session.0,
                qjs::OP_START_SCAN,
                if previous == Ok(ResourceState::Stopping) {
                    Err(Error::Disconnected)
                } else {
                    result
                },
            );
        }
        backend::Event::ScanStopped { session } => {
            let _ = runtime.engine.scan_stopped(session);
            if let Some(slot) = runtime.slot_mut(session) {
                slot.stop_scan_ready = false;
            }
            runtime.complete_operation(session.0, qjs::OP_STOP_SCAN, Ok(()));
        }
        backend::Event::Advertisement {
            session,
            peer,
            rssi,
            data,
            ..
        } => {
            if let Some(record) = runtime.engine.advertisement(session, peer, rssi, data) {
                if let Some(owner) = runtime.session_owner(session) {
                    runtime.push(
                        owner,
                        Notice::Advertisement {
                            session: session.0,
                            peer: record,
                        },
                    );
                }
            }
        }
        backend::Event::AdvertisingStarted { session, status } => {
            if let Some(slot) = runtime.slot_mut(session) {
                slot.advertising_launching = status == 0;
            }
            let result = if status == 0 {
                Ok(())
            } else {
                Err(Error::Backend(status as i32))
            };
            let previous = runtime
                .engine
                .session(session)
                .map(|session| session.advertising);
            if previous == Ok(ResourceState::Starting) {
                let _ = runtime.engine.advertising_started(session, result);
            } else if previous == Ok(ResourceState::Stopping) && status != 0 {
                let _ = runtime.engine.advertising_stopped(session);
                if let Some(slot) = runtime.slot_mut(session) {
                    slot.stop_advertising_ready = false;
                }
                runtime.complete_operation(session.0, qjs::OP_STOP_ADVERTISING, Ok(()));
            }
            runtime.complete_operation(
                session.0,
                qjs::OP_START_ADVERTISING,
                if previous == Ok(ResourceState::Stopping) {
                    Err(Error::Disconnected)
                } else {
                    result
                },
            );
        }
        backend::Event::AdvertisingStopped { session, status } => {
            if let Some(slot) = runtime.slot_mut(session) {
                slot.stop_advertising_ready = false;
                slot.advertising_launching = false;
            }
            let _ = runtime.engine.advertising_stopped(session);
            runtime.complete_operation(
                session.0,
                qjs::OP_STOP_ADVERTISING,
                if status == 0 {
                    Ok(())
                } else {
                    Err(Error::Backend(status as i32))
                },
            );
        }
        backend::Event::Incoming {
            session,
            request,
            peer,
            ..
        } => {
            if let Some(service) = runtime.slot(session).and_then(|slot| slot.service) {
                let available = runtime
                    .events
                    .iter()
                    .filter(|event| event.is_none())
                    .count();
                let reserved = runtime.pending.iter().flatten().count();
                let link = if available >= reserved + 2 {
                    runtime.engine.accept(session, peer, runtime.now)
                } else {
                    Err(Error::Busy)
                };
                incoming = Some((service, request, link));
            }
        }
        backend::Event::Assigned { link, .. } => {
            let _ = runtime.engine.acl_connected(link);
        }
        backend::Event::Opened {
            link,
            cid,
            remote_receive_mtu,
        } => {
            let _ = runtime.engine.channel_opened(link, cid, remote_receive_mtu);
        }
        backend::Event::Data { link, data, .. } => {
            let _ = runtime.engine.receive(link, data);
        }
        backend::Event::Credits { .. } => (),
        backend::Event::Submitted { link, send } => {
            for slot in &mut runtime.transmitting {
                if *slot == Some(link) {
                    *slot = None;
                }
            }
            let _ = runtime.engine.transmission_submitted(link, send);
            if let Some(send) = send {
                for i in 0..REQUESTS {
                    if runtime.pending[i].is_some_and(|request| {
                        request.operation == qjs::OP_SEND
                            && request.handle == link.0
                            && request.send == Some(send)
                    }) {
                        runtime.finish(i, None, Ok(()));
                    }
                }
            }
        }
        backend::Event::Closed { link, reason, .. } => {
            let _ = runtime.engine.channel_closed(
                link,
                if reason == 0 {
                    Error::Disconnected
                } else {
                    Error::Backend(reason as i32)
                },
            );
            runtime.complete_operation(link.0, qjs::OP_DISCONNECT, Ok(()));
        }
        backend::Event::Failed {
            session,
            link,
            error,
        } => {
            if let Some(link) = link {
                for slot in &mut runtime.disconnecting {
                    if *slot == Some(link) {
                        *slot = None;
                    }
                }
                let _ = runtime.engine.close_link(link, error);
            } else {
                if let Some(slot) = runtime.slot_mut(session) {
                    if slot.closing {
                        slot.close_submitted = false;
                    }
                }
                // Session-level Bluelet work is Register or Close. GAP start/
                // stop errors have their own callbacks or synchronous result.
                runtime.complete_operation(session.0, qjs::OP_START_ADVERTISING, Err(error));
                let _ = runtime.engine.advertising_started(session, Err(error));
            }
        }
        backend::Event::ServiceClosed { session } => {
            let links: [Option<LinkId>; DEVICE_MAX_LINKS] = ::core::array::from_fn(|index| {
                runtime
                    .engine
                    .links()
                    .filter(|link| link.session == session)
                    .nth(index)
                    .map(|link| link.id)
            });
            for link in links.into_iter().flatten() {
                let _ = runtime.engine.channel_closed(link, Error::Disconnected);
                runtime.complete_operation(link.0, qjs::OP_DISCONNECT, Ok(()));
            }
            runtime.complete_operation(session.0, qjs::OP_CLOSE, Ok(()));
            if let Some(slot) = runtime
                .slots
                .iter_mut()
                .find(|slot| slot.is_some_and(|slot| slot.id == session))
            {
                *slot = None;
            }
        }
    }
    runtime.collect_core_events();
    incoming
}
fn receive(event: backend::Event<'_>) {
    let incoming = locked(|runtime| apply_event(runtime, event));
    if let Some((service, request, link)) = incoming {
        match link {
            Ok(link) => {
                if let Err(error) = backend::accept(service, request, link) {
                    let _ = backend::reject(service, request);
                    locked(|runtime| {
                        let _ = runtime.engine.close_link(link, error);
                        runtime.collect_core_events();
                    });
                }
            }
            Err(_) => {
                let _ = backend::reject(service, request);
            }
        }
    }
    wake_owners();
}

fn next_work(runtime: &mut Runtime) -> Work {
    for slot in runtime.slots.iter_mut().flatten() {
        let Some(service) = slot.service else {
            continue;
        };
        if slot.closing && !slot.close_submitted {
            slot.close_submitted = true;
            return Work::Close(service);
        }
        if slot.stop_scan_ready && !slot.closing {
            slot.stop_scan_ready = false;
            return Work::StopScan(service);
        }
        if slot.stop_advertising_ready && !slot.closing {
            slot.stop_advertising_ready = false;
            return Work::StopAdvertising(service);
        }
        if slot.advertising_ready && !slot.closing {
            slot.advertising_ready = false;
            if let Ok(session) = runtime.engine.session(slot.id) {
                if session.advertising != ResourceState::Starting {
                    continue;
                }
                let advertisement = Advertisement {
                    service: session.service,
                    spsm: slot.spsm,
                    data: slot.data,
                };
                let mut data = [0; 31];
                if let Ok(len) = advertisement.encode(&mut data) {
                    slot.advertising_launching = true;
                    return Work::Advertise(slot.id, service, data, len);
                }
            }
        }
    }
    if let Some((link, _)) = runtime.engine.next_disconnect() {
        if !runtime.disconnecting.contains(&Some(link)) {
            if let Ok(info) = runtime.engine.link(link) {
                if let Some(slot) = runtime.slot(info.session) {
                    if let Some(service) = slot.service {
                        if let Some(slot) =
                            runtime.disconnecting.iter_mut().find(|slot| slot.is_none())
                        {
                            *slot = Some(link);
                        }
                        return Work::Disconnect(service, link);
                    }
                }
            }
        }
    }
    for _ in 0..DEVICE_MAX_LINKS {
        if let Some(tx) = runtime.engine.next_transmission() {
            if runtime.transmitting.contains(&Some(tx.link)) {
                continue;
            }
            if let Ok(info) = runtime.engine.link(tx.link) {
                if let Some(service) = runtime.slot(info.session).and_then(|slot| slot.service) {
                    if let Some(slot) = runtime.transmitting.iter_mut().find(|slot| slot.is_none())
                    {
                        *slot = Some(tx.link);
                    }
                    return Work::Send(service, tx);
                }
            }
        }
    }
    Work::None
}
fn pump() {
    if PUMPING.swap(true, Ordering::AcqRel) {
        return;
    }
    for _ in 0..16 {
        let work = locked(next_work);
        if matches!(work, Work::None) {
            break;
        }
        if let Err(error) = perform(work) {
            locked(|runtime| match work {
                Work::Send(_, tx) => {
                    for slot in &mut runtime.transmitting {
                        if *slot == Some(tx.link) {
                            *slot = None;
                        }
                    }
                    if error != Error::Busy {
                        let _ = runtime.engine.close_link(tx.link, error);
                    }
                }
                Work::Disconnect(_, link) => {
                    for slot in &mut runtime.disconnecting {
                        if *slot == Some(link) {
                            *slot = None;
                        }
                    }
                    if error != Error::Busy {
                        let _ = runtime.engine.channel_closed(link, error);
                    }
                }
                Work::Close(service) => {
                    for slot in runtime.slots.iter_mut().flatten() {
                        if slot.service == Some(service) {
                            slot.close_submitted = false;
                        }
                    }
                }
                Work::Advertise(id, _, _, _) => {
                    if let Some(slot) = runtime.slot_mut(id) {
                        slot.advertising_launching = false;
                    }
                    let _ = runtime.engine.advertising_started(id, Err(error));
                    runtime.complete_operation(id.0, qjs::OP_START_ADVERTISING, Err(error));
                }
                Work::StopScan(service) | Work::StopAdvertising(service) => {
                    for slot in runtime.slots.iter_mut().flatten() {
                        if slot.service == Some(service) {
                            if matches!(work, Work::StopScan(_)) {
                                slot.stop_scan_ready = true;
                            } else {
                                slot.stop_advertising_ready = true;
                            }
                        }
                    }
                }
                _ => (),
            });
            if error == Error::Busy {
                break;
            }
        }
        locked(|runtime| runtime.collect_core_events());
    }
    PUMPING.store(false, Ordering::Release);
}

fn close_owner(owner: u32) {
    locked(|runtime| {
        for request in &mut runtime.pending {
            if request.is_some_and(|request| request.owner == owner) {
                *request = None;
            }
        }
        runtime.remove_owner_events(owner);
        let ids = runtime
            .slots
            .map(|slot| slot.filter(|slot| slot.owner == owner).map(|slot| slot.id));
        for id in ids.into_iter().flatten() {
            runtime.engine.close_session(id);
            let slot = runtime.slot_mut(id).unwrap();
            slot.owner = 0;
            slot.closing = true;
            slot.advertising_ready = false;
        }
        runtime.collect_core_events();
    });
    pump();
}
fn next_event(owner: u32, output: &mut qjs::Event) -> bool {
    pump();
    locked(|runtime| {
        runtime.collect_core_events();
        runtime.flush_completions();
        *output = qjs::Event::EMPTY;
        if let Some(index) = runtime
            .events
            .iter()
            .position(|event| event.is_some_and(|event| event.owner == owner))
        {
            let event = runtime.events[index].take().unwrap();
            // Compact to preserve FIFO across reused slots.
            runtime.events[index..].rotate_left(1);
            match event.notice {
                Notice::Complete {
                    token,
                    session,
                    link,
                    peer,
                    max_message,
                    error,
                } => {
                    output.kind = qjs::EVENT_COMPLETE;
                    output.token = token;
                    output.session = session;
                    output.link = link;
                    output.max_message_bytes = max_message;
                    if let Some(peer) = peer {
                        output.peer = peer.encode();
                        output.peer_len = 14;
                    }
                    if let Some(error) = error {
                        output.error = error_code(error);
                        if let Error::Backend(raw) = error {
                            output.backend_error = raw;
                        }
                    }
                }
                Notice::Advertisement { session, peer } => {
                    output.kind = qjs::EVENT_ADVERTISEMENT;
                    output.session = session;
                    output.peer = peer.peer.encode();
                    output.peer_len = 14;
                    output.rssi = peer.rssi as i32;
                    output.data_len = peer.data.as_bytes().len();
                    output.data[..output.data_len].copy_from_slice(peer.data.as_bytes());
                }
                Notice::Connection {
                    session,
                    link,
                    peer,
                    max_message,
                } => {
                    output.kind = qjs::EVENT_CONNECTION;
                    output.session = session;
                    output.link = link;
                    output.peer = peer.encode();
                    output.peer_len = 14;
                    output.max_message_bytes = max_message;
                }
                Notice::Disconnect {
                    session,
                    link,
                    reason,
                } => {
                    output.kind = qjs::EVENT_DISCONNECT;
                    output.session = session;
                    output.link = link;
                    output.error = error_code(reason);
                    if let Error::Backend(raw) = reason {
                        output.backend_error = raw;
                    }
                }
            }
            return true;
        }
        let id = runtime
            .engine
            .links()
            .find(|link| link.owner == owner && link.state == LinkState::Connected)
            .map(|link| link.id);
        if let Some(id) = id {
            if let Ok(Some(message)) = runtime.engine.take_received(id) {
                output.kind = qjs::EVENT_DATA;
                output.session = runtime.engine.link(id).unwrap().session.0;
                output.link = id.0;
                output.data_len = message.as_bytes().len();
                output.data[..output.data_len].copy_from_slice(message.as_bytes());
                return true;
            }
        }
        false
    })
}

pub static ADAPTER: qjs::Adapter = qjs::Adapter {
    capabilities,
    create_session,
    request,
    close_owner,
    next_event,
};

unsafe extern "C" fn timeout_tick(ctx: *const ContextV1, _: i32, _: *mut c_void) {
    let context = &*(ctx as *const CanopusContext);
    let clock = ((*context.time).monotonic_ms)(ctx);
    locked(|runtime| {
        runtime.now += clock.wrapping_sub(runtime.last_clock) as u64;
        runtime.last_clock = clock;
        runtime.engine.tick(runtime.now);
        runtime.collect_core_events();
    });
    // The Supervisor's ordinary background task is allowed to issue GAP IPC.
    // This timer supplies deadlines and retries resources held during shutdown;
    // Bluetooth events themselves wake the QJS task immediately through notify.
    pump();
    wake_owners();
}

pub unsafe fn install(context: *const CanopusContext) -> Result<(), Error> {
    if ACTIVE.load(Ordering::Acquire) {
        return Ok(());
    }
    if context.is_null() || (*context).time.is_null() {
        return Err(Error::Unsupported);
    }
    let opaque = context.cast::<ContextV1>();
    if !canopus_abi::services_has(opaque, canopus_abi::SVC_TIME, 2) {
        return Err(Error::Unsupported);
    }
    backend::install(receive)?;
    let time = &*(*context).time;
    let timer = (time.timer_create)(opaque, 100, 1, timeout_tick, ::core::ptr::null_mut());
    if timer < 0 {
        return Err(Error::Backend(timer));
    }
    let background = (time.timer_set_background)(opaque, timer, 1);
    let start = if background == 0 {
        (time.timer_start)(opaque, timer)
    } else {
        background
    };
    if start != 0 {
        (time.timer_destroy)(opaque, timer);
        return Err(Error::Backend(start));
    }
    let clock = (time.monotonic_ms)(opaque);
    locked(|runtime| {
        runtime.last_clock = clock;
    });
    CONTEXT.store(context.cast_mut(), Ordering::Release);
    TIMER.store(timer, Ordering::Release);
    ACTIVE.store(true, Ordering::Release);
    Ok(())
}

/// Roll back activation before the native Feature publishes any JS owners.
pub unsafe fn rollback_before_feature() {
    ACTIVE.store(false, Ordering::Release);
    let context = CONTEXT.swap(::core::ptr::null_mut(), Ordering::AcqRel);
    let timer = TIMER.swap(-1, Ordering::AcqRel);
    if !context.is_null() && timer >= 0 {
        let opaque = context.cast::<ContextV1>();
        let time = &*(*context).time;
        (time.timer_stop)(opaque, timer);
        (time.timer_destroy)(opaque, timer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(runtime: &mut Runtime, owner: u32, name: &str) -> SessionId {
        let id = runtime.engine.create_session(owner, name).unwrap();
        let index = runtime.slots.iter().position(Option::is_none).unwrap();
        runtime.slots[index] = Some(Slot {
            id,
            owner,
            service: None,
            spsm: 0,
            data: AdvertisingData::EMPTY,
            advertising_ready: false,
            advertising_launching: false,
            stop_scan_ready: false,
            stop_advertising_ready: false,
            closing: false,
            close_submitted: false,
        });
        id
    }

    #[test]
    fn completions_keep_their_owner_after_service_slot_is_released() {
        let mut runtime = Runtime::new();
        let id = session(&mut runtime, 42, "chat");
        let index = runtime
            .reserve(Request {
                owner: 42,
                operation: qjs::OP_CLOSE,
                handle: id.0,
                session: id,
                token: 9,
                send: None,
                completed: None,
            })
            .unwrap();
        runtime.engine.close_session(id);
        runtime.finish(index, None, Ok(()));
        runtime.slots = [None; MAX_SESSIONS];
        let notice = runtime.events.iter().flatten().next().unwrap();
        assert_eq!(notice.owner, 42);
        assert!(
            matches!(notice.notice, Notice::Complete { token:9, session, error:None, .. } if session == id.0)
        );
    }

    #[test]
    fn operation_completion_does_not_cross_session_or_operation() {
        let mut runtime = Runtime::new();
        let a = session(&mut runtime, 1, "chat");
        let b = session(&mut runtime, 2, "other");
        for (token, id, op) in [
            (1, a, qjs::OP_START_ADVERTISING),
            (2, b, qjs::OP_START_ADVERTISING),
            (3, a, qjs::OP_START_SCAN),
        ] {
            runtime
                .reserve(Request {
                    owner: id.0,
                    operation: op,
                    handle: id.0,
                    session: id,
                    token,
                    send: None,
                    completed: None,
                })
                .unwrap();
        }
        runtime.complete_operation(a.0, qjs::OP_START_ADVERTISING, Err(Error::Backend(13)));
        assert_eq!(runtime.pending.iter().flatten().count(), 2);
        let events: Vec<_> = runtime.events.iter().flatten().collect();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0].notice,
            Notice::Complete {
                token: 1,
                error: Some(Error::Backend(13)),
                ..
            }
        ));
    }

    #[test]
    fn scan_flood_cannot_displace_reserved_completions() {
        let mut runtime = Runtime::new();
        let id = session(&mut runtime, 1, "chat");
        let peer = PeerRecord {
            peer: Peer {
                address: [1; 6],
                address_type: 0,
            },
            spsm: 0x80,
            data: AdvertisingData::EMPTY,
            rssi: -40,
        };
        for _ in 0..100 {
            runtime.push(
                1,
                Notice::Advertisement {
                    session: id.0,
                    peer,
                },
            );
        }
        assert_eq!(
            runtime.events.iter().flatten().count(),
            EVENTS - REQUESTS - 2
        );
        for token in 1..=REQUESTS {
            let index = runtime
                .reserve(Request {
                    owner: 1,
                    operation: qjs::OP_STOP_SCAN,
                    handle: id.0,
                    session: id,
                    token: token as u32,
                    send: None,
                    completed: None,
                })
                .unwrap();
            runtime.finish(index, None, Ok(()));
        }
        assert_eq!(
            runtime
                .events
                .iter()
                .flatten()
                .filter(|event| matches!(event.notice, Notice::Complete { .. }))
                .count(),
            REQUESTS
        );
        assert_eq!(
            runtime.reserve(Request {
                owner: 1,
                operation: qjs::OP_STOP_SCAN,
                handle: id.0,
                session: id,
                token: 100,
                send: None,
                completed: None,
            }),
            Err(Error::Busy)
        );
    }

    #[test]
    fn failed_start_finishes_a_deferred_stop_for_both_resources() {
        for advertising in [false, true] {
            let mut runtime = Runtime::new();
            let id = session(&mut runtime, 1, "chat");
            let operation = if advertising {
                runtime.engine.begin_advertising(id).unwrap();
                runtime.engine.begin_stop_advertising(id).unwrap();
                runtime.slot_mut(id).unwrap().stop_advertising_ready = true;
                qjs::OP_STOP_ADVERTISING
            } else {
                runtime.engine.begin_scan(id, false).unwrap();
                runtime.engine.begin_stop_scan(id).unwrap();
                runtime.slot_mut(id).unwrap().stop_scan_ready = true;
                qjs::OP_STOP_SCAN
            };
            runtime
                .reserve(Request {
                    owner: 1,
                    operation,
                    handle: id.0,
                    session: id,
                    token: 1,
                    send: None,
                    completed: None,
                })
                .unwrap();
            let event = if advertising {
                backend::Event::AdvertisingStarted {
                    session: id,
                    status: 1,
                }
            } else {
                backend::Event::ScanStarted {
                    session: id,
                    status: 1,
                }
            };
            apply_event(&mut runtime, event);
            let state = runtime.engine.session(id).unwrap();
            assert_eq!(
                if advertising {
                    state.advertising
                } else {
                    state.scan
                },
                ResourceState::Idle
            );
            assert!(runtime.pending.iter().all(Option::is_none));
            assert!(matches!(
                runtime.events[0].unwrap().notice,
                Notice::Complete { error: None, .. }
            ));
        }
    }

    #[test]
    fn failed_psm_registration_does_not_cancel_independent_scan() {
        let mut runtime = Runtime::new();
        let id = session(&mut runtime, 1, "chat");
        runtime.engine.begin_scan(id, false).unwrap();
        runtime.engine.begin_advertising(id).unwrap();
        for (token, operation) in [(1, qjs::OP_START_SCAN), (2, qjs::OP_START_ADVERTISING)] {
            runtime
                .reserve(Request {
                    owner: 1,
                    operation,
                    handle: id.0,
                    session: id,
                    token,
                    send: None,
                    completed: None,
                })
                .unwrap();
        }
        apply_event(
            &mut runtime,
            backend::Event::Failed {
                session: id,
                link: None,
                error: Error::Busy,
            },
        );
        assert_eq!(runtime.pending.iter().flatten().count(), 1);
        assert_eq!(
            runtime.pending.iter().flatten().next().unwrap().operation,
            qjs::OP_START_SCAN
        );
        assert_eq!(
            runtime.engine.session(id).unwrap().scan,
            ResourceState::Starting
        );
        assert_eq!(
            runtime.engine.session(id).unwrap().advertising,
            ResourceState::Idle
        );
    }

    #[test]
    fn asynchronous_close_failure_keeps_cleanup_retryable() {
        let mut runtime = Runtime::new();
        let id = session(&mut runtime, 1, "chat");
        let slot = runtime.slot_mut(id).unwrap();
        slot.closing = true;
        slot.close_submitted = true;
        apply_event(
            &mut runtime,
            backend::Event::Failed {
                session: id,
                link: None,
                error: Error::Busy,
            },
        );
        assert!(!runtime.slot(id).unwrap().close_submitted);
    }

    #[test]
    fn removing_another_owner_preserves_connection_disconnect_fifo() {
        let mut runtime = Runtime::new();
        let peer = Peer {
            address: [1; 6],
            address_type: 0,
        };
        runtime.push(
            2,
            Notice::Disconnect {
                session: 2,
                link: 3,
                reason: Error::Disconnected,
            },
        );
        runtime.push(
            1,
            Notice::Connection {
                session: 1,
                link: 4,
                peer,
                max_message: 512,
            },
        );
        runtime.remove_owner_events(2);
        runtime.push(
            1,
            Notice::Disconnect {
                session: 1,
                link: 4,
                reason: Error::Disconnected,
            },
        );
        assert!(matches!(
            runtime.events[0].unwrap().notice,
            Notice::Connection { .. }
        ));
        assert!(matches!(
            runtime.events[1].unwrap().notice,
            Notice::Disconnect { .. }
        ));
    }

    #[test]
    fn full_unsolicited_queue_cannot_lose_an_accepted_completion() {
        let mut runtime = Runtime::new();
        let id = session(&mut runtime, 1, "chat");
        let index = runtime
            .reserve(Request {
                owner: 1,
                operation: qjs::OP_CLOSE,
                handle: id.0,
                session: id,
                token: 42,
                send: None,
                completed: None,
            })
            .unwrap();
        for i in 0..EVENTS {
            runtime.push(
                1,
                Notice::Disconnect {
                    session: 1,
                    link: i as u32,
                    reason: Error::Disconnected,
                },
            );
        }
        runtime.finish(index, None, Ok(()));
        assert!(runtime.pending[index].unwrap().completed.is_some());
        runtime.events[0] = None;
        runtime.events.rotate_left(1);
        runtime.flush_completions();
        assert!(runtime.pending[index].is_none());
        assert!(matches!(
            runtime.events[EVENTS - 1].unwrap().notice,
            Notice::Complete {
                token: 42,
                error: None,
                ..
            }
        ));
    }

    #[test]
    fn completion_preserves_session_when_link_handle_is_gone() {
        let mut runtime = Runtime::new();
        let id = session(&mut runtime, 1, "chat");
        let index = runtime
            .reserve(Request {
                owner: 1,
                operation: qjs::OP_DISCONNECT,
                handle: 999,
                session: id,
                token: 3,
                send: None,
                completed: None,
            })
            .unwrap();
        runtime.finish(index, None, Ok(()));
        assert!(
            matches!(runtime.events[0].unwrap().notice, Notice::Complete { session, .. } if session == id.0)
        );
    }

    #[test]
    fn service_retirement_clears_core_disconnect_for_already_closed_links() {
        let mut runtime = Runtime::new();
        let id = session(&mut runtime, 1, "chat");
        runtime.engine.begin_advertising(id).unwrap();
        runtime.engine.advertising_started(id, Ok(())).unwrap();
        let link = runtime
            .engine
            .accept(
                id,
                Peer {
                    address: [1; 6],
                    address_type: 0,
                },
                0,
            )
            .unwrap();
        runtime.engine.close_session(id);
        runtime.collect_core_events();
        assert_eq!(runtime.engine.next_disconnect(), Some((link, None)));
        apply_event(&mut runtime, backend::Event::ServiceClosed { session: id });
        assert!(runtime.engine.next_disconnect().is_none());
    }

    #[test]
    fn data_adapter_event_carries_session_and_complete_binary_sdu() {
        let mut runtime = Runtime::new();
        let id = session(&mut runtime, 1, "chat");
        runtime.engine.begin_scan(id, false).unwrap();
        runtime.engine.scan_started(id, Ok(())).unwrap();
        let peer = Peer {
            address: [1; 6],
            address_type: 0,
        };
        let advertisement = Advertisement {
            service: ServiceId::new("chat").unwrap(),
            spsm: 0x80,
            data: AdvertisingData::EMPTY,
        };
        let mut bytes = [0; 31];
        let len = advertisement.encode(&mut bytes).unwrap();
        runtime
            .engine
            .advertisement(id, peer, -40, &bytes[..len])
            .unwrap();
        let (link, _) = runtime.engine.connect(id, peer, 0).unwrap();
        runtime.engine.channel_opened(link, 0x40, 512).unwrap();
        runtime.engine.receive(link, b"\x01\x04chat").unwrap();
        runtime.engine.transmission_submitted(link, None).unwrap();
        runtime.collect_core_events();
        runtime.engine.receive(link, &[0, 128, 255]).unwrap();
        locked(|state| *state = runtime);
        let mut event = qjs::Event::EMPTY;
        assert!(next_event(1, &mut event));
        assert_eq!(event.kind, qjs::EVENT_DATA);
        assert_eq!(event.session, id.0);
        assert_eq!(event.link, link.0);
        assert_eq!(&event.data[..event.data_len], &[0, 128, 255]);
        assert!(!next_event(1, &mut event));
        locked(|state| *state = Runtime::new());
    }

    #[test]
    fn duplicate_tokens_are_rejected_without_losing_original_request() {
        let mut runtime = Runtime::new();
        let request = Request {
            owner: 1,
            operation: qjs::OP_START_SCAN,
            handle: 1,
            session: SessionId(1),
            token: 99,
            send: None,
            completed: None,
        };
        runtime.reserve(request).unwrap();
        assert_eq!(runtime.reserve(request), Err(Error::InvalidArgument));
        assert_eq!(runtime.pending.iter().flatten().count(), 1);
    }
}
