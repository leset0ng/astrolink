# P65 3.100.043 GAP loop and pipe ownership recovery

Scope: kernel/libuv/file/group lifetime only. Read-only analysis of
`/Users/lesetong/develop/mifw/analysis-p65-043/vela_ap.bin.i64`.
Input image SHA-256 independently rechecked:
`a19b601477569765103eaf468b937bd8026b4d68ffa5c896bde222a79e475858`.
No source, target-pack, or IDB changes were made. Function descriptions below
are recovered semantics; existing `sub_*` names are not firmware exports.

## Finding and implementation decision

Two assumptions in the earlier blocker are disproved by the exact image:

1. The GAP business loop in the firmware-global client is the MiWear task's
   **TLS default loop**, not an independent `uv_loop_new` loop. The actual
   producer-to-setter chain is established below.
2. P65 has **no N66-style mutex at the filelist base**. Offset 0 contains a
   row-count byte and a table reference/share-count byte, followed by the row
   pointer array at +4. Treating that address as an nxmutex would corrupt it.

A retained file-slot reference does not pin its task group or the storage
containing the slot. Do not extend N66's scheduler-walk/then-filelist-mutex
algorithm to this image: the supposed mutex does not exist.

The strongest implementation path is to dispatch one bootstrap callback onto
the existing Supervisor UI pump, prove that callback's current TLS default
loop equals the client's business loop, and capture/duplicate the endpoint on
that current group-owning task. This removes the foreign-group lifetime gap.
The public SDK already exposes `ctx->ui->post` for this purpose. The callback
must reject a task/loop mismatch; its safety must not depend merely on the
service being named UI. Runtime dispatch identity still needs to be checked.

If choosing the existing foreign `capture_default_loop` path instead, its
correctness remains conditional on the firmware MiWear group being alive
through the entire capture. This report does not prove a general group pin
that would allow arbitrary foreign groups to be captured after leaving a
scheduler walk.

## Exact GAP business-loop producer chain

`0x0C92CE28` is the MiWear main entry:

```text
0x0C92CE30 BL  0x0C62B4EC
0x0C92CE3A LDR R6, =0x3C479C68
0x0C92CE40 STR R0, [R6]
...
0x0C92CE72 LDR R0, [R6]
0x0C92CE74 BL  0x0C6220C8  ; uv_run(loop, mode=0)
```

`0x0C62B4EC` is the default-loop accessor, independently established by its
read/init/write sequence:

```text
0x0C62B4F0 BL  0x0C626620  ; current task's uvstate
0x0C62B4F4 LDR R4, [R0,#0x300] ; uvstate+768
...
0x0C62B506 BL  0x0C626620
0x0C62B50A ADDS R0,#0x80
0x0C62B50C BL  0x0C626360  ; initialize embedded loop at uvstate+128
...
0x0C62B51E ADDS R4,#0x80
0x0C62B520 STR R4,[R0,#0x300]
```

Initialization timer `0x0C92CD50` calls startup `0x0C92C750` in its first
state. Startup loads global `0x3C479C68` and passes it unchanged to the GAP
client bootstrap:

```text
0x0C92C93E LDR R4,[R8] ; R8 points at 0x3C479C68 in this function
0x0C92C94E MOV R0,R4
0x0C92C950 BL  0x0CA3F3D8
```

`0x0CA3F3D8` preserves this loop argument while constructing the Bluetooth
client, publishing the pointer at `0x3C4942B0`, then sets its business loop:

```text
0x0CA3F3DC MOV R4,R0
0x0CA3F3DE BL  0x0C8BE8C0  ; thunk -> 0x1C001B54 creates client
0x0CA3F3E2 LDR R5,=0x3C4942B0
0x0CA3F3E4 STR R0,[R5]
0x0CA3F3E8 MOV R1,R4
0x0CA3F3EA BL  0x0C8BED18  ; thunk -> 0x1C001C10
```

Setter `0x1C001C10(client,loop)` writes `*(client+52)=loop` when client is
non-null. Client construction `0x1C001B54` and its initializer `0x1C00C590`
allocate a separate response loop at client+48. That response loop is distinct
from the business loop at client+52; it is not the loop Astrolink should wake.

