# Mirage / Q66 .155 validation environment

Observed on 2026-10-29 UTC. This records emulator setup and inspected implementation; it does **not** assert Astrolink or two-device LE CoC passed.

## Ready environment

- CLI: `/Users/lesetong/develop/Mirage/Packages/MirageKit/.build/debug/miragectl` (already built).
- QEMU: `/Users/lesetong/develop/Mirage/External/qemu-miwear/build/qemu-system-arm`, version `11.1.2`.
- QEMU repository ref: `ceac001066440c7ed1a4ff71b6e0923bc352cfbd`; Mirage ref: `e4180b7542bdc2f54fef4f732a76c4637470748d`.
- Both the CLI's bundled QEMU and submodule build have SHA-256 `f355b89c09292585671a68b26a74f3866822f1b19977a58ae1a927c7fa398a10`.
- Isolated library: `/tmp/astrolink-mirage-validation`. Default user library and its four existing devices were not changed.
- Device: `astrolink-q66-155`, UUID `0C5A5DAC-5C16-486F-94B7-E5A8E634B5F3`, profile `q66`, firmware `miwear.watch.q66tc_v4.100.155_full_f1c824fe`.
- Current Bluetooth peer-host endpoint: `127.0.0.1:64317`; read `runtime.json` for the port after a future boot.
- AP SHA-256: `ea0bdf1920cb30223d616432af00565ca67622e6468328f5eab155f8cdc2fb9f` (matches the investigation target).
- Device is running with production Canopus Supervisor loaded. Actual `canopusDeviceInfo` result:

```json
{"frameworkRevision":1,"safeMode":false,"moduleCount":0,"safeModeReason":0,"errorCode":0,"flags":83886080,"scanSummary":0,"targetId":"xiaomi-band-11-4.100.155","requestSlots":16}
```

Actual `canopusModules` result was `[]`; companion `deviceInfo` returned firmware `4.100.155`; `quickApps` returned `[]`.

## CLI setup and controls

`MIRAGE_LIBRARY` overrides the library without changing user preferences (`MirageLibrary.swift:48-55`). From Astrolink's root:

```sh
export MIRAGE_LIBRARY=/tmp/astrolink-mirage-validation
MIRAGECTL=/Users/lesetong/develop/Mirage/Packages/MirageKit/.build/debug/miragectl
"$MIRAGECTL" firmware import ../../mifw/miwear.watch.q66tc_v4.100.155_full_f1c824fe.bin
# create prints an authkey: filter/capture it; do not retain that output.
"$MIRAGECTL" create astrolink-q66-155 --firmware miwear.watch.q66tc_v4.100.155_full_f1c824fe --profile q66 \
  2>&1 | sed -E 's/AuthKey [0-9a-fA-F]+/AuthKey [REDACTED]/g'
"$MIRAGECTL" boot astrolink-q66-155 --no-shell
"$MIRAGECTL" tap astrolink-q66-155 106 260
"$MIRAGECTL" screenshot astrolink-q66-155 /tmp/astrolink-screen.png
"$MIRAGECTL" shutdown astrolink-q66-155
"$MIRAGECTL" fs ls astrolink-q66-155 nand_data /app
```

Do not recreate the ready device merely to repeat these commands. The CLI has `fs put/get`, tap/swipe/button and screenshot, but no companion app/module installation subcommand. `miragectl boot` initially reported `shell: nsh pid 9`; actual `sh help` and `sh ls /dev` timed out. That injection status is not evidence of a working NSH in this user firmware. `--no-shell` is the validated boot mode.

## Supervisor bootstrap actually performed

The existing SDK file `../Canopus-Private/watchfaces/canopus-installer-prod/xiaomi-band-11/未命名表盘.bin` was installed through the real companion bridge as watchface `000000000041` (244,962 bytes, 1.4 s) and selected with `setWatchface`. Its UI rendered. Initially Run failed with `Installer profile unavailable`.

