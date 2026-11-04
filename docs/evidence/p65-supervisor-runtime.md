# P65 Supervisor command-line bring-up

Scope: isolated Mirage P65 firmware 3.100.043 only. No physical device, normal
Mirage library, installer UI, source/target-pack edits, or Astrolink installation.
The device remains running with its verified Supervisor resident at handoff.

## Verified outcome

`/tmp/astrolink-p65-channel5-status/summary.json` records a successful real
companion `status` call with:

```json
{
  "ok": true,
  "deviceInfo": {"firmwareVersion": "3.100.043", "model": "M2523W1"},
  "canopusDeviceInfo": {
    "targetId": "redmi-watch-6-3.100.043",
    "frameworkRevision": 1,
    "safeMode": false,
    "safeModeReason": 0,
    "errorCode": 0,
    "moduleCount": 0,
    "requestSlots": 16
  },
  "canopusModules": []
}
```

This was the real encrypted firmware SAR/Canopus path through AstroBox core,
not a fake Feature or synthetic companion response. The stock QuickApp list
also returned successfully. No Astrolink module has been installed or probed.

## Environment and usable artifacts

```sh
export MIRAGE_LIBRARY=/tmp/astrolink-p65-validation
export MIRAGE_QEMU=/tmp/astrolink-p65-qemu-build/qemu-system-arm
P65_CTL=/tmp/astrolink-p65-mirage-build/debug/miragectl
P65_BRIDGE=/tmp/astrolink-p65-channel5-astrobox.dylib
```

Device name `astrolink-p65-043`, UUID
`A85AE15E-DCCD-4804-A89C-94E75D515805`.

The supplied exact-target Supervisor was used without rebuilding:

- Local ELF: `/Users/lesetong/develop/rust/Canopus-Private/watchfaces/canopus-installer/build/redmi-watch-6-3.100.043/canopus_supervisor.elf`
- SHA-256: `52c9e345ff5d0ff3c2b20247278d19f571aa3a3f9ca3ef81cd243c38afb0f20e`
- Device ELF: `/data/canopus/supervisor.elf`
- Device stage2: `/data/canopus/stage2.bin`
- Device bootstrap: `/data/canopus/bootstrap.sh`

The bootstrap source is retained at `/tmp/astrolink-p65-bootstrap.sh`.
`/tmp/astrolink-p65-stage-bootstrap.py` reproduces offline staging using the
existing built production stage1/stage2/ELF, then boots the device. It performs
shutdown, so do not invoke it while keeping this resident instance.

## Correct loading path

The target's existing `loader/bootstrap.toml` specifies staged-kmem-mpu,
cave entry, `stage0_mpu=false`, and `image_mpu_lease=true`. The production
`watchfaces/canopus-installer-prod/redmi-watch/main.lua` function
`load_supervisor_cave` already defines the exact equivalent command sequence.
This experiment transcribed that sequence into an NSH script:

1. Stage built stage2 and the ELF in `/data/canopus/` using offline NAND writes.
2. Write the built stage1 words to cave+0x100 = `0x2019E900`.
3. Write its exact six-word LR-preserving trampoline at `0x2019E800`:
   `4803B510 47984B03 60104A01 BF00BD10 2019E840 2019E901`.
4. Execute `exec 0x2019E801`.
5. Read result word `0x2019E840`, then list `/dev/canopus`.

The command returned result **0** and `/dev/canopus` existed. Full NSH output
is `/tmp/astrolink-p65-stage-exec.log`; staging output is
`/tmp/astrolink-p65-stage-staging.log`.

After a future reboot, load once with:

```sh
MIRAGE_LIBRARY=/tmp/astrolink-p65-validation \
  /tmp/astrolink-p65-mirage-build/debug/miragectl sh astrolink-p65-043 \
  '. /data/canopus/bootstrap.sh'
```

Do not rerun the bootstrap while this Supervisor is resident. Native
`insmod /data/canopus_supervisor.elf canopus_supervisor` is not the selected
P65 loading path: it rebooted this isolated image before returning a prompt.
The profile explicitly requires an executable resident-image MPU lease that
this stock insmod path does not supply; staged loading does supply it and
works. The precise instruction that caused the direct-insmod reboot was not
captured, so the MPU diagnosis is a path/contract explanation rather than an
instruction-level fault claim. The parent requested handoff once the staged
path was verified, instead of further destructive fault reproduction.

## Two separate companion-connection issues

### Seeded Classic link key mismatch

The initial isolated `AstroBox/link-keys` cache produced HCI Authentication
Complete status `0x05`. The bridge reports all such failures as "pairing was
declined", even though no numeric-comparison dialog appeared in this case.
The old isolated cache was backed up to `/tmp/astrolink-p65-original-link-keys`
and emptied. A fresh standard SSP exchange then completed with status0,
stored a new type7 key, and enabled encryption mode2. The new isolated key
cache is now valid and should be retained. No key material is printed here.

Stock initial tutorials were dismissed through `miragectl tap` only, without
using an installer. The scripts that attempted automatic confirmation and
preserved their output are under `/tmp/astrolink-p65-connect-confirm.py` and
`/tmp/astrolink-p65-status-pair.py`; they are not needed after successful bond.

### SDP service ordering

The generic bridge discovers actual advertised SPP channels `[6,5]` and opens
the first accepted service, channel6. The real SarController then sends
L1StartReq and that SPP channel closes before companion authentication.

A `/tmp`-only copy of the Bluetooth crate and AstroBoxBridge was compiled:

- `/tmp/astrolink-p65-channel5-bluetooth/`
- `/tmp/astrolink-p65-channel5-bridge/`
- `/tmp/astrolink-p65-channel5-build.log`
- Usable dylib: `/tmp/astrolink-p65-channel5-astrobox.dylib`

The sole behavioral change in the copied Bluetooth source is stable channel
ordering before rfcomm_open_mux: put advertised channel5 before other channels.
It still uses real SDP, RFCOMM, SSP/encryption, SarController, native AstroBox
AUTH, and Canopus methods. No reply or target identity is substituted.
Channel5 completed actual companion authentication and all status queries.

The original bridge artifact at
`/tmp/astrolink-p65-bridge-build/release/libmirage_astrobox.dylib` was restored
by rebuilding from the original shared source; that original artifact retains
its channel6-first behavior. Use the separate channel5 dylib for P65 work.

## Ready-to-run companion command

```sh
python3 scripts/mirage-smoke.py \
  --library /tmp/astrolink-p65-validation \
  --device astrolink-p65-043 \
  --bridge /tmp/astrolink-p65-channel5-astrobox.dylib \
  --miragectl /tmp/astrolink-p65-mirage-build/debug/miragectl \
  --output /tmp/astrolink-p65-NEXT-UNIQUE-DIRECTORY \
  --settle 0 --timeout 25 status
```

Use the same prefix for `install`, `canopus enable`, `canopus activate`, and
`probe` once the parent has a reviewed module package. Existing evidence
`/tmp/astrolink-p65-channel5-status/` includes redacted events, summary,
screenshot, and guest-log tails.

QEMU interrupt logging was briefly enabled for staged execution and disabled
before handoff (`log none`); no ongoing trace load remains. GDB was detached
and the CPU resumed. Temporary RSP/QMP helpers are
`/tmp/astrolink-p65-rsp.py` and `/tmp/astrolink-p65-qmp.py`.
