# Target support matrix

Astrolink needs four firmware hosts at the same time: the MiWear GAP IPC client
(scan/advertising), the Bluelet legacy LE CoC owner, the libuv/NuttX task
plumbing, and the QuickJS Feature registry. A device is only adapted when all
four are recovered for its **exact** firmware and recorded in
`../Canopus-Private/targets/<target>/symbols`, because `canopus build` and
`canopus verify` accept only addresses declared in that pack.

The addresses and structure offsets of an adapted target live in
`src/abi/<target>.rs`. A target whose hosts have not been recovered declares
`abi_unsupported!()`: the module still links and loads, publishes the Feature as
unsupported and never calls firmware. `tests/targets.rs` keeps
`Canopus.toml`'s include list and the adapted modules in sync, so a device
cannot be silently added or dropped.

| Target | Firmware | Status | Notes |
| --- | --- | --- | --- |
| `xiaomi-band-11-4.100.155` | 4.100.155 (Q66) | adapted, probed 18/18 | Recovery target; `docs/ble-abi.md`, `docs/qjs-abi.md` |
| `xiaomi-band-11-4.100.139` | 4.100.139 (Q66) | adapted | Ported with `tools/fw-match`; `docs/target-139-recovery.md` |
| `xiaomi-band-11-4.100.179` | 4.100.179 (Q66) | adapted, probed 18/18 | Ported with `tools/fw-match`; `docs/target-179-recovery.md` |
| `xiaomi-band-10-pro-3.101.043` | 3.101.043 (P67) | implemented; native probe in progress | Independent ferry registration, 112-byte async, generic GAP/CoC; initial module activation verified, first QuickApp run reset firmware |
| `xiaomi-band-10-pro-3.101.036` | 3.101.036 (P67) | pending | Same P67 line as `.043`; its pack was built from `.036` matches, so it ports in the other direction once `.043` has a complete ABI |
| `redmi-watch-6-3.100.043` | 3.100.043 (P65) | unsupported | No Feature runtime (`FeatureManagerQjs` x0) and no Bluelet BLE host; pack records the finding |
| `xiaomi-band-9-3.1.32` | 3.1.32 (N66) | unsupported | Has libuv, Bluelet LE CoC and GAP, but no Feature runtime (`FeatureManagerQjs` x0); pack records the finding |
| `xiaomi-band-9-pro-3.1.190` | 3.1.190 (N67) | unsupported | Same verdict as its `3.1.32` sibling; its own AP is in a `ZZ~` container that does not extract, so the evidence is explicitly sibling-level |
| `xiaomi-band-10-3.1.28` | 3.1.28 (O66) | unsupported | No Feature runtime and no L2CAP channel names; a Supervisor and Manager do run there, but no module has ever registered |

The `unsupported` rows are decided, not deferred: Astrolink publishes its
service through the firmware's QuickJS Feature runtime, and those images were
built without it. Each pack carries a
`capability_finding.feature_runtime_absent` record naming the probe that
established it, so the verdict is auditable and cannot be mistaken for a
missing address.

## P67 native integration

P67 now has its own ferry registration implementation in `src/qjs/ferry.rs`.
It wraps the real FrameworkExt application factory and Application initializer
through RAM virtual-table pointers, then registers the resident descriptor into
each actual application's native registry. It retains the stock host, runtime,
manager and application behavior. Creating an unrelated manager from a minimal
host does not publish a Feature in those applications.

Measured lifecycle arguments remain native FeaturePrototype/FeatureInstance
pointers. Their manager and JSValue offsets match the shared lifecycle code.
The target allocates a 112-byte async handle, uses the recovered `uv_close`,
and retains the loop pipe file for notification across NuttX task groups.

The generic GAP producer takes the ordinary 28-byte advertising parameters;
the five-argument Bluelet queue retains separate run and cancel callbacks.
Ordinary CoC request/event layouts, the original-SDU send handler and the
state-dependent disconnect policy are recorded from this exact firmware.
All six required QuickJS value functions have direct engine entries.

Earlier notes claiming missing QuickJS wrappers, a mandatory writev composition,
JS objects as lifecycle arguments, a four-argument queue, or unavailable
`uv_close` confused interfaces or missed function boundaries. Those claims are
superseded by `docs/p67-shim-contract.md`, `docs/p67-native-contract.md`, and the
exact target-pack evidence committed in Canopus `ae80a30`.

