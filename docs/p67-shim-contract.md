# What p67 offers, and what the module has to do instead

Measured from the 3.101.043 image; the addresses live in the target pack. This is
the design brief for `src/abi/xiaomi_band_10_pro_3_101_043.rs`, because almost
every difference from Band 11 turns out to be a *shape* difference rather than a
missing address.

## Feature registration: sixteen-byte host

```
Host { token: u32, sink: u32, abort: u8 (must be 0), pad: [u8; 3], package: *const u8 }
```

`feature_registry_create 0xC8660E4(host)` reads `+4`, checks `+8`, and passes
`+12` to `feature_framework_init 0xC86E764` as a `std::string`; nothing is
dereferenced during registration. `feature_register_feature 0xC862898(registry,
descriptor)` then publishes the module's static descriptor. Delivery to JS is the
part that still needs the ferry host (`host+0` builds the adapter).

## Sending: the caller allocates, the handler frees

`bt_l2cap_submit_cid 0xC805BE8(buffer, cid)` writes the cid at `+4` and posts a
work item; the handler re-fragments in place and **frees the buffer on every
exit**. The stock buffer from `l2cap_stock_alloc 0xC7EAFBC` is
`{ u16 total, u16 payload offset, u16 cid, u16 handle, u8 header type, u8 flags,
payload at 4+offset }`. A zero-length payload is legal: the handler compares
`total - offset == 0` and sends header only.

## Receiving: a twelve-byte node

`gap_host_stock_receive 0xC7EC47C` allocates `{ handle, buffer, end }` (12 bytes,
`end` is a pointer, not a length), and the data handler at
`bt_queue_external(dword_2013BDB4, sub_C7ED8E0, node, 1)` owns it afterwards.
There is no SDU buffer or fragment node type on this build.

## Cross-task posting: no uv loop needed

`bt_queue_external 0xC7EB9CC(owner, handler, arg, flag)` uses a firmware-global
queue (page list `0x2013BD70`, lock `0x2013BD80`, wake `0x2013BD84`, drained by
the Bluelet FSM task at `0xC7EB038`). The handler is called as
`handler(owner, event, arg)` on the FSM task; `owner` is a caller token the queue
never dereferences; the poster allocates `arg` and the handler frees it.

**The trap**: if the queue lock is still null the call does not enqueue — it logs
and calls `handler(0, 221, arg)` synchronously. Event `221` therefore means
"ran inline", and the module must tolerate it.

Because this primitive exists, the module does not need to capture a uv loop on
p67: no async handle, no `uv_close`.

## LE CoC: numbers known, meanings inferred

`handler(event, data)` with `event` in `{1,2,3,5,6,7,8,12,13,14}`; the work item
flag is always `1` (`COC_QUEUE_EVENT`). The firmware never names them, so the
module routes by what each branch reads:

| event | data fields used | branch |
| --- | --- | --- |
| 1 | `+0` u16, `+4`, `+32`, `+52`, `+56` | channel set-up, then disconnect |
| 2 | `+0` u16, `+8` array | connection/assignment path |
| 3 | `+108`, `+114` | configuration check |
| 5 | `+4` u16 cleared | **data**, handled inline, node freed by us |
| 6 | `+0` u16 | close by cid |
| 7 | `+4` dword | close/credit by dword |
| 8 | `+2` u16 | local flow |
| 12 | flags `+17` bit0, count `+16`, pairs from `+4` | request |
| 13 | same flags, `+6`, 24-byte address at `+24` | incoming request |
| 14 | `+132` count, `+130`, `+106` array | credit/flow update |

The connect request block is `{ cid u16 at 0, psm u16 at 2, flags u8 at 8 (bit0
LE), psm entry dword at 12, cid pair at 16/20, state u8 at 23 (1 LE, 2 BR) }` —
a different shape from Band 11's, which carries status fields this one has none
of.

## GAP: flag-driven, not struct-driven

- Scan: `app_lescan_start 0xC710BCC` → `gap_scan_start_impl 0xC3B1374(adapter,
  params8, filters, callbacks)`; the message keeps only five bytes of the
  parameter block, so treat the two words as opaque adapter-command arguments.
  The callback table at `0x200DFAF0` declares its own size (`0x10`) and holds the
  report/started/stopped entries at `+4/+8/+0xC`.
- Scan report: `+7` is a signed byte (RSSI, the `LDRSB` gives it away), `+9`
  address type, `+0xA` length, `+0xC` the six address bytes.
- Advertising: `bt_le_start_advertising 0xC70ED94(flags)` — there is **no 28-byte
  parameter block to fill**. Bits select the mode and the interval is derived
  inside (160/480/1600/32 units, logged as `625*units/1000` ms), transmit power
  is a constant `-10` written into the advert parameters, and advertising type is
  `5` or `7` from bit 0. So the module passes a flag, not a structure.

## ArrayBuffer

`JS_GetArrayBuffer 0xC2D362C`: tag `-1`, class 19 or 20, then
`buffer = *(void**)(object + 32)`, `detached = *(u8*)(buffer + 4)`,
`data = *(void**)(buffer + 0)`, `length = *(u32*)(buffer + 8)`. The producer
clears `buffer + 4` as a sixteen-bit store, so detached and shared are one byte
each, not bits of a word.
