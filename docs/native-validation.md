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
