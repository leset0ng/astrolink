# Legacy QuickApp extension and LE CoC investigation

This pass revisits the `feature_runtime_absent` findings for N66/O66/P65.
It does not adapt a target or promote an address into its target pack. Every
address below comes from static inspection of the exact firmware image; no
address is approved for runtime use. The current P67 contracts in
`p67-native-contract.md`, not its older blocked notes, are the comparison.

The important distinction is **implementation absent** versus **current
integration not recovered**. These older images contain native extension and
Bluetooth code despite not containing `FeatureManagerQjs` strings. Some
matched `bt_l2cap_*` records in their packs are mislabeled and must not be
called. A negative string census cannot substitute for local producer and
consumer evidence.

## Xiaomi Smart Band 9 3.1.32 (N66)

Image: `analysis-n66nfc_v3.1.32/vela_ap.bin`, SHA-256
`9c02dab4020b2cc9666ee7d34cf27d311b76aadcec519a38361bbcbd94c53264`, XIP
`0x0C100000`. The inspected database was generated from this exact AP in the
firmware analysis directory; no source binary was changed.

### Native extension entry

Application init `0x0C403238` creates `@aiot/core` through
`0x0C1EEDE0(ctx, name, 0x0C3F5ADC)`. The initializer and the application
then traverse a RAM extension list at sentinel `0x2007B0B0`:

| Node field | Meaning |
| --- | --- |
| `+4` | next node |
| `+8` | `init(ctx, module)` |
| `+12` | `export(ctx, module)` |
| `+16` | inline name used in extension logs |

Export happens before the built-in Feature bytecode and application source.
This is a concrete per-application native extension route, not just a QuickJS
log string. The list is empty in the stock image. Link-time insertion and
synchronization must be implemented and reviewed rather than inventing the
newer `feature_register_feature` contract.

The Application primary vtable begins at `0x2C6B7688` after two prefix words;
init slot `+8` is `0x0C403238` and destroy slot `+96` is `0x0C404920`.
`__runApplicationExt 0x0C3FC51C` calls init/run and later destroy on the app's
task. The destroy routine reaches runtime through Application `+296`, drains
and closes the loop, then destroys JS. Runtime `+20` is its JS context and
`+24` its loop. These are the integration checkpoints; the exact ownership
and public import format are still open.

### Ordinary LE CoC

`bts_ble_listen_l2cap_channel 0x0C3D53F8` reaches the L2CAP owner at
`0x200AC004`. Registration selects LE transport 3 and allocates dynamic PSM
`0x80..0xff`; the owner handler is `0x0C3AF6A0`. Its callback uses event 9 for
registration, 1 for incoming and 2 for assigned. `0x0C3BF32C` fills the
incoming request's credits/MTU/MPS at `+54/+52/+62` before accept processing.

Normal sending is:

```text
bts_ble_send_packet 0x0C3D5450
  -> queue 0x0C3BA6B0
  -> CID submit 0x0C3BEC18
  -> CID assignment 0x0C3B2928
  -> owner send 0x0C3B1668
```

`0x0C3B1668` resolves local CID from the word at buffer `+4`, uses channel
MPS `+106`, prefixes the first fragment with a two-byte SDU length, and emits
first/continuation/last flags. The close/data callbacks are event 6/7.
Outbound connect wrapper `0x0C3B287C` posts owner handler `0x0C3B219C`;
that handler consumes PSM, callback, address and configuration in the request
and emits event 2 with the assigned CID. The low-level details still need
measurement against the shared backend, not an assumption that P67 offsets
are interchangeable.

The bttool independently exercises listen through GAP interface `+176` and
send through `+180`. Lower-level BLE scan/advertising names are present, but
complete GAP callback/stop ownership is still to be recovered.

### Bad existing L2CAP records

Do not use these pack entries:

- `bt_l2cap_connect 0x0C39157C`: a structure-field wrapper, not the connect
  owner handler.
- `bt_l2cap_submit_cid 0x0C16A768`: a VFS-like lock/call/unlock wrapper, not
  SDU submission.

The locally traced functions above are candidates for fresh evidence, not
renamed replacements approved for calling.

## Xiaomi Smart Band 10 3.1.28 (O66)

