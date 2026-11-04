# P65 v45 isolated runtime validation

Historical v45 failure record. The captured root cause, virtual-table correction
and v46 runtime result are in `docs/p65-v46-runtime-validation.md`. The v46 run
preserves slot27 correctly and reaches JavaScript; its native-interface loading
still fails before lifecycle checks.

Scope: Redmi Watch 6 exact firmware `3.100.043`, isolated Mirage library
`/tmp/astrolink-p65-validation`. No physical-radio acceptance is claimed.
Machine-readable build and runtime results are in
`docs/evidence/p65-v45-validation.json`. The full static producer/consumer
reports live alongside it as `p65-gap-contract.md`, `p65-loop-ownership.md`
and `p65-coc-events.md`.

## Outcome

The experimental v45 module and source lifecycle probe install through the real
encrypted companion transport. Official CPC2 ACTIVATE succeeds, and an actual
module query returns version45, state13 (`BOOT_RESIDENT`), activationError0.
This proves native installation and activation, not QuickApp compatibility.

Launching `com.canopus.astrolink.probe` with the module resident reboots the
firmware before any native check or terminal completion event is captured.
The repeated launch under QEMU exception tracing records an INVSTATE UsageFault
on CPU0, immediately followed by reset. No lifecycle pass, GAP operation,
HELLO or SDU acceptance is claimed. Bidirectional CoC testing is blocked here.

The regular source keeps `SUPPORTED=false`, and its normal build output was
restored to the gated path. Only `/tmp/astrolink-p65-validation-v45.canopus`
contains the enabled experimental code. No runtime support or target-pack
approval state is promoted.

## Verified artifacts and checks

- 42 Rust host tests pass (13 runtime,26 core,3 target guards).
- P65 and Q66 `.155` device builds pass ARM Clippy with `-D warnings`.
- `cargo fmt --check`, `git diff --check` and exact-target generation `--check`
  pass after rebuilding the current SDK CLI.
- The experimental ELF passes unchanged Canopus verification:71369 resident
  bytes,0 undefined symbols,1227 relocations.
- ELF SHA-256:`a6e99d15e62cf5d0c33cc73a1226141b6d8f3a3cc470e6190c6960c1005f35c2`.
- The single-target v45 package passes package verification.
- Source probe typechecking and RPK build pass. Probe SHA-256:
  `65597050d2f4a6df1b77f4db98f04139ca1542151d235d30a914c26e5cb9710f`.

## Command-line bring-up prerequisites

Use the production staged cave loader, as documented in
`docs/evidence/p65-supervisor-runtime.md`. Direct stock `insmod` reboots this
image; its exact fault instruction was not captured. The selected target profile
requires the executable-image MPU lease supplied by stage1/stage2.

Wait until the real MiWear task and its UI loop have initialized before running
the bootstrap. Loading too early can create `/dev/canopus` without a functioning
owner pump. Waking the screen and verifying actual `canopusDeviceInfo` is required
before trusting installation readiness. On this exact setup:

- Device bootstrap:`/data/canopus/bootstrap.sh`, generated from built stage1
  and the existing production trampoline; execute once per boot, never twice
  while resident.
- Create `/data/canopus/inbox` before CPC2 staging. Missing inbox returned
  command15 (`STAGE_BEGIN`) Failed. Offline NAND creation was used on the fresh
  isolated device, then the Supervisor was reloaded.
- Short console commands avoid the native UART input limit. The raw-console
  helper uses `cd /data`, `cd canopus`, then `. bootstrap.sh` rather than one
  long command. This changes neither the bootstrap bytes nor firmware calls.
- Use `/tmp/astrolink-p65-channel5-astrobox.dylib`: actual P65 SAR companion
  works on advertised RFCOMM channel5; generic channel6-first selection closes
  during SAR handshake. Only a temporary bridge copy changes channel order.
- Fresh standard SSP was needed after backing up/removing the isolated stale
  Classic link-key cache. Authentication and encryption then passed. No key
  material is retained in this report.

