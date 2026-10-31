//! Call-shape shims, one set per target module.
//!
//! Lives outside `src/abi/` because that directory holds one module per
//! exact firmware and `tests/targets.rs` treats every file there as a target.
//!
//! An adapted target does not always bind one firmware function per operation.
//! Two shapes exist:
//!
//! * **direct** — the firmware exports a function with the contract the module
//!   wants, so the shim is a single indirect call (`abi_firmware_shims!`);
//! * **composed** — the firmware has the pieces but no matching public shape,
//!   so the target builds the behavior from measured primitives (p67 applies
//!   its state-dependent disconnect policy through the channel primitives).
//!
//! The module always calls the shim, never the constant, so a target is free to
//! answer either way without the call sites knowing. Type aliases are avoided on
//! purpose: these are the raw ABI types, and `qjs/feature.rs` passes its own
//! `Context`/`Value` aliases into them.

/// Emits the seven shims in direct firmware-call form.
///
/// A target that needs a different shape defines the same seven functions
/// itself instead of invoking this macro.
#[macro_export]
macro_rules! abi_firmware_shims {
    () => {
        /// Firmware async handle storage, in words.
        pub const FEATURE_ASYNC_WORDS: usize = 16;

        #[inline(always)]
        /// QuickJS: create a string value from `length` bytes at `bytes`.
        ///
        /// # Safety
        ///
        /// The context must be the live runtime's, and every pointer argument
        /// must be valid for the access the firmware performs.
        pub unsafe fn qjs_new_string_len(
            context: *mut ::core::ffi::c_void,
            bytes: *const u8,
            length: usize,
        ) -> u64 {
            let call: unsafe extern "C" fn(*mut ::core::ffi::c_void, *const u8, usize) -> u64 =
                ::core::mem::transmute($crate::abi::F_QJS_NEW_STRING_LEN | 1);
            call(context, bytes, length)
        }

        #[inline(always)]
        /// QuickJS: copy a string value into a NUL-terminated C string.
        ///
        /// Returns the allocation, which the caller releases through
        /// `F_QJS_FREE_CSTRING`, and reports the length through `length`.
        ///
        /// # Safety
        ///
        /// The context must be the live runtime's, and every pointer argument
        /// must be valid for the access the firmware performs.
        pub unsafe fn qjs_to_cstring_len2(
            context: *mut ::core::ffi::c_void,
            length: *mut usize,
            value: u64,
            flags: i32,
        ) -> *const u8 {
            let call: unsafe extern "C" fn(
                *mut ::core::ffi::c_void,
                *mut usize,
                u64,
                i32,
            ) -> *const u8 = ::core::mem::transmute($crate::abi::F_QJS_TO_CSTRING_LEN2 | 1);
            call(context, length, value, flags)
        }

        #[inline(always)]
        /// QuickJS: define an own property whose name is a byte string.
        ///
        /// Returns a negative value on failure, like the firmware function.
        ///
        /// # Safety
        ///
        /// The context must be the live runtime's, and every pointer argument
        /// must be valid for the access the firmware performs.
        pub unsafe fn qjs_define_property_value_str(
            context: *mut ::core::ffi::c_void,
            object: u64,
            name: *const u8,
            value: u64,
            flags: i32,
        ) -> i32 {
            let call: unsafe extern "C" fn(
                *mut ::core::ffi::c_void,
                u64,
                *const u8,
                u64,
                i32,
            ) -> i32 = ::core::mem::transmute($crate::abi::F_QJS_DEFINE_PROPERTY_VALUE_STR | 1);
            call(context, object, name, value, flags)
        }

        #[inline(always)]
        /// QuickJS: create an ArrayBuffer value holding a copy of `length` bytes.
        ///
        /// # Safety
        ///
        /// The context must be the live runtime's, and every pointer argument
        /// must be valid for the access the firmware performs.
        pub unsafe fn qjs_new_array_buffer_copy(
            context: *mut ::core::ffi::c_void,
            data: *const u8,
            length: usize,
        ) -> u64 {
            let call: unsafe extern "C" fn(*mut ::core::ffi::c_void, *const u8, usize) -> u64 =
                ::core::mem::transmute($crate::abi::F_QJS_NEW_ARRAY_BUFFER_COPY | 1);
            call(context, data, length)
        }

        #[inline(always)]
        /// QuickJS: read an ArrayBuffer value's payload and length.
        ///
        /// Reports `length` through `length`, returns the payload or null.
        ///
        /// # Safety
        ///
        /// The context must be the live runtime's, and every pointer argument
        /// must be valid for the access the firmware performs.
        pub unsafe fn qjs_get_array_buffer(
            context: *mut ::core::ffi::c_void,
            length: *mut usize,
            value: u64,
        ) -> *const u8 {
            let call: unsafe extern "C" fn(*mut ::core::ffi::c_void, *mut usize, u64) -> *const u8 =
                ::core::mem::transmute($crate::abi::F_QJS_GET_ARRAY_BUFFER | 1);
            call(context, length, value)
        }

        #[inline(always)]
        /// QuickJS: unwrap a typed array into its buffer value.
        ///
        /// Fills `offset`, `length` and `element_size`, returns the buffer
        /// value or the exception tag.
        ///
        /// # Safety
        ///
        /// The context must be the live runtime's, and every pointer argument
        /// must be valid for the access the firmware performs.
        pub unsafe fn qjs_get_typed_array_buffer(
            context: *mut ::core::ffi::c_void,
            value: u64,
            offset: *mut usize,
            length: *mut usize,
            element_size: *mut usize,
        ) -> u64 {
            let call: unsafe extern "C" fn(
                *mut ::core::ffi::c_void,
                u64,
                *mut usize,
                *mut usize,
                *mut usize,
            ) -> u64 = ::core::mem::transmute($crate::abi::F_QJS_GET_TYPED_ARRAY_BUFFER | 1);
            call(context, value, offset, length, element_size)
        }

        /// Apply the target's disconnect policy on the Bluelet owner task.
        ///
        /// # Safety
        /// `channel` must be live and owned by the current Bluelet task.
        pub unsafe fn disconnect_channel(channel: usize, policy: u16) {
            let call: unsafe extern "C" fn(usize, u16) =
                core::mem::transmute($crate::abi::F_L2CAP_DISCONNECT_CID | 1);
            call(channel, policy);
        }

        #[inline(always)]
        /// Hands one outbound payload to the channel's send path.
        ///
        /// # Safety
        ///
        /// `channel` must be live and `data` must point at `length` readable bytes.
        pub unsafe fn queue_send(
            owner: usize,
            channel: usize,
            cid: u16,
            data: *const u8,
            length: u16,
        ) -> isize {
            let queue = core::ptr::read_unaligned(
                (channel + $crate::abi::COC_CHANNEL_QUEUE) as *const usize,
            );
            if queue == 0 {
                return -1;
            }
            let payload_offset = 12u16 + $crate::abi::SDU_BUFFER_PAYLOAD as u16;
            // An empty SDU still has its first-fragment two-byte length field.
            let end = payload_offset.wrapping_add(if length == 0 { 2 } else { length });
            let allocate: unsafe extern "C" fn(u32) -> *mut u8 =
                core::mem::transmute($crate::abi::F_ALLOC | 1);
            let buffer = allocate(end as u32 + $crate::abi::SDU_BUFFER_ALLOC_TAIL as u32);
            if buffer.is_null() {
                return -1;
            }
            core::ptr::write_unaligned(buffer.add($crate::abi::SDU_BUFFER_END).cast::<u16>(), end);
            core::ptr::write_unaligned(
                buffer.add($crate::abi::SDU_BUFFER_OFFSET).cast::<u16>(),
                payload_offset,
            );
            if payload_offset > 7 {
                *buffer.add($crate::abi::SDU_BUFFER_META) = $crate::abi::SDU_BUFFER_META_VALUE;
            }
            if length == 0 {
                // Stock code loops only while the payload length is non-zero, so
                // a zero-length SDU still needs its one first+last fragment.
                *buffer.add($crate::abi::SDU_BUFFER_META) = 1;
                core::ptr::write_unaligned(
                    buffer
                        .add($crate::abi::SDU_BUFFER_PAYLOAD + payload_offset as usize)
                        .cast::<u16>(),
                    0,
                );
            } else {
                core::ptr::write_unaligned(
                    buffer.add($crate::abi::SDU_BUFFER_CID).cast::<u32>(),
                    cid as u32,
                );
                core::ptr::copy_nonoverlapping(
                    data,
                    buffer.add($crate::abi::SDU_BUFFER_PAYLOAD + payload_offset as usize),
                    length as usize,
                );
                // The stock handler owns the original SDU and creates its own
                // fragment nodes before releasing that original allocation.
                let send: unsafe extern "C" fn(usize, u32, *mut u8) -> i32 =
                    core::mem::transmute($crate::abi::F_L2CAP_SEND | 1);
                return send(owner, $crate::abi::COC_QUEUE_EVENT, buffer) as isize;
            }
            let allocation = allocate($crate::abi::FRAGMENT_NODE_SIZE as u32);
            if allocation.is_null() {
                let release: unsafe extern "C" fn(*mut u8) =
                    core::mem::transmute($crate::abi::F_FREE | 1);
                release(buffer);
                return -1;
            }
            // Stock queue insertion preserves the node header; initialize next/prev
            // just like the firmware node allocator before handing it to the queue.
            core::ptr::write_bytes(allocation, 0, $crate::abi::FRAGMENT_NODE_SIZE);
            let node = allocation.add($crate::abi::FRAGMENT_NODE_PAYLOAD);
            core::ptr::write_unaligned(
                node.add($crate::abi::FRAGMENT_NODE_BUFFER).cast::<u32>(),
                buffer as usize as u32,
            );
            *node.add($crate::abi::FRAGMENT_NODE_FLAGS) = $crate::abi::FRAGMENT_FLAGS_FIRST_LAST;
            let push: unsafe extern "C" fn(usize, *mut u8) -> i32 =
                core::mem::transmute($crate::abi::F_L2CAP_QUEUE_PUSH | 1);
            let flush: unsafe extern "C" fn(usize, usize) -> i32 =
                core::mem::transmute($crate::abi::F_L2CAP_CREDIT_FLUSH | 1);
            push(queue, node);
            flush(queue, channel) as isize
        }

        /// Writes one byte to a pinned file, returning the firmware's result.
        ///
        /// # Safety
        ///
        /// The context must be the live runtime's, and every pointer argument
        /// must be valid for the access the firmware performs.
        pub unsafe fn write_byte(file: *mut ::core::ffi::c_void, byte: u8) -> isize {
            let call: unsafe extern "C" fn(*mut ::core::ffi::c_void, *const u8, usize) -> isize =
                ::core::mem::transmute($crate::abi::F_FILE_WRITE | 1);
            call(file, &byte, 1)
        }
    };
}
