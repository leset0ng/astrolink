# P67 v42 runtime validation

This record covers isolated Mirage devices running exact P67 firmwares
`3.101.043` (`probe-p67`) and `3.101.036` (`astrolink-p67-036`). Both pass their
own native lifecycle probe; `.043` also passes both CoC connection directions.
These are emulator results, not physical-radio approval. The library is
`/tmp/astrolink-mirage-validation`; the normal user library is not involved.

## Repairs

Both P67 ABI modules now use `TCB_TLS = 100`. Exact `.043` and `.036` producer
and consumer instructions establish this independently. The previous value
104 read the wrong task field and prevented GAP business-loop owner lookup.
The complete chain and instruction addresses are in `p67-native-contract.md`.
The shared task lookup and pipe ownership implementation did not need a change.

The QuickApp probe no longer waits for a brightness success callback before
starting. A missing callback left its page indefinitely at the initial state,
outside the runner's own step deadlines. Screen policy is requested separately;
the native tests start immediately. Regression tests cover a brightness API
that never invokes callbacks and one that throws synchronously.

## Environment requirements

Use a current, exact-target Supervisor with Time service version 2. Astrolink
requires background timers and correctly refuses a Supervisor without them.
The stale installer artifact in this session returned activation error `-95`
at module initialization stage 2; this was distinct from the TCB lookup error.
Rebuilding with the existing SDK script restored the required service:

```sh
CANOPUS_TARGET=xiaomi-band-10-pro-3.101.043 \
  bash ../Canopus-Private/scripts/build_canopus_supervisor.sh
```

Verified Supervisor SHA-256:
`5dc88c9e7f38ec622cdf5f94b13e537fdc176c6926b080880f4fdfdb0e4ff3c0`.
It has 166,546 resident bytes, no undefined symbols, and passes the normal ELF
verifier. Rebuild before staging both the installer resource and the boot copy
at `/data/canopus/supervisor.elf`.

Installing a newer module stages a generation switch. A successful install is
not proof that the new code is active. Apply the normal `RESTORE_AFTER_BOOT`
flow before loading the module, then verify the actual version and activation
status through the companion channel. Direct `ACTIVATE` on old metadata loaded
v41 in an early attempt despite the v42 package having installed successfully.

P67 UI registration must run on the UI task. The SDK's
`scripts/emu/ui_pages.py` documents why an NSH-driven installation can block.
The successful run used the production P67 installer's LVGL timer for startup
restoration after native `insmod`. That is historical context for this evidence,
not the workflow for further work: the project owner requires command-line
installation and activation. Module delivery itself used the command-line
`scripts/mirage-smoke.py ... install --module ...` path. Do not repeat installer
UI operations to install Astrolink. Avoid starting extra NSH Lua interpreters
during a UI transaction or a QuickApp run. A separate pre-module fault capture
recorded stock `uv_async_send` asserting at `async.c:213`; it is not evidence of
an Astrolink lifecycle fault.

Before launching a probe manually on P67, enable INFO logging. The current
`scripts/mirage-smoke.py --probe-log-source nsh probe` command does this
automatically before launching the QuickApp:

```sh
MIRAGE_LIBRARY=/tmp/astrolink-mirage-validation \
  /path/to/miragectl sh probe-p67 "setlogmask i"
```

The ordinary firmware startup sets its syslog mask to `0x1f`, suppressing
QuickApp `console.log` at INFO priority. `setlogmask i` changes the RAM mask to
`0x7f`; it must be repeated after a reboot. Earlier summaries with zero parsed
checks and no terminal event do not establish how far the native runner got.
No firmware patch or persistent system-mode property is needed to collect logs.

## Build and host verification

- Both P67 feature selections pass all 35 Rust host tests.
- Both ARM device feature selections pass Clippy with `-D warnings`.
- `cargo fmt --check` passes, including the `.036` signature-array layout.
- The QuickApp tests pass 13 checks, including both probe startup regressions;
  TypeScript checking and source probe RPK compilation pass.
- The v42 package builds all five exact targets through the unchanged verifier.
- Both P67 ELFs have 50,917 resident bytes, zero undefined symbols, and 1,255
  relocations.

