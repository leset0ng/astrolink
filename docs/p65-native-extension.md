# Redmi Watch 6 P65 native extension and LE CoC investigation

Scope: exact firmware `3.100.043`, AP
`analysis-p65-043/vela_ap.bin`, 11,793,540 bytes, SHA-256
`a19b601477569765103eaf468b937bd8026b4d68ffa5c896bde222a79e475858`, XIP
`0x0C0C0000`. This is static code evidence from the matching IDB, with
instruction-level checks. It is not a completed ABI or runtime validation.
Descriptive names below are assigned by the analysis; the firmware does not
export them. All addresses need individual pack evidence before use.

The old `feature_runtime_absent` finding is not an adequate blocker. P65
contains an older per-context native module route and a complete ordinary LE
CoC implementation, but it does not have the newer Feature lifecycle used by
Q66/P67. It needs a distinct legacy integration.

## Per-application native extension route

`0x0C9FBA60` allocates the 0x610-byte RuntimeContext and creates its JS
runtime/context. `0x0C9FB8C4` creates `@aiot/core` with
`0x0CA32840(ctx, name, init_callback=0x0C9F986C)`. Its callback runs the
built-in initializers then extension dispatch. Native exports are attached
through `0x0C71321C` and `0x0C7132D8`. The extension export runs before
`0x0C9D24E4` evaluates the built-in Feature bytecode and before application
source evaluation.

The RAM extension list sentinel is `0x3C3CF238`, initially self-linked:

| Field | Meaning |
| --- | --- |
| `+4` | next node |
| `+8` | `init(ctx, module)` at dispatcher `0x0C9FB768` |
| `+12` | `export(ctx, module)` at dispatcher `0x0C9FB7C4` |
| `+16` | inline extension name used for logs |

This is an actual per-context native extension path, not just leftover JS
strings. A resident node must use an audited append/link mechanism and
synchronization. The module object lifetime, per-context owner cleanup and
custom manifest import mapping remain open. A node alone is not proof that
`@app-module/canopus.astrolink` satisfies the new `$app_require$` behavior.

`0x0C9FBE10(runtime)` drains the loop then releases runtime and JS resources;
the owning application calls it at `0x0C9FCD48` and clears Application +296.
This is the legacy owner-withdrawal checkpoint to use when designing cleanup.
P67 FeaturePrototype/FeatureInstance pointers cannot be substituted. The QJS
value/binary-buffer helpers, application loop and cross-task pipe ownership
still need exact-image measurements.

## Scanning and advertising

The stock JS BLE advertising wrapper `0x0C9CEBA0` builds a 28-byte parameter
block and obtains the adapter via `0x0CA3F474`. It reaches generic producer
`0x1C00A908(adapter, params, data, len, response, response_len, callbacks)`,
which copies the configuration/payloads and issues command **67**. The service
reply path `0x1C059990` installs a 12-byte callback table and queues
`0x1C05979C`; stop `0x1C059A88` releases the matching advertising set. The
parameter byte meanings and handle lifetime still need verification.

The generic scan paths are:

- `0x1C00AA40(adapter, settings8, callbacks)` sends command **212**.
- `0x1C00AAA4(adapter, settings8, filters16, callbacks)` sends **213**.
- Stop `0x1C00B39C(adapter, phy)` sends **238**.

Report consumer `0x0CB926BC` reads address +0..5, signed RSSI +7, address
type +8, packet type +9, data length +10 and payload **+11**. P67's payload
offset +12 is wrong for this image. The report/callback ownership, exact
callback size and client-instance/adapter-ready contracts are not yet
measured. These entries show the host exists; they do not yet make a usable
Astrolink GAP backend.

## Ordinary LE CoC

The L2CAP owner is loaded from global `0x3C492798`. The public wrappers use
20-byte work records: owner +0, argument +4, handler +8, cancel +12 and flag
+16. Work must run on the Bluetooth owner task.

| Operation | Public wrapper | Owner handler |
| --- | --- | --- |
| Connect | `0x0C15AE48` | `0x1C067018` |
| PSM register | `0x0C15ADBC` | `0x1C065DA0` |
| PSM unregister | `0x0C15AE00` | `0x1C0686D4` |
| Accept | `0x0C15AEA0` | `0x1C067ED8` |
| Configuration | `0x0C15AEF8` | `0x1C068EF8` |
| Disconnect/reject | `0x0C15AE74` | `0x1C0684F8` |
| Original-SDU send | `0x0C15AF24` | `0x1C067408` |

