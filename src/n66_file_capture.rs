//! N66 current-group file capture. Static target contracts, no runtime approval.
//! The current executing task keeps its own group alive; hold its filelist
//! mutex while checking and duplicating borrowed slots. The legacy backend
//! captures the GAP pipe during activation, before any foreign-group request.

use crate::abi;
const FILE_SIZE: usize = 24;
const FILE_FLAGS: usize = 0;
const CAPTURE_FILELIST_COUNT: usize = 16;
const CAPTURE_FILELIST_ARRAY: usize = 20;
const CURRENT_TASK: usize = abi::CURRENT_TASK;
const EBADF: i32 = -9;
const EAGAIN: i32 = -11;
const EINVAL: i32 = -22;
const ENOENT: i32 = -2;

#[inline]
unsafe fn read_word(address: usize) -> usize {
    core::ptr::read_unaligned(address as *const usize)
}

#[inline]
unsafe fn mutex_lock(filelist: usize) -> i32 {
    let call: unsafe extern "C" fn(usize) -> i32 = core::mem::transmute(abi::F_NXMUTEX_LOCK | 1);
    call(filelist)
}

#[inline]
unsafe fn mutex_unlock(filelist: usize) -> i32 {
    let call: unsafe extern "C" fn(usize) -> i32 = core::mem::transmute(abi::F_NXMUTEX_UNLOCK | 1);
    call(filelist)
}

/// Check without dereferencing a borrowed file beyond the current group lock.
#[inline]
unsafe fn pipe_slot(file: usize, write: bool) -> bool {
    if file == 0 {
        return false;
    }
    let flags = read_word(file + FILE_FLAGS);
    let inode = read_word(file + abi::FILE_INODE);
    inode != 0
        && read_word(inode + abi::FILE_INODE) == abi::INODE_OPS_PIPE
        && if write {
            flags & 2 != 0 && flags & 0x40 != 0
        } else {
            flags & 1 != 0
        }
}

#[inline]
unsafe fn borrowed_slot(filelist: usize, fd: i32, write: bool) -> Result<usize, i32> {
    let count =
        core::ptr::read_unaligned((filelist + CAPTURE_FILELIST_COUNT) as *const u8) as i32 * 8;
    if fd < 0 || fd >= count {
        return Err(EBADF);
    }
    let rows = read_word(filelist + CAPTURE_FILELIST_ARRAY);
    if rows == 0 {
        return Err(EBADF);
    }
    let file = read_word(rows + 4 * (fd as usize >> 3)) + FILE_SIZE * (fd as usize & 7);
    if pipe_slot(file, write) {
        Ok(file)
    } else {
        Err(EBADF)
    }
}

/// Duplicate the current JS owner's write endpoint. `out` is either all-zero or
/// an already-owned file from a previous successful call. A valid existing pipe
/// is reused without a second pipe-open. Errors never close the borrowed source.
///
/// # Safety
/// Call on the current group-owning JS task. `out` points to 24 writable bytes
/// reserved for an owned file. The caller excludes concurrent use/close of it.
unsafe fn capture_filelist(
    out: *mut core::ffi::c_void,
    loop_pointer: usize,
    filelist: usize,
) -> i32 {
    if out.is_null() || loop_pointer == 0 {
        return EINVAL;
    }
    let destination = out as usize;
    let mut result = mutex_lock(filelist);
    if result != 0 {
        return result;
    }
    if pipe_slot(destination, true) {
        mutex_unlock(filelist);
        return 0;
    }
    if read_word(destination + FILE_FLAGS) != 0 || read_word(destination + abi::FILE_INODE) != 0 {
        mutex_unlock(filelist);
        return EINVAL;
    }
    let read_fd = core::ptr::read_unaligned((loop_pointer + abi::LOOP_READ_FD) as *const i32);
    let write_fd = core::ptr::read_unaligned((loop_pointer + abi::LOOP_WRITE_FD) as *const i32);
    let source = if read_fd == write_fd {
        Err(EINVAL)
    } else {
        borrowed_slot(filelist, read_fd, false)
            .and_then(|read| borrowed_slot(filelist, write_fd, true).map(|write| (read, write)))
            .map(|(_, write)| write)
    };
    match source {
        Ok(source) => {
            let duplicate: unsafe extern "C" fn(usize, usize) -> i32 =
                core::mem::transmute(abi::F_FILE_DUP2_NATIVE | 1);
            result = duplicate(source, destination);
        }
        Err(error) => result = error,
    }
    let unlock_result = mutex_unlock(filelist);
    if result != 0 {
        result
    } else {
        unlock_result
    }
}

