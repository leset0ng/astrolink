//! Bluelet GAP and legacy LE CoC transport, shared by every adapted target.
//! Firmware addresses and structure offsets come from `crate::abi`; the logic
//! below is target independent. `docs/ble-abi.md` records the recovery.
use super::*;
use crate::abi;
use ::core::{
    cell::UnsafeCell,
    ptr,
    sync::atomic::{AtomicU16, AtomicU32, AtomicU8, AtomicUsize, Ordering},
};

const SLOTS: usize = 4;
const CHANNELS: usize = 8;
const RECEIVE_MTU: u16 = 512;
const RECEIVE_MPS: u16 = 64;
const INITIAL_CREDITS: u16 = 5;
const STARTING: usize = 1;
static SINK: AtomicUsize = AtomicUsize::new(0);
static CLIENT: AtomicUsize = AtomicUsize::new(0);
static CLIENT_STARTING: AtomicUsize = AtomicUsize::new(0);
static ADAPTER_REGISTERED: AtomicUsize = AtomicUsize::new(0);
struct GapPipe(UnsafeCell<crate::loop_dispatch::PipeWake>);
// Initialization is serialized by CLIENT_STARTING; sends hold loop+76.
unsafe impl Sync for GapPipe {}
static GAP_PIPE: GapPipe = GapPipe(UnsafeCell::new(crate::loop_dispatch::PipeWake::EMPTY));
static NEXT_REQUEST: AtomicU32 = AtomicU32::new(1);

extern "C" fn adapter_state_changed(_: usize, _: i32) {}
// Only state changes are needed. Pairing and connection callbacks stay stock.
#[repr(C)]
struct AdapterCallbacks {
    state: Option<extern "C" fn(usize, i32)>,
    reserved: [Option<unsafe extern "C" fn()>; 15],
}
static ADAPTER_CALLBACKS: AdapterCallbacks = AdapterCallbacks {
    state: Some(adapter_state_changed),
    reserved: [None; 15],
};

#[derive(Clone, Copy)]
struct Channel {
    cid: u16,
    request: u32,
    link: Option<LinkId>,
    opened: bool,
    remote_mtu: u16,
    pending: *mut Work,
}
impl Channel {
    const EMPTY: Self = Self {
        cid: 0,
        request: 0,
        link: None,
        opened: false,
        remote_mtu: 0,
        pending: ptr::null_mut(),
    };
}
struct OwnerState {
    channels: [Channel; CHANNELS],
    connecting: Option<LinkId>,
}
struct Slot {
    session: AtomicU32,
    closing: AtomicU8, // 0 active, 1 closing, 2 notifying, 3 vacant
    psm: AtomicU16,
    scan: AtomicUsize,
    advertising: AtomicUsize,
    scan_stopping: AtomicU8,
    adv_stopping: AtomicU8,
    live: AtomicUsize,
    work: AtomicUsize,
    owner: UnsafeCell<OwnerState>,
}
// Only Bluelet's owner accesses `owner`. Cross-task state consists of atomics.
unsafe impl Sync for Slot {}
impl Slot {
    const fn new() -> Self {
        Self {
            session: AtomicU32::new(0),
            closing: AtomicU8::new(3),
            psm: AtomicU16::new(0),
            scan: AtomicUsize::new(0),
            advertising: AtomicUsize::new(0),
            scan_stopping: AtomicU8::new(0),
            adv_stopping: AtomicU8::new(0),
            live: AtomicUsize::new(0),
            work: AtomicUsize::new(0),
            owner: UnsafeCell::new(OwnerState {
                channels: [Channel::EMPTY; CHANNELS],
                connecting: None,
            }),
        }
    }
}
static STATE: [Slot; SLOTS] = [const { Slot::new() }; SLOTS];

macro_rules! fw {
    ($address:expr, $signature:ty $(, $argument:expr)* $(,)?) => {{
        let function: $signature = ::core::mem::transmute(($address as usize) | 1);
        function($($argument),*)
    }};
}
unsafe fn alloc(size: usize) -> *mut u8 {
    let p = fw!(
        abi::F_ALLOC,
        unsafe extern "C" fn(u32) -> *mut u8,
        size as u32
    );
    if !p.is_null() {
        ptr::write_bytes(p, 0, size);
    }
    p
}
unsafe fn free(p: *mut u8) {
    fw!(abi::F_FREE, unsafe extern "C" fn(*mut u8), p);
}
unsafe fn word(p: *const u8, offset: usize) -> u16 {
    ptr::read_unaligned(p.add(offset).cast())
}
unsafe fn put_word(p: *mut u8, offset: usize, value: u16) {
    ptr::write_unaligned(p.add(offset).cast(), value);
}
unsafe fn put_dword(p: *mut u8, offset: usize, value: u32) {
    ptr::write_unaligned(p.add(offset).cast(), value);
}
unsafe fn pointer(address: usize) -> usize {
    ptr::read_volatile(address as *const usize)
}
fn emit(event: Event<'_>) {
    let value = SINK.load(Ordering::Acquire);
    if value != 0 {
        unsafe {
            ::core::mem::transmute::<usize, EventSink>(value)(event);
        }
    }
}
// A stop/failed-start callback can clear the last native handle. Keep its
// generation pinned until it has emitted its final event, even when Bluelet
// concurrently finishes closing the service on another task.
struct CallbackGuard(usize);
impl CallbackGuard {
    fn enter(index: usize) -> Self {
        STATE[index].work.fetch_add(1, Ordering::AcqRel);
        Self(index)
    }
}
impl Drop for CallbackGuard {
    fn drop(&mut self) {
        STATE[self.0].work.fetch_sub(1, Ordering::AcqRel);
        retire(self.0);
    }
}
fn session(index: usize) -> SessionId {
    SessionId(STATE[index].session.load(Ordering::Acquire))
}
fn valid(service: Service, allow_closing: bool) -> Result<&'static Slot, Error> {
    let state = STATE
        .get(service.slot as usize)
        .ok_or(Error::InvalidArgument)?;
    if state.session.load(Ordering::Acquire) != service.session.0 || service.session.0 == 0 {
        return Err(Error::Disconnected);
    }
    let closing = state.closing.load(Ordering::Acquire);
    if closing > 1 || (!allow_closing && closing != 0) {
        return Err(Error::Disconnected);
    }
    Ok(state)
}
fn pin(service: Service, allow_closing: bool) -> Result<CallbackGuard, Error> {
    let index = service.slot as usize;
    if index >= SLOTS {
        return Err(Error::InvalidArgument);
    }
    let guard = CallbackGuard::enter(index);
    valid(service, allow_closing)?;
    Ok(guard)
}
fn retire(index: usize) {
    let state = &STATE[index];
    if state.psm.load(Ordering::Acquire) != 0
        || state.scan.load(Ordering::Acquire) != 0
        || state.advertising.load(Ordering::Acquire) != 0
        || state.live.load(Ordering::Acquire) != 0
        || state.work.load(Ordering::Acquire) != 0
    {
        return;
    }
    if state
        .closing
        .compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        // A foreign task can pin an operation between the first count check
        // and this CAS. The notifying state bars new operations; recheck before
        // publishing retirement so its generation can never be reused early.
        if state.work.load(Ordering::Acquire) != 0 {
            state.closing.store(1, Ordering::Release);
            return;
        }
        emit(Event::ServiceClosed {
            session: session(index),
        });
        state.closing.store(3, Ordering::Release);
        state.session.store(0, Ordering::Release);
    }
}