Offline `fs ls` proved that package contained only `.179` profile/stage/boot/Supervisor files. While the isolated device was shut down, the following **existing production assets** from `../Canopus-Private/watchfaces/canopus-installer-prod/xiaomi-band-11/` were deployed using `miragectl fs put` to `/app/watchface/market/000000000041/` on volume `nand_data`:

- `main.lua`, `manager_icon.bin`;
- `canopus_loader_profile-xiaomi-band-11-4.100.155.bin`;
- `canopus_stage1-xiaomi-band-11-4.100.155.bin`;
- `canopus_stage2-xiaomi-band-11-4.100.155.bin`;
- `canopus_supervisor-xiaomi-band-11-4.100.155.bin`;
- `canopus_boot-xiaomi-band-11-4.100.155.bin`.

Example (device must be stopped):

```sh
"$MIRAGECTL" fs put astrolink-q66-155 nand_data \
  ../Canopus-Private/watchfaces/canopus-installer-prod/xiaomi-band-11/canopus_loader_profile-xiaomi-band-11-4.100.155.bin \
  /app/watchface/market/000000000041/canopus_loader_profile-xiaomi-band-11-4.100.155.bin
```

After a normal `boot --no-shell`, the same installed watchface showed Ready. Tapping Run at `(106,383)` executed the unchanged production loader. Screenshot `/tmp/astrolink-q66-155-bootstrap.png` shows the system notification “Canopus 已加载，管理器可用”; the real encrypted companion Canopus queries above then succeeded. These screenshots are transient local evidence, not committed artifacts. No transport, firmware image, Astrolink source, or QEMU source was changed. For a distributable installer, rebuild the watchface package with all `.155` assets; the old prepacked file alone is insufficient.

Supervisor is resident in the current boot. Bootstrap/persistence after reboot has not been tested here; rerun the installed production Run action after reboot if required.

## Companion bridge without exposing credentials

Existing CLI `../../Mirage/AstroBoxBridge/target/smoke` takes authkey as an argument **and prints connect parameters**, so it was not used. The existing static library was linked into a temporary dylib; the same `mab_init` / `mab_call` C API was invoked in-process. This uses the real companion core, not a replacement Bluetooth transport.

```sh
clang -dynamiclib \
  -Wl,-force_load,../../Mirage/AstroBoxBridge/target/release/libmirage_astrobox.a \
  -framework CoreAudio -framework Security -framework IOKit -framework IOBluetooth \
  -framework CoreBluetooth -framework WebKit -framework ApplicationServices \
  -framework CoreGraphics -framework Carbon -framework CoreVideo \
  -framework CoreFoundation -framework AppKit -framework Foundation \
  -liconv -lobjc -lc -lm -Wl,-dead_strip -o /tmp/astrolink-mirage-astrobox.dylib
```

Runnable Python bridge session (credentials remain in process memory; substitute real artifact paths only after building them):

```python
import ctypes, json, pathlib, threading, time
library = pathlib.Path('/tmp/astrolink-mirage-validation')
device = library / 'Devices/0C5A5DAC-5C16-486F-94B7-E5A8E634B5F3'
record = json.loads((device / 'device.json').read_text())
runtime = json.loads((device / 'runtime.json').read_text())
key = record['binding']['authKey']
bridge = ctypes.CDLL('/tmp/astrolink-mirage-astrobox.dylib')
callback_type = ctypes.CFUNCTYPE(None, ctypes.c_char_p, ctypes.c_void_p)
results, condition = {}, threading.Condition()

@callback_type
def callback(raw, _):
    event = json.loads(raw.decode().replace(key, '[REDACTED]'))
    if event.get('type') == 'result':
        print(json.dumps(event, ensure_ascii=False), flush=True)
        with condition:
            results[event['id']] = event
            condition.notify_all()

bridge.mab_init.argtypes = [callback_type, ctypes.c_void_p]
bridge.mab_call.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
bridge.mab_call.restype = ctypes.c_uint64
bridge.mab_init(callback, None)

def call(method, params=None):
    request = bridge.mab_call(method.encode(), json.dumps(params or {}).encode())
    deadline = time.monotonic() + 45
    with condition:
        while request not in results and time.monotonic() < deadline:
            condition.wait(1)
    result = results.get(request)
    if not result or not result.get('ok'):
        raise RuntimeError(f'{method}: {result}')
    return result['result']

try:
    call('connect', {
        'host': '127.0.0.1', 'port': runtime['bluetoothPort'], 'authKey': key,
        'stateDir': str(device / 'AstroBox'), 'timeoutSecs': 30,
        'bandAddr': record['identity']['bluetoothAddress'],
    })
    call('deviceInfo')
    call('canopusDeviceInfo')
    call('canopusModules')
    call('quickApps')
    # call('install', {'path': '/ABS/astrolink.canopus', 'kind': 'canopus'})
    # call('install', {'path': '/ABS/test.rpk', 'kind': 'quickapp'})
    # call('launchQuickApp', {'packageName': 'THE_ACTUAL_MANIFEST_PACKAGE'})
finally:
    call('disconnect')
```

