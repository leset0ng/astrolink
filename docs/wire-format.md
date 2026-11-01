# Astrolink wire format (frame version 2)

Version 2 removes the literal service name from the air. A Session's identity is
a 128-bit UUID derived from the application's `serviceId`, which leaves the
legacy advertisement payload free for application data. Version 1 frames are
rejected: there is no compatibility path, and the version byte is the only
difference the decoder needs to refuse them.

## Advertising packet

One legacy `ADV_IND` (31 bytes of AD data, no scan response):

| Offset | Bytes | Field |
| --- | --- | --- |
| 0 | 3 | Flags AD `02 01 02` (the firmware wrappers insert none) |
| 3 | 1 | AD length `19 + data length` |
| 4 | 1 | AD type `0x21` (Service Data - 128-bit UUID) |
| 5 | 16 | identity UUID derived from `serviceId` |
| 21 | 1 | frame version `2` |
| 22 | 1 | dynamic LE SPSM (`0x80..=0xFF`) |
| 23 | 0–8 | application data |

Total `23 + data length`, so the packet is full at 8 data bytes. The packet
carries no service name, no name length and no manufacturer company identifier;
no 16-bit UUID is invented.

## Identity derivation

`uuid = FNV-1a-128(UTF-8 serviceId)`

- offset basis `0x6c62272e07bb014262b821756295c58d`
- prime `0x0000000001000000000000000000013b`
- output is the 128-bit state in big-endian order

Worked examples: `"chat"` → `697f5b54ac757277b806e97b40614553`,
`"chat2"` → `4af5c2759583d94f70814aa837b05e5b`.

Both peers derive the same UUID from the same name, so the scanner can filter
precisely and the name never costs air time. Two Sessions on one device that
derive the same UUID are refused with `BUSY`; in practice that means byte-exact
duplicate names.

## Decoding rules

A scan report is accepted only when all of the following hold:

1. One complete AD structure of type `0x21` with a 16-byte UUID equal to the
   receiving Session's derived UUID. Other services' AD structures are skipped.
2. That AD is at least `1 + 16 + 2` bytes, its version byte is `2`, and its SPSM
   is `>= 0x80`.
3. The payload after SPSM is at most 8 bytes.
4. No second matching AD in the same report, and no truncated AD anywhere in it.

Anything else yields no report at all; partial or ambiguous data is never
delivered. Reports that do not match are dropped before the peer cache, so a
peer never appears in the wrong Session's discovery list.

## Handshake

The first CoC SDU on every Link is fixed size:

| Offset | Bytes | Field |
| --- | --- | --- |
| 0 | 1 | frame version `2` |
| 1 | 16 | identity UUID |

17 bytes. A peer whose version or UUID differs is closed with
`INVALID_ARGUMENT` before any application SDU is exposed, so a mis-targeted PSM
cannot deliver data to the wrong Session. The remote receive MTU must be at
least 17.

## Local limits

| Limit | Value | Enforced by |
| --- | --- | --- |
| `serviceId` UTF-8 bytes | 64 | `ServiceId::new`, mirrored in `getCapabilities().maxServiceIdBytes` |
| advertising data bytes | 8 | `AdvertisingData::new`, mirrored in `maxAdvertisingDataBytes` |
| AD data bytes per packet | 31 | firmware legacy advertising |

The 64-byte name bound is local policy only: it keeps the string sane for
logging and UI, and is not constrained by the radio.

## Extension slots

The advertisement has no spare byte at 8 data bytes. Further discovery payload
(application data beyond 8 bytes) belongs in a scan response, whose layout would
repeat the identity UUID so attribution stays exact:

```
flags 3 | len(19+n) 0x21 uuid16 version 2 spsm 0x80..0xff data[0..n]     n <= 11
```

Scan-response support exists in the firmware ABI on Q66/P67/O66 (`adv_create`
takes a response buffer of up to 256 bytes), but every target in the legacy N66
family starts advertising through a single-payload wrapper instead. Any such
extension therefore needs a per-target capability flag rather than a global
assumption.
