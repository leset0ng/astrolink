# Q66 `.155` BLE backend ABI

This record describes the Q66 `.155` values of `src/abi/xiaomi_band_11_4_100_155.rs`
and the target-independent logic in `src/backend/bluelet.rs`. Callable addresses
are taken from the target pack and must stay in sync with
`targets/xiaomi-band-11-4.100.155/symbols`. The source of
truth is the unpacked Q66 AP, build `user-4.100.155-cn-202609041500`, SHA-256
`ea0bdf1920cb30223d616432af00565ca67622e6468328f5eab155f8cdc2fb9f`.
The database used was
`/Users/lesetong/develop/mifw/analysis-q66-177/155/q66-ap.i64`.
Addresses below are even instruction VAs; ARM function pointers set bit 0.
All pointer and integer words in this ABI are 32-bit little endian.

**Validation status:** the functions and layouts below were recovered from
instruction operands, producers, consumers and stock callers. The Rust backend
was cross-compiled for `thumbv8m.main-none-eabi`. This does not constitute
physical-device radio, concurrency, power-management or BluetoothAudio coexistence
validation. The module must be resident after publishing these callback pointers.
No P67 or other Q66 firmware addresses are inferred from this record.

## Runtime boundary

`backend::install(EventSink)` installs a resident Rust event consumer.
`backend::acquire(SessionId)` allocates one of four per-session callback slots.
The `Service` token is opaque and contains its originating session generation.
The exported functions are listed in `src/backend/mod.rs`; they do not expose
firmware allocation pointers to the core or JavaScript.

GAP methods copy their inputs and post work to the process-lifetime Bluetooth
business loop (`client+56`) through the existing lock-protected libuv completion
queue. They must not run on the client response loop (`client+52`), which must
remain available to complete synchronous IPC. A QuickApp task also cannot use
the Bluetooth task group's numeric socket descriptor directly. CoC methods copy
their inputs and enqueue work onto the existing Bluelet owner. An `Ok(())`
means the work was queued; the resulting callback/event carries the actual
operation result.

`EventSink` is called from firmware tasks. It must copy borrowed data before
returning and must not call QuickJS or synchronous GAP IPC. The module runtime
serializes its Engine and posts to the correct QuickJS owner separately.
Every outward channel event contains the original `LinkId`, so a delayed event
is never routed to a new link merely because its CID has been reused. Inbound
requests receive a separate `IncomingId`; accepting an old token fails.

## Task dispatch and allocation

| VA | Recovered signature/operation | Evidence |
| --- | --- | --- |
| `0x0C351724` | `void *alloc(u32 size)` | Same allocator used by every request and event producer. |
| `0x0C34CD2C` | `void free(void *)` | Stock send, accept, disconnect and IPC dispatch free paths. |
| `0x0C863BB4` | `enqueue(owner,run,cancel,arg,event:u8)` | Locks semaphore at `0x200BE8F4`, appends a 20-byte work entry to a 408-byte ring page, wakes semaphore `0x200BE8E8`. |
| `0x0C470ED8` | Bluelet thread entry | Removes queued work and calls `run(owner,event,arg)`; calls cancellation on shutdown. |
| `0x200BE890` | L2CAP owner pointer | All stock L2CAP queue wrappers load this owner. |
| `0x0C3F4CC4` | `uv__work_submit(loop,work20,kind,worker,done)` | Original pool entry; its worker wake resolves an fd in the worker's task group and is not used by this backend. |
| `0x0C355564` | `nxsched_foreach(callback(tcb,arg),arg)` | Enumerates TCBs under scheduler/BASEPRI protection; callback return is ignored. |
| `0x0C332BA2` | `files_getfilep(filelist,fd) -> file` | Pins the selected 32-byte file; paired with `fs_putfilep` at `0x0C334998`. |

The work prefix is 20 bytes: worker, done, loop, queue next/previous. GAP posts
ready work directly to `loop+68` under its `loop+76` mutex, then signals the
existing async at `loop+104`. Firmware worker completion `0x0C3F7FA8` uses the
same queue, lock and wake. The target drain `0x0C3F7F14` detaches under that lock,
unlinks work before calling done and never reads it after that callback, allowing
the backend to free its containing work there. No public `uv_queue_work` request
accounting, foreign-thread `uv_async_init`, or new async handle is involved.