Task descriptor words at `0x0C275424` (also mirrored at `0x1C185660`) are:
name `0x3C185494` ("miwear"), priority 0x68, stack 0x20000,
entry `0x0C92CE29` (Thumb). The "miwear_bluetooth" name at `0x3C18549C`
belongs to the following descriptor, not to this entry. Numeric PIDs are not
part of the identity contract.

## TLS/default-loop lookup layout

The current source constants used by `capture_default_loop` are independently
consistent with this image:

| Field | Verified value | Local producer/consumer |
| --- | --- | --- |
| current TCB global | `0x201738F8` | `0x0C4C61C0`, `0x0C4C5980` |
| TCB group pointer | `+8` | `0x0C4C5980`, `0x0C4C596C` |
| TCB TLS pointer used by pthread state | `+68` | `0x0C4C61C0`, `0x0C509014` |
| TLS first word -> taskinfo | `+0` | `0x0C509008` dereferences `0x0C509014` result |
| taskinfo key slots | `+20 + 4*key` | `0x0C509030`, `0x0C509048` |
| allowed key | `0..3` | bounds in `0x0C509030`/`0x0C509048` |
| libuv key global | `0x3C44C388` | `0x0C626624`, `0x0C626632` |
| uvstate default loop | `+768` | `0x0C62B4F4`, `0x0C62B520` |

`0x0C4C61C0(0,out)` copies TCB+64/+68/+72 to out+0/+4/+8;
`0x0C509014` returns the second output word; `0x0C509008` dereferences it.
Thus a read-only current-owner check can use the already recovered structure
chain without invoking an accessor that lazily allocates a new default loop.
A null/missing slot should cause capture to wait/fail, not initialize another
loop while checking ownership.

## P65 filelist and file layouts

Group filelist is inline at group+88. `0x0C4C596C(tcb)` returns
`*(tcb+8)+88`; `0x0C4C5980()` applies the same operation to current TCB.
Their veneers `0x1C165DA8`/`0x1C1674B8` feed the VFS lookup paths.

Initializer `0x1C154030(filelist)`:

```text
0x1C154032 MOVW R1,#0x101
0x1C154038 ADD R2,R0,#0x10
0x1C15403E STRH R1,[R3],#8
0x1C154042 STRD R3,R2,[R0,#4]
```

| filelist offset | Meaning |
| --- | --- |
| +0, u8 | allocated row count, initially 1 |
| +1, u8 | table reference/share count, initially 1 |
| +4, pointer | row-pointer array, initially filelist+8 |
| +8, pointer | built-in first row pointer, initially filelist+16 |
| +16 | first inline row, 8 slots of 32 bytes |

No filelist mutex is initialized, acquired, or released by this family of
lookup/allocation/teardown routines. Slot accesses and publication use BASEPRI
0x80. An inode/global lock or the pipe driver's own mutex is a different object
and is not a substitute for filelist/group lifetime protection.

Slot address:

```c
rows = *(uint32_t **)(filelist + 4);
slot = rows[fd >> 3] + 32 * (fd & 7); // require 0 <= fd < 8*count
```

Verified file fields: flags +0; slot reference count +4; position +8..15;
inode +16; private state +20. A retained `struct file *` still points into the
row allocation. Independent file storage must provide 32 writable bytes;
Astrolink's `[u64;4]` storage already does so.

### Lookup and release

`0x1C1542B4(filelist,fd)` performs the row lookup under BASEPRI, checks
inode(+16) and slot refs(+4) are nonzero, increments refs, returns the slot.
It performs **no descriptor bounds check**, so callers must validate count and
fd before calling it. Critical instructions are `0x1C1542CA` (rows pointer),
`0x1C1542E0` (32-byte stride), `0x1C1542E4` (inode),
`0x1C1542E8` (refs), and `0x1C1542F0` (incremented refs store).

`0x1C154700(fd,out)` resolves the current group's filelist, clears `*out`,
checks `0 <= fd < 8*count`, then retains the slot under BASEPRI:
`0x1C154716` count; `0x1C154728..734` slot; `0x1C154738` inode;
`0x1C15473C..742` refs check/increment. It returns 0, -9 (bad fd),
or -11 (no current table).