Image: `analysis-o66tc_v3.1.28/vela_ap.bin`, SHA-256
`fb726472416e99ba11d1c20ce8f0d4f0831a6e9a673d63bb5918e2c3921d8845`, XIP
`0x0C0A0000`. Temporary derived IDB: `/tmp/astrolink-o66-analysis/o66-ap.bin.i64`.

### Native extension entry

Application init `0x0C3F034C` creates `@aiot/core` with
`0xC1A0390(ctx, name, 0x0C3EA108)`. It iterates the extension list with the
same `next+4`, `init+8`, `export+12`, `name+16` contract as N66. Export runs
before stock Feature bytecode and application source. The RAM list container
is around `0x2008F8BC`/`0x2008F8C0`; the exact container representation and
insertion synchronization must be measured before changing it. Stock destroy
`0x0C3ED8E8` supplies a separate cleanup checkpoint. Do not claim that this
already provides safe late-callback cancellation.

### GAP and ordinary LE CoC

The GAP clients are locally different from P67:

- `0x0C4CC828` copies scan parameters/filters and emits IPC command **213**;
  its stop sibling emits **214**. P67's 237/238 are not portable.
- `0x0C4C6DDC` copies the 28-byte advertising configuration and payloads and
  emits IPC command **67**.

The exact report layout, IPC instance and callback/termination ownership are
still required.

The actual L2CAP owner is `0x200B75A4`; the stack initializer
`0x0C4D32D0` allocates it. Recovered ordinary-channel operations include:

| Operation | Local function |
| --- | --- |
| Find channel by CID +116 | `0x0C3C068C` |
| PSM lookup at registry +4 | `0x0C3C0964` |
| PSM registration, type 3 PSM 0x80..0xff | `0x0C3BB060` |
| Ordinary connect, public wrapper | `0x0C3C1E54`, `0x0C3C6448` |
| Ordinary incoming request, 68-byte layout | `0x0C3C1288` |
| Ordinary accept | `0x0C3C352A`, `0x0C3C6470` |
| Disconnect/reject | `0x0C3C33B8`, `0x0C3C64CC` |
| Original-SDU submit/send | `0x0C3C64F4`, `0x0C3C54C8` |

The connect/incoming layouts include CID +0, PSM +2, callback +12,
address/type +16/+22, MTU +52, credits +60 and MPS +62. The send handler
uses MPS +106, two-byte SDU length and stock fragments. Empty SDUs require
an explicit two-byte-zero fragment, as on P67. External owner work submission
`0x0C3ACDB0` has five arguments and separate run/cancel callbacks.

The pack's `bt_l2cap_connect 0x0C38C2C4` instead reaches VFS poll code with
`vfs/fs_poll.c:279`; its `submit_cid 0x0C1206B8` and `disconnect 0x0C2FBCA0`
are also unrelated. The local candidates above are static findings only.

## Redmi Watch 6 3.100.043 (P65)

Image: `analysis-p65-043/vela_ap.bin`, SHA-256
`a19b601477569765103eaf468b937bd8026b4d68ffa5c896bde222a79e475858`, XIP
`0x0C0C0000`. See `docs/p65-native-extension.md` for the independent IDA
trace and Bluetooth conclusion. Earlier matching candidates are not treated
as proof of a working L2CAP API.

## Consequences for adaptation

1. **N66 and O66 are feasible recovery targets**, not targets whose hardware
   or firmware has been shown to lack the required hosts. They need legacy
   extension backends plus exact GAP/CoC/task/QuickJS contracts.
2. Their invalid L2CAP pack names are defects to quarantine in review. Do not
   replace them en masse from this report: each replacement needs its own
   producer/consumer evidence record and exact-image verification.
3. A legacy bridge must preserve the public TypeScript primitive contract or
   provide an explicit compatibility layer. `@aiot/core` export does not by
   itself prove that `@app-module/canopus.astrolink` follows the newer
   `$app_require$` behavior.
4. The order for implementation is old-extension lifecycle, QuickJS values
   and buffers, GAP report/stop, CoC receive/credits/disconnect, then NuttX
   task/loop and exact signatures. Build success is not runtime acceptance.
5. All three targets still lack device or isolated-runtime validation for
   Astrolink. None is marked `SUPPORTED = true` by this investigation.