`src/loop_dispatch.rs` retains the default loop's actual pipe rather than using
its integer write fd in the caller's task group. It identifies the owning task
by `TCB+100 -> TLS+0 -> taskinfo+16+4*uv_key -> uvstate+664`, where `uv_key` is
read from `0x200BD444`. The matched task's `TCB+8` group embeds its filelist at
`+96`; rows are eight entries. Both `loop+312/+316` endpoints are pinned while
inside the scheduler critical section. Outside that section they must share the
same pipe inode with that target's pipe-operations table and correct
read/write/nonblocking flags. The identity is per target (`0x2CB04FFC` on
`.155`, `0x2CB0500C` on `.139`, `0x2CB02E4C` on `.179`): the handlers are the
same, the address is not, and a foreign value rejects every pair. The full
offset table and how each value was recovered is in `docs/loop-abi.md`.

The write file is duplicated independently before both pins are released.
Only the process-lifetime firmware business loop uses this cross-task lookup;
QJS captures its module async's pipe from its own task and releases it in the
`uv_close` callback. Neither path caches a PID or borrows another task's fd.

`enqueue` calls `cancel(0,221,arg)` synchronously if its queue is unavailable.
Its return is a semaphore operation's result, not an acceptance status. The
backend never interprets it as permission to free a submitted argument.
The backend copies commands into its own allocation, counts work before
submission, and rolls the count back on allocation failure. A queued callback
or cancellation owns and releases that allocation exactly once.

`0x0C863310` is the **owner-only** intrusive work-list helper used by the public
CoC wrappers. It does not provide the semaphore protection in `0x0C863BB4`.
The backend first uses `0x0C863BB4`, then invokes the corresponding recovered
handler directly on the owner task. This avoids calling the intrusive helper
from a foreign thread and permits exact binding of event 2 to a pending LinkId.

The module caps outstanding work at 12 per session and allows at most one
waiting transmission per channel. Session state uses atomics for cross-task
resource accounting; channel tables are accessed only by Bluelet. A callback
holds a work reference until its final event has been copied, preventing a
concurrent close from retiring/reusing its session slot halfway through it.

## GAP client API

The backend uses the process-lifetime MiWear Bluetooth IPC instance at
`0x200DA4F8`. Stock scan and advertising callers, including
`0x0C7F92FC`, `0x0C7EF964`, and payment/unlock callers, use this instance.
It adds a module-owned adapter callback registration without replacing the
stock callback table. `0x0C46A44C(instance)` returns adapter state, with 4 the
ready state. If state is OFF, the existing enable IPC is requested and the
original start operation remains pending while the adapter starts.

| VA | Exact caller ABI |
| --- | --- |
| `0x0C46B68E` | `scan_create(instance,params5,optional_filter16,callbacks16) -> handle` |
| `0x0C46B730` | `scan_stop(instance,handle) -> ipc_status` |
| `0x0C46B5A0` | `adv_create(instance,params28,adv_data,len:u32,scan_response,len:u16,callbacks12) -> handle` |
| `0x0C46B664` | `adv_stop(instance,handle) -> ipc_status` |
| `0x0C46A218` | Synchronous 708-byte IPC exchange; locks `instance+8` and awaits response. |

The scan create wrapper allocates a 616-byte client handle and stores its
callback descriptor at handle+12. Its five parameter bytes are, as recovered
from `0x0C478B28` and scanner consumer `0x0C4679FC`:

| Byte | Meaning | Backend value |
| --- | --- | --- |
| 0 | Scan mode (CLI accepts 0..2) | 2, same as MiWear scan |
| 1 | Legacy mode | 0 |
| 2 | Active/passive type | 1, active |
| 3 | PHY | 0, 1M |
| 4 | Filter policy | 0, no whitelist |

The scan callback descriptor is exactly four words:

```c
struct ScanCallbacks {
    uint32_t size;                       // 16, from stock tables
    void (*result)(void *handle, const uint8_t *report);
    void (*started)(void *handle, uint8_t status);
    void (*stopped)(void *handle);
};
```

`0x0C464262` calls these fields and frees its temporary report after `result`
returns. For event 359 it constructs report+12 from IPC data; the alternate
batched path builds the same layout on its own stack. Reports are borrowed:

| Report offset | Field |
| --- | --- |
| 0..5 | Address, stock byte order |
| 6 | Additional stack address/identity classification; not used |
| 7 | Signed RSSI |
| 8 | Address type |
| 9 | Packet type |
| 10 | Data length in bytes |
| 12.. | Complete advertising/report bytes |

The reported length is checked against 31 before constructing a Rust slice.
The protocol decoder ignores nonmatching or incomplete AD structures.

The advertising callback descriptor has reserved word 0 followed by
`started(handle,advertising_id:u8,status:u8)` and
`stopped(handle,advertising_id:u8)`. Stock descriptors at `0x2009BE4C` and
`0x200839F4` and dispatcher `0x0C463FA2` confirm its 12-byte shape. For event 280
that dispatcher passes packet+84 as the second argument. Its producer
`0x0C4696C0` handles the actual stopped-state notification (`state == 1`), removes
the advertiser from the service list, and calls the stopped hook with the ID
from notification+4. This is the same ID passed to started, not an error status.
The module completes stop only upon this callback; synchronous IPC failures
still emit the original backend error. The dispatcher frees the client
advertising handle after a failed start or a stopped callback; the backend must
not free it. Similarly, `0x0C464262` frees the scan handle after failure/stop.

Advertising parameters are 28 bytes. `0x0C4784E8` creates this layout and
`0x0C46938C` consumes it:

| Parameter offset | Field | Backend setting |
| --- | --- | --- |
| 0..5 | Directed peer address | Zero |
| 6 | Advertising mode/type | 5: legacy `ADV_IND` |
| 7 | Directed peer address type | Zero |
| 8..13 | Explicit local address | Zero |
| 14 | Local address type | 0, public |
| 15 | Signed transmit power | -10 dBm, stock unlock/payment policy |
| 16..19 | Interval in 0.625 ms units | 320 (200 ms) |
| 20..23 | Duration | Zero (until explicit stop) |
| 24 | Channel selection | Zero, stack default all primary channels |
| 25 | Filter policy | Zero |
| 26..27 | Reserved | Zero |

The create wrappers copy advertising data and scan response **unchanged**.
`0x0C46938C` again copies the supplied buffers into the stack request; it does
not insert a Flags AD structure. The CLI at `0x0C4784E8` explicitly constructs
its own `02 01 0A` flags. Consequently `protocol::Advertisement::encode` includes
its own flags. Backend submission accepts at most 31 bytes and uses no scan
response, so application data is delivered from one complete advertisement.

The frozen wire layout (frame version 2) is Flags (3 bytes) followed by one
Service Data AD (AD length/type plus 16 UUID bytes) carrying the derived
identity UUID, then version/SPSM and up to 8 bytes of user data. The service
name is never transmitted; see `docs/wire-format.md`. It requires no invented
manufacturer company identifier and no assigned 16-bit UUID.

## Legacy LE CoC request and events

| Wrapper VA | Owner handler VA | Operation |
| --- | --- | --- |
| `0x0C86EF90` | `0x0C86A268` | Register dynamic PSM |
| `0x0C86EFCC` | `0x0C872E30` | Unregister PSM |
| `0x0C86F010` | `0x0C871558` | Connect |
| `0x0C86F038` | `0x0C873020` | Accept/reject |
| `0x0C86F094` | `0x0C872EBC` | Disconnect |
| `0x0C86F0BC` | `0x0C86E7F8` | Submit SDU |

`0x0C869884(u16 psm,u8 transport)` finds a registry entry. Transport 3 selects
dynamic LE registration (`0x80..0xff`); transport 2 selects dynamic Classic.
The registry entry contains callback32+0, PSM16+4, transport8+6 and has an
8-byte intrusive list header immediately before it. Register retains that
entry; event 9 is a separate 8-byte copy owned by the callback. Dynamic register
loops until it finds a free value and would wrap forever if all values were
occupied. The backend preflights a bounded `0x80..0xff` search on the same owner
using **both** lookup arguments and returns BUSY if full.

Connect and accept use 68-byte zero-initialized requests:

| Offset | Meaning / evidence |
| --- | --- |
| 0 | Local CID, filled by connect handler before event 2; supplied for accept |
| 2 | SPSM for connect / incoming request |
| 4 | Accept status: 0 success, 4 resource rejection |
| 6 | Accept status detail, zero |
| 8 | Connect flags bit 0: LE when 1 |
| 12 | Channel callback function pointer |
| 16..21 | Peer address |
| 22 | Peer address type |
| 23 | Internal transport, filled by connect handler |
| 24..67 | Configuration area |
| 52 | Local receive MTU, `configuration+28` |
| 54 | Classic option flags; left zero for LE |
| 60 | Initial receive credits, `configuration+36` |
| 62 | Local receive MPS, `configuration+38` |

