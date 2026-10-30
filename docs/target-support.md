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
| `xiaomi-band-11-4.100.155` | 4.100.155 (Q66) | adapted | Recovery target; `docs/ble-abi.md`, `docs/qjs-abi.md` |
| `xiaomi-band-11-4.100.139` | 4.100.139 (Q66) | adapted | Ported with `tools/fw-match`; `docs/target-139-recovery.md` |
| `xiaomi-band-11-4.100.179` | 4.100.179 (Q66) | adapted | Ported with `tools/fw-match`; `docs/target-179-recovery.md` |
| `xiaomi-band-10-pro-3.101.043` | 3.101.043 (P67) | pending | Bluelet LE CoC confirmed present; GAP client and Feature registry open |
| `xiaomi-band-10-pro-3.101.036` | 3.101.036 (P67) | pending | Same P67 line as `.043` |
| `redmi-watch-6-3.100.043` | 3.100.043 (P65) | pending | Different product line; only identity and strings checked so far |
| `xiaomi-band-9-3.1.32` | 3.1.32 (N66) | pending | Pack records classic Bluetooth only |
| `xiaomi-band-9-pro-3.1.190` | 3.1.190 (N67) | pending | Pack records classic Bluetooth only |
| `xiaomi-band-10-3.1.28` | 3.1.28 (O66) | pending | Pack records classic Bluetooth only |

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

All three adapted targets build and pass `canopus verify` with the same 50,541
resident bytes and 1,244 relocations, which is expected: one code image,
different addresses. `docs/native-validation.md` records the 18-check native
lifecycle probe on the Q66 `.155` device.

The `.139` and `.179` devices in the isolated Mirage library have the Supervisor
resident and the module installed, but they do not reach `ACTIVE` without
another activation step: after `Run` the registry reports `INSTALLED`/`ENABLED`
and the module never loads. That is Supervisor boot plumbing on those devices,
not an ABI result, so no probe result is claimed for them. Two further findings
from the same session are recorded so they are not rediscovered:

- The Supervisor does not autoload on a `.139`/`.179` device after reboot even
  with `canopusSetBootPersistence {pinned: true}`, while the `.155` device does.
  `scripts/mirage-bootstrap.py` therefore presses `Run` after each boot.
- `canopus package` for three targets produces a package the Supervisor rejects
  (`Canopus install returned Failed`); a single-artifact package built from the
  same sources installs. Probe and ship per target until that is fixed.

## Adding a target

1. Record the symbol and data records in
   `../Canopus-Private/targets/<target>/symbols` with their evidence ids.
2. Regenerate the pack so `canopus_target_generated` exposes the constants.
3. Add `src/abi/<target>.rs`, reusing `crate::abi` names one to one.
4. Add the `target-<id>` feature in `Cargo.toml` and the id to
   `[targets] include` in `Canopus.toml`.
5. Build, `canopus package verify`, then run the probe on that firmware.
6. Commit the pack and the module separately.
