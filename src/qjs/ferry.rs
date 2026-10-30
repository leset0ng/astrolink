//! Ferry registration for p67: extend each real application's own manager.
use super::*;
use canopus_abi::{CanopusContext, HookType};
use canopus_runtime::hook::Hook;

/// Install the ferry manager-construction hook before launching QuickApps.
///
/// # Safety
/// Activation must be serialized with QuickApp creation. `ctx`, the adapter,
/// and the module's code and state must remain resident until reboot.
pub unsafe fn install(ctx: *const CanopusContext, adapter: &'static Adapter) -> Result<(), i32> {
    if ctx.is_null() {
        return Err(-22);
    }
    if (*STATE.0.get()).hook_id >= 0 {
        return Ok(());
    }
    let result = fw!(
        abi::F_UV_MUTEX_INIT,
        unsafe extern "C" fn(*mut c_void) -> i32
    )(MUTEX.0.get().cast());
    if result != 0 {
        return Err(result);
    }
    (*STATE.0.get()).adapter = Some(adapter);
    READY.store(true, Ordering::Release);
    match Hook::install(
        ctx,
        abi::F_FEATURE_MANAGER_CREATE,
        create_manager as *const () as usize,
        &mut (*STATE.0.get()).original_dispatch,
        HookType::Auto,
    ) {
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

unsafe extern "C" fn create_manager(host: usize) -> usize {
    // The firmware builds the context/package tuple and borrows it during the
    // constructor. Keep its complete manager and event adapter intact.
    let original = (*STATE.0.get()).original_dispatch;
    let manager = fw!(original, unsafe extern "C" fn(usize) -> usize)(host);
    if manager == 0 {
        return manager;
    }
    let registry = ptr::read_unaligned((manager + abi::MANAGER_REGISTRY) as *const usize);
    if registry == 0 {
        return manager;
    }
    trace(20);
    // Construction is synchronous on the application's JS task, before it
    // evaluates require(). The native registry reports success as exactly 1.
    let result = fw!(
        abi::F_FEATURE_REGISTER,
        unsafe extern "C" fn(usize, *const Description) -> i32
    )(registry, &DESCRIPTION);
    if result == 1 {
        trace(21);
    } else {
        trace(97);
        trace(result as u32);
    }
    manager
}