First-time pairing requires tapping the real confirmation at `(106,478)` while connect is pending. The ready instance is already paired with its per-device `AstroBox` state directory; do not replace that directory with another device's state. Only one peer host owns the Bluetooth chardev at a time.

Inspected bridge implementation:

- `Mirage/AstroBoxBridge/include/mirage_astrobox.h:27-45`: documented connect/install/launch methods.
- `Mirage/AstroBoxBridge/src/ops.rs:120-145`: native `.canopus` takes `install_native_package`, the encrypted CPC2 staging path.
- `Mirage/AstroBoxBridge/src/ops.rs:147-174`: `.rpk` manifest supplies package name; quick apps take `MassDataType::ThirdPartyApp`.
- `Mirage/AstroBoxBridge/src/ops.rs:333-345`: `launchQuickApp` verifies the installed package and invokes the real firmware launch request.
- Module staging path is `/data/canopus/inbox/<token>.{cmi,ko}.next` (`Canopus-Private/docs/external-module-migration.md:18`); use companion native installation rather than manually copying an RPK for modules.
- Actual watchface data is `/data/app/watchface/market/000000000041/`. QuickApp filesystem root has not been established on this exact device (no quick app installed yet); do not infer it from an older historical doc.

No Astrolink artifact was installed by this setup task. QuickApp install/launch is supported by inspected bridge code and successful list/connect, but was not yet exercised with an actual test RPK.

## Real BLE support and hard two-emulator limit

Inspected source ref is recorded above; local files were clean when inspected.

| Function | Evidence / observed result |
| --- | --- |
| LE legacy/extended advertising and scan reports | Implemented in `hw/misc/bes_btss_link.c:315-575`. Report delivery is change/start driven, without radio timing; active scan can get scan response. This is not evidence for RSSI, power, or repeated-report timing. |
| External peer-host transport | `bes_btss_link.c:1345-1383` accepts H4 Command and ACL packets from a **host** and executes them for the second controller inside one QEMU instance. It is not an over-air/controller-to-controller link. |
| LE Create Connection (`0x200D`) and Extended Create (`0x2043`) | Explicitly unmodelled (`bes_btss_link.c:318-323`); absent from air-command dispatch `:1191-1244`. Both opcodes occur only in the generic Command Status list (`bes_btss.c:289-298`), which returns status 0 then does no connection work (`:663-668`). |
| ACL transfer | `bes_btss_link.c:1293-1332` requires the single existing link handle. That link is created by BR/EDR Create/Accept Connection, not an LE connection. Existing ACL forwarding cannot establish an LE CoC path. |

Practical probe on the isolated controller (external host port, before companion connect):

```text
HCI Reset              0x0C03 -> 04 0e 04 01 03 0c 00
Read Local Version     0x1001 -> 04 0e 0c 01 01 10 00 0b 00 00 0b b0 02 00 00
LE Read Buffer Size    0x2002 -> 04 0e 07 01 02 20 00 fb 00 08
LE Set Scan Parameters 0x200B -> 04 0e 04 01 0b 20 00
LE Set Scan Enable     0x200C -> 04 0e 04 01 0c 20 00
LE Create Connection   0x200D -> 04 0f 04 00 01 0d 20
LE Create Conn Cancel  0x200E -> 04 0e 04 01 0e 20 00
```