Type 3 registration allocates LE PSM `0x80..0xff`. Channels are 124 bytes,
with callback +20, PSM +112, CID +116, peer CID +118 and state +120; LE CIDs
avoid `<=0x3f` and `>0x7f`. Connect reads PSM +2, LE flag +8, callback +12,
peer +16 and local configuration +24. Incoming requests are 68 bytes, with
MTU +52, credits +60 and MPS +62. Accept uses status/detail +4/+6 and local
configuration +24. These values are independently measured here, not copied
from P67, and still require full backend mapping.

The send handler fragments into eight-byte list nodes, prefixes the first
fragment with a two-byte SDU length, and drains credits through
`0x1C067374`. Its explicit empty-payload branch returns without emitting a
zero-length SDU. A correct backend must construct the zero-length prefix
separately, just as on P67; ordinary submit alone silently drops it. Receive
reassembly, allocation/free and credit ownership remain to be measured.

### Existing pack defect

The current `bt_l2cap_connect 0x1C06B134` record actually enqueues the
disconnect/reject handler `0x1C0684F8`. It must not be used as connect. The
XIP connect wrapper candidate above is separate. `bt_l2cap_submit_cid
0x0C15AF24` agrees with the local stock send path but remains unapproved;
the separate disconnect record was not shown to be the required API.

## Correct next status and work

The correct status is **host capabilities found; legacy ABI and lifecycle
incomplete**, not `feature_runtime_absent`. The remaining sequence is:

1. Extension-node linkage, custom import compatibility and owner teardown.
2. QJS values/buffers, app loop and cross-task wake.
3. GAP parameters/report/stop lifetime and adapter state.
4. CoC receive, credit/disconnect and zero-SDU ownership.
5. Exact target signatures, then isolated lifecycle and bidirectional CoC
   tests. None of these steps has been replaced by this static investigation.

## Static integration update

Two exact-image follow-up recoveries closed most scalar gaps without changing
the runtime support gate:

- Kernel/VFS: retained 32-byte files, three-argument `file_dup3`, retained
  get/put calls, 128-byte async handles, busy word 6 and pending word 31. Loop
  queue/mutex/async offsets are 64/72/96; pipe descriptors are 360/364.
  `GROUP_FILELIST=88`, `UVSTATE_LOOP=768`, `D_UV_KEY=0x3C44C388` and
  `INODE_OPS_PIPE=0x3C169F7C` are now evidenced.
- GAP: adapter register/get-state/enable are `0x1C008608`, `0x1C008720` and
  `0x1C0086D0`; ready state is 4 and the business loop is client+52. Scan and
  advertising stop use their actual wrappers at `0x1C00AB28` and
  `0x1C00A9AC`, not the adapter-state producer previously suspected.
- Ordinary LE CoC: open=3, data=7, local-flow=8 and credits=17. Complete,
  receive and credit layouts, the channel finder and the SDU metadata/tail
  are recovered. The target shim constructs empty SDUs with an explicit
  first+last fragment because the stock sender drops them.

`src/abi/redmi_watch_6_3_100_043.rs` now carries these constants, four exact
entry signatures and the checked empty-SDU implementation. The retained-file
protocol is deliberately different from N66's borrowed 24-byte slots.

The remaining GAP and loop questions were resolved by the exact-image
integration audit below. Full earlier reports were preserved at
`/tmp/watch6-gap-coc-static.md` and `/tmp/watch6-libuv-static.md`; the new
self-contained reports are retained in this repository.

## Integration recovery update

The exact-image producer/consumer reports are now retained as:

- `docs/evidence/p65-gap-contract.md`: advertising enum, scan filtering,
  callbacks, adapter state and client-wrapper lifetimes.
- `docs/evidence/p65-loop-ownership.md`: default-loop identity, file-table
  lifetime, retained files and owner-task capture.
- `docs/evidence/p65-coc-events.md`: CoC event payloads, allocation ownership
  and empty-SDU metadata.

