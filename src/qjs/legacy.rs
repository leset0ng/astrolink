//! Legacy `@aiot/core` extension registration and Application teardown.
//!
//! The legacy host has no Feature descriptor registry. Its per-application
//! extension list supplies the JS owner task. The initializer wraps that
//! application's destruction checkpoint before publishing the primitive.
use super::js::{clear_exception, free, object, Context, Shared};
use super::*;
use crate::abi;
use canopus_abi::CanopusContext;
use core::{cell::UnsafeCell, ffi::c_void, mem, ptr, sync::atomic::Ordering};

macro_rules! fw {
    ($address:expr, $ty:ty) => {
        mem::transmute::<usize, $ty>($address | 1)
    };
}

const FRAMEWORK_VTABLE: usize = abi::D_FRAMEWORK_VTABLE;
const APPLICATION_VTABLE: usize = abi::D_APPLICATION_VTABLE;
static APPLICATION_DISPATCH: Shared<[usize; abi::APPLICATION_COPY_WORDS]> =
    Shared(UnsafeCell::new([0; abi::APPLICATION_COPY_WORDS]));

#[repr(C)]
struct ExtensionNode {
    previous: usize,
    next: usize,
    initialize: unsafe extern "C" fn(Context, usize),
    export: unsafe extern "C" fn(Context, usize),
    name: [u8; 32],
}
unsafe impl Sync for ExtensionNode {}

const fn extension_name() -> [u8; 32] {
    let name = b"canopus.astrolink";
    let mut result = [0; 32];
    let mut index = 0;
    while index < name.len() {
        result[index] = name[index];
        index += 1;
    }
    result
}

static EXTENSION: Shared<ExtensionNode> = Shared(UnsafeCell::new(ExtensionNode {
    previous: 0,
    next: 0,
    initialize: initialize_extension,
    export: export_extension,
    name: extension_name(),
}));

static INSTALLED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Install the legacy extension and Application-destruction dispatch.
///
/// # Safety
/// Called by serialized module activation with all QuickApps closed. The
/// extension and vtable copies stay resident until reboot.
pub unsafe fn install(ctx: *const CanopusContext, adapter: &'static Adapter) -> Result<(), i32> {
    if ctx.is_null() {
        return Err(-22);
    }
    if INSTALLED.load(Ordering::Acquire) {
        crate::activation_detail(100);
        return Ok(());
    }
    crate::activation_detail(101);
    let framework = ptr::read_volatile(abi::D_FRAMEWORK as *const usize);
    if !ram_object(framework) {
        return Err(-1101);
    }
    let implementation =
        ptr::read_unaligned((framework + abi::FRAMEWORK_IMPLEMENTATION) as *const usize);
    if !ram_object(implementation) {
        return Err(-1102);
    }
    if ptr::read_volatile(framework as *const usize) != FRAMEWORK_VTABLE {
        return Err(-1103);
    }
    let rc = fw!(
        abi::F_UV_MUTEX_INIT,
        unsafe extern "C" fn(*mut c_void) -> i32
    )(MUTEX.0.get().cast());
    if rc != 0 {
        return Err(rc);
    }
    let prefix_bytes = abi::VTABLE_PREFIX_WORDS * mem::size_of::<usize>();
    ptr::copy_nonoverlapping(
        (APPLICATION_VTABLE - prefix_bytes) as *const usize,
        APPLICATION_DISPATCH.0.get().cast::<usize>(),
        abi::APPLICATION_COPY_WORDS,
    );
    (*APPLICATION_DISPATCH.0.get())
        [abi::VTABLE_PREFIX_WORDS + abi::LEGACY_APPLICATION_DESTROY_SLOT] =
        destroy_application as *const () as usize;
    (*STATE.0.get()).adapter = Some(adapter);

    let framework_mutex = (implementation + abi::FRAMEWORK_MUTEX) as *mut c_void;
    crate::activation_detail(102);
    fw!(abi::F_UV_MUTEX_LOCK, unsafe extern "C" fn(*mut c_void))(framework_mutex);
    let first = ptr::read_unaligned((implementation + abi::FRAMEWORK_APPS_BEGIN) as *const usize);
    let end = ptr::read_unaligned((implementation + abi::FRAMEWORK_APPS_END) as *const usize);
    if first != end {
        fw!(abi::F_UV_MUTEX_UNLOCK, unsafe extern "C" fn(*mut c_void))(framework_mutex);
        (*STATE.0.get()).adapter = None;
        return Err(-1104);
    }
    let sentinel = abi::LEGACY_EXTENSION_LIST;
    crate::activation_detail(103);
    let first_extension = ptr::read_unaligned((sentinel + mem::size_of::<usize>()) as *const usize);
    if first_extension != 0
        && (!ram_object(first_extension)
            || ptr::read_unaligned(first_extension as *const usize) != sentinel)
    {
        fw!(abi::F_UV_MUTEX_UNLOCK, unsafe extern "C" fn(*mut c_void))(framework_mutex);
        (*STATE.0.get()).adapter = None;
        return Err(-1105);
    }
    READY.store(true, Ordering::Release);
    crate::activation_detail(104);
    append_extension();
    INSTALLED.store(true, Ordering::Release);
    crate::activation_detail(105);
    fw!(abi::F_UV_MUTEX_UNLOCK, unsafe extern "C" fn(*mut c_void))(framework_mutex);
    Ok(())
}