| Artifact | SHA-256 |
| --- | --- |
| P67 `.043` ELF | `fb36c8a5c8e875e840c48a5d5ed547f12cc25d7075663bcb8e401a320f303c71` |
| P67 `.036` ELF | `bf4551496fbc9631003f2560f05a44854e92b8e6e595579c08ed8edc3cb62d17` |

## Native acceptance

The final real companion query confirmed version **42**, state **13**
(`BOOT_RESIDENT`), and activation error **0**. The native runner then passed
**18/18** checks and emitted terminal event `complete`, sequence 37, passed 18.
The parser recorded no failure events. Its fault watcher observed no assertion
or hardware exception during the bounded 90-second window.

The checks cover Feature load, capabilities, service-name byte boundaries,
Session creation and closure, scan start/stop and repeated stop, one-byte
advertising start/stop and repeated stop, repeated close, and two Session
recreation/cleanup rounds. These are actual native firmware operations.

Evidence is in `/tmp/astrolink-p67-v42-native-probe/summary.json` and
`guest-logs/nsh-syslog.log`. A compact result is retained at
`docs/evidence/p67-043-v42.json`. The probe RPK SHA-256 is
`872ebcf43bcfb764409c9878e954ad168757633ab0e2853066a673507a0f3ad5`.
The final screenshot is dark because the completed runner releases its
keep-screen-on request; the terminal log event is the acceptance signal.

## Command-line installation and activation

The Mirage bridge now exposes `canopusActivate` through the real encrypted CPC2
ACTIVATE command. The operation lives in AstroBox core's
`activate_module_cpc2`; the bridge only dispatches it. Only the Supervisor's
terminal `Completed` response is accepted. The existing smoke command can now
activate a module without an NSH Lua interpreter or an installer UI.

The `.043` bridge test at `/tmp/astrolink-p67-v42-cpc2-activate` confirmed v42,
state 13 and activation error 0 after a fresh native CLI load. `.036` was
installed and activated entirely by this command-line path, on its own device.

For an already imported, paired isolated device with no generation switch
pending, use a current exact-target Supervisor and prepare both directories.
On a stopped device, `fs put ... nand_data ... /canopus/supervisor.elf` stages
the file that the booted firmware sees as `/data/canopus/supervisor.elf`.
Then run the following, substituting the CLI path and device selector:

```sh
export MIRAGE_LIBRARY=/tmp/astrolink-mirage-validation
miragectl sh astrolink-p67-036 "insmod /data/canopus/supervisor.elf canopus_supervisor"
miragectl sh astrolink-p67-036 "mkdir /data/canopus/inbox"
```

The parent `/data/canopus` must already exist. A missing inbox caused the first
`.036` STAGE_BEGIN request to fail; creating it made the same package install
successfully. Do not treat a failed staging command as an ABI rejection.

With `BRIDGE` pointing to a rebuilt dylib that exports the new dispatcher:

```sh
python3 scripts/mirage-smoke.py --library "$MIRAGE_LIBRARY" \
  --device astrolink-p67-036 --bridge "$BRIDGE" --settle 0 \
  install --module dist/astrolink-0.1.0.canopus \
  --quickapp examples/quickapp/dist/astrolink-probe-source.rpk
python3 scripts/mirage-smoke.py --library "$MIRAGE_LIBRARY" \
  --device astrolink-p67-036 --bridge "$BRIDGE" --settle 0 \
  canopus enable --module astrolink
python3 scripts/mirage-smoke.py --library "$MIRAGE_LIBRARY" \
  --device astrolink-p67-036 --bridge "$BRIDGE" --settle 0 \
  canopus activate --module astrolink
python3 scripts/mirage-smoke.py --library "$MIRAGE_LIBRARY" \
  --device astrolink-p67-036 --bridge "$BRIDGE" --settle 20 \
  --probe-log-source nsh probe
```

Do not rerun `insmod` on a resident Supervisor. After an update rather than a
first install, the pending generation must be switched before loading; always
check the actual version reported by the Supervisor.

## Independent `.036` lifecycle result

