//! Exact Q66 .155 libuv wake bridge across NuttX task-group fd tables.
use core::{
    ffi::c_void,
    mem, ptr,
    sync::atomic::{AtomicU32, Ordering},
};

macro_rules! fw {
    ($address:expr, $ty:ty) => {
        mem::transmute::<usize, $ty>($address | 1)
    };
}

/// An independently owned pipe file, never a borrowed numeric descriptor.
/// Its enclosing owner must serialize producers and exclude close during send.
#[repr(C, align(8))]
pub(crate) struct PipeWake {
    file: [u64; 4],
    open: bool,
}
impl PipeWake {
    pub(crate) const EMPTY: Self = Self {
        file: [0; 4],
        open: false,
    };

    /// Resolve the fd on its loop's owning task, retain it, then duplicate it
    /// into independent storage before releasing the table-slot reference.
    pub(crate) unsafe fn capture_current(&mut self, loop_pointer: usize) -> Result<(), i32> {
        if self.open {
            return Ok(());
        }
        let fd = ptr::read_unaligned((loop_pointer + 316) as *const i32);
        let mut source = ptr::null_mut::<c_void>();
        let result = fw!(
            0x0c332c90,
            unsafe extern "C" fn(i32, *mut *mut c_void) -> i32
        )(fd, &mut source);
        if result != 0 {
            return Err(result);
        }
        let result = self.duplicate(source);
        fw!(0x0c334998, unsafe extern "C" fn(*mut c_void) -> i32)(source);
        result
    }

    /// Resolve a process-lifetime default loop by its owning task's TLS
    /// identity, not by the caller's fd table or a fixed PID.
    pub(crate) unsafe fn capture_default_loop(&mut self, loop_pointer: usize) -> Result<(), i32> {
        if self.open {
            return Ok(());
        }
        let key = ptr::read_volatile(0x200bd444 as *const u32);
        if key > 3 || loop_pointer == 0 {
            return Err(-95);
        }
        let mut lookup = Lookup {
            loop_pointer,
            key,
            read: ptr::null_mut(),
            write: ptr::null_mut(),
        };
        fw!(
            0x0c355564,
            unsafe extern "C" fn(unsafe extern "C" fn(usize, *mut Lookup), *mut Lookup)
        )(find_loop_files, &mut lookup);
        let result = if pipe_pair(lookup.read, lookup.write) {
            self.duplicate(lookup.write)
        } else {
            Err(-9)
        };
        // No driver close or duplication runs inside nxsched_foreach's
        // scheduler/interrupt critical section. Only pinned files escape it.
        for file in [lookup.read, lookup.write] {
            if !file.is_null() {
                fw!(0x0c334998, unsafe extern "C" fn(*mut c_void) -> i32)(file);
            }
        }
        result
    }

    /// `source` must be pinned by its owning filelist reference for this call.
    pub(crate) unsafe fn duplicate(&mut self, source: *mut c_void) -> Result<(), i32> {
        let copied = fw!(
            0x0c341c7a,
            unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32
        )(source, self.file.as_mut_ptr().cast(), 0);
        self.open = copied == 0;
        if copied == 0 {
            Ok(())
        } else {
            Err(copied)
        }
    }

    pub(crate) unsafe fn close(&mut self) {
        if self.open {
            self.open = false;
            fw!(0x0c340a24, unsafe extern "C" fn(*mut c_void) -> i32)(
                self.file.as_mut_ptr().cast(),
            );
        }
    }

    /// Preserve this firmware's uv_async_send busy/pending protocol while
    /// writing the retained pipe directly. The handle and file must be pinned.
    pub(crate) unsafe fn signal(&mut self, handle: *mut u32) -> i32 {
        if !self.open || handle.is_null() {
            return -9;
        }
        let busy = &*handle.add(6).cast::<AtomicU32>();
        let pending = &*handle.add(15).cast::<AtomicU32>();
        if pending.load(Ordering::Acquire) != 0 {
            return 0;
        }
        busy.fetch_add(1, Ordering::AcqRel);
        let mut result = 0;
        if pending.swap(1, Ordering::AcqRel) == 0 {
            let byte = 1u8;
            loop {
                let written = fw!(
                    0x0c33cc4c,
                    unsafe extern "C" fn(*mut c_void, *const u8, usize) -> isize
                )(self.file.as_mut_ptr().cast(), &byte, 1);
                if written == -4 {
                    continue;
                }
                if written != 1 && written != -11 {
                    result = if written < 0 { written as i32 } else { -5 };
                    pending.store(0, Ordering::Release);
                }
                break;
            }
        }
        busy.fetch_sub(1, Ordering::AcqRel);
        result
    }
}

struct Lookup {
    loop_pointer: usize,
    key: u32,
    read: *mut c_void,
    write: *mut c_void,
}
fn ram_pointer(address: usize) -> bool {
    (0x20000000..0x40000000).contains(&address) && address & 3 == 0
}
unsafe fn word(address: usize) -> usize {
    ptr::read_unaligned(address as *const usize)
}
unsafe extern "C" fn find_loop_files(tcb: usize, lookup: *mut Lookup) {
    let lookup = &mut *lookup;
    if !lookup.read.is_null() || !lookup.write.is_null() || tcb == 0 {
        return;
    }
    let tls = word(tcb + 100);
    if !ram_pointer(tls) {
        return;
    }
    let taskinfo = word(tls);
    if !ram_pointer(taskinfo) {
        return;
    }
    let uvstate = word(taskinfo + 16 + 4 * lookup.key as usize);
    if !ram_pointer(uvstate) || word(uvstate + 664) != lookup.loop_pointer {
        return;
    }
    let group = word(tcb + 8);
    if !ram_pointer(group) {
        return;
    }
    let filelist = group + 96;
    let count = ptr::read_unaligned(filelist as *const u8) as i32 * 8;
    let read_fd = ptr::read_unaligned((lookup.loop_pointer + 312) as *const i32);
    let write_fd = ptr::read_unaligned((lookup.loop_pointer + 316) as *const i32);
    if read_fd < 0 || write_fd < 0 || read_fd >= count || write_fd >= count || read_fd == write_fd {
        return;
    }
    lookup.read =
        fw!(0x0c332ba2, unsafe extern "C" fn(usize, i32) -> *mut c_void)(filelist, read_fd);
    lookup.write =
        fw!(0x0c332ba2, unsafe extern "C" fn(usize, i32) -> *mut c_void)(filelist, write_fd);
}
unsafe fn pipe_pair(read: *mut c_void, write: *mut c_void) -> bool {
    if read.is_null() || write.is_null() {
        return false;
    }
    let read_flags = word(read as usize);
    let write_flags = word(write as usize);
    let inode = word(read as usize + 16);
    inode != 0
        && inode == word(write as usize + 16)
        && read_flags & 1 != 0
        && write_flags & 2 != 0
        && write_flags & 0x40 != 0
        && word(inode + 16) == 0x2cb04ffc
}
