# Exact-target SDK build and address verification

Astrolink's native integration targets Q66 `4.100.155`, AP SHA-256
`ea0bdf1920cb30223d616432af00565ca67622e6468328f5eab155f8cdc2fb9f`.
Build verification uses the sibling `Canopus-Private` checkout. The SDK changes
below were made on 2026-10-29; neither change implies physical-device approval.

## Rust module reachability

SDK commit `09be67e` changes the Rust module's final relocatable link in
`cli/canopus/src/project.rs` to use:

```text
ld.lld -r --gc-sections -u canopus_module_descriptor -u canopus_mod_stop ...
```

The descriptor and stop entry are explicit roots. Descriptor relocations retain
lifecycle callbacks and their reachable implementation; LLD also retains
`.init_array`/`.fini_array` and the constructor/destructor functions referenced by
them. Unreachable Rust runtime sections are discarded before the firmware ELF is
verified. The module remains an ARM ET_REL object with all runtime references
resolved; the loader still performs its normal relocation and entry processing.

Previously the partial link retained unused Rust archive sections. Astrolink's
public UTF-8 helpers caused large libcore objects to be extracted even while the
device entry was still the scaffold. The resulting ELF occupied 197,132 resident
bytes, including Unicode and floating-point tables. Some aligned table words
coincidentally fell into the firmware address range, and the address verifier
correctly reported unknown addresses. Adding those incidental values to an
allowlist would hide the underlying linking problem.

With reachability enabled, the same scaffold occupied 1,144 resident bytes and
passed the unchanged verifier: 13 sections, no undefined symbols, 17 relocations,
one constructor and one destructor. These numbers describe the isolated scaffold
check, not the completed BLE module's size.

A fresh target-bound Rust template was also built outside the repository. Adding
an unused exported Rust float parser produced a byte-identical verified ELF
(SHA-256 `ef6f35315d75be0be3774fde5a51f5bfa6bd77244599dd54f725982b3d2b7975`):
1,136 resident bytes, 13 sections, 16 relocations, one constructor/destructor.
Inspection retained the module descriptor, prepare/stop, activation/query code
and constructor/destructor, and removed the unused parser. This exercises the
failure mechanism without changing the verifier or registering table values.

## Exact firmware address records

SDK commit `0835af0` adds 32 QJS and BLE functions used by the native integration to
`targets/xiaomi-band-11-4.100.155/symbols/`, with shared evidence records:

- `EVID-ASTROLINK-4155-QJS-001`: native JS values/functions, Feature registration,
  owner-manager task posting, and mutex operations.
- `EVID-ASTROLINK-4155-BLE-001`: GAP scan/advertising handles and owner-task LE CoC
  registration, channel connection/accept/disconnect, send and queue operations.

Every new function has its even entry and Thumb callable address, fixed exact
firmware hash, AAPCS prototype, ownership/context notes and evidence reference.
All remain `restricted`, `STATIC_RECOVERED`, `approval_state=PENDING` and
`device=not_probed`. The existing verifier derives its allowed-address catalog
from symbol records. Recording a private address permits verification of this
explicit module use; it does not grant public generated-callable approval.

Target-derived C veneer and Rust binding metadata were regenerated together.
No firmware address range was expanded, and no verifier scan was disabled.
Existing heap, adapter and Bluelet work-queue records were reused. The exact AP
hash and all 32 new instruction-entry prefixes were independently checked against
the extracted image (VA base `0x0c0c0000`) and captured in the evidence bundles.

The recovered ABI and ownership details are documented in
[QJS ABI](qjs-abi.md) and [BLE ABI](ble-abi.md). Simulator import and lifecycle
results and physical radio evidence are separate integration gates.

## Checks completed

- Rebuilt `canopus-cli` and used that executable for exact-target builds.
- Three existing CLI integration tests passed.
- Nineteen `canopus-core` generated-stability tests passed, including private ABI
  Thumb addresses and the pinned public-approval gates.
- Eighteen `canopus-core` library tests passed.
- Four codegen-approval gate tests, eight schema-fixture tests and two SDK hygiene
  tests passed.
