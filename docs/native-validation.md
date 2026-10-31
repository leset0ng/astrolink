# Native integration validation, 2026-10-30

This is simulator evidence for Q66 `4.100.155`, not physical radio validation.
The isolated Mirage device is `astrolink-q66-155-e2e`, UUID
`AAF78EDF-4371-4E16-98D3-930F37172E1C`, library
`/tmp/astrolink-mirage-fresh`. Only the parent agent owns this device.

## Confirmed defects and repairs

- The copied client constructor attempted to copy a 13-byte socket path into
  a 14-byte slice, which panics. The copied constructor was subsequently removed;
  the firmware already owns a process-lifetime GAP client at `0x200DA4F8`.
- `cancel_work` used to inspect `work.action` after freeing `work`. Operation
  metadata is now read before release.
- `binary_bytes` used to dereference `payload+4` before checking whether the
  ArrayBuffer payload was null. It now rejects that state before dereferencing.
- A refused incoming CoC request with no LinkId could fail allocating its reject
  request and emit a session-level error without cleaning up the CID. It now
  disconnects the CID on the Bluelet owner rather than failing an unrelated
  advertising request.
- The old smoke harness accepted launch acknowledgement as success. `probe`
  now requires the native runner's terminal `complete` event and rejects its
  failure or failed cleanup events. The actual v31 failure log is a negative
  regression input for this parser.

## v31: shared client, GAP called from QuickApp task

ELF SHA-256: `c86bde79acd27d94f98c4603c3920c027cad203a9ba8b2f5d2b426470cbce8d6`.
The verified image has 48,907 resident bytes, no undefined symbols, 1,218
relocations, one constructor and one destructor. The package is 96,256 bytes.

Evidence directory: `/tmp/astrolink-v31-probe`.
`events.jsonl` confirms version 31, state 13 (BOOT_RESIDENT), activation error 0.
The native runner passed six checks: Feature loading, capability lookup, valid
6-byte UTF-8 service name, closing its Session, rejection of a 7-byte service
name, and creation of the chat Session.

The start-scan step failed with `BACKEND (1)`. The live SRAM log reports
`bt_socket_client_sendrecv, bt socket send ret: -1 error: 88` twice, in QuickApp
thread 434. Its cleanup succeeded. The companion remained connected until the
harness deliberately disconnected after 30 seconds; no successful scanning,
advertising or CoC result is claimed.

Bounded QMP inspection found the existing client at `0x3C35C488`, fd 18 at
client+48, response loop at +52, and business loop at +56. ENOTSOCK on a foreign
task is consistent with NuttX task-group descriptor tables; it is not evidence
that fd 18 was globally closed. A conditional close(18) breakpoint hit an
unrelated property-server call at LR `0x0C7E1D2D`, showing that a numeric fd alone
is insufficient to identify a socket across tasks.

## Exact dispatch and cleanup ABI

`0x0C46A218` sends a synchronous 708-byte request and waits on client+36. The
client response loop (+52) signals that semaphore; running the synchronous
request on that loop would prevent its own response from being handled.
Firmware boot `0x0C7F6C60` stores global loop `0x200DA540` in client+56 for
business callbacks. GAP requests must be executed in that business context,
without blocking the response loop or Bluelet protocol owner.

`uv_async_t` is 64 bytes, callback at +48. `uv_close` is `0x0C3EA6E4` with
signature `void(handle, close_callback)`. It queues closing handles and invokes
the close callback from `uv_run`; freeing a handle directly from its async
callback is invalid. Firmware `0x0C736FE0` closes an async handle with callback
`0x0C735A72`, which frees it at `0x0C735A76`.

Cross-thread `uv_async_send` uses atomics. `uv_async_init` mutates the target
loop's lists without internal locking. A production dispatcher must not assume
that foreign-thread handle initialization is generally thread-safe merely
because firmware has one constrained instance of it.

## v32: first assertion captured before reboot

ELF SHA-256: `fbcf656566ddf2d774ff498503c3e53f795ecbfdcd433c38681878dc2a902ef0`.
The package is 96,768 bytes; 49,423 resident bytes; 1,230 relocations.