Advertising public mode5 is independently traced to properties0xD3 and legacy
HCI ADV_IND type0. The existing interval320/power0xF6 block is correct.
Filter16 is a UUID16 service-data filter with its enable byte at+12; tail
bytes+14/+15 are unused in the audited manager path. Astrolink keeps command212
and filters its UUID128 identity in the module. The native scanner has only two
registrations, shared with stock firmware consumers; four module Sessions do
not imply four simultaneous native scans.

The global GAP client's business loop is the MiWear task's TLS default loop:
`0x0C92CE28 -> 0x0C62B4EC -> 0x3C479C68 -> 0x0CA3F3D8 -> 0x1C001C10 -> client+52`.
It is distinct from the client's response loop at+48. P65 has no filelist mutex
at the N66 offset. Retained file-slot references survive ordinary close but do
not keep a foreign group's rows alive during final group teardown.

P65 therefore prepares its GAP wake endpoint only after an actual GAP request,
through the existing Supervisor UI-post service. The callback requires the
current task's TLS default loop to equal client+52, then retains both pipe
endpoints, validates flags/shared inode/pipe operations, duplicates the writer
into independent storage and releases both slot references. Deferred operations
wait for this asynchronous preparation. A mismatched owner fails rather than
using that task's unrelated numeric descriptors. QuickApp notification pipes
also use the verified current-owner pair capture. No sleeping duplication runs
inside a scheduler walk, and no foreign file-table pointer escapes one.
On P65 duplication failure, clear the partially written destination without
closing it: the native error path has already dropped its added inode reference.
A later attempt therefore starts from zeroed, unowned storage.

Advertising failed-start callbacks now reclaim the terminal 12-byte client
wrapper after forwarding the result. Exact thunks `0x1C166660` and `0x1C168AF0`
use the same `F_ALLOC`/`F_FREE` heap. Resource failures never link an advertiser;
timeout status2 unlinks it before calling started. Native stopped delivery
requires a live list entry, so failed-start cleanup cannot race a subsequent
native stopped free through these paths. Successful-start and stopped ownership
remain native.

The shared backend now handles P65 OPEN3, DATA7, LOCAL_FLOW8 and CREDITS17;
its old P65 exclusions and duplicate receive arm are removed. OPEN uses CID+108
and remote receive MTU+72. CREDITS uses CID+0/added+2 after native accounting and
queue drain. LOCAL_FLOW is ignored and freed, never treated as send completion.
All callback payloads use the measured allocation/free family. Empty SDUs retain
the existing explicit two-byte-zero fragment, with metadata1 as measured in the
stock fragment producer; nonempty original SDUs keep metadata3.

The new current-task global is recorded with static evidence in
`EVID-ASTROLINK-P65-INTEGRATION-004` in the P65 target pack and generated bindings.
It remains restricted/PENDING. The isolated v45 experiment passed real
installation and activation (version45/state13/error0), but probe startup hit
INVSTATE. Independent fault capture proved that the copied Application vtable
was missing primary slot27: the 29-word copy included two prefix words and
only slots0..26, while firmware 0x0C9FD416 reads vptr+0x6C before a possible
BLX R3 at 0x0C9FD558. The saved target and PC were zero.

The P65 copy is now 30 words, retaining slot27. The fresh v46 runtime regression
reads the original 0x0C9FC601 from the copied table and reaches JavaScript without
the original startup fault. Its next failure was native-interface loading.

Follow-up runtime/static correlation found two wrong generated bindings:
property-definition0x0C6D3320 was a global-constructor installer, corrected to
0x0C6D3210; Bluelet queue0x1C1676C8 was owner-local insertion without idle wake,
corrected to the synchronized external queue0x1C167D40. The P65 ArrayBuffer
payload pointer is object+40, not+8. The SDK also now reads the unshadowed
legacy global because the toolkit supplies a separate globalThis wrapper.

With those corrections, fresh v49 activation reaches state13/error0 and the
formal source lifecycle probe passes18/18, including scan/advertising and two
Session recreation/release rounds. Bidirectional CoC and physical-radio
acceptance remain pending; `SUPPORTED` remains false. See
`docs/p65-native-publication.md` and `docs/evidence/p65-v49-validation.json`.
The v45/v46 reports preserve their respective failure and regression evidence.