`0x0C86A2F4(channel,request+24)` consumes these configuration offsets. Its
LE branch always reads MTU+28/credits+36/MPS+38 rather than using the Classic
option mask. The backend uses MTU 512, MPS 64, initial credits 5. These are module
buffer policies within the proven legacy mode, not measured controller maxima.

`0x0C871558` allocates a CID, copies the address to its stack, and synchronously
calls the callback with event 2 and the original request. The callback owns that
request; the handler does not subsequently dereference it. The backend records
`connecting: LinkId` while invoking this owner handler; event 2 binds its new
CID to that generation before any ready/data events can arrive.

Inbound legacy code `0x14` at `0x0C872F2C` looks up transport 3, validates
SCID `0x40..0x7f`, and records peer receive MTU/MPS/credits. `0x0C86E1DC` creates
a 68-byte event 1 and transfers it to the registered callback. This event does
not contain peer receive MTU; accept supplies our own receive limit independently.
`0x0C873020` consumes and frees the accept request itself. Its response builder
`0x0C872CC2` emits LE code `0x15` on signaling CID 5. There is no guessed Classic
connect struct in this device path.

| Event | Allocation / ownership | Fields used |
| --- | --- | --- |
| 1 | Request 68, callback owns/free | CID+0,PSM+2,address+16,type+22 |
| 2 | Original connect request 68, callback owns/free | CID+0; generation bound synchronously |
| 3 | Separate completion 120, callback owns/free | Peer receive MTU+72,CID+108 |
| 6 | Separate disconnect 12, callback owns/free | CID+0,reason+2,address+4 |
| 7 | Stock buffer, callback owns/free | End16+0,offset16+2,CID16+4,data=`base+4+offset` |
| 9 | Registry copy 8, callback owns/free | Allocated PSM16+4 |
| 17 | Credit update 4, callback owns/free | CID16+0,added16+2 |

`0x0C86EAFC` creates completion 120, copies local channel config+24 to event+0
and remote config+68 to event+44, then address+100, CID+108,remoteCID+110,
PSM+112,outbound+116. Thus remote receive MTU is event+72, not event+28. The local
receive limit is never inferred from the peer's MTU.

`0x0C872A94` emits event 6 and frees the channel's allocation **after** the
callback. It releases host resources through stock link ownership operations.
The backend does not send an ACL disconnect, power down the adapter, clear bonds,
or touch companion/audio connection tables.

Unregister transport 3 deliberately disconnects inbound channels of that PSM
(`0x0C872E30`). Therefore `stop_advertising` only stops our advertising handle;
the core rejects new inbound requests while preserving existing links. Full
Session close unregisters and closes the module's channels. This distinction
is required to meet “stop advertising leaves established links usable.”

## LE ACL connection path

An explicit parallel LE IPC connection request would duplicate work already
performed by the stock CoC host. The recovered path is:

```text
connect owner 0x0C871558
  -> link find/create 0x0C871500 / 0x0C8714C4
  -> host acquire 0x0C86D7C0(address,0x4002) when no existing ACL
     -> peer record 0x0C868B68(address,address_type,LE)
     -> host link 0x0C86663C
     -> LE create 0x0C86710C
```

`0x0C86710C` constructs legacy LE Create Connection with scan interval/window
48, default connection interval 24..40, supervision timeout at least 500,
and uses the peer address/type; its alternate branch constructs Extended LE
Create Connection. A previously connected host link goes straight to CoC open
via `0x0C86E310`. This uses the shared stock ACL ownership rather than adding
another controller host or disconnecting unrelated ACLs.

## Send buffers, credits, receive and empty SDUs

A stock send buffer has `end:u16` at +0 and `offset:u16` at +2. Payload length
is `end-offset`, payload begins `base+4+offset`. A buffer allocated with
payload length N and headroom H has `offset=H+4`, `end=N+offset`, allocation
`end+6` bytes, and metadata byte+8=3 when offset>7. This is exactly the formula
in `0x0C862EC2`. The backend uses headroom 12 and places local CID at +4.