GAP work uses `uv__work_submit` at `0x0C3F4CC4`: a no-op pool worker followed
by the actual operation in the existing business loop's done callback. Its
20-byte prefix is worker, done, loop, queue[2]. The drain unlinks the node before
calling done, and never reads the work again, so the containing work allocation
can be freed in done. No foreign-thread uv handle initialization is needed.

The device still reset after the start-scan step. Reproduction with a breakpoint
at `0x0C3F5758` captured the actual first failure before assertion logging:

- PC `0x0C3F5758`, the `uv_async_send` wake-write assertion preparation.
- R3 (errno) = 9, EBADF; R5 (wake fd) = 85.
- Current task name recovered from its errno/task-local storage:
  `miwear_bluetooth`.
- Stack return `0x0C491DA7`, the Feature manager task-post wake.
- Native SRAM captured at the breakpoint confirms the six preliminary checks
  passed, followed by `start` for the start-scan step. There is no terminal
  completion event and no successful scan/advertising claim.

The independent bounded capture was sanitized into
`/tmp/astrolink-v32-first-assert-native.log`; its temporary 64 KiB raw memory
file was deleted immediately after extraction. The breakpoint register/stack
transcript is in `/tmp/pi-bg/job-23818-589.log`.

This identifies a second descriptor-context boundary in `qjs::notify`: posting
through `0x0C491CD0` from the Bluetooth task calls `uv_async_send` on the JS
manager's wake fd, which is invalid in that caller's descriptor table. Fixing
GAP dispatch alone does not fix JS notification. A proper cross-task-group wake
must retain the target wake resource and its lifetime; polling or suppressing
the assertion would not satisfy the event-delivery contract.

## v33: fd-independent JS wake and remaining scan-stop failure

ELF SHA-256: `59dc0c18495ad2fff64cfce7189e0f3f4933562046da3c6740d5fc27954dc925`.
Package size 97,792 bytes, resident size 50,013 bytes, 1,239 relocations.

The binding now initializes its own async on the QJS owner loop and duplicates
the loop's pipe write endpoint into an owned 32-byte kernel file. Producers use
file_write with the same libuv busy/pending atomic protocol, avoiding a foreign
numeric descriptor lookup. Feature destruction withdraws the endpoint under
the producer guard, then uv_close releases it on the owner loop. Independent
static review confirmed the pipe's one-byte writes, nonblocking duplicate,
pending offsets, and shutdown close-callback drain.

The first native run (`/tmp/astrolink-v33-probe`) successfully completed start
scan, stop scan, and repeated stop (9 preliminary checks), with no wake
assertion or unexpected companion disconnection. However its JS task entered
background after automatic screen-off and timers paused; stop completion took
21.6 seconds. This is insufficient for operation-deadline acceptance.

The probe now uses system.brightness.setKeepScreenOn during the run and restores
normal screen behavior on finish/destruction. A foreground run
(`/tmp/astrolink-v33-awake-probe`) passed seven checks including start scan in
3.255 seconds, then correctly failed stop scan after 10 seconds and cleanup
after another 10 seconds. The companion stayed connected until intentional
harness disconnect. No advertising or CoC pass is inferred.

A live entry/return breakpoint on scan_stop `0x0C46B730` confirmed a valid
shared client and scan handle and a return value of 0. The remaining issue is
after accepted stop submission, before its completion reaches the JS request;
it is not a synchronous socket-send hang. Runtime stop-event semantics remain
under investigation.

The corrected notification bridge and complete CoC data path still require
runtime validation. Host tests and controller regressions are separate gates.

## v34: direct business-loop completion queue

ELF SHA-256: `a5d82c50ddceedd532ae15c6470d2249b220c1486a3c049e3f6aa17cfe9c0845`.
Package size 98,816 bytes; resident size 50,565 bytes; 1,246 relocations.
Thirty host tests, target Clippy and the unchanged ELF/package verifier passed.