No LE Connection Complete appeared during the 1.5 s observation after Create. The bounded observation alone is not proof of absence; the inspected command dispatch establishes the missing implementation. Successful generic Command Status must not be recorded as successful LE connection.

Conclusion: this ready environment supports real firmware module loading, Feature/QJS smoke tests and companion installation. The current QEMU cannot validate two-emulator LE ACL / LE CoC end to end. Connecting the two TCP ports would feed controller events into a host-command parser and does not solve that limit. Two actual devices are required for the initial radio acceptance unless QEMU gains a separately implemented and validated LE controller/air model; no fake BLE or alternate transport was introduced.

## Reusable smoke CLI and QuickApp installation (2026-10-29 17:13 UTC)

The later integration pass added `scripts/mirage-smoke.py`. It drives the same real `mab_init` / `mab_call` API, reads the selected device's binding in-process, and redacts sensitive fields and long hexadecimal values before writing/printing bridge events. Public artifact SHA-256 values remain in `summary.json` for reproduction. No credentials are command-line arguments. It does not install or activate an artifact unless explicitly requested.

From the repository root, use the ready library/device/bridge:

```sh
python3 scripts/mirage-smoke.py \
  --library /tmp/astrolink-mirage-validation --device astrolink-q66-155 \
  --bridge /tmp/astrolink-mirage-astrobox.dylib \
  --output /tmp/astrolink-smoke-status status

python3 scripts/mirage-smoke.py \
  --library /tmp/astrolink-mirage-validation --device astrolink-q66-155 \
  --bridge /tmp/astrolink-mirage-astrobox.dylib \
  --output /tmp/astrolink-smoke-app-install \
  install --quickapp examples/quickapp/dist/astrolink-example-source.rpk

python3 scripts/mirage-smoke.py \
  --library /tmp/astrolink-mirage-validation --device astrolink-q66-155 \
  --bridge /tmp/astrolink-mirage-astrobox.dylib \
  --output /tmp/astrolink-smoke-app-launch \
  launch com.canopus.astrolink.example --page pages/index

python3 scripts/mirage-smoke.py \
  --library /tmp/astrolink-mirage-validation --device astrolink-q66-155 \
  --bridge /tmp/astrolink-mirage-astrobox.dylib \
  --output /tmp/astrolink-smoke-query canopus query --module astrolink
```

`install` accepts `--module /ABS/verified.canopus`, `--quickapp /ABS/built.rpk`, or both. No module was installed in this pass: the parent will supply the verified module path. The typed Canopus commands are `query`, `enable --module ID`, `disable --module ID`, and `scan`. There is no fabricated `activate` bridge operation:

- `Mirage/AstroBoxBridge/src/lib.rs:98-106` routes the available Canopus methods; `src/ops.rs:414-418` forwards set-enabled.
- `Mirage/External/AstroBox/modules/core/src/device/canopus.rs:537-557` translates set-enabled to CPC2 ENABLE/DISABLE.
- `Canopus-Private/manager/service/canopus_supervisor.c:668-714` records next-boot intent and returns REBOOT_REQUIRED. ENABLE is not immediate ACTIVATE.
- CPC2 `CanopusCommand::Activate = 11` exists in core, but no corresponding bridge method is exposed. The smoke script does not silently replace activation with another command.

Every invocation records `events.jsonl`, `summary.json`, `screen.png` when the screenshot command succeeds, and `guest-logs/{console,qemu,stdout,uartN}.log` tails. Default tail limit is 262,144 source bytes per existing log, adjustable with `--log-bytes`. Existing guest logs are read without taking the console connection or changing UART configuration. Use `capture` instead of `status` to collect logs/screenshots without connecting the companion. `--settle` controls the final capture delay, `--timeout` bounds each companion request, and `--miragectl` selects another CLI. Evidence defaults to a newly created `/tmp/astrolink-smoke-*` directory when `--output` is omitted. Screenshots record the actual panel state, including sleep/black frames; script success means the requested transport operation succeeded, not that UI contents have been validated.