/// True only for the exact build selected by the loader and audited entry bytes.
/// This is a binary/ABI check; it is not a claim of radio or hardware validation.
pub fn supported() -> bool {
    if !abi::SUPPORTED || option_env!("CANOPUS_TARGET_ID") != Some(abi::TARGET_ID) {
        return false;
    }
    abi::SIGNATURES.iter().all(|(address, bytes)| {
        bytes.iter().enumerate().all(|(offset, byte)| unsafe {
            ptr::read_volatile((*address + offset) as *const u8) == *byte
        })
    })
}
pub fn install(sink: EventSink) -> Result<(), Error> {
    if !supported() {
        return Err(Error::Unsupported);
    }
    let sink_pointer = sink as usize;
    match SINK.compare_exchange(0, sink_pointer, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => {}
        Err(existing) if existing == sink_pointer => {}
        Err(_) => return Err(Error::Busy),
    }
    // Do not open the GAP socket during Supervisor activation: Bluelet's
    // server may not be ready yet. Construction is deferred to first use.
    Ok(())
}
pub fn acquire(id: SessionId) -> Result<Service, Error> {
    if !supported() || SINK.load(Ordering::Acquire) == 0 {
        return Err(Error::Unsupported);
    }
    if id.0 == 0 {
        return Err(Error::InvalidArgument);
    }
    for (index, slot) in STATE.iter().enumerate() {
        if slot
            .session
            .compare_exchange(0, id.0, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            slot.closing.store(0, Ordering::Release);
            return Ok(Service {
                slot: index as u8,
                session: id,
            });
        }
    }
    Err(Error::Busy)
}

pub fn client_ready() -> bool {
    CLIENT.load(Ordering::Acquire) != 0
}

pub fn ensure_client() -> Result<(), Error> {
    if client_ready() {
        return Ok(());
    }
    if CLIENT_STARTING.swap(1, Ordering::AcqRel) != 0 {
        // The first caller publishes either ClientReady or ClientFailed. All
        // deferred sessions are woken by that outcome; none waits silently.
        return Ok(());
    }
    let result = (|| {
        // Firmware boot creates this process-lifetime client and its response
        // loop. Creating a second client would duplicate the IPC identity and
        // can block while starting another loop. We only add our own callback
        // registration; the stock callback table remains untouched.
        let client = unsafe { pointer(abi::D_ADAPTER_INSTANCE) };
        if client == 0 {
            return Err(Error::NotReady);
        }
        let business_loop = unsafe { pointer(client + abi::ADAPTER_LOOP) };
        if business_loop == 0 {
            return Err(Error::NotReady);
        }
        unsafe {
            (*GAP_PIPE.0.get())
                .capture_default_loop(business_loop)
                .map_err(Error::Backend)?;
        }
        if ADAPTER_REGISTERED.load(Ordering::Acquire) == 0 {
            let registration = unsafe {
                fw!(
                    abi::F_ADAPTER_REGISTER,
                    unsafe extern "C" fn(usize, *const usize) -> usize,
                    client,
                    (&ADAPTER_CALLBACKS as *const AdapterCallbacks).cast()
                )
            };
            if registration == 0 {
                return Err(Error::Busy);
            }
            ADAPTER_REGISTERED.store(registration, Ordering::Release);
        }
        match CLIENT.compare_exchange(0, client, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => {}
            Err(existing) if existing == client => {}
            Err(_) => return Err(Error::Busy),
        }
        Ok(())
    })();
    CLIENT_STARTING.store(0, Ordering::Release);
    match result {
        Ok(()) => emit(Event::ClientReady),
        Err(error) => emit(Event::ClientFailed { error }),
    }
    result
}

fn adapter() -> Result<usize, Error> {
    let adapter = CLIENT.load(Ordering::Acquire);
    if adapter == 0 {
        return Err(Error::Busy);
    }
    unsafe {
        crate::qjs::trace(adapter as u32);
        crate::qjs::trace(
            ptr::read_unaligned((adapter + abi::ADAPTER_STATE_WORD) as *const i32) as u32,
        );
    }
    let state = unsafe {
        fw!(
            abi::F_ADAPTER_GET_STATE,
            unsafe extern "C" fn(usize) -> i32,
            adapter
        )
    };
    unsafe {
        crate::qjs::trace(200 + state as u32);
    }
    if state == 0 {
        let result = unsafe {
            fw!(
                abi::F_ADAPTER_ENABLE,
                unsafe extern "C" fn(usize) -> i32,
                adapter
            )
        };
        unsafe {
            crate::qjs::trace(result as u32);
        }
        if result != 0 {
            return Err(Error::Backend(result));
        }
    }
    if state != abi::ADAPTER_READY_STATE {
        return Err(Error::NotReady);
    }
    Ok(adapter)
}

