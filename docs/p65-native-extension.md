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
reply path `0x0C059990` installs a 12-byte callback table and queues
`0x0C05979C`; stop `0x0C059A88` releases the matching advertising set. The
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

The remaining blockers are narrower but still substantive: the 28-byte
advertising enum/connectable combination, filter+14/+15 semantics, and the
custom GAP loop's owner/group lifetime. A custom loop is not necessarily the
TLS default loop, and a retained file does not pin a foreign group's file
rows. Therefore `SUPPORTED` remains false and no installation, lifecycle,
HELLO or SDU runtime claim is made. Full reports are preserved at
`/tmp/watch6-gap-coc-static.md` and `/tmp/watch6-libuv-static.md`.