`0x1C15479C(file)` decrements file+4 under BASEPRI
(`0x1C1547AE..7B4`), leaves the critical section, and only on zero calls
`file_close` at `0x1C1547C0 -> 0x1C1562E4`.
This entry is a real callable even though the IDB does not make it a standalone
Hex-Rays function. Its return value is zero for a non-final release and the
close result for a final release.

### Growth and teardown expose the foreign-pointer gap

`0x1C154124(filelist,new_rows)` allocates new rows of 256 bytes
(`0x1C154168..16C`), then under BASEPRI copies old pointers, replaces
filelist+4 and count+0 (`0x1C1541A0..1C8`). It frees the obsolete pointer array
outside BASEPRI; existing row allocations remain stable while the group/table
is alive. Merely retaining a pointer-array address across scheduling is unsafe.

`0x1C154238(filelist)` decrements share count +1; on its last reference it
iterates every 32-byte slot and calls `file_close` **directly**
(`0x1C15425A..262`), then frees dynamic row allocations
(`0x1C154282..28A`) and the pointer array (`0x1C154272..27A`).
It does not wait for the per-file +4 refs acquired by files_getfilep.
The inline first row is part of the task-group allocation itself.

Group exit `0x0C4C2850` reaches final-group cleanup at `0x0C4C28B4`, calls
this teardown through `0x0C8BE5C8` at `0x0C4C28CC..28D0`, and ultimately frees
the group at `0x0C4C2914..2916`. Therefore a borrowed or retained slot pointer
escaping a scheduler walk is not by itself a valid group pin. Incrementing the
filelist's byte at +1 manually is also not a demonstrated group-memory pin.

## Independent duplication and pipe driver ownership

`0x1C156380(source,destination,flags)` is the native three-argument file_dup3:
flags 0 accepted, only bit 0x400 otherwise allowed. It:

1. reads source inode (+16), calls inode addref `0x1C154F90`;
2. closes any prior destination with `0x1C156298`;
3. copies flags, position, inode and driver-private state;
4. for a character device calls inode operations->open(destination), after
   setting destination flags bit 0x40 (nonblocking);
5. rolls back its added inode reference on open failure.

Instruction anchors: `0x1C15638A` source inode; `0x1C1563A6` inode addref;
`0x1C1563BA..3DA` field copy; `0x1C1563EA..3FE` private copy and open call.
It does not initialize destination+4 as a descriptor-table reference. The
owned standalone file should be closed using `file_close`, not fs_putfilep.
It must start zeroed. On failed duplication, do not assume its partially
written fields constitute an owned file or close it as one; reset storage
before a fresh attempt according to the returned ownership result.

P65 pipe inode operation table `0x3C169F7C` contains:
open `0x0C4CF6D5`, close `0x0C4CF5A9` (Thumb).
Open obtains the pipe-private recursive mutex (`0x0C4CF6D4`, call to
`0x0C4F8B18`), increments open count at private+54, increments writer count
+50 for flags&2 or reader count +51 for flags&1. The 0x40 flag prevents waiting
for an opposite endpoint, but **does not remove mutex acquisition**. The
mutex routine can wait on a semaphore at `0x0C4F8B44 -> 0x0C4C6A08`.

Inode addref `0x1C154F90` also takes an inode lock through
`0x1C154F50 -> 0x1C1682E0 -> 0x0C4C73B8`, which may sleep. This proves
file_dup3 must not be executed inside nxsched_foreach's interrupt critical
section even for a nonblocking pipe.

Pipe close `0x0C4CF5A8` decrements private+54 and frees pipe-private state on
its final close when the anonymous-pipe flag at private+52 contains bit 2.
A successful independent duplicate invokes real pipe open and thus owns both
its inode reference and its writer/open accounting. A raw memcpy or inode-only
snapshot is not an independent pipe endpoint.

`file_close` entry `0x1C1562E4` invokes driver close, drops the inode reference,
and clears file+16 on success. Use that entry once when the module's retained
wake endpoint is no longer used, after producers are excluded.

## Scheduler walk constraints and recommended capture protocol