- `canopus target generate xiaomi-band-11-4.100.155 --check` passed.
- Fresh target Rust template and unused-float-parser regression probe passed the
  normal build/verifier pipeline with constructor/destructor entries preserved.

## First integrated artifact

After wiring the resident lifecycle, exact BLE backend, QuickJS Feature and
module runtime, `canopus package --build` passed the unchanged verifier and
`canopus package verify dist/astrolink-0.1.0.canopus` succeeded.

First integrated snapshot:

- ELF: `build/xiaomi-band-11-4.100.155/astrolink.elf`.
- ELF SHA-256:
  `4388ffc3310b54b64722a3b9828bdaa1c87d9819337cbf8b33e6427d75ea2db1`.
- Resident size: 42,663 bytes; 15 sections; no undefined symbols; 1,011
  relocations; one constructor and one destructor.
- Package: `dist/astrolink-0.1.0.canopus`, 84,992 bytes, one exact-target artifact.

These numbers describe the first integrated build on 2026-10-29; later integration
fixes require rebuilding and recording their own hashes. Module loading, Feature
execution and physical BLE behavior are separate runtime checks.


## Activation -38: Flash interception boundary

The first real Supervisor activation returned `-38` from the QJS hook request.
Inspection of the actual implementation establishes that this is not a missing
address-catalog entry:

- `runtime/hook/canopus_hook.c:253` returns UNSUPPORTED for a Flash function
  without a registered RAM dispatch mapping or writable platform code view.
- `manager/service/canopus_supervisor_platform.c:119` permits inline patching
  only inside the requesting module's own Umem executable image; its write view
  points to the corresponding PSRAM data allocation.
- `FeatureRegisterFeature` at `0x0c495f2c` resides in firmware Flash, so that
  module-only code view correctly rejects it. An allowlist entry cannot turn
  Flash into writable PSRAM.
- `runtime/hook/canopus_hook.h` and `ADR-CAN-008-wearpacket-l2-channel.md` explicitly
  exclude Flash writes and unsupported Cortex-M33 Flash remapping. The Thumb
  hook test also asserts that read-only Flash is rejected for all selector types.

No broad hook exception or invented writable Flash alias was added. Safe
integration requires a recovered RAM dispatch/lifecycle registration opportunity,
then registration of the exact owned RAM mapping if the hook service is used.
The existing static function catalog remains valid, but the proposed direct
Flash inline interception is not executable through this Supervisor.

The actual `.155` Supervisor and installer assets were rebuilt with
`scripts/build_canopus_supervisor.sh`; the output was byte-identical to the
tracked production assets. It passed the unchanged verifier (133,596 file bytes,
185,910 resident bytes, 18 sections, zero undefined symbols, 2,260 relocations,
two constructors and one destructor). Reinstalling those same assets does not
change this Flash interception boundary.

Files below are under
`../Canopus-Private/watchfaces/canopus-installer-prod/xiaomi-band-11/`:

| Asset | SHA-256 |
| --- | --- |
| `main.lua` | `6824c35c689e24cf767e109b4da410ac37be1cf9052274b8fbb4d4aca7b96f09` |
| `canopus_loader_profile-xiaomi-band-11-4.100.155.bin` | `25bcf7861b83af75be88e3e581b7a6a0f8b9514ff078d7f56d4a0f35735a7257` |
| `canopus_stage1-xiaomi-band-11-4.100.155.bin` | `e7352a9b7fdad066810ac739864b67bc79e6207b0eceb7907e9b3da793e3204a` |
| `canopus_stage2-xiaomi-band-11-4.100.155.bin` | `1a8e7fdbd5fe53c9dd1a5d6197124aedf2d52c6eadf9f1aaf37316a6aeb334dc` |
| `canopus_supervisor-xiaomi-band-11-4.100.155.bin` | `2e53115b28ca29f400fd9f1ff8233198ab7a11176880408f9032e1cebdd279f0` |
| `canopus_boot-xiaomi-band-11-4.100.155.bin` | `9c5279cae17d3fb4dd22d61fda0bbe70d937862e9b3e944e88451244652b724f` |

The optional Thumb/Unicorn test was attempted in the available system Python and
explicitly skipped with exit 77 because its `unicorn` dependency is absent; no
execution result is inferred from that skip.