/// Capture from the current task's descriptor table while its group is alive.
pub unsafe fn capture_current(out: *mut core::ffi::c_void, loop_pointer: usize) -> i32 {
    let current = read_word(CURRENT_TASK);
    if current == 0 {
        return EAGAIN;
    }
    let group = read_word(current + abi::TCB_GROUP);
    if group == 0 {
        return EAGAIN;
    }
    capture_filelist(out, loop_pointer, group + abi::GROUP_FILELIST)
}

#[inline]
fn ram_pointer(address: usize) -> bool {
    (0x20000000..0x40000000).contains(&address) && address & 3 == 0
}

struct LoopLookup {
    loop_pointer: usize,
    group: usize,
}

/// Identity reads only: nxsched_foreach holds BASEPRI and the scheduler lock.
/// A custom uv_loop_new loop is not the default loop in uvstate+584, so match
/// its actual pipe endpoints instead of looking for a TLS entry that cannot
/// identify it. Ambiguous matches are rejected rather than guessing a group.
unsafe extern "C" fn find_loop_group(tcb: usize, lookup: *mut LoopLookup) {
    let lookup = &mut *lookup;
    if lookup.group == usize::MAX || !ram_pointer(tcb) {
        return;
    }
    let group = read_word(tcb + abi::TCB_GROUP);
    if !ram_pointer(group) {
        return;
    }
    let read_fd =
        core::ptr::read_unaligned((lookup.loop_pointer + abi::LOOP_READ_FD) as *const i32);
    let write_fd =
        core::ptr::read_unaligned((lookup.loop_pointer + abi::LOOP_WRITE_FD) as *const i32);
    if read_fd == write_fd {
        return;
    }
    let filelist = group + abi::GROUP_FILELIST;
    let (Ok(read), Ok(write)) = (
        borrowed_slot(filelist, read_fd, false),
        borrowed_slot(filelist, write_fd, true),
    ) else {
        return;
    };
    if read_word(read + abi::FILE_INODE) != read_word(write + abi::FILE_INODE) {
        return;
    }
    lookup.group = if lookup.group == 0 || lookup.group == group {
        group
    } else {
        usize::MAX
    };
}

/// Resolve the process-lifetime btservice group by its unique pipe identity,
/// then duplicate only after leaving the scheduler critical section. Unlike an
/// arbitrary foreign app, the firmware's btservice group outlives this module.
pub unsafe fn capture_btservice(out: *mut core::ffi::c_void, loop_pointer: usize) -> i32 {
    if loop_pointer == 0 {
        return EINVAL;
    }
    let mut lookup = LoopLookup {
        loop_pointer,
        group: 0,
    };
    let foreach: unsafe extern "C" fn(
        unsafe extern "C" fn(usize, *mut LoopLookup),
        *mut LoopLookup,
    ) = core::mem::transmute(abi::F_NXSCHED_FOREACH | 1);
    foreach(find_loop_group, &mut lookup);
    if !ram_pointer(lookup.group) {
        return ENOENT;
    }
    capture_filelist(out, loop_pointer, lookup.group + abi::GROUP_FILELIST)
}

/// # Safety
/// `file` must be a zeroed or owned 24-byte file returned by capture_current.
pub unsafe fn release_owned(file: *mut core::ffi::c_void) {
    if file.is_null() {
        return;
    }
    let close: unsafe extern "C" fn(usize) -> i32 = core::mem::transmute(abi::F_FILE_CLOSE | 1);
    close(file as usize);
}
