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

