//! Ferry Application factory/init registration on p67.
//! Keep the stock FrameworkExt and ApplicationExt behavior, replacing only
//! their manager-registration checkpoint with this module's resident callbacks.
use super::*;
use canopus_abi::{CanopusContext, HookType};
use canopus_runtime::hook::Hook;

// Both tables retain the two Itanium ABI prefix words. Only one primary
// dispatch slot changes in each table; secondary Application vptrs stay stock.
const FRAMEWORK_VTABLE: usize = abi::D_FRAMEWORK_VTABLE;
const APPLICATION_VTABLE: usize = abi::D_APPLICATION_VTABLE;
static FRAMEWORK_DISPATCH: Shared<[usize; abi::FRAMEWORK_COPY_WORDS]> =
    Shared(UnsafeCell::new([0; abi::FRAMEWORK_COPY_WORDS]));
static APPLICATION_DISPATCH: Shared<[usize; abi::APPLICATION_COPY_WORDS]> =
    Shared(UnsafeCell::new([0; abi::APPLICATION_COPY_WORDS]));
/// Install the synchronous ferry Application factory/init dispatcher on p67.
/// The framework must exist and no QuickApp launch may overlap activation.
/// The module remains resident after this succeeds.
///
/// # Safety
/// `ctx` must be a valid Supervisor context for this exact target. Call from
/// serialized module activation while QuickApps are closed. The adapter and
/// this module's code/data must remain live until reboot. No firmware code is
/// modified: the Supervisor pointer hook replaces a heap object's RAM vptr.
pub unsafe fn install(ctx: *const CanopusContext, adapter: &'static Adapter) -> Result<(), i32> {
    if ctx.is_null() {
        return Err(-22);
    }
    if (*STATE.0.get()).hook_id >= 0 {
        return Ok(());
    }
    let framework = ptr::read_volatile(abi::D_FRAMEWORK as *const usize);
    if !ram_object(framework) {
        return Err(-16);
    }
    let implementation =
        ptr::read_unaligned((framework + abi::FRAMEWORK_IMPLEMENTATION) as *const usize);
    if !ram_object(implementation) {
        return Err(-16);
    }
    let current = ptr::read_volatile(framework as *const usize);
    // A distinct extension can own the same RAM dispatcher. Report BUSY rather
    // than replace a table whose full layout/ownership has not been audited.
    if current != FRAMEWORK_VTABLE {
        return Err(-16);
    }
    let rc = fw!(
        abi::F_UV_MUTEX_INIT,
        unsafe extern "C" fn(*mut c_void) -> i32
    )(MUTEX.0.get().cast());
    if rc != 0 {
        return Err(rc);
    }
    let prefix_bytes = abi::VTABLE_PREFIX_WORDS * ::core::mem::size_of::<usize>();
    ptr::copy_nonoverlapping(
        (FRAMEWORK_VTABLE - prefix_bytes) as *const usize,
        FRAMEWORK_DISPATCH.0.get().cast::<usize>(),
        abi::FRAMEWORK_COPY_WORDS,
    );
    ptr::copy_nonoverlapping(
        (APPLICATION_VTABLE - prefix_bytes) as *const usize,
        APPLICATION_DISPATCH.0.get().cast::<usize>(),
        abi::APPLICATION_COPY_WORDS,
    );
    (*FRAMEWORK_DISPATCH.0.get())[abi::VTABLE_PREFIX_WORDS + abi::FRAMEWORK_FACTORY_SLOT] =
        create_application as *const () as usize;
    (*APPLICATION_DISPATCH.0.get())[abi::VTABLE_PREFIX_WORDS + abi::APPLICATION_INIT_SLOT] =
        initialize_application as *const () as usize;
    (*STATE.0.get()).adapter = Some(adapter);
    // The real framework's recursive render/list mutex guards its app vector.
    let framework_mutex = (implementation + abi::FRAMEWORK_MUTEX) as *mut c_void;
    fw!(abi::F_UV_MUTEX_LOCK, unsafe extern "C" fn(*mut c_void))(framework_mutex);
    let first = ptr::read_unaligned((implementation + abi::FRAMEWORK_APPS_BEGIN) as *const usize);
    let end = ptr::read_unaligned((implementation + abi::FRAMEWORK_APPS_END) as *const usize);
    if first != end {
        fw!(abi::F_UV_MUTEX_UNLOCK, unsafe extern "C" fn(*mut c_void))(framework_mutex);
        (*STATE.0.get()).adapter = None;
        return Err(-16);
    }
    READY.store(true, Ordering::Release);
    let result = Hook::install(
        ctx,
        framework,
        FRAMEWORK_DISPATCH
            .0
            .get()
            .cast::<usize>()
            .add(abi::VTABLE_PREFIX_WORDS) as usize,
        &mut (*STATE.0.get()).original_dispatch,
        HookType::Pointer,
    );
    fw!(abi::F_UV_MUTEX_UNLOCK, unsafe extern "C" fn(*mut c_void))(framework_mutex);
    match result {
        Ok(id) => {
            (*STATE.0.get()).hook_id = id;
            Ok(())
        }
        Err(error) => {
            READY.store(false, Ordering::Release);
            (*STATE.0.get()).adapter = None;
            Err(error)
        }
    }
}

fn ram_object(address: usize) -> bool {
    (0x20000000..0x40000000).contains(&address) && address & 3 == 0
}

unsafe extern "C" fn create_application(framework: usize, uri: *const c_char) -> usize {
    trace(10);
    // The stock factory returns a constructed Application and has not yet
    // started its JS task. Replace its own primary dispatch pointer now.
    let original = ptr::read_unaligned(
        (FRAMEWORK_VTABLE + abi::FRAMEWORK_FACTORY_SLOT * ::core::mem::size_of::<usize>())
            as *const usize,
    );
    let application = fw!(
        original,
        unsafe extern "C" fn(usize, *const c_char) -> usize
    )(framework, uri);
    if application != 0 && ptr::read_volatile(application as *const usize) == APPLICATION_VTABLE {
        trace(11);
        ptr::write_volatile(
            application as *mut usize,
            APPLICATION_DISPATCH.0.get().cast::<usize>().add(2) as usize,
        );
    }
    application
}

unsafe extern "C" fn initialize_application(
    application: usize,
    options: usize,
    loop_pointer: usize,
) -> i32 {
    let original = ptr::read_unaligned(
        (APPLICATION_VTABLE + abi::APPLICATION_INIT_SLOT * ::core::mem::size_of::<usize>())
            as *const usize,
    );
    let result = fw!(original, unsafe extern "C" fn(usize, usize, usize) -> i32)(
        application,
        options,
        loop_pointer,
    );
    trace(20);
    // Runs synchronously on this Application's JS task between its stock
    // initialize and run calls. Registry mutation cannot race require/eval.
    let runtime = ptr::read_unaligned((application + abi::APPLICATION_RUNTIME) as *const usize);
    if runtime == 0 {
        return result;
    }
    let manager = ptr::read_unaligned((runtime + abi::RUNTIME_MANAGER) as *const usize);
    if manager == 0 {
        return result;
    }
    let registry = ptr::read_unaligned((manager + abi::MANAGER_REGISTRY) as *const usize);
    if registry == 0 {
        return result;
    }
    trace(21);
    let registered = fw!(
        abi::F_FEATURE_REGISTER,
        unsafe extern "C" fn(usize, *const Description) -> i32
    )(registry, &DESCRIPTION);
    if registered != 1 {
        trace(97);
        trace(registered as u32);
    }
    result
}