Executed checks:

| Operation | Actual observation / saved evidence |
| --- | --- |
| `status` | firmware `.155`, Supervisor exact target, zero modules; `/tmp/astrolink-smoke-status/`. |
| Source RPK installation | `install` returned success, package `com.canopus.astrolink.example`, 27,012 bytes, 0.4 s; inventory listed the app; `/tmp/astrolink-smoke-quickapp-install/`. |
| Bytecode RPK installation | `install` returned success for the same package, 33,737 bytes, 0.3 s; `/tmp/astrolink-smoke-bytecode-install/`. This was the last installed variant in this pass. |
| Launch with empty page and `pages/index` | Bridge returned success, but screenshots were black. Waking the panel displayed the original Canopus installer. Missing-native error UI was **not** observed; `/tmp/astrolink-smoke-missing-native-launch-awake/`, `/tmp/astrolink-smoke-bytecode-launch/`. |
| `canopus query --module astrolink` | returned `null`; module remains uninstalled; `/tmp/astrolink-smoke-query/`. |
| Script validation | Python syntax parsed; recursive sensitive-field/known-key/long-hex redaction checked, including preservation of public SHA-256. Selected binding key absent from saved text evidence. |

The launch acknowledgement is weaker than application-render evidence: `Mirage/External/AstroBox/modules/core/src/device/xiaomi/components/thirdparty_app.rs:42-45` sends a launch packet; `:178-192` constructs that packet. The wrapper has no application-render completion response. Neither source nor bytecode launch should be reported as a successful missing-native UI test yet.

For diagnosis, the existing `qemu-miwear/emu/ramlog.py` strategy was also applied over this device's `qmpAux` Unix socket: `pmemsave` of PSRAM into an immediately deleted temporary file, extraction of timestamped syslog lines, and redaction before retained output. It found old boot-time QuickApp framework initialization, not a conclusive recent launch/QJS exception. This ad hoc memory inspection is not part of the reusable script; no raw RAM dump is retained. Current UART/console logs are empty in this user firmware, while `stdout.log` contains QEMU board/storage initialization.

## Verified module install / boot-activation attempt (2026-10-29 17:16 UTC)

The parent authorized `dist/astrolink-0.1.0.canopus` only after its final verification. Before installation this pass independently checked 89,088 package bytes and the embedded exact `.155` ELF SHA-256 `d15c99812c8337094a05d739689b02c24f702fae3f4f55670e34ec98a805ddc2`.

1. Real companion native install succeeded in 5.1 s, reported exact target `.155`, and listed Astrolink state `3` (INSTALLED), lifecycle `1`, activation error `0`. Evidence: `/tmp/astrolink-smoke-module-install/`.
2. `canopus enable --module astrolink` succeeded; query returned state `5` (ENABLED), activation error `0`. Evidence: `/tmp/astrolink-smoke-module-enable/`.
3. Device was shut down and booted with `--no-shell`, preserving registry and installed artifacts. After waking the panel, normal production installer Run was clicked at `(106,383)`.
4. Supervisor became queryable and remained outside safe mode. **Activation did not pass**: Astrolink state `15` (FAILED), raw activation error `4294967258` = signed `-38` (UNSUPPORTED); Supervisor raw error `4294967276` = signed `-20`. Evidence: `/tmp/astrolink-smoke-module-active-retry/{events.jsonl,summary.json,screen.png,registers.txt,guest-logs/}`.

This was not a system crash: companion/device info continued working, QMP reported running, and captured CPU state was PC `0x0027b440`, LR `0x0027b037`. UART logs and timestamped SRAM/PSRAM syslog extractions were preserved after redaction; temporary raw RAM files were immediately deleted. They did not identify the exact failing initialization stage. Old Supervisor assets are still loaded: the SDK `.155` production resource was built on Oct 28, before this task's new Feature hook support. An obsolete hook engine rejecting the new target is a diagnostic lead, not yet a proven root cause. The parent/SDK agent must supply the newly built Supervisor assets before another activation conclusion.