GAP commands now enter the firmware's existing completion queue directly and
wake its business loop through an independently owned pipe file. Task lookup
uses the default-loop TLS identity and pins both pipe endpoints in the owning
filelist; it does not assume a fixed PID or global descriptor namespace. An
independent firmware audit confirmed the layouts, critical sections, file
reference ownership and completion-queue ABI documented in `ble-abi.md`.

The first post-reboot query ran before the Supervisor was ready and timed out;
no native result is attributed to it. The subsequent foreground probe
(`/tmp/astrolink-v34-foreground-probe`) confirmed version 34, BOOT_RESIDENT,
activation error 0, and passed ten checks:

- Start scan: 16 ms; stop scan: 37 ms; repeated stop: 19 ms.
- Start advertising with one user byte: 24 ms.
- Advertising stop reported `BACKEND (1)`; Session cleanup then passed.

The run's native log records real Extended Advertising Enable, Disable and
Remove Advertising Set commands. It contains no wake assertion. The advertising
stop callback ABI remains to be corrected/validated; this run is deliberately
reported as failed, not as a complete scan/advertising or CoC pass.

## v35: complete native lifecycle probe passes

ELF SHA-256: `cf7cdc308b8e4afa973ed418ebf117bd320ddb506b49e6150351f2bbdddd069c`.
Package size 98,816 bytes; resident size 50,537 bytes; 1,246 relocations.
Thirty host tests, target Clippy and the unchanged ELF/package verifier passed.

The stopped advertising callback's second argument is the advertising ID, not
an error status. The service producer `0x0C4696C0` and client dispatcher
`0x0C463FA2` establish this ABI. The backend now reports completion only from
that actual stopped notification and retains IPC errors through `Event::Failed`.
It does not reinterpret an IPC failure as success.

`/tmp/astrolink-v35-probe/summary.json` confirms version 35, BOOT_RESIDENT,
activation error 0, terminal `complete`, all 18 checks passed and no failed
checks/cleanup. It covers Feature/capabilities, valid/invalid UTF-8 service names,
scan start/stop/repeated stop, advertising start/stop/repeated stop, Session
close/repeated close and two subsequent Session create/close cycles. The probe
held the screen awake and restored normal screen behavior on completion.

This establishes the native GAP and Session lifecycle gate on the isolated
simulator. CoC establishment, HELLO, binary SDUs and peer/controller cleanup
remain separate integration gates.

## v35: outgoing real LE CoC and binary traffic

`/tmp/astrolink-v35-peer-traced.json` reports `ok=true` against the standard HCI
peripheral at the isolated controller's actual peer endpoint. The real example
scanned/discovered the peer, initiated LE, sent CoC request 0x14, received response
0x15, and exchanged the exact `chat` HELLO. Firmware and peer CIDs were distinct
(`0x47` and `0x40`), exercising real credit routing in both directions.

The peer received actual firmware SDUs of 0, 1, 32, 511 and 512 bytes. Its
511/512-byte example requests received byte-verified firmware responses. The
example's boundary action also verified a 32-byte binary response, a maximum
512-byte response, and rejection of an oversized message. The peer finished
with no pending K-frames/ACL/completions and no CoC/LE cleanup errors.
`/tmp/astrolink-v35-coc-live/guest-logs/ram-syslog.log` independently records
QuickApp consumption of the peer's actual 0-byte and 1-byte messages, replies
for 511/512 bytes, and successful example boundary checks. This is protocol
integration over the controller model, not a physical radio claim.

## Incoming admission defect reproduced on v35

Both `/tmp/astrolink-v35-peer-central.json` and
`/tmp/astrolink-v35-peer-central-traced.json` completed real LE establishment
against the module's connectable `chat` advertisement, then received CoC result
`0x0004`. Focused resident-code breakpoints recorded `Engine::accept` reading
Session advertising state Idle and taking the BUSY branch before
`backend::reject`; the transcript is `/tmp/pi-bg/job-23818-687.log`.

Advertising-resource termination cannot itself revoke the service's listener
intent: a peer first establishes the LE ACL, which terminates connectable
advertising, and only then sends its CoC request. The repair gives the Session
an explicit admission state. Successful advertising opens admission; an explicit
stop closes it immediately even if the controller already terminated the set;
Session close removes admission entirely. Failed/cancelled starts never open it.
The existing advertising resource state still tracks the actual handle and is
released on the native stopped callback. New regressions cover this ordering,
explicit stop after automatic termination, and failed/cancelled start admission.

