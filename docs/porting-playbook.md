# Fast porting recipe

This is the shortest path that still produces evidence for every address. It is
what the `.139`/`.179` ports actually took; the parts that felt slow are listed
as traps at the end so they are not repeated.

The rule that keeps it fast: **match the existing pack, do not rediscover the
module's needs.** An adapted pack already names every firmware entry the module
calls, so a new firmware is a matching problem (a few hundred known queries)
rather than a reverse-engineering problem (find four hosts from scratch).

## 0. Inputs, once per firmware

An AP image plus an IDB, and the target's pack scaffold:

```sh
python ../Canopus-Private/tools/fw-match/extract_corpus.py \
  --idb ~/develop/mifw/analysis-<dev>/vela_ap.bin.i64 \
  --target-id <target> --output ../Canopus-Private/targets/fw-corpus/<target>.json
python ../Canopus-Private/tools/fw-match/fingerprint_corpus.py \
  --corpus ../Canopus-Private/targets/fw-corpus/<target>.json \
  --firmware ~/develop/mifw/analysis-<dev>/vela_ap.bin \
  --output ../Canopus-Private/targets/fw-corpus/<target>.json
```

Do the same for the **source** firmware. Pick the source by proximity: same
family, nearest build. `.139 → .155` transfers far better than `.139 → .179`,
and `.179` is best ported from `.155`. Never port across product lines.

## 1. Match the whole pack in one pass

```sh
python ../Canopus-Private/tools/fw-match/ensemble_match.py \
  --source-symbols ../Canopus-Private/targets/<source>/symbols \
  --source-corpus .../<source>.json --target-corpus .../<target>.json \
  --target-id <target> --target-firmware-sha256 <sha> \
  --output build/ensemble-<source>-to-<target>.json
```

Expect ~99% of the pack's entries to come back as unique full-body anchors. The
report's `ACCEPTED` set is the fast path; everything else is the residual you
review. Reward short functions with an oracle: ten reviewed critical-path
entries in the source pack catch systematic drift early and cost minutes.

## 2. Accept, review, and map data in that order

```sh
# normalized pseudocode second opinion for the residual
python ../Canopus-Private/tools/fw-match/port_review.py dump|compare ...
# write the accepted records with the port provenance
python ../Canopus-Private/tools/fw-match/port_pack.py ... --out-dir out/symbols
# data objects: only those the module actually dereferences
python ../Canopus-Private/tools/fw-match/map_data.py ... <address...>
```

Ported records must carry `STATIC_RECOVERED` + `PENDING` and no device proof from
the source. A match is accepted on equal body or identical pseudocode **plus**
consistent anchored callees; anything weaker goes to the overrides file with a
written reason.

## 3. Split the work by what actually varies

This is the part that saves the most time on a sibling firmware:

| Layer | Varies per target? | How to settle it |
| --- | --- | --- |
| Firmware entry addresses (GAP, LE CoC, libuv, `qjs_*`, Feature) | yes | steps 1–2 |
| Data globals the module dereferences (`uv_loop_key`, pipe ops, MiWear instance, L2CAP owner) | yes | `map_data.py`, one record each |
| NuttX/libuv **structure offsets** (loop, TCB, filelist, async words) | measured identical across the Band 11 line | re-read once per **family** with the anchor list in `docs/loop-abi.md`; only re-derive if a probe check fails |
| Framework/manager vtables and Feature slots | layout identical within a family, addresses move | `port_pack.py` output |
| Module-side layout (channel tables, work records, session fields) | no | nothing to do |

So a sibling port is: ~250 addresses, ~4 data globals, one offset audit, done.

## 4. Build, then prove on device

```sh
cd ../astrolink
../Canopus-Private/target/debug/canopus build      # verify catches a wrong or unrecorded address
cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt
python3 scripts/mirage-bootstrap.py --library <lib> --target <target> --device <dev> \
    --bridge <dylib> --module dist/<pkg>.canopus --probe examples/quickapp/dist/astrolink-probe-source.rpk
```

`canopus build` refusing an address that is not in the pack is the cheapest gate:
it turns "did I record this?" into a build error instead of a device mystery.

The bootstrap script is the whole device step: pair, install the installer
watchface, stage assets, boot, press `Run`, wait for `BOOT_RESIDENT`, install the
package, verify the resident version, run the 18-check probe. It refuses to trust
an install whose version did not become resident.

## 5. Commit in three steps

1. pack records + regenerated `canopus-target-generated` (one target per commit)
2. the `src/abi/<target>.rs` module, every constant annotated with its anchoring
   function
3. docs: the port's evidence page, and the probe summary once a device passes

Keeping the pack separate from the module means a wrong address has exactly one
commit to revert.

## When the shape differs, not just the address

Two Firmware facts that an address-only abi module cannot express, both found on
p67, so a port has to decide them in code rather than by substituting a number:

- `F_FILE_WRITE(file, buf, len)` assumes a scalar write. p67 has no such
  function: only `file_writev(file, iovec)` (`0xC1D2540`) and `nx_write`
  (`0xC1D25F4`), which builds a one-element iovec and calls it. Adapting means
  constructing an iovec, not finding another address.
- `F_FS_PUTFILEP(file)` exists on p67 as a real one-argument release, but IDA
  never marked it as a function (it sits as a `loc_`), so a symbol-table-driven
  search misses it. Trust the reference-count decrement plus the list release,
  not the function list.

The general lesson: recover the *contract* (arguments, ownership, failure) for
each binding, and note when a target cannot satisfy the current call shape. That
is a module change with a review, not a pack edit.

## Traps that cost this session hours

- **A same-`versionCode` install never applies.** The Supervisor stages
  `/canopus/inbox/<id>.ko.next`, reports `Failed`, and the rebooted device keeps
  the old image. Any probe after such an install measures the previous build.
  Bump the version code, or remove the module first, and check the resident
  version before trusting a probe result.
- **`BOOT_RESIDENT` (13) is the only state worth probing.** `INSTALLED` (3) and
  `ENABLED` (5) look healthy and answer queries; the module is not running.
- **`INODE_OPS_PIPE` is per target.** The ten handlers behind the table are the
  same, the address is not. A foreign value fails with `BACKEND (-9)` at the
  first loop wake, which reads like a scan bug and is not.
- **The device is not the bottleneck; the install path is.** Rebuilding, matching
  and verifying all three targets takes seconds. Only the device cycle costs
  minutes, so make it one command and never iterate by hand.
- **Do not open the emulator UI.** Every observation it gives you is available
  from `miragectl screenshot` and the companion bridge, and tapping by hand is
  how the same-version install was mistaken for a working one.