The exact `.036` AP SHA-256 is
`662d67f5e247e31e194d3161024890ba93b9d29d70b290fadb9aac8ce8ec3c81`.
Its SDK previously had only `owner-pump.json`, so it built Time v2 but no native
L2 transport. The exact image's RX, send, session, crypto, SAR list and Bluetooth
loop were recovered and recorded in `EVID-PORT-1036-L2-001` and
`evidence/l2-sram-probe.json`. New symbols remain restricted,
STATIC_RECOVERED/PENDING; no approval state was promoted. Target-derived SDK
bindings were regenerated and all 19 generated-stability checks passed.

The L2-enabled `.036` Supervisor passes normal ELF verification: SHA-256
`27f39dc6731c9ce54a2e1985f6990cd72bb0eb3f2f58175e0bf7bdb8389d88c6`,
162,930 resident bytes, no undefined symbols and 2,190 relocations. No shared
L2 implementation change was required.

The real companion installed the `.036` artifact from the v42 package, enabled
it, activated it, and independently reported version **42**, state **13**,
activation error **0**. The first native probe emitted `complete`, sequence 37,
with **18/18** checks passed and no failures. Evidence:
`/tmp/astrolink-p67-036-v42-native-probe`; compact result:
`docs/evidence/p67-036-v42.json`. Its install, enable and activation evidence
are the adjacent `...-install-ready`, `...-enable` and `...-activate` directories.
No installer watchface was installed or used on this device. A second fresh
probe at `/tmp/astrolink-p67-036-v42-auto-log-probe` also passed 18/18 after
explicitly setting the firmware's mask to WARN first; the updated CLI restored
INFO before launch and captured the actual terminal result.

## `.043` incoming and outgoing CoC

The `.043` device booted with the already validated LE controller extension,
QEMU commit `42b97e2cd6`, SHA-256
`99b67e01d4a4c19d2352461da54a43b16d194fea9cd19a893df3cc756195390b`.
The firmware BLE host and actual QuickApp exchange standard LE/L2CAP traffic
with `scripts/ble-peer-test.py` at the external H4 host endpoint. No module
callbacks or radio packets were fabricated. Companion and peer endpoint
ownership was sequential. No debugger was attached during either accepted run.

| Firmware role | Peer evidence | Result |
| --- | --- | --- |
| Incoming CoC, firmware peripheral | `/tmp/astrolink-p67-v42-peer-central.json` | PASS; firmware CID 0x47, peer CID 0x40 |
| Outgoing CoC, firmware central | `/tmp/astrolink-p67-v42-peer-outgoing.json` | PASS; firmware CID 0x49, peer CID 0x40 |

Both runs observed real LE Connection Complete, CoC open, the exact `chat`
HELLO, complete firmware SDUs of **0/1/32/511/512 bytes**, byte-verified
511/512-byte replies, drained K-frame/ACL queues, and successful CoC and LE
cleanup with no errors. Distinct local/remote CIDs exercised credit routing.
The actual QuickApp logs also confirm consumption of the peer's 0/1-byte
messages, replies of 511/512 bytes, binary comparison, maximum-size response,
0/1-byte transmission and oversized-message rejection.

The accepted outgoing run reuses the same QuickApp Session after the incoming
peer's disconnect and an intermediate reconnect, covering channel retirement
and new CID allocation. Preliminary outgoing attempts missed UI actions while
the panel was off or omitted the example's raw 0/1-byte send action; their peer
summaries correctly failed and are not counted as accepted runs.

QuickApp logs are retained at
`/tmp/astrolink-p67-v42-{incoming,outgoing}-live/guest-logs/nsh-syslog.log`.
After traffic, a fresh probe context passed all **18/18** lifecycle checks again
at `/tmp/astrolink-p67-v42-post-coc-probe`. This includes closing the example's
scan/listener resources. Compact protocol evidence is retained at
`docs/evidence/p67-043-v42-coc.json`.

The `.036` result covers lifecycle/GAP only; it does not inherit `.043` CoC
acceptance. Physical-radio range/timing, throughput, phone/Audio coexistence,
screen-off guarantees and long-duration stability remain unverified.