## Actual installation and activation

The validated command prefix is:

```sh
python3 scripts/mirage-smoke.py \
  --library /tmp/astrolink-p65-validation \
  --device astrolink-p65-043 \
  --bridge /tmp/astrolink-p65-channel5-astrobox.dylib \
  --miragectl /tmp/astrolink-p65-mirage-build/debug/miragectl \
  --output /tmp/UNIQUE-EVIDENCE-DIRECTORY --settle 0 --timeout 25
```

Append these operations in sequence, giving each a distinct evidence directory:

```sh
install --module /tmp/astrolink-p65-validation-v45.canopus \
  --quickapp examples/quickapp/dist/astrolink-probe-source.rpk
canopus enable --module astrolink
canopus activate --module astrolink
```

The actual install evidence is `/tmp/astrolink-p65-v45-install-awake/`.
The install returned success for both artifacts; module query reported
version45/state3. Enable returned state5. Activation evidence is
`/tmp/astrolink-p65-v45-activate/` and returned state13/error0.
A later explicit activation reproduced the same result in
`/tmp/astrolink-p65-v45-capture-activate/`.

## Latest Supervisor cross-check

After the startup fault, remote `origin/main` was queried directly and matched
local HEAD `81ba8d9ad90a6b3ab29622795aed8580148c26ec`. This includes P65 fix
`7b0dceefaa312e475be83179bfab3822ea42a8a7`, which reserves the full Supervisor
MPU window in stage2 so another executable allocation cannot overlap its tail.

The Supervisor and stage2 were rebuilt again from that current checkout and
both files were read back from the stopped isolated device's NAND. They match
byte for byte:

- Supervisor SHA-256:
  `52c9e345ff5d0ff3c2b20247278d19f571aa3a3f9ca3ef81cd243c38afb0f20e`.
- Stage2 SHA-256:
  `9eaceae46993e2a4e2ea11b479275f813e4edad7bd4da3a92c0842961552ced1`.

The failing probe therefore already used the current Supervisor and the fixed
stage2; replacing them with this identical rebuild would not test new code.
The exact faulting call still requires capture. This check does not identify
the cause of the INVSTATE UsageFault.

## Startup fault boundary

The first native attempt used:

```sh
# Use the same validated prefix, with the following overrides/operation:
--output /tmp/astrolink-p65-v45-native-probe \
  --settle 40 --timeout 25 --probe-log-source nsh probe
```

The script enabled INFO logging, confirmed version45/state13/error0, and sent
the real QuickApp launch. Launch acknowledges queueing only. The guest console
then starts a new firmware boot. No ASTROLINK check records or terminal event
were captured, and subsequent dmesg capture failed because that reboot removed
the injected shell.

The second launch evidence is `/tmp/astrolink-p65-v45-fault-reproduce/`.
Exception tracing around that launch contains:

```text
Taking exception 18 [v7M INVSTATE UsageFault] on CPU 0
...taking pending secure exception 6
...loading from element 6 of secure vector table at 0x20166018
...loaded new PC 0xc50dad9
Loaded reset SP 0x2019ffe0 PC 0xc0c0011 from vector table
```

The exact invalid instruction address, branch target and module initialization
stage were not captured. The shared exception entry at0x0C50DAD8 receives both
ordinary interrupts and faults, so this entry alone cannot localize the cause.
No speculative ABI or function-pointer correction was made from INVSTATE alone.

As a bounded control, after the guest reboot and without reloading Astrolink,
the same standard QuickApp launch was sent again. It did not add another reset
marker during the observation window (`/tmp/astrolink-p65-stock-probe-baseline/`).
There were no native test events and the screenshot was dark; that control does
not establish successful source evaluation or a passing probe.

Next recovery scope: capture the UsageFault's stacked PC/LR/xPSR and the exact
legacy extension callback target, then audit the producer/consumer call at that
address. Verify early extension initialization and native callback ownership
before retrying the lifecycle probe. Do not infer the faulting function merely
from the absence of JavaScript logs.
