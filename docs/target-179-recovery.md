# Xiaomi Band 11 4.100.179 ABI port

Recovered 2026-10-30 from the exact `.179` IDB of
`miwear.watch.q66tc_v4.100.179_full_a93678a9.bin` (inner `vela_ap.bin`,
12,293,060 bytes, SHA-256 `307adabe0ff61c77be73d045a8260e279cddd131c7a535a2678eb34199278a2b`),
which is the image `targets/xiaomi-band-11-4.100.179/target.toml` records.
All addresses below are even ARM entry addresses; callables set bit 0.

The `.179` text layout is **not** the `.155` layout: `bt_alloc` is `0x0C3514EC`
here against `0x0C351724` in `.155`. The two images share source, not addresses,
so nothing was copied from `.155`.

## Method

Source side: the reviewed `.155` symbol records and the `.155` IDB. Target side:
the `.179` IDB.

1. Whole-body match. Every `.155` function body was hashed and looked up in the
   `.179` function inventory; misses fell back to "same size + same first 24
   bytes". 46 of 48 source functions matched by body hash.
2. Structural confirmation for each candidate: the complete Thumb *mnemonic*
   sequence and the *set of structure immediates* (operands in `0x10..0xfff`,
   i.e. field offsets and sizes) had to be identical to `.155`. This is what
   pins the layout below; a `.179` function that only looks similar would fail
   on either check.
3. Decompiler comparison of the consumers that define the CoC and GAP layouts:
   the connect owner handler, the request configuration consumer, the channel
   completion builder, the scan report consumer and the stock advertising
   builder. Three of them are byte-identical between `.155` and `.179`
   (`0x0C47841C` builder, `0x0C86CF54` completion, `0x0C467970` scan consumer),
   which fixes the request, event and report offsets exactly.
4. Data objects were recovered from the *readers*, not by copying `.155`: the
   `.179` counterpart of each `.155` reader was located by whole-body match and
   the literal it loads is the `.179` address.

## Addresses

Callables that needed a judgment call:

| Symbol | `.179` | Why not the other candidate |
| --- | --- | --- |
| `ble_scan_stop` | `0x0C46B6A4` | Both stop wrappers are 14 bytes with an identical mnemonic sequence. The wrapper it calls issues a **238**-byte scan IPC request; the advertising stop wrapper issues **68** bytes. |
| `ble_advertising_stop` | `0x0C46B5D8` | Same pair, 68-byte advertising request. |
| `l2cap_unregister_psm_handler` | `0x0C871314` | Two 30-byte handlers differ only in the disconnect policy they apply after the channel lookup: `9` here, matching the `.155` record, against `5` for the other. |
| `uv_mutex_lock` | `0x0C3F4B60` | No body match: the uv wrapper differs only by call target. Identified as the `thread.c:358` assertion wrapper around the kernel mutex lock. |
| `uv_mutex_unlock` | `0x0C3F4B7C` | The `thread.c:378` counterpart. |
| `l2cap_disconnect_channel` | `0x0C871218` | The callable the stock disconnect handler invokes with policy 6/7; `.155` uses `0x0C872DC0` for it. The `.155` pack has no record for this callable at all. |

Data objects:

| Object | `.155` | `.179` | Evidence |
| --- | --- | --- | --- |
| L2CAP owner | `0x200BE890` | `0x200BE740` | Read by `0x0C871288` and by the matched `0x0C861E5C`; 41/41 matched readers agree. |
| MiWear BT IPC instance | `0x200DA4F8` | `0x200D93E8` | 37 of 44 matched readers of the `.155` object load this address. |
| QuickJS framework | `0x200D972C` | `0x200D861C` | All matched readers of the `.155` object load this one. |
| Framework vtable | `0x2CA8065C` | `0x2CA7E494` | The matched reader assigns `&FrameworkExt_primary_vtable`; the whole `.data.rel.ro` region moved by a uniform `-0x21C8`, which the Application-vtable evidence independently reproduces. |
| Application vtable | `0x2CA7C2D4` | `0x2CA7A10C` | The matched `Application_construct` assigns `&Application_primary_vtable`; its `.data.rel.ro` reference set maps one-to-one under the same `-0x21C8` shift. |
| uv loop key | `0x200BD444` | `0x200BD2E8` | The matched `uv_loop` initialiser (`nuttx.c:569`) reads and stores this key in exactly the `.155` shape. |

## Structure offsets

None changed. Each was checked on `.179`, not inherited:

- CoC request 68 bytes: CID `+0`, PSM `+2`, status `+4`, flags `+8`, callback
  `+12`, address `+16`, address type `+22`, transport `+23`. The `.179` connect
  handler reads exactly these and passes `request+24` to the configuration
  consumer, which takes MTU from `+28`, credits `+36` and MPS `+38` of that
  pointer, i.e. request offsets 52/60/62.
- Channel queue `+4`, credit-queue credits `+14`, disconnect policies 5/6/7/9.
- Completion event: local config `+0`, remote config `+44` (so remote MTU
  `+72`), address `+100`, CID `+108`, remote CID `+110`, PSM `+112`, outbound
  `+116`, size 120 -- byte-identical builder.
- Scan report: address `+0`, RSSI `+7`, type `+8`, packet type `+9`, length
  `+10`, data `+12` -- byte-identical consumer.
- GAP: 5-byte scan parameters, 616-byte scan client handle, 708-byte scan IPC,
  advertising parameter size 28 with its 712/744/748-byte IPC, 16-byte scan
  callback descriptor, 12-byte advertising callback descriptor -- all present in
  the identical immediate set of `ble_scan_start` / `ble_advertising_start`.
- libuv: ready-queue head `+68`, mutex `+76`, async `+104`, loop pipe fds
  `312`/`316`; `uv__work_submit` keeps its single `24` size immediate
  (`0x0C3F4C2C`).
- QuickJS framework/Application object offsets and vtable slot indices are
  unchanged: every `qjs_*` entry point matched with an identical mnemonic
  sequence, and the Application constructor's reference set maps one-to-one.

## Pack state

`targets/xiaomi-band-11-4.100.179/symbols` already carried these records from
the earlier reviewed port; this run added the six data objects and
`l2cap_disconnect_channel`. All 49 independently re-derived addresses agree with
the recorded `entry_address` values, so the two methods corroborate each other.

The generated Rust bindings were **stale**: the records existed but
`generated_xiaomi_band_11_4_100_179.rs` had never been regenerated, so the
`CANOPUS_FW_*` constants were missing and the module could not reference them.
`canopus target generate xiaomi-band-11-4.100.179` fixed that.

`canopus build --target xiaomi-band-11-4.100.179` passes verification with the
same 50,541 resident bytes and 1,244 relocations as `.155`, which is expected:
the code is shared, only the embedded addresses differ.

## Limits

Static recovery only. Physical-device validation, radio behaviour, phone/Audio
coexistence and heap stability are not established by this record.
