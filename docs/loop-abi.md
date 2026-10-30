# Default loop and task layout per target

`src/loop_dispatch.rs` has to reach the firmware's process-lifetime libuv loop
from the module's own thread, because the MiWear GAP client completes its
requests on that loop and the backend has no other wake path. There is no
exported getter for it, so the dispatch walks NuttX task structures instead:

```
TCB + TCB_TLS (100)                      -> TLS block
  TLS + 0                                -> task_info
    task_info + TASKINFO_UVSTATE (16) + TASKINFO_UVSTATE_STRIDE (4) * uv_key
                                         -> uv state
      uv_state + UVSTATE_LOOP (664)      -> the loop
        loop + LOOP_QUEUE_HEAD (68)      -> ready-work intrusive queue
        loop + LOOP_MUTEX (76)           -> its mutex
        loop + LOOP_ASYNC (104)          -> the loop's async handle
        loop + LOOP_READ_FD (312)        -> pipe read descriptor
        loop + LOOP_WRITE_FD (316)       -> pipe write descriptor

TCB + TCB_GROUP (8)                      -> task group
  group + GROUP_FILELIST (96)            -> file list
    filelist[(fd / FILELIST_ROW)]        -> 32-byte file record
      file + FILE_INODE (16)             -> inode
        inode + 16                       -> i_ops, compared with INODE_OPS_PIPE
```

`uv_key` comes from the `uv_loop_key` global, which is recorded per target.
`ASYNC_BUSY_WORD` (6) and `ASYNC_PENDING_WORD` (15) are the `u32` words of the
`uv_async_t` that the dispatch arms and reads back, so they scale with the
handle size rather than the loop.

## Offsets and identities

Everything except `INODE_OPS_PIPE` is identical across the Band 11 line. That is
a measurement, not an assumption: each value below was re-read from the target's
own image by finding the function that owns the field.

| Constant | .155 | .139 | .179 | Anchor |
| --- | --- | --- | --- | --- |
| `LOOP_QUEUE_HEAD` | 68 | 68 | 68 | `uv_loop_close` asserts an empty list at `loop+68` while holding `loop+76` |
| `LOOP_MUTEX` | 76 | 76 | 76 | same function: `uv_mutex_lock(loop+76)` |
| `LOOP_ASYNC` | 104 | 104 | 104 | the async the firmware workers signal |
| `LOOP_READ_FD` / `WRITE_FD` | 312 / 316 | 312 / 316 | 312 / 316 | `uv_loop_close` compares both with `-1` and closes them |
| `ASYNC_BUSY_WORD` / `PENDING_WORD` | 6 / 15 | 6 / 15 | 6 / 15 | `uv_async_start` clears `handle+24` and `handle+60` |
| `UVSTATE_LOOP` | 664 | 664 | 664 | `uv__loop_new` allocates 712 bytes; `uv_loop_close` compares slot 166 with the loop |
| `TASKINFO_UVSTATE` / `_STRIDE` | 16 / 4 | 16 / 4 | 16 / 4 | `uv_key_get/set` index `task_info + 4*(key+4)` |
| `UV_KEY_LIMIT` | 3 | 3 | 3 | `uv_key_get/set` reject `key > 3` |
| `TCB_TLS` | 100 | 100 | 100 | the `sched_get_stackinfo` accessor reads `tcb+100`, and TLS+0 is the task info |
| `TCB_GROUP` / `GROUP_FILELIST` | 8 / 96 | 8 / 96 | 8 / 96 | `fs_getfilep` walks `tcb+8` then the group's `+96` list |
| `FILELIST_ROW` / `FILE_INODE` | 8 / 16 | 8 / 16 | 8 / 16 | `files_getfilep` uses `/8` and `&7` over 32-byte file rows, inode at `file+16` |
| `INODE_OPS_PIPE` | `0x2cb04ffc` | **`0x2cb0500c`** | **`0x2cb02e4c`** | `nx_pipe` hands this table to `register_inode`; the ten words are the same pipe handlers on all three |

The loop's pipe identity is the one value that moves: `.139` is `+0x10` from
`.155`, `.179` is `-0x21b0`. The table behind it is a copy of the same
`file_operations` handlers, so a pointer from another target makes
`capture_default_loop` reject every candidate pair. The symptom is precise:
`capture_default_loop` returns `-9`, the Feature reports `BACKEND (-9)` at the
first operation that needs a loop wake (a scan start), and nothing else in the
module misbehaves.

## A sibling from another line is a different layout

`p67` (Band 10 Pro) keeps the same `best1503` SoC but is a different board build,
and both the loop and the way it is reached differ:

```
tcb + 100                        -> task info
  *(task_info + 16 + 4*key)      -> 808-byte TLS block   (key = *(0x201033AC), limit 7)
    TLS block + 760              -> the default loop     (embedded at TLS block + 136)
```

- The loop is not in a `uv_key`-indexed state array; libuv keeps its state in a
  pthread key, and the loop is an embedded struct inside that state.
- `uv__get_loop` (`0xC2DE814`) is a real function on that build. A p67 target
  should call it instead of walking NuttX structures — a code change in
  `loop_dispatch`, to be made deliberately rather than by bending offsets.
- The loop's pipe descriptors are at `+360/+364`, created lazily by
  `uv_async_init` behind a `loop+360 == -1` guard; `+312/+316` is a plain list
  on that build.
- The async handle's pending word is 27 (`+108`), not 15 (`+60`).
- The uv key array allows 8 slots, not 4.

So the offset table above is a **Band 11** result. Re-derive it for any other
board path rather than assuming the SoC implies the layout, and check the libuv
source list first: a different file set is enough to know the layout moved.

## Porting the next target

The values are cheap to get right and expensive to guess, so recover them in
this order:

1. `uv_loop_close` — it names four constants at once (queue head, mutex, both
   fds) and is recognisable by its two `-1` comparisons plus the close calls.
2. `uv__loop_new` — size and the slot that stores the loop pointer.
3. `uv_async_start` / `uv_async_init` — the two `u32` words it clears.
4. `uv_key_get/set` — the task-info array and the key limit.
5. The TLS accessor in `sched_get_stackinfo.c`, then `fs_getfilep` and
   `files_getfilep` for the task-group filelist walk.
6. `nx_pipe` — anchored by its `/var/pipe/%d` name format and the 2048-byte
   FIFO allocation, and by the `register_inode` call it makes.

Record `INODE_OPS_PIPE` and the `uv_loop_key` global in the target pack, then
reference them from `src/abi/<target>.rs` instead of a literal. Offsets stay in
the abi module; the pack is for addresses that a verifier can check.