// Invoke synchronous GAP IPC only on the client's business loop. The
// response loop and foreign task-group descriptor tables cannot run it.
fn start_scan_owner(service: Service) -> Result<(), Error> {
    let _operation = pin(service, false)?;
    let state = valid(service, false)?;
    let adapter = adapter()?;
    state
        .scan
        .compare_exchange(0, STARTING, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| Error::Busy)?;
    // Stock MiWear client parameters: mode2, legacy0, active1, PHY1M0, policy0.
    let params = [2u8, 0, 1, 0, 0];
    const _: () = assert!(abi::SCAN_PARAM_SIZE == 5);
    let handle = unsafe {
        fw!(
            abi::F_SCAN_CREATE,
            unsafe extern "C" fn(usize, *const u8, *const u8, *const ScanCallbacks) -> usize,
            adapter,
            params.as_ptr(),
            ptr::null(),
            &SCAN_CALLBACKS[service.slot as usize]
        )
    };
    if handle == 0 {
        unsafe {
            crate::qjs::trace(210);
        }
        state
            .scan
            .compare_exchange(STARTING, 0, Ordering::AcqRel, Ordering::Acquire)
            .ok();
        return Err(Error::Busy);
    }
    // An early failure callback may already have cleared the handle.
    state
        .scan
        .compare_exchange(STARTING, handle, Ordering::AcqRel, Ordering::Acquire)
        .ok();
    Ok(())
}
fn stop_scan_owner(service: Service) -> Result<(), Error> {
    let _operation = pin(service, true)?;
    let state = valid(service, true)?;
    let handle = state.scan.load(Ordering::Acquire);
    if handle == 0 {
        return Ok(());
    }
    if handle == STARTING {
        return Err(Error::Busy);
    }
    if state.scan_stopping.swap(1, Ordering::AcqRel) != 0 {
        return Ok(());
    }
    let adapter = CLIENT.load(Ordering::Acquire);
    let result = unsafe {
        fw!(
            abi::F_SCAN_STOP,
            unsafe extern "C" fn(usize, usize) -> i32,
            adapter,
            handle
        )
    };
    if result != 0 {
        state.scan_stopping.store(0, Ordering::Release);
        return Err(Error::Backend(result));
    }
    Ok(())
}
fn start_advertising_owner(service: Service, data: &[u8]) -> Result<(), Error> {
    let _operation = pin(service, false)?;
    if data.len() > 31 || data.is_empty() {
        return Err(Error::InvalidArgument);
    }
    let state = valid(service, false)?;
    if state.psm.load(Ordering::Acquire) == 0 {
        return Err(Error::Busy);
    }
    let adapter = adapter()?;
    state
        .advertising
        .compare_exchange(0, STARTING, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| Error::Busy)?;
    // bt_le_adv_param_t, recovered from CLI and advertising.c consumer.
    let mut params = [0u8; abi::ADV_PARAM_SIZE];
    params[abi::ADV_PARAM_MODE] = 5; // legacy ADV_IND, connectable+scannable, no directed peer
    params[abi::ADV_PARAM_TX_POWER] = 0xf6; // tx power -10dBm, same stock payment/unlock policy
    let interval = abi::ADV_PARAM_INTERVAL;
    params[interval..interval + 4].copy_from_slice(&320u32.to_le_bytes()); // 200ms at 0.625ms units
    let handle = unsafe {
        fw!(
            abi::F_ADVERTISING_CREATE,
            unsafe extern "C" fn(
                usize,
                *const u8,
                *const u8,
                u32,
                *const u8,
                u16,
                *const AdvCallbacks,
            ) -> usize,
            adapter,
            params.as_ptr(),
            data.as_ptr(),
            data.len() as u32,
            ptr::null(),
            0,
            &ADV_CALLBACKS[service.slot as usize]
        )
    };
    if handle == 0 {
        state
            .advertising
            .compare_exchange(STARTING, 0, Ordering::AcqRel, Ordering::Acquire)
            .ok();
        return Err(Error::Busy);
    }
    state
        .advertising
        .compare_exchange(STARTING, handle, Ordering::AcqRel, Ordering::Acquire)
        .ok();
    Ok(())
}
fn stop_advertising_owner(service: Service) -> Result<(), Error> {
    let _operation = pin(service, true)?;
    let state = valid(service, true)?;
    let handle = state.advertising.load(Ordering::Acquire);
    if handle == 0 {
        return Ok(());
    }
    if handle == STARTING {
        return Err(Error::Busy);
    }
    if state.adv_stopping.swap(1, Ordering::AcqRel) != 0 {
        return Ok(());
    }
    let adapter = CLIENT.load(Ordering::Acquire);
    let result = unsafe {
        fw!(
            abi::F_ADVERTISING_STOP,
            unsafe extern "C" fn(usize, usize) -> i32,
            adapter,
            handle
        )
    };
    if result != 0 {
        state.adv_stopping.store(0, Ordering::Release);
        return Err(Error::Backend(result));
    }
    Ok(())
}

