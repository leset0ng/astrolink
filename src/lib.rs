//! astrolink - Canopus no_std Rust module. The C shim `canopus build` links in
//! registers `canopus_module_descriptor` with the Supervisor
//! (sdk/c/canopus_module_entry.h) and calls `canopus_mod_stop` on unload.
#![cfg_attr(feature = "device", no_std)]

pub mod backend;
pub mod core;
pub mod protocol;
pub mod qjs;
#[cfg(any(all(feature = "device", target_arch = "arm"), test))]
mod runtime;

#[cfg(feature = "device")]
mod device {
    use canopus_abi::*;
    use canopus_runtime::*;

    const MAGIC: u32 = 0x41535431; // "AST1"

    /// The context with its service objects (`struct canopus_context_v1`).
    fn context<'a>(ctx: *const ContextV1) -> Option<&'a CanopusContext> {
        unsafe { (ctx as *const CanopusContext).as_ref() }
    }

    #[no_mangle]
    pub extern "C" fn canopus_mod_prepare(_ctx: *const ContextV1) -> i32 {
        0
    }

    #[no_mangle]
    pub extern "C" fn canopus_mod_stop(_ctx: *const ContextV1) -> i32 {
        RESULT_REBOOT_REQUIRED as i32
    }

    extern "C" fn activate(ctx: *const ContextV1) -> i32 {
        let Some(c) = context(ctx) else {
            return -22;
        };
        if canopus_target_generated::canopus_identity_guard() != 0 {
            return -95;
        }
        #[cfg(target_arch = "arm")]
        unsafe {
            if let Err(error) = crate::runtime::install(c) {
                return match error {
                    crate::core::Error::Backend(raw) => raw,
                    _ => -95,
                };
            }
            if let Err(error) = crate::qjs::install(c, &crate::runtime::ADAPTER) {
                return error;
            }
        }
        if unsafe { services_has(ctx, SVC_LOG, 1) } && !c.log.is_null() {
            unsafe { ((*c.log).info)(ctx, c"astrolink native BLE feature active".as_ptr()) };
        }
        0
    }

    extern "C" fn deactivate(_ctx: *const ContextV1) -> i32 {
        RESULT_REBOOT_REQUIRED as i32
    }

    extern "C" fn query(w: *mut StatusWriterV1) -> i32 {
        let Some(w) = (unsafe { w.as_mut() }) else {
            return -1;
        };
        if !unsafe { status_put_u32(w, MAGIC) } {
            return -1;
        }
        status_writer_publish(w);
        0
    }

    const fn pack<const N: usize>(s: &[u8]) -> [u8; N] {
        let mut out = [0u8; N];
        let mut i = 0;
        while i < s.len() && i < N {
            out[i] = s[i];
            i += 1;
        }
        out
    }

    #[no_mangle]
    pub static canopus_module_descriptor: ModuleDescriptorV1 = ModuleDescriptorV1 {
        struct_size: core::mem::size_of::<ModuleDescriptorV1>() as u32,
        abi_major: ABI_MAJOR,
        abi_minor: ABI_MINOR,
        flags: 0,
        module_id: pack(b"astrolink"),
        module_version: pack(b"0.1.0"),
        build_id: pack(b"astrolink-0.1.0"),
        // An exact target id, or "portable-thumbv8m"; set by `canopus build`.
        target_id: pack(env!("CANOPUS_TARGET_ID").as_bytes()),
        prepare: Some(canopus_mod_prepare),
        activate: Some(activate),
        deactivate: Some(deactivate),
        stop: Some(canopus_mod_stop),
        query: Some(query),
        publish_native_app: None,
        publish_native_app_stage: None,
    };
}

#[cfg(all(feature = "device", not(test)))]
#[panic_handler]
fn panic(_info: &::core::panic::PanicInfo) -> ! {
    loop {}
}