`0x0C86E7F8` consumes that buffer, fragments by peer MPS (channel+106), queues
stock fragment nodes at channel+4, frees the original SDU, and calls stock drain
`0x0C86B40E`. That drain decrements peer credit queue+14 for each emitted PDU.
Each fragment node is a normal 8-byte list payload preceded by 8-byte list
header; node+0 points to the buffer and node+4 has first/last flags. The stock
node allocator clears the header and payload before use. Explicit empty-SDU
nodes must do the same: queue insertion preserves those links, and queue pop
uses `next` as an address. An uninitialized heap header can therefore cause an
invalid write and reboot.
`0x0C862C8E(list,node)` appends the node and `0x0C86B40E(queue,channel)` handles
framing, credits and transmission.

One firmware edge case matters: `0x0C86E7F8` only enters its fragment loop while
SDU length is nonzero. Passing an empty SDU would silently free it without
transmission. The backend instead creates the exact same stock first+last
fragment (`flags=0x81`, two-byte SDU length 0) and appends it to the **same**
credit-controlled queue. It uses no second HCI path or application envelope.
The receiver at `0x0C873794` recognizes length 0 plus its two-byte SDU length
field and emits a zero-byte event 7 normally. On Band 11 `.179`, the corresponding
queue primitives are push `0x0C8610E6` and pop `0x0C8611D4`; the latter reads
`node-8` and writes `next-4` when it is nonzero. Run the exact-firmware
poisoned/cleared-node regression with `python3 scripts/test-empty-sdu-queue.py
--image PATH/vela_ap.bin`; it requires `pip install unicorn`.

The stack fragment queue is not intrinsically bounded. Before handing over a
new SDU, the backend checks that the channel's fragment queue is empty and that
credits are nonzero. If not, it retains one counted command for that channel;
further pending submissions return BUSY. Event 17 resumes this command after
stock credit handling has drained its older fragments. Each handed-over SDU is
at most 512 bytes. `Submitted` means stack responsibility has transferred, not
remote JS receipt. Disconnect cancels and frees a retained command.

`0x0C873794` reassembles complete SDUs and emits event 7 with the CID in the low
word at buffer+4. It automatically adds five receive credits when the remaining
count is at most ten. Holding a JS queue therefore does not throttle reception.
The backend validates completed length before constructing its borrowed slice;
the core's per-link queue enforces the application memory limit and closes on
overflow. The firmware owns its fragment and reassembly machinery throughout.

## Close, late callbacks and integration requirements

`close(Service)` sets closing before beginning GAP stop and queued CoC teardown.
If scan/advertising creation is still in flight it returns BUSY for that stop;
its eventual started event still contains the original SessionId. The runtime
must retry close from its native timer after owner destruction as well as from
normal event draining. There are no synchronous IPC calls in firmware callbacks.
A failed stop retains its handle so a later close can retry.

A callback slot becomes vacant only after its PSM is removed, scan/advertising
stop callbacks have cleared their handles, channels are closed, counted work
and waiting sends are gone, and the last callback returns. Handle checks discard
report/stop callbacks for a handle no longer owned by that slot. `ServiceClosed`
contains the retired SessionId. Queued core events always retain generation IDs.

The runtime must mark a transmission in flight after `backend::send` returns
Ok and avoid submitting its same core queue head again before `Submitted` or
`Failed`. It must not retry an already accepted SDU merely because it has no
remote application ACK. Backend failures preserve original codes in
`Error::Backend(i32)`.

`backend::supported()` requires the loader-selected exact target ID and compares
12 bytes at scan create, advertising create, owner enqueue and connect handler
against the audited image. This protects against accidental wrong-image calls;
it is not a substitute for full package/firmware identity enforcement.

## Reproducible compile check

The implementation was checked with an isolated no_std harness containing the
real core, protocol and backend modules and the command:

```sh
CANOPUS_TARGET_ID=xiaomi-band-11-4.100.155 \
  rustc --edition=2021 --crate-type=rlib \
  --target thumbv8m.main-none-eabi --cfg 'feature="device"' \
  /tmp/astrolink-ble-check.rs -o /tmp/astrolink-ble-check.rlib
```

The complete module/package and QuickApp integration are verified separately.
IDA function names prefixed `astrolink_audit_` and ownership comments were saved
back to the `.155` database for the recovered entry points above.