#[repr(C)]
struct ScanCallbacks {
    size: u32,
    report: extern "C" fn(usize, *const u8),
    started: extern "C" fn(usize, u8),
    stopped: extern "C" fn(usize),
}
#[repr(C)]
struct AdvCallbacks {
    reserved: u32,
    started: extern "C" fn(usize, u8, u8),
    // Event 280 carries the advertising ID, not an operation status.
    stopped: extern "C" fn(usize, u8),
}
fn scan_report(index: usize, handle: usize, report: *const u8) {
    if STATE[index].scan.load(Ordering::Acquire) != handle {
        return;
    }
    if report.is_null() || STATE[index].closing.load(Ordering::Acquire) != 0 {
        return;
    }
    unsafe {
        let len = *report.add(abi::SCAN_REPORT_LENGTH) as usize;
        if len > 31 {
            return;
        }
        let mut address = [0; 6];
        ptr::copy_nonoverlapping(
            report.add(abi::SCAN_REPORT_ADDRESS),
            address.as_mut_ptr(),
            6,
        );
        emit(Event::Advertisement {
            session: session(index),
            peer: Peer {
                address,
                address_type: *report.add(abi::SCAN_REPORT_ADDRESS_TYPE),
            },
            rssi: *report.add(abi::SCAN_REPORT_RSSI) as i8,
            packet_type: *report.add(abi::SCAN_REPORT_PACKET_TYPE),
            data: ::core::slice::from_raw_parts(report.add(abi::SCAN_REPORT_DATA), len),
        });
    }
}
fn scan_started(index: usize, handle: usize, status: u8) {
    let state = &STATE[index];
    let expected = state.scan.load(Ordering::Acquire);
    if expected != STARTING && expected != handle {
        return;
    }
    if status == 0 {
        state.scan.store(handle, Ordering::Release);
    } else {
        state.scan.store(0, Ordering::Release);
        state.scan_stopping.store(0, Ordering::Release);
    }
    emit(Event::ScanStarted {
        session: session(index),
        status,
    });
    retire(index);
}
fn scan_stopped(index: usize) {
    STATE[index].scan.store(0, Ordering::Release);
    STATE[index].scan_stopping.store(0, Ordering::Release);
    emit(Event::ScanStopped {
        session: session(index),
    });
    retire(index);
}
fn adv_started(index: usize, handle: usize, status: u8) {
    let expected = STATE[index].advertising.load(Ordering::Acquire);
    if expected != STARTING && expected != handle {
        return;
    }
    if status == 0 {
        STATE[index].advertising.store(handle, Ordering::Release);
    } else {
        STATE[index].advertising.store(0, Ordering::Release);
        STATE[index].adv_stopping.store(0, Ordering::Release);
    }
    emit(Event::AdvertisingStarted {
        session: session(index),
        status,
    });
    retire(index);
}
fn adv_stopped(index: usize) {
    STATE[index].advertising.store(0, Ordering::Release);
    STATE[index].adv_stopping.store(0, Ordering::Release);
    emit(Event::AdvertisingStopped {
        session: session(index),
        status: 0,
    });
    retire(index);
}
macro_rules! callbacks {
    ($i:expr, $report:ident, $scan_start:ident, $scan_stop:ident, $adv_start:ident, $adv_stop:ident, $coc:ident) => {
        extern "C" fn $report(h: usize, p: *const u8) {
            let _guard = CallbackGuard::enter($i);
            scan_report($i, h, p);
        }
        extern "C" fn $scan_start(h: usize, s: u8) {
            let _guard = CallbackGuard::enter($i);
            scan_started($i, h, s);
        }
        extern "C" fn $scan_stop(h: usize) {
            let _guard = CallbackGuard::enter($i);
            if STATE[$i].scan.load(Ordering::Acquire) == h {
                scan_stopped($i);
            }
        }
        extern "C" fn $adv_start(h: usize, _: u8, s: u8) {
            let _guard = CallbackGuard::enter($i);
            adv_started($i, h, s);
        }
        extern "C" fn $adv_stop(h: usize, _advertising_id: u8) {
            let _guard = CallbackGuard::enter($i);
            if STATE[$i].advertising.load(Ordering::Acquire) == h {
                adv_stopped($i);
            }
        }
        extern "C" fn $coc(e: u32, p: *mut u8) -> i32 {
            let _guard = CallbackGuard::enter($i);
            unsafe {
                coc_event($i, e, p);
            }
            0
        }
    };
}
callbacks!(
    0,
    scan_report0,
    scan_started0,
    scan_stopped0,
    adv_started0,
    adv_stopped0,
    coc0
);
callbacks!(
    1,
    scan_report1,
    scan_started1,
    scan_stopped1,
    adv_started1,
    adv_stopped1,
    coc1
);
callbacks!(
    2,
    scan_report2,
    scan_started2,
    scan_stopped2,
    adv_started2,
    adv_stopped2,
    coc2
);
callbacks!(
    3,
    scan_report3,
    scan_started3,
    scan_stopped3,
    adv_started3,
    adv_stopped3,
    coc3
);
static SCAN_CALLBACKS: [ScanCallbacks; SLOTS] = [
    ScanCallbacks {
        size: 16,
        report: scan_report0,
        started: scan_started0,
        stopped: scan_stopped0,
    },
    ScanCallbacks {
        size: 16,
        report: scan_report1,
        started: scan_started1,
        stopped: scan_stopped1,
    },
    ScanCallbacks {
        size: 16,
        report: scan_report2,
        started: scan_started2,
        stopped: scan_stopped2,
    },
    ScanCallbacks {
        size: 16,
        report: scan_report3,
        started: scan_started3,
        stopped: scan_stopped3,
    },
];
static ADV_CALLBACKS: [AdvCallbacks; SLOTS] = [
    AdvCallbacks {
        reserved: 0,
        started: adv_started0,
        stopped: adv_stopped0,
    },
    AdvCallbacks {
        reserved: 0,
        started: adv_started1,
        stopped: adv_stopped1,
    },
    AdvCallbacks {
        reserved: 0,
        started: adv_started2,
        stopped: adv_stopped2,
    },
    AdvCallbacks {
        reserved: 0,
        started: adv_started3,
        stopped: adv_stopped3,
    },
];
const COC_CALLBACKS: [extern "C" fn(u32, *mut u8) -> i32; SLOTS] = [coc0, coc1, coc2, coc3];

