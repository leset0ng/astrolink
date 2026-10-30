# Xiaomi Band 11 `.139` recovery

How the `.139` ABI module (`src/abi/xiaomi_band_11_4_100_139.rs`) was obtained
from the validated `.155` backend, and what was **not** re-derived.

## Inputs

| Item | Value |
| --- | --- |
| AP image | `vela_ap.bin` from `miwear.watch.q66tc_v4.100.139_full_a02b7af5.bin` |
| AP SHA-256 | `31ce82257f7c127950dc5070b86316730cf468a41f0d004559e41e7d923b2c74` (matches `target.toml`) |
| IDB | `analysis-q66-139/vela_ap.bin.i64`, 35186 functions |
| Source IDB | `analysis-q66-177/155/q66-ap.i64` (.155) |
| Tooling | `Canopus-Private/tools/fw-match`: `extract_corpus.py`, `fingerprint_corpus.py` (capstone), `ensemble_match.py`, `port_pack.py`, `port_review.py` |

## Method

1. Corpora (35195 / 35185 functions) were extracted from both IDBs with
   `extract_corpus.py` and enriched with Thumb instruction fingerprints
   (`fingerprint_corpus.py`: sha256 over `size:mnemonic:operands` tokens with
   PC-relative literal, control-flow and pointer substitution).
2. `ensemble_match.py` matched 312 of 315 source function symbols with 27 041
   unique full-body anchors; every astrolink symbol matched with
   `bytes = cfg = context = data = 1.00`, an equal instruction fingerprint and
   full anchored caller/callee agreement.
3. `port_pack.py` accepted 47 records as `STATIC_RECOVERED`. Four bodies
   (`bt_free`, `ble_scan_stop`, `ble_advertising_stop`, `qjs_free_cstring`) are
   shorter than the eight-instruction full-body-anchor minimum and were
   accepted manually: their `.139` code is **byte-identical** to `.155`,
   including literal pools and callees.
4. `port_review.py` gave an independent Hex-Rays comparison as a second
   opinion: 8 identical, 32 similar (>= 0.90), 8 different. The different ones
   are decompiler typing/label noise on equal-sized bodies with equal
   immediates, so the records keep `approval_state = PENDING` and no device
   proof, and the port does not upgrade any source semantic status.
5. The framework dispatcher (15 words) and the Application dispatcher
   (52 words) were compared word by word: `.139` equals `.155` with **every
   entry shifted by the same +0x10**, the same shift the L2CAP owner handlers
   show.

Independent cross-check: nine `.139` symbols already existed in the pack
(`bt_alloc`, `bt_free`, `bt_adapter_register`, `bt_adapter_get_state`,
`bt_l2cap_owner`, `bt_shared_adapter`, `bt_queue_external`,
`gap_host_receive_slot`, `gap_host_stock_receive`). All nine predicted
addresses equal the pre-existing records exactly.

## Structure offsets

All offsets used by the ported functions are identical to `.155`: the 48 pairs
have identical immediate-constant multisets and identical basic-block geometry,
and the instruction tokens keep every non-PC-relative immediate. In addition:

- the ready-queue machinery (`uv__work_submit` `0xc3f4cc4`, worker completion
  `0xc3f7fa8`, drain `0xc3f7f14`) has equal bodies and equal constants, which is
  where loop `+68` (queue head), `+76` (mutex) and `+104` (async) are encoded;
- eight equal-bodied functions encode the default loop's pipe descriptors
  `+312`/`+316`;
- `uv_key_create` (nuttx.c:546, `0xc3efba8`) and the loop-slot getter
  (nuttx.c:569, `0xc3f0700`) confirm the `0x200bd444` key global and the loop
  slot vector.

## Not re-derived

`src/loop_dispatch.rs` walks `TCB+100 -> TLS+0 -> taskinfo+16+4*uv_key ->
uvstate+664`, then `TCB+8 -> group+96` for the file list, and checks
`file+16 -> inode+16 == 0x2cb04ffc`. Those NuttX task/file descriptors were
recovered from the `.155` audit and were **not** independently re-derived for
`.139` in this pass; no equal-bodied function covers that chain. The residual
risk is a wrong file list lookup, not a wrong firmware call, and the failure is
loud and safe: `PipeWake::capture_default_loop` returns `-9`/`-95`,
`ensure_client` reports `ClientFailed`, and no GAP work is posted. It must be
confirmed by the probe on this firmware before any device claim.

## Data addresses

Recovered by access consensus: a matched function keeps the same literal
instruction position, and the target's literal is read from that instruction.

| Record | `.155` | `.139` | Supporting functions |
| --- | --- | --- | --- |
| `bt_l2cap_owner` | `0x200be890` | `0x200be890` | 37 |
| `bt_miwear_ipc_instance` | `0x200da4f8` | `0x200da4f8` | 62 |
| `gap_host_receive_slot` | `0x200bda80` | `0x200bda80` | 1 |
| `bt_shared_adapter` | `0x200c3148` | `0x200c3148` | 15 |
| `uv_loop_key` | `0x200bd444` | `0x200bd444` | 2 |
| `qjs_framework_instance` | `0x200d972c` | `0x200d972c` | 9 |
| `qjs_framework_vtable` | `0x2ca8065c` | `0x2ca8066c` | 2 |
| `qjs_application_vtable` | `0x2ca7c2d4` | `0x2ca7c2e4` | 2 |

## Symbols added to the `.139` pack

42 ported records plus six authored ones: `l2cap_disconnect_channel`
(`0xc872dd0`; equal 32-instruction body with the audited `.155` `0xc872dc0`
policy, called by the `.139` disconnect handler at `0xc872e40`) and the five
data records above. Nine names already existed in the pack and were left
untouched.

## Build

`canopus build --target xiaomi-band-11-4.100.139` -> PASS, 50541 resident bytes
and 1244 relocations, identical to the `.155` artifact; only the addresses
inside the relocations differ.