The .043 module builds and passes ELF verification. Its initial v39 installation
reached BOOT_RESIDENT with activationError zero in the isolated Mirage device.
The first native QuickApp launch reset firmware, so lifecycle and radio checks
are **not yet passed**. First-fault debugging and final-build verification are
in progress; a successful module activation alone does not close that gate.

## What "recovered" requires

For each target, all of the following must be evidenced from its own IDB, never
inherited from another firmware:

1. GAP: scan create/stop, advertising create/stop, adapter register/get-state/
   enable, the MiWear IPC instance global, and the scan report and advertising
   parameter layouts.
2. LE CoC: the L2CAP owner global, dynamic LE PSM lookup/register/release, the
   connect, accept, disconnect and send owner handlers, the credit queue drain,
   the 68-byte connect/accept request layout, and the channel and event field
   offsets.
3. Task plumbing: the default loop's ready queue, mutex, async handle and pipe
   endpoints, the NuttX TCB/TLS/filelist offsets used to reach them, and the
   file duplication primitives.
4. QuickJS: the `qjs_*` entry points, the Feature registration entry, the
   framework and Application vtables with their dispatch slots, and the
   framework/manager/instance object offsets.
5. Twelve entry bytes of the four sanity-checked functions, so the module
   refuses a mismatched image instead of calling it.

A target stops when any item can only be guessed. Recording a plausible-looking
address without producer/consumer evidence is the one outcome this matrix is
meant to prevent; the correct result is `abi_unsupported!()` plus a note here.

## Validation status

Four targets currently build and pass `canopus verify`. Exact sizes and
relocation counts vary by host implementation and are recorded with the
corresponding validation build; build success establishes no runtime result. The 18-check native lifecycle probe passes on the Q66
`.155` device (`docs/native-validation.md`) and on the Q66 `.179` device.

`.139` is adapted, statically recovered and builds, but it has no device result.
In Mirage its Supervisor never answered the companion channel on any boot: the
installer watchface reports `Run completed`, the Run button is pressed after each
boot, and the module is never reachable. `.155` and `.179` answer on the same
library, the same installer build and the same activation script, so the
difference is on the `.139` device side, not in this module. Rebuilding that
target's installer assets from the current pack did not change it, and the guest
logs on that firmware carry no application output to diagnose with, so the
investigation stops here rather than guessing. Its offsets and addresses are
evidenced statically in `docs/loop-abi.md` and `docs/target-139-recovery.md`.

Device findings worth keeping, so they are not rediscovered:

- An install of the **same** `versionCode` only stages
  `/canopus/inbox/astrolink.ko.next` and the Supervisor reports `Failed`; the
  rebooted device keeps running the previous `.ko`. Removing the module first
  makes the install replace it. A probe that follows such an install measures
  the old image unless the staged sha is checked.
- A freshly installed module stops at `INSTALLED` (3). It needs
  `canopusSetEnabled(true)` and one more Supervisor start before the registry
  reports `BOOT_RESIDENT` (13), which is the only state in which the probe is
  meaningful.
- The Supervisor does not autoload on a `.139`/`.179` device after a reboot even
  with `canopusSetBootPersistence {pinned: true}`, while the `.155` device does.
  `scripts/mirage-bootstrap.py` presses `Run` after each boot for that reason.
- An earlier revision of this matrix claimed a three-target package is
  rejected while a single-target one installs. That was wrong: the rejected
  installs reused a version code, so they were staged and never applied. One
  package with three artifacts and a fresh version code installs and runs
  (version 38 on the `.179` device).

## Adding a target

The fast path is written down in `docs/porting-playbook.md`: a sibling firmware
is a matching job against the existing pack, not a rediscovery of the module's
needs.

1. Record the symbol and data records in
   `../Canopus-Private/targets/<target>/symbols` with their evidence ids.
2. Regenerate the pack so `canopus_target_generated` exposes the constants.
3. Add `src/abi/<target>.rs`, reusing `crate::abi` names one to one.
4. Add the `target-<id>` feature in `Cargo.toml` and the id to
   `[targets] include` in `Canopus.toml`.
5. Build, `canopus verify`, then `scripts/mirage-bootstrap.py --module <pkg>
   --probe <rpk>`, which installs, reboots and runs the probe headless.
6. Commit the pack and the module separately.