#[derive(Clone, Copy)]
enum Action {
    StartScan,
    StopScan,
    StartAdvertising {
        length: u8,
    },
    StopAdvertising,
    Register,
    Connect {
        link: LinkId,
        peer: Peer,
        psm: u8,
    },
    Accept {
        request: IncomingId,
        link: Option<LinkId>,
    },
    Send {
        link: LinkId,
        send: Option<SendId>,
        length: u16,
    },
    Disconnect {
        link: LinkId,
    },
    Close,
}
#[repr(C)]
struct Work {
    // Firmware uv__work: worker, done, loop, and two queue pointers.
    // libuv owns these 20 bytes until it invokes the done callback.
    #[allow(dead_code)]
    work_prefix: [u32; 5],
    service: Service,
    action: Action,
    data: [u8; RECEIVE_MTU as usize],
}
fn enqueue(service: Service, action: Action, bytes: &[u8]) -> Result<(), Error> {
    let state = valid(
        service,
        matches!(
            action,
            Action::Close | Action::Disconnect { .. } | Action::StopScan | Action::StopAdvertising
        ),
    )?;
    let owner = unsafe { pointer(abi::D_L2CAP_OWNER) };
    if owner == 0 {
        return Err(Error::Busy);
    }
    state
        .work
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < 12).then_some(n + 1)
        })
        .map_err(|_| Error::Busy)?;
    if let Err(error) = valid(
        service,
        matches!(
            action,
            Action::Close | Action::Disconnect { .. } | Action::StopScan | Action::StopAdvertising
        ),
    ) {
        state.work.fetch_sub(1, Ordering::AcqRel);
        retire(service.slot as usize);
        return Err(error);
    }
    let work = unsafe { alloc(::core::mem::size_of::<Work>()).cast::<Work>() };
    if work.is_null() {
        state.work.fetch_sub(1, Ordering::AcqRel);
        return Err(Error::Busy);
    }
    unsafe {
        ptr::write(
            work,
            Work {
                work_prefix: [0; 5],
                service,
                action,
                data: [0; RECEIVE_MTU as usize],
            },
        );
        (&mut (*work).data)[..bytes.len()].copy_from_slice(bytes);
    }
    if matches!(
        action,
        Action::StartScan
            | Action::StopScan
            | Action::StartAdvertising { .. }
            | Action::StopAdvertising
    ) {
        let client = CLIENT.load(Ordering::Acquire);
        let business_loop = if client == 0 {
            0
        } else {
            unsafe { pointer(client + abi::ADAPTER_LOOP) }
        };
        if business_loop == 0 {
            unsafe { free(work.cast()) };
            state.work.fetch_sub(1, Ordering::AcqRel);
            return Err(Error::NotReady);
        }
        // Post a ready completion to the same queue used by libuv workers.
        // Stock worker wakeups resolve integer fds in the worker's group;
        // this bridge instead signals the retained target-loop pipe directly.
        unsafe {
            (*work).work_prefix[0] = gap_prepare as *const () as usize as u32;
            (*work).work_prefix[1] = gap_done as *const () as usize as u32;
            (*work).work_prefix[2] = business_loop as u32;
            let mutex = business_loop + abi::LOOP_MUTEX;
            fw!(abi::F_UV_MUTEX_LOCK, unsafe extern "C" fn(usize), mutex);
            let head = business_loop + abi::LOOP_QUEUE_HEAD;
            let node = work as usize + 12;
            let tail = pointer(head + 4);
            ptr::write_unaligned(node as *mut usize, head);
            ptr::write_unaligned((node + 4) as *mut usize, tail);
            ptr::write_unaligned(tail as *mut usize, node);
            ptr::write_unaligned((head + 4) as *mut usize, node);
            let result = (*GAP_PIPE.0.get()).signal((business_loop + abi::LOOP_ASYNC) as *mut u32);
            if result != 0 {
                // The loop cannot detach the newly inserted node until this
                // mutex is released. Undo submission before reporting failure.
                ptr::write_unaligned(tail as *mut usize, head);
                ptr::write_unaligned((head + 4) as *mut usize, tail);
            }
            fw!(abi::F_UV_MUTEX_UNLOCK, unsafe extern "C" fn(usize), mutex);
            if result != 0 {
                free(work.cast());
                state.work.fetch_sub(1, Ordering::AcqRel);
                return Err(Error::Backend(result));
            }
        }
        return Ok(());
    }
    // CoC requests use Bluelet's owner.
    // The return value is a semaphore operation, not an acceptance status.
    // Failure/cancellation goes through cancel_work, including owner shutdown.
    unsafe {
        fw!(
            abi::F_BT_QUEUE_EXTERNAL,
            unsafe extern "C" fn(
                usize,
                extern "C" fn(usize, u32, *mut Work) -> i32,
                extern "C" fn(usize, u32, *mut Work) -> i32,
                *mut Work,
                u8,
            ) -> i32,
            owner,
            run_work,
            cancel_work,
            work,
            1
        );
    }
    Ok(())
}
pub fn start_scan(service: Service) -> Result<(), Error> {
    enqueue(service, Action::StartScan, &[])
}
pub fn stop_scan(service: Service) -> Result<(), Error> {
    let state = valid(service, true)?;
    if state.scan.load(Ordering::Acquire) == 0 {
        return Ok(());
    }
    enqueue(service, Action::StopScan, &[])
}
pub fn start_advertising(service: Service, data: &[u8]) -> Result<(), Error> {
    if data.len() > 31 || data.is_empty() {
        return Err(Error::InvalidArgument);
    }
    enqueue(
        service,
        Action::StartAdvertising {
            length: data.len() as u8,
        },
        data,
    )
}
pub fn stop_advertising(service: Service) -> Result<(), Error> {
    let state = valid(service, true)?;
    if state.advertising.load(Ordering::Acquire) == 0 {
        return Ok(());
    }
    enqueue(service, Action::StopAdvertising, &[])
}
pub fn register(service: Service) -> Result<(), Error> {
    enqueue(service, Action::Register, &[])
}
pub fn connect(service: Service, link: LinkId, peer: Peer, spsm: u8) -> Result<(), Error> {
    if spsm < abi::COC_PSM_MIN as u8 || peer.address_type > 3 {
        return Err(Error::InvalidArgument);
    }
    enqueue(
        service,
        Action::Connect {
            link,
            peer,
            psm: spsm,
        },
        &[],
    )
}
pub fn accept(service: Service, request: IncomingId, link: LinkId) -> Result<(), Error> {
    enqueue(
        service,
        Action::Accept {
            request,
            link: Some(link),
        },
        &[],
    )
}
pub fn reject(service: Service, request: IncomingId) -> Result<(), Error> {
    enqueue(
        service,
        Action::Accept {
            request,
            link: None,
        },
        &[],
    )
}
pub fn send(
    service: Service,
    link: LinkId,
    token: Option<SendId>,
    bytes: &[u8],
) -> Result<(), Error> {
    if bytes.len() > RECEIVE_MTU as usize {
        return Err(Error::InvalidArgument);
    }
    enqueue(
        service,
        Action::Send {
            link,
            send: token,
            length: bytes.len() as u16,
        },
        bytes,
    )
}
pub fn disconnect(service: Service, link: LinkId) -> Result<(), Error> {
    if matches!(valid(service, true), Err(Error::Disconnected)) {
        emit(Event::Closed {
            link,
            cid: 0,
            reason: 0,
        });
        return Ok(());
    }
    enqueue(service, Action::Disconnect { link }, &[])
}
pub fn close(service: Service) -> Result<(), Error> {
    let _operation = pin(service, true)?;
    let state = valid(service, true)?;
    state.closing.store(1, Ordering::Release);
    // Initiate all cleanup even if one IPC request fails; its handle stays owned
    // and a subsequent close() can retry it. No premature slot reuse.
    let scan = stop_scan(service);
    let advertising = stop_advertising(service);
    enqueue(service, Action::Close, &[])?;
    scan.and(advertising)
}
extern "C" fn gap_prepare(_: *mut Work) {}
extern "C" fn gap_done(work: *mut Work, status: i32) {
    if work.is_null() {
        return;
    }
    unsafe {
        let service = (*work).service;
        let stopping = matches!((*work).action, Action::StopScan | Action::StopAdvertising);
        let result = if status != 0 {
            Err(Error::Backend(status))
        } else if let Err(error) = valid(service, stopping) {
            Err(error)
        } else {
            perform(pointer(abi::D_L2CAP_OWNER), &*work)
        };
        if let Err(error) = result {
            emit(Event::Failed {
                session: service.session,
                link: None,
                operation: failure_operation((*work).action),
                error,
            });
        }
        let index = service.slot as usize;
        free(work.cast());
        STATE[index].work.fetch_sub(1, Ordering::AcqRel);
        retire(index);
    }
}
extern "C" fn cancel_work(_: usize, reason: u32, work: *mut Work) -> i32 {
    if work.is_null() {
        return 0;
    }
    unsafe {
        let service = (*work).service;
        let link = action_link((*work).action);
        let operation = failure_operation((*work).action);
        if let Action::Disconnect { link } = (*work).action {
            emit(Event::Closed {
                link,
                cid: 0,
                reason: reason as u16,
            });
        }
        free(work.cast());
        emit(Event::Failed {
            session: service.session,
            link,
            operation,
            error: Error::Backend(reason as i32),
        });
        STATE[service.slot as usize]
            .work
            .fetch_sub(1, Ordering::AcqRel);
        retire(service.slot as usize);
    }
    0
}
fn failure_operation(action: Action) -> u8 {
    match action {
        Action::StartScan => 1,
        Action::StopScan => 2,
        Action::StartAdvertising { .. } => 3,
        Action::StopAdvertising => 4,
        _ => 0,
    }
}
fn action_link(action: Action) -> Option<LinkId> {
    match action {
        Action::Connect { link, .. } | Action::Send { link, .. } | Action::Disconnect { link } => {
            Some(link)
        }
        Action::Accept { link, .. } => link,
        _ => None,
    }
}
extern "C" fn run_work(owner: usize, _: u32, work: *mut Work) -> i32 {
    if work.is_null() {
        return 0;
    }
    unsafe {
        let service = (*work).service;
        let index = service.slot as usize;
        let result = if STATE[index].closing.load(Ordering::Acquire) != 0
            && !matches!((*work).action, Action::Close | Action::Disconnect { .. })
        {
            Err(Error::Disconnected)
        } else if matches!((*work).action, Action::Send { .. }) {
            match defer_send(work) {
                Ok(true) => return 0,
                Ok(false) => perform(owner, &*work),
                Err(error) => Err(error),
            }
        } else {
            perform(owner, &*work)
        };
        if let Err(error) = result {
            emit(Event::Failed {
                session: service.session,
                link: action_link((*work).action),
                operation: failure_operation((*work).action),
                error,
            });
        }
        free(work.cast());
        STATE[index].work.fetch_sub(1, Ordering::AcqRel);
        retire(index);
    }
    0
}
// This state is inspected only on the Bluelet owner task. Never let module
// submissions grow the stock's otherwise unbounded credit-fragment queue.
unsafe fn defer_send(work: *mut Work) -> Result<bool, Error> {
    let Action::Send { link, .. } = (*work).action else {
        return Ok(false);
    };
    let state = &STATE[(*work).service.slot as usize];
    let channel = (*state.owner.get())
        .channels
        .iter_mut()
        .find(|c| c.link == Some(link) && c.opened)
        .ok_or(Error::Disconnected)?;
    if !channel.pending.is_null() {
        return Err(Error::Busy);
    }
    let host = fw!(
        abi::F_L2CAP_FIND_CHANNEL,
        unsafe extern "C" fn(usize, u16) -> usize,
        0,
        channel.cid
    );
    if host == 0 {
        return Err(Error::Disconnected);
    }
    let queue = ptr::read_unaligned((host + abi::COC_CHANNEL_QUEUE) as *const usize);
    if queue == 0 {
        return Err(Error::Disconnected);
    }
    let head = ptr::read_unaligned(queue as *const usize);
    let credits = ptr::read_unaligned((queue + abi::COC_CREDIT_QUEUE_CREDITS) as *const u16);
    if head != 0 || credits == 0 {
        channel.pending = work;
        return Ok(true);
    }
    Ok(false)
}
unsafe fn resume_send(index: usize, cid: u16) {
    let pending = if let Some(channel) = (*STATE[index].owner.get())
        .channels
        .iter_mut()
        .find(|c| c.cid == cid)
    {
        let pending = channel.pending;
        channel.pending = ptr::null_mut();
        pending
    } else {
        ptr::null_mut()
    };
    if !pending.is_null() {
        run_work(pointer(abi::D_L2CAP_OWNER), 1, pending);
    }
}
unsafe fn cancel_pending(work: *mut Work) {
    if work.is_null() {
        return;
    }
    let service = (*work).service;
    emit(Event::Failed {
        session: service.session,
        link: action_link((*work).action),
        operation: 0,
        error: Error::Disconnected,
    });
    free(work.cast());
    STATE[service.slot as usize]
        .work
        .fetch_sub(1, Ordering::AcqRel);
}
unsafe fn config(request: *mut u8) {
    put_word(request, abi::COC_CONFIG_MTU, RECEIVE_MTU);
    put_word(request, abi::COC_CONFIG_CREDITS, INITIAL_CREDITS);
    put_word(request, abi::COC_CONFIG_MPS, RECEIVE_MPS);
}
unsafe fn request() -> Result<*mut u8, Error> {
    let p = alloc(abi::COC_REQUEST_SIZE);
    if p.is_null() {
        Err(Error::Busy)
    } else {
        Ok(p)
    }
}
unsafe fn disconnect_cid(_owner: usize, cid: u16) -> Result<(), Error> {
    // Exactly the policy called by C872EBC after its channel lookup. Invoke it
    // on the same owner without allocating a throwaway four-byte IPC request,
    // so module heap pressure cannot prevent a queued disconnect from running.
    let channel = fw!(
        abi::F_L2CAP_FIND_CHANNEL,
        unsafe extern "C" fn(usize, u16) -> usize,
        0,
        cid
    );
    if channel != 0 {
        fw!(
            abi::F_L2CAP_DISCONNECT_CID,
            unsafe extern "C" fn(usize, u16),
            channel,
            abi::COC_DISCONNECT_POLICY
        );
    }
    Ok(())
}
unsafe fn perform(owner: usize, work: &Work) -> Result<(), Error> {
    let index = work.service.slot as usize;
    let state = &STATE[index];
    match work.action {
        Action::StartScan => start_scan_owner(work.service)?,
        Action::StopScan => stop_scan_owner(work.service)?,
        Action::StartAdvertising { length } => {
            start_advertising_owner(work.service, &work.data[..length as usize])?
        }
        Action::StopAdvertising => stop_advertising_owner(work.service)?,
        Action::Register => {
            if state.psm.load(Ordering::Acquire) != 0 {
                return Err(Error::Busy);
            }
            // Stock's dynamic allocator wraps forever when the registry is
            // full. Preflight on this same owner task bounds that search.
            let mut selected = None;
            for psm in abi::COC_PSM_MIN..=abi::COC_PSM_MAX {
                if fw!(
                    abi::F_L2CAP_FIND_LE_PSM,
                    unsafe extern "C" fn(u16, u8) -> usize,
                    psm,
                    abi::COC_PSM_TRANSPORT
                ) == 0
                {
                    selected = Some(psm);
                    break;
                }
            }
            let selected = selected.ok_or(Error::Busy)?;
            // Registry nodes have an eight-byte list header before the payload.
            let allocation = alloc(abi::COC_REGISTER_NODE_SIZE);
            if allocation.is_null() {
                return Err(Error::Busy);
            }
            let p = allocation.add(abi::COC_REGISTER_NODE_PAYLOAD);
            put_dword(
                p,
                abi::COC_REGISTER_NODE_CALLBACK,
                COC_CALLBACKS[index] as usize as u32,
            );
            put_word(p, abi::COC_REGISTER_NODE_PSM, selected);
            *p.add(abi::COC_REGISTER_NODE_TRANSPORT) = abi::COC_PSM_TRANSPORT;
            fw!(
                abi::F_L2CAP_REGISTER_PSM,
                unsafe extern "C" fn(usize, u32, *mut u8) -> i32,
                owner,
                abi::COC_QUEUE_EVENT,
                p
            );
        }
        Action::Connect { link, peer, psm } => {
            if !(*state.owner.get()).channels.iter().any(|c| c.cid == 0) {
                return Err(Error::Busy);
            }
            // The stock allocator loops over the shared LE CID range until
            // something is free; exhaustion otherwise spins the owner forever.
            // No other owner work can consume a CID between this and connect.
            let mut cid_available = false;
            for cid in abi::COC_CID_MIN..=abi::COC_CID_MAX {
                if fw!(
                    abi::F_L2CAP_FIND_CHANNEL,
                    unsafe extern "C" fn(usize, u16) -> usize,
                    0,
                    cid
                ) == 0
                {
                    cid_available = true;
                    break;
                }
            }
            if !cid_available {
                return Err(Error::Busy);
            }
            let p = request()?;
            put_word(p, abi::COC_REQUEST_PSM, psm as u16);
            *p.add(abi::COC_REQUEST_FLAGS) = 1;
            put_dword(
                p,
                abi::COC_REQUEST_CALLBACK,
                COC_CALLBACKS[index] as usize as u32,
            );
            ptr::copy_nonoverlapping(peer.address.as_ptr(), p.add(abi::COC_REQUEST_ADDRESS), 6);
            *p.add(abi::COC_REQUEST_ADDRESS_TYPE) = peer.address_type;
            config(p);
            (*state.owner.get()).connecting = Some(link);
            // This owner handler emits event2 synchronously, binding the newly
            // allocated CID to `link` before any ready/data event can occur.
            fw!(
                abi::F_L2CAP_CONNECT,
                unsafe extern "C" fn(usize, u32, *mut u8) -> i32,
                owner,
                abi::COC_QUEUE_EVENT,
                p
            );
            (*state.owner.get()).connecting = None;
            // close() can arrive on another task while the owner handler is
            // running. Its event2 callback must not destroy a channel that the
            // handler still uses after that callback returns.
            if state.closing.load(Ordering::Acquire) != 0 {
                if let Some(cid) = (*state.owner.get())
                    .channels
                    .iter()
                    .find(|channel| channel.link == Some(link))
                    .map(|channel| channel.cid)
                {
                    disconnect_cid(owner, cid)?;
                }
            }
        }
        Action::Accept {
            request: token,
            link,
        } => {
            let channel = (*state.owner.get())
                .channels
                .iter_mut()
                .find(|c| c.cid != 0 && c.request == token.0)
                .ok_or(Error::Disconnected)?;
            // Bind before the fallible allocation: Failed(link) must leave a
            // discoverable native channel for the runtime's disconnect(link).
            channel.link = link;
            let p = match request() {
                Ok(request) => request,
                Err(_) if link.is_none() => {
                    // A rejected incoming channel has no LinkId for the runtime
                    // to clean up. Disconnect its CID on this owner task instead
                    // of emitting a session-level Failed(None).
                    let cid = channel.cid;
                    channel.request = 0;
                    disconnect_cid(owner, cid)?;
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
            put_word(p, abi::COC_REQUEST_CID, channel.cid);
            put_word(
                p,
                abi::COC_REQUEST_STATUS,
                if link.is_some() { 0 } else { 4 },
            );
            channel.request = 0;
            config(p);
            fw!(
                abi::F_L2CAP_ACCEPT,
                unsafe extern "C" fn(usize, u32, *mut u8) -> i32,
                owner,
                abi::COC_QUEUE_EVENT,
                p
            );
        }
        Action::Disconnect { link } => {
            let cid = (*state.owner.get())
                .channels
                .iter()
                .find(|c| c.link == Some(link))
                .map(|c| c.cid);
            if let Some(cid) = cid {
                disconnect_cid(owner, cid)?;
            } else {
                // A connect may have been canceled before event2 allocated a
                // CID. Complete teardown for its generation even in that case.
                emit(Event::Closed {
                    link,
                    cid: 0,
                    reason: 0,
                });
            }
        }
        Action::Send { link, send, length } => {
            let channel = (*state.owner.get())
                .channels
                .iter()
                .find(|c| c.link == Some(link) && c.opened)
                .copied()
                .ok_or(Error::Disconnected)?;
            if length > channel.remote_mtu {
                return Err(Error::InvalidArgument);
            }
            let channel_pointer = fw!(
                abi::F_L2CAP_FIND_CHANNEL,
                unsafe extern "C" fn(usize, u16) -> usize,
                0,
                channel.cid
            );
            if channel_pointer == 0 {
                return Err(Error::Disconnected);
            }
            if abi::queue_send(
                owner,
                channel_pointer,
                channel.cid,
                work.data.as_ptr(),
                length,
            ) < 0
            {
                return Err(Error::Busy);
            }
            emit(Event::Submitted { link, send });
        }
        Action::Close => {
            let psm = state.psm.swap(0, Ordering::AcqRel);
            if psm != 0 {
                let p = alloc(8);
                if p.is_null() {
                    state.psm.store(psm, Ordering::Release);
                    return Err(Error::Busy);
                }
                put_word(p, abi::COC_REGISTER_NODE_PSM, psm);
                *p.add(abi::COC_REGISTER_NODE_TRANSPORT) = abi::COC_PSM_TRANSPORT;
                fw!(
                    abi::F_L2CAP_RELEASE_PSM,
                    unsafe extern "C" fn(usize, u32, *mut u8) -> i32,
                    owner,
                    abi::COC_QUEUE_EVENT,
                    p
                );
            }
            // Copy the IDs: callbacks can synchronously clear channel entries.
            let mut cids = [0; CHANNELS];
            for (to, c) in cids.iter_mut().zip((*state.owner.get()).channels.iter()) {
                *to = c.cid;
            }
            for cid in cids {
                if cid != 0 {
                    disconnect_cid(owner, cid)?;
                }
            }
        }
    }
    Ok(())
}
unsafe fn coc_event(index: usize, event: u32, p: *mut u8) {
    if p.is_null() {
        return;
    }
    let state = &STATE[index];
    let owner = pointer(abi::D_L2CAP_OWNER);
    match event {
        abi::COC_EVENT_PSM_REGISTERED => {
            let psm = word(p, abi::COC_EV_PSM_CID);
            state.psm.store(psm, Ordering::Release);
            emit(Event::Registered {
                session: session(index),
                spsm: psm as u8,
            });
        }
        abi::COC_EVENT_INCOMING | abi::COC_EVENT_ASSIGNED => {
            let cid = word(p, abi::COC_EV_REQUEST_CID);
            let vacant = (*state.owner.get())
                .channels
                .iter()
                .position(|c| c.cid == 0);
            let mut address = [0; 6];
            ptr::copy_nonoverlapping(p.add(abi::COC_EV_REQUEST_ADDRESS), address.as_mut_ptr(), 6);
            let peer = Peer {
                address,
                address_type: *p.add(abi::COC_EV_REQUEST_ADDRESS_TYPE),
            };
            let link = if event == abi::COC_EVENT_ASSIGNED {
                (*state.owner.get()).connecting
            } else {
                None
            };
            if let Some(position) = vacant {
                let token = NEXT_REQUEST.fetch_add(1, Ordering::Relaxed).max(1);
                (*state.owner.get()).channels[position] = Channel {
                    cid,
                    request: if event == abi::COC_EVENT_INCOMING {
                        token
                    } else {
                        0
                    },
                    link,
                    opened: false,
                    remote_mtu: 0,
                    pending: ptr::null_mut(),
                };
                state.live.fetch_add(1, Ordering::AcqRel);
                if event == abi::COC_EVENT_INCOMING && state.closing.load(Ordering::Acquire) == 0 {
                    emit(Event::Incoming {
                        session: session(index),
                        request: IncomingId(token),
                        peer,
                        cid,
                        spsm: word(p, abi::COC_EV_REQUEST_PSM) as u8,
                    });
                } else if let Some(link) = link {
                    emit(Event::Assigned { link, cid });
                }
            }
            if (vacant.is_none() || state.closing.load(Ordering::Acquire) != 0)
                && event == abi::COC_EVENT_INCOMING
            {
                put_word(p, abi::COC_REQUEST_STATUS, 4);
                config(p);
                fw!(
                    abi::F_L2CAP_ACCEPT,
                    unsafe extern "C" fn(usize, u32, *mut u8) -> i32,
                    owner,
                    abi::COC_QUEUE_EVENT,
                    p
                );
                retire(index);
                return;
            }
            // event2 is a synchronous notification inside C871558. Leave
            // its live channel intact until the owner handler returns;
            // perform(Connect) handles a concurrent close immediately then.
        }
        abi::COC_EVENT_OPEN => {
            let cid = word(p, abi::COC_EV_COMPLETE_CID);
            if let Some(channel) = (*state.owner.get())
                .channels
                .iter_mut()
                .find(|c| c.cid == cid)
            {
                channel.opened = true;
                channel.remote_mtu = word(p, abi::COC_EV_COMPLETE_REMOTE_MTU);
                if let Some(link) = channel.link {
                    emit(Event::Opened {
                        link,
                        cid,
                        remote_receive_mtu: channel.remote_mtu,
                    });
                }
            }
            if state.closing.load(Ordering::Acquire) != 0 {
                let _ = disconnect_cid(owner, cid);
            }
        }
        abi::COC_EVENT_CLOSED => {
            let cid = word(p, abi::COC_EV_CLOSE_CID);
            let reason = word(p, abi::COC_EV_CLOSE_REASON);
            if let Some(position) = (*state.owner.get())
                .channels
                .iter()
                .position(|c| c.cid == cid)
            {
                let channel = (*state.owner.get()).channels[position];
                (*state.owner.get()).channels[position] = Channel::EMPTY;
                state.live.fetch_sub(1, Ordering::AcqRel);
                cancel_pending(channel.pending);
                if let Some(link) = channel.link {
                    emit(Event::Closed { link, cid, reason });
                }
            }
        }
        abi::COC_EVENT_DATA | abi::COC_EVENT_CREDITS => {
            let cid = word(
                p,
                if event == abi::COC_EVENT_DATA {
                    abi::COC_EV_DATA_CID
                } else {
                    abi::COC_EV_CREDIT_CID
                },
            );
            if let Some(channel) = (*state.owner.get())
                .channels
                .iter()
                .find(|c| c.cid == cid)
                .copied()
            {
                if let Some(link) = channel.link {
                    if event == abi::COC_EVENT_CREDITS {
                        emit(Event::Credits {
                            link,
                            cid,
                            added: word(p, abi::COC_EV_CREDIT_ADDED),
                        });
                    } else {
                        let end = word(p, abi::COC_EV_DATA_END);
                        let offset = word(p, abi::COC_EV_DATA_OFFSET);
                        if end < offset || end - offset > RECEIVE_MTU {
                            emit(Event::Failed {
                                session: session(index),
                                link: Some(link),
                                operation: 0,
                                error: Error::ReceiveOverflow,
                            });
                            let _ = disconnect_cid(owner, cid);
                        } else {
                            emit(Event::Data {
                                link,
                                cid,
                                data: ::core::slice::from_raw_parts(
                                    p.add(abi::COC_EV_DATA_BASE + offset as usize),
                                    (end - offset) as usize,
                                ),
                            });
                        }
                    }
                }
            }
        }
        abi::COC_EVENT_LOCAL_FLOW => {
            // C86DA74 emits a separately allocated six-byte local flow report.
            // Legacy LE submission is governed by event17 credits; this event
            // changes neither connection state nor responsibility for an SDU.
        }
        _ => { /* no event4/5 producer exists in this legacy LE channel path */ }
    }
    if event == abi::COC_EVENT_CREDITS {
        resume_send(index, word(p, abi::COC_EV_CREDIT_CID));
    }
    free(p);
    retire(index);
}

const _: () = {
    assert!(::core::mem::size_of::<ScanCallbacks>() as u32 == abi::SCAN_CALLBACKS_SIZE);
    assert!(::core::mem::size_of::<AdvCallbacks>() == abi::ADV_CALLBACKS_SIZE);
};