The standard activation contract is supported by inspected code: production `main.lua:696-703` sends `CMD_RESTORE_AFTER_BOOT` (14) before Manager registration. `canopus_supervisor_module.c:107` imports registry metadata in the constructor; `canopus_supervisor.c:2082-2155` loads and activates enabled installed modules during restore. Thus ENABLE → reboot → normal installer Run is the supported path; a separate bridge ACTIVATE method is unnecessary for this flow. State names/values are in `sdk/c/canopus_runtime.h:25-45`: ACTIVE is `9`, BOOT_RESIDENT is `13`, FAILED is `15`.

A first post-reboot Run attempt was swallowed by screen sleep and the installer remained Ready. That produced a Canopus query timeout before Supervisor bootstrap, not a module failure. Wake at `(106,260)`, then click Run immediately, and observe a progress step before interpreting a timeout as activation failure.

For future isolated QEMU builds, `QEMULocator.candidates()` in `MirageKit/Runtime/LaunchPlan.swift:52-54` checks `MIRAGE_QEMU` first. `MIRAGE_QEMU=/tmp/NEW_BUILD/qemu-system-arm` on the next `miragectl boot` selects that binary without changing user preferences or the original QEMU checkout. The currently running device still uses the original binary.


## QuickApp black-screen diagnosis resolved (2026-10-29 17:31 UTC)

The original smoke script disconnected the companion immediately after `launchQuickApp` returned. The bridge/core contract only acknowledges that the Xiaomi launch packet was **enqueued**, not that it was sent or the app rendered. The disconnect could discard that queued launch. This was a harness lifecycle bug, not evidence of a QJS compiler failure.

`scripts/mirage-smoke.py` now keeps the real companion connection open for `--settle` after a launch, then disconnects and captures evidence. A 3 s interval produced a visible Astrolink page on the same running device, using the existing bytecode package and `/pages/index`:

```sh
python3 scripts/mirage-smoke.py \
  --library /tmp/astrolink-mirage-validation --device astrolink-q66-155 \
  --bridge /tmp/astrolink-mirage-astrobox.dylib --settle 3 \
  --output /tmp/astrolink-smoke-launch-link-drain \
  launch com.canopus.astrolink.example --page /pages/index
```

Actual screenshot `/tmp/astrolink-smoke-launch-link-drain/screen.png` shows “Astrolink”, status “模块不可用”, Scan/Advertising controls, nearby-device empty state, send/receive counters and Ping/binary test controls. This is successful missing-native UI evidence. It does not assert Feature registration or module activation; the installed Astrolink native module is still the previous FAILED build. No module reinstall or reboot occurred during this diagnosis. Earlier black screenshots remain historical evidence of the old harness behavior, not a current UI blocker.

A bounded live-log probe independently found current timestamped syslog text around SRAM `0x20100000..0x2010ffff`. One 64 KiB `pmemsave` read after launch was parsed in-process, with known-key/hex redaction; only sanitized timestamped lines and addresses are retained at `/tmp/astrolink-smoke-launch-link-drain/guest-logs/live-sram-64k.log`. The temporary raw memory file was immediately deleted. This avoids the older helper's whole-PSRAM dump and avoids mistaking a long stale log copy for the active ring. The exact logger head/tail control structure is not recovered; the observed address window is specific to this running `.155` image.

## Current strict native probe and recovery

Transport-only commands report transport success. The newer `probe` command is
stricter: after confirming native module residency and launching the probe RPK,
it captures the bounded `.155` SRAM log and requires the runner's terminal
`complete` event. A native `failed` event, failed cleanup, or absent terminal
completion makes the command fail even if installation and launch were
acknowledged. See `native-validation.md` for current native results; historical
installation/launch passes above do not establish successful scanning or CoC.

`canopus exit-safe-mode --safe-mode-action keep|disableAll|rollbackAll` uses the
real recovery operation. Success schedules a recovery trial for the next boot;
safe mode remains active for the current boot. Reboot and check
`canopusDeviceInfo.safeMode` before installing or enabling modules. `disableAll`
permits clean recovery without automatically restarting a faulty module, then
the updated module can be enabled separately.