## v36: both real LE CoC roles pass without debugger instrumentation

ELF SHA-256: `4c21222f3a653217cffca61e9fa333e541bb47bf09298f3410df01282763445e`.
Package size 98,816 bytes; resident size 50,541 bytes; 1,244 relocations.
Thirty-two host tests, target Clippy and unchanged ELF/package verification pass.

Both `/tmp/astrolink-v36-peer-central.json` (firmware incoming) and
`/tmp/astrolink-v36-peer-peripheral.json` (firmware outgoing) report `ok=true`.
Each observed actual LE establishment, successful CoC, matching HELLO, firmware
SDUs of 0/1/32/511/512 bytes, byte-verified 511/512-byte responses, all outgoing
queues drained, and successful CoC/LE cleanup. No debugger was attached during
these v36 runs. The second run reuses the same live QuickApp Session after the
first peer's disconnect, covering channel retirement and reconnect with a new
CID. After both runs, `/tmp/astrolink-v36-probe/summary.json` records all 18
native lifecycle checks passing again: start/stop scan 5/6 ms, start/stop
advertising 59/15 ms, repeated operations and Session releases all within 38 ms.
This also exercises destroying the example context with active scan/listener
resources before the probe starts. Selected actual events, peer summaries,
QuickApp consumption logs and final artifact SHA-256 values are archived in
`evidence/q66-155-v36.json`; full packet captures remain at the paths above.

The incoming run's actual QuickApp log is
`/tmp/astrolink-v36-incoming-live/guest-logs/ram-syslog.log`: it records connection,
0/1-byte consumption, 511/512-byte replies, binary comparison, max-size response,
zero/one-byte submission and oversized-message rejection. The two roles use the
same published Session/Link API and native Bluelet transport.

Independent static analysis confirmed automatic advertiser termination reaches
`0x0C86B18C -> 0x0C87EAA0(event4) -> 0x0C4697A0 -> 0x0C4696C0 -> IPC 280`.
The client callback carries only advertising ID; the connection identity and
termination reason are not preserved. Session listener intent is therefore the
appropriate admission scope, with explicit stop and close remaining authoritative.

These runs establish the simulated firmware integration, not RF/hardware
capacity, phone/Audio coexistence, background guarantees or long-duration heap
stability. Those remain physical-device/stability acceptance work.

## v37: per-target ABI layout, unchanged Q66 `.155` behaviour

ELF SHA-256: `fc08f9d9256e0b96c18e5e30b8eedd5e5fe637c42613ddef3de4e2c79bfac0be`.
Package size 98,816 bytes; resident size 50,541 bytes; 1,244 relocations.

Firmware addresses and structure offsets moved into `src/abi/<target>.rs`; the
GAP, CoC, libuv and QuickJS logic is shared and target independent. The `.155`
values are unchanged, which the rebuild confirms: the resident size and
relocation count are identical to v36.

Device `astrolink-q66-155` in `/tmp/astrolink-mirage-validation`, module
`version 37`, `activationError 0`. `/tmp/astrolink-v37-probe/summary.json`
records `ok=true`, terminal `complete` and all 18 lifecycle checks passing,
the same set as v35/v36. This run does not repeat the v36 two-role LE CoC peer
integration; that evidence stays attributed to v36, whose firmware call path
this build preserves value for value.

## N66 3.1.32: legacy lifecycle and remaining CoC gate

Module v65 completed all 18 native lifecycle checks on the isolated N66 device:
legacy native-extension loading, capabilities, service-name boundaries,
scan/advertising start and stop, idempotent stop, and Session close/rebuild.
The captured UI is `/tmp/astrolink-astrolink-n66-032-probe/screen.png`.
This firmware did not yield the RAM console stream used by the generic probe
parser, so the harness's `summary.ok` is not claimed as a pass; the 18/18 UI
and native event trace are the actual evidence.