fn ram_object(address: usize) -> bool {
    (0x20000000..0x40000000).contains(&address) && address & 3 == 0
}

unsafe fn append_extension() {
    let sentinel = abi::LEGACY_EXTENSION_LIST;
    let extension = EXTENSION.0.get() as usize;
    let first = ptr::read_unaligned((sentinel + mem::size_of::<usize>()) as *const usize);
    if first == 0 {
        ptr::write_unaligned(sentinel as *mut usize, extension);
    } else {
        ptr::write_unaligned(first as *mut usize, extension);
    }
    (*EXTENSION.0.get()).previous = sentinel;
    (*EXTENSION.0.get()).next = first;
    ptr::write_unaligned(
        (sentinel + mem::size_of::<usize>()) as *mut usize,
        extension,
    );
}

unsafe extern "C" fn destroy_application(application: usize) {
    let runtime =
        ptr::read_unaligned((application + abi::LEGACY_APPLICATION_RUNTIME) as *const usize);
    if runtime != 0 {
        let context =
            ptr::read_unaligned((runtime + abi::LEGACY_RUNTIME_CONTEXT) as *const Context);
        if !context.is_null() {
            on_destroy(context, runtime);
        }
    }
    let original = ptr::read_unaligned(
        (APPLICATION_VTABLE + abi::LEGACY_APPLICATION_DESTROY_SLOT * mem::size_of::<usize>())
            as *const usize,
    );
    fw!(original, unsafe extern "C" fn(usize))(application);
}

unsafe extern "C" fn initialize_extension(context: Context, module: usize) {
    crate::activation_detail(200);
    let runtime =
        ptr::read_unaligned((context as usize + abi::LEGACY_CONTEXT_RUNTIME) as *const usize);
    if runtime == 0 || current(context).is_some() {
        return;
    }
    let application = abi::legacy_application(runtime);
    if !ram_object(application) {
        return;
    }
    let dispatch = APPLICATION_DISPATCH
        .0
        .get()
        .cast::<usize>()
        .add(abi::VTABLE_PREFIX_WORDS) as usize;
    let current_dispatch = ptr::read_volatile(application as *const usize);
    if current_dispatch != APPLICATION_VTABLE && current_dispatch != dispatch {
        return;
    }
    // This callback runs on the application's owner task during bootstrap.
    // Install cleanup before any JS object can create native resources.
    ptr::write_volatile(application as *mut usize, dispatch);
    crate::activation_detail(201);
    let loop_pointer = runtime + abi::LEGACY_RUNTIME_LOOP;
    open_owner(context, runtime, loop_pointer);
    crate::activation_detail(202);
    publish_primitive(context, runtime, module);
    crate::activation_detail(203);
}

unsafe extern "C" fn export_extension(context: Context, module: usize) {
    #[cfg(feature = "target-xiaomi-band-9-3-1-32")]
    if abi::legacy_declare(context, module, c"canopusAstrolink".as_ptr().cast()) < 0 {
        clear_exception(context);
    }
    #[cfg(not(feature = "target-xiaomi-band-9-3-1-32"))]
    let _ = (context, module);
}

unsafe fn publish_primitive(context: Context, runtime: usize, module: usize) {
    if current(context).is_none() {
        return;
    }
    let object = object(context);
    if object == super::js::EXCEPTION {
        clear_exception(context);
        on_destroy(context, runtime);
        return;
    }
    if !expose_owner(context, object) {
        free(context, object);
        if runtime != 0 {
            on_destroy(context, runtime);
        }
        return;
    }
    // The stock core-export copy leaves N66's custom export null. Publish
    // the real primitive through the measured global-object API instead.
    let _ = module;
    let result = abi::legacy_publish(context, object);
    if result < 0 {
        clear_exception(context);
        if runtime != 0 {
            on_destroy(context, runtime);
        }
    }
}