`0x0C4C6078(callback,arg)` masks BASEPRI to 0x80, invokes scheduler lock
`0x0C4C60D8`, walks TCB pointers in table `0x201738F4` bounded by
`0x201738E4`, invokes callbacks, unlocks through `0x0C4C6110`, restores
BASEPRI. The callbacks must not duplicate/close files or take sleeping locks.

A discovery walk may read the TLS/default-loop chain above and record a
matching TCB/group identity, deduplicating threads of the same group and
rejecting ambiguity. It may verify fds and same-pipe inode under the critical
section. However, after it returns, that group pointer is still borrowed.
There is **no verified filelist mutex** under which a general foreign group
can then be safely revalidated and duplicated. A second scheduler walk narrows
but does not eliminate the gap between walking and sleeping duplication.

Recommended P65 implementation:

1. During asynchronous bootstrap, post a callback using the current
   Supervisor UI service. Do not block the UI waiting for its own callback.
2. In the callback, read current TCB -> TLS -> taskinfo -> libuv key slot;
   require its uvstate+768 loop to equal `*(global_client+52)` and the expected
   stock loop global when used. Fail/retry on absent/mismatched identity.
3. On that owning task call fs_getfilep for both loop fd endpoints. Validate
   distinct nonnegative fds, read flags&1, write flags&2 and flags&0x40,
   matching nonzero inode and `*(inode+16)==0x3C169F7C`. This pair validation
   avoids accepting an unrelated numeric descriptor.
4. Call file_dup3(write, zeroed_owned_file32, 0) after retaining both slots.
   The current executing task keeps its own group alive across sleeping
   driver/inode locks. Ordinary descriptor close cannot destroy the retained
   source until the matching put; row growth preserves existing rows.
5. Release each successfully retained slot with fs_putfilep, exactly once,
   on success or failure. Publish readiness only after duplication succeeds.
6. Later producers write the independent `struct file` directly and preserve
   the recovered async busy/pending protocol. Do not invoke stock uv_async_send
   across task groups, since its numeric fd is resolved in the caller's group.
7. If the callback can never run on the actual owner, retain unsupported/not
   ready until another independently proved owner dispatch/group-pin exists.

The public service route is present in current workspace sources:
`CanopusUiObj.post` in `sdk/rust/canopus-abi/src/lib.rs:461`;
`canopus_supervisor_ui_post` in
`manager/service/canopus_supervisor_services.c:1111`;
`svc_ui_jobs_poll` at line 1129 is invoked only for the foreground heartbeat
at line 1186. Background polling does not dispatch these UI jobs. Screen-off
bootstrap progress and real callback thread identity require runtime checks.

## Existing wake protocol evidence

`uv_async_init` `0x0C621BDC` initializes a 128-byte handle, stores handle loop
at +4, callback +112, busy word +24 and pending word +124. It creates a
nonblocking pipe and stores read/write fds at loop+360/+364.
`uv_async_send` `0x0C621CA0` increments busy at +24, atomically sets pending
at +124, writes one byte only for a 0->1 transition, decrements busy. It
retries EINTR and tolerates EAGAIN. Exact numeric write-fd load is at
`0x0C621CE8`; the write at `0x0C621CFE` is descriptor-based.
The existing PipeWake::signal implementation's constants (word 6 and word 31)
match this image; owned-file signaling preserves group independence.

## Remaining validation and limits

- No device/emulator execution was done for this report.
- Prove that the actual installed Supervisor UI post runs on the matched
  MiWear default-loop owner; implement the identity check so mismatch fails.
- Exercise initial capture, repeated capture, independent close, Session
  recreation, and operation from QuickApp and Bluetooth tasks without touching
  the wrong numeric descriptors. Test duplicate failure cleanup explicitly.
- Owner callback progress during screen-off remains an operational question;
  it is not a reason to use an unpinned foreign filelist pointer.
- General arbitrary foreign-group lifetime retention is unresolved; no group
  pin primitive was approved or guessed. A naked filelist pointer/share-byte
  increment is insufficient evidence.
- These static findings do not authorize changing `SUPPORTED` alone; GAP and
  legacy JS lifecycle/runtime requirements are outside this report's scope.