The working N66 path uses an independent btservice pipe file. Task enumeration
performs identity reads only; duplication happens after leaving the scheduler
critical section. Wakeups write that retained file directly instead of asking
`uv_async_send` to resolve a descriptor in the producer's foreign group. The
legacy GAP owner register takes an owner pointer as its second argument and
the raw callback table as its third.

The cleaned v67 build has ELF SHA-256
`40611d88fcdeec5d4e29b5983c5a91e60bd4d920a2b988c980832fefd6d283a5`,
53,005 resident bytes, zero undefined symbols and 1,291 relocations. Host
tests and target Clippy pass. Bidirectional CoC is **not** validated yet. A
dedicated automatic peer probe was built, but a fresh isolated Supervisor
repeatedly failed module installation at `STAGE_WRITE` while writing
`.ko.upload`; `/tmp/n66-coc-v67-install/summary.json` preserves that separate
installation blocker. No scan-report, LE connection, HELLO or SDU success is
inferred from the lifecycle result.

## v43: shared message pool, 16-entry link table, per-session channel table

Two changes, each with its own host-test run before the device run:

1. `dc50387` moved the 512-byte SDU buffers out of every `Link` into one fixed
   pool of 40 messages (`MessageId` handles, free-slot bitmap). `Link` dropped
   from 4,256 to 160 bytes. A queued SDU keeps its slot until
   `transmission_submitted()` confirms host acceptance, so a `BUSY` retry still
   carries the original bytes; closing a Link releases every slot its TX and RX
   queues hold.
2. `e3bb441` derived the per-session Bluelet channel table from
   `DEVICE_MAX_LINKS` instead of the previous 8. A Session can own every Link,
   and its ninth outgoing connect used to record no CID and emit no event: the
   Link waited out the 15-second establishment deadline while the firmware-side
   channel was left without a disconnect.

Measured on the host: `Link` 4,256 -> 160 bytes, `Engine<1>` 4,784 -> 21,336,
`Engine<16>` 68,624 -> 23,736. The pool is unconditional, so every supported
target pays it: `.155` resident bytes 50,541 -> 70,785 (+20,244, +40%), `.data`
9,316 -> 29,036 and now 41% of the image. Targets whose ABI reports
`SUPPORTED = false` still build at 1.3 KB because LTO drops the untouched
runtime static.

ELF SHA-256 `be24a5e610c67ad24e14bc536572a867810e3609b483746059bd8082b84a5c05`,
70,785 resident bytes, 1,229 relocations. Package
`dist/astrolink-0.1.0.canopus`, 724,992 bytes, 8 targets, versionCode 43,
package SHA-256 `043fd15ff4718bd6015aa4d648e57cb2a99016734ce3cc6f540b97536c1652cb`;
`canopus package verify` passes.

Device `astrolink-q66-155`, firmware `miwear.watch.q66tc_v4.100.155_full_f1c824fe`,
module `version 43`, `state 13` (BOOT_RESIDENT), `activationError 0`. The probe
reported `ok=true`, terminal `complete` and all 18 lifecycle checks passing;
evidence is `/tmp/astrolink-astrolink-q66-155-probe`, with the 18 checks read
from `guest-logs/ram-syslog.log`. Observed step times: capabilities 2 ms,
6-byte service accepted 2 ms, boundary Session close 97 ms, 7-byte rejection
2 ms, Session create 2 ms, scan start/stop 32/15 ms, repeated scan stop 7 ms,
advertising start/stop 26/23 ms, repeated advertising stop 22 ms, Session close
26 ms, repeated close 4 ms, Session rebuild/release 4/29/4/24 ms.

This run does not repeat the v36 two-role LE CoC peer integration; that evidence
stays attributed to v36, whose firmware call path this build preserves. The
16 entries are a static table capacity: no run has established more than one
Link at a time, the controller's simultaneous LE link limit is still unmeasured,
and the +20 KB static cost has not been measured against the remaining heap.
Bidirectional CoC, radio capacity, phone/Audio coexistence and screen-off
stability remain the separate acceptance work listed above.
