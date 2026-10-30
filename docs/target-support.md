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
| `xiaomi-band-10-pro-3.101.043` | 3.101.043 (P67) | ABI recovered, integration absent | All four hosts present, but the Feature host is the `ferry`/AIOTJS C++ framework, not the Band 11 Feature framework — see below |
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

## Why P67 is a discovery job, not a port

Measured, not assumed (`../Canopus-Private/targets/xiaomi-band-10-pro-3.101.043/evidence/p67-astrolink-coverage.md`):

- Matching `.155` against `.043` proposes 313 of 316 functions, but only 46 of
  the 206 proposals this pack can verify independently agree with an address it
  already recovered — 22%. Whole-body identity is worse: 19 of 342 names.
- `uv_loop_t` is a different build. Queue head, mutex and async keep 68/76/104,
  but the loop's pipe descriptors live at `+360/+364` behind a `-1` guard, and
  the per-task loop state is 288 bytes instead of 712.
- libuv carries two more sources and NuttX three more than on q66.
- The loop is reached through a pthread key into an 808-byte TLS block whose
  `+760` word holds the default loop, and `uv__get_loop` exists as a callable
  function there — so adapting it means calling that getter, not re-deriving the
  q66 offset chain.

So the 40 remaining bindings have to be found on that image with local evidence
(producer/consumer, `__FILE__` literals, the `[FEATURE]` and `[bluelet]` format
strings), not transferred. Filling them from cross-line scores would put
plausible wrong addresses into a pack that is supposed to prevent exactly that.

## Why P67 is not a port and not yet a backend

The address layer is done: 41 recovered addresses plus six compositions, every
one with producer evidence or device measurement
(`../Canopus-Private/targets/xiaomi-band-10-pro-3.101.043/evidence/p67-astrolink-coverage.md`).
What is missing is not addresses, it is the contract:

- **Feature host.** On Band 11 the module constructs a framework object and an
  Application, calls the Feature registration entry and reaches the manager
  through a vtable at a fixed global. On P67 the Feature runtime is entered
  through C++ virtual dispatch on **JS objects whose class id is assigned at run
  time** (`ensureJsClass` reads a `JSClassID` global), and the object that holds
  the app list and the framework mutex belongs to the `ferry`/AIOTJS application
  framework — a different subsystem from the one the Band 11 constants describe.
  Reusing those offsets would produce plausible wrong values, and there is no
  measured equivalent yet.
- **Data path.** P67 has no SDU buffer or 16-byte fragment node. Sending hands a
  caller-allocated stock buffer (`+0` total length, `+2` payload offset, `+4`
  cid, `+8` header nibble, `+10` flags, payload at `+4+offset`; the handler frees
  it) to `bt_l2cap_submit_cid`, which re-fragments in place into 8-byte nodes.
  Receiving delivers a 12-byte `{handle, buffer, end}` node. Both are shimmable,
  but the module's send and receive paths have to change shape.
- **Handle teardown.** `uv_close` has no located entry on this image and the
  unlock primitive could not be isolated, so the module's async-handle lifetime
  has no verified implementation.

So P67 is where Band 9/10 are not: the pieces exist and the addresses are
evidenced. Finishing it means writing a **second integration** — registration
through `ferry`, its own send/receive shims, and a teardown path — not filling in
offsets.

## What stops the P67 module

The pack now carries 41 of the 47 firmware bindings, every one with
producer/consumer evidence, plus the structures the module needs
(`evidence/p67-astrolink-coverage.md` §7/7b). Four things still stand between that
pack and a buildable `src/abi/xiaomi-band-10-pro-3.101.043.rs`:

- `qjs_new_string_len`: the allocation half is known (`len + 17` through the
  runtime malloc hook at `rt+16`/`+8`, refcount `+0`, length `+4`, hash `+8`, NUL
  at `+16+len`), but not the wrapper that turns that storage into a tag `-7`
  value. `0xC278140` interns an atom instead.
- `qjs_to_cstring_len2`: no candidate in the 340-380 byte range satisfies
  "allocate `len+1`, copy, NUL-terminate, return the raw pointer"; the one size
  match is a `StringBuffer` helper.
- The array-buffer trio (`new_array_buffer_copy`, `get_array_buffer`,
  `get_typed_array_buffer`): their class ids are built at runtime. The
  `ArrayBuffer`/`Uint8Array` names live in a name blob that no pointer in the
  image references, so ids cannot be read statically. `.155`'s `19` is plausibly
  the same, but the pack policy is no address without evidence.
- Two call shapes must change on the module side, deliberately: `F_FILE_WRITE`
  has to become the writev form (`0xC1D2540`, or `nx_write 0xC1D2640`), and
  `define_property_value_str` is inlined here, so the module must compose atom
  create (`0xC294DCC`) + define + atom release (`0xC2780B0`) itself.

Getting the rest needs either a runtime check on a device (the class ids) or a
pass that identifies the runtime's class registration; neither is a guess this
pack should contain.

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
different addresses. The 18-check native lifecycle probe passes on the Q66
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
