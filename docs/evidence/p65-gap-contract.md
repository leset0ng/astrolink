# Redmi Watch 6 3.100.043: exact-image GAP contract

Scope: `analysis-p65-043/vela_ap.bin`, 11,793,540 bytes, SHA-256
`a19b601477569765103eaf468b937bd8026b4d68ffa5c896bde222a79e475858`.
The digest was recomputed locally. Analysis database:
`/Users/lesetong/develop/mifw/analysis-p65-043/vela_ap.bin.i64`.
This report is static producer/consumer evidence only; it establishes no
installation, runtime, radio, or file/group lifetime result. Source and target
pack were not changed. Supporting pseudocode and disassembly are retained at
`/tmp/astrolink-p65-gap-raw.txt`. Eight relevant IDB functions were annotated
and the database saved. Addresses below are even instruction VAs; callable
Thumb addresses use `VA | 1`.

## Findings that settle the remaining GAP questions

1. Public advertising mode **5 is the correct legacy, undirected,
   connectable/scannable ADV_IND choice on this exact image**. The existing
   Astrolink assignment `params[0] = 5` is correct.
2. The scanned callback's report payload starts at **+11**, including after
   the IPC round trip. IPC serialization temporarily stores data at a +12
   boundary, then reconstructs the native report at +11.
3. Filter16 is a UUID16 service-data filter, enabled by **filter+12**. It is
   unsuitable for Astrolink's UUID128 identity. The audited scanner path has
   no reader of filter+14/+15; there is no reason to invent flags for them.
   Use unfiltered command212, as the current P65 `create_scan` does.
4. Adapter register is `(client, raw_callback_table) -> subscription_cookie`.
   Get-state is `(client) -> state_byte`. Ready state is **4**. Existing P65
   backend call shapes for these entries match the exact image.
5. Scan/advertising stop take the **client wrapper**, not PHY, adapter state,
   or advertising ID. Their synchronous return is not a meaningful operation
   status. Wait for their terminal callbacks.
6. The existing docs contain wrong-segment service advertising VAs:
   `0x0C059990`, `0x0C05979C`, `0x0C059A88` are all `FF` bytes in this IDB.
   The real code is **0x1C059990**, **0x1C05979C**, **0x1C059A88**.
7. There is an asynchronous advertising-start failure ownership asymmetry:
   the client event254 consumer does not free its wrapper, whereas scan's
   failed-start consumer does. Current shared `adv_started` drops that
   handle, so P65 needs failure-wrapper reclamation or an explicit retained
   terminal wrapper policy before treating this path as leak-free.

## Advertising mode and the 28-byte public block

The complete local chain is:

```
stock producers 0x0C65A2A4 / 0x0C9CEBA0 / CLI 0x0CB919C8
  -> XIP thunk 0x0C8BDE48
  -> client producer 0x1C00A908 (IPC67)
  -> service dispatcher 0x1C00BF34
  -> service constructor 0x1C059990
  -> owner worker 0x1C05979C
  -> thunk 0x1C166450 -> converter 0x0CB8E6EC
  -> platform converter 0x0C97C398
  -> queue producer 0x1C062280
  -> Bluelet advertising owner handler 0x1C060220
```

### Independent local producer and final consumer checks

`0x0C65A2A4` is a stock BLE advertising producer. These instructions select
mode5 when its input bit0 is set and mode7 otherwise, then submit that same
28-byte block at `0x0C65A390`:

```
0x0C65A346 ANDS.W R10,R4,#1
0x0C65A34A ITE EQ
0x0C65A34C MOVEQ R3,#7
0x0C65A34E MOVNE R3,#5
0x0C65A356 STRB R3,[R7,...public_params+0]
0x0C65A360 MOVS R3,#0xF6
0x0C65A362 STRH R3,[R7,...public_params+20]
0x0C65A390 BL sub_C8BDE48
```

The stock JS advertising producer `0x0C9CEBA0` independently selects5 for its
true boolean and7 otherwise before calling `0x0C8BDE48` at `0x0C9CEFF8`.

The native CLI at `0x0CB919C8` names types using the exact strings:

| String VA | String | Base value selected |
| --- | --- | --- |
| 0x3C3A96A0 | `adv_ind` | 0 |
| 0x3C3A96A8 | `direct_ind` | 1 |
| 0x3C3A96B4 | `nonconn_ind` | 3 |
| 0x3C3A96C0 | `scan_ind` | 2 |
| 0x3C3A96CC | `scan_rsp_ind` | 4 |
| 0x3C3A9714 | `legacy` | mode adds5 |

The same CLI adds10 for `ext` (0x3C22582C), and leaves the base value for
`auto` (0x3C26AE1C). Thus the combined public enum is not a standalone
connectable flag.

`0x0CB8E6EC` marshals public mode+0 into platform input+4, with ID in input+0.
`0x0C97C398` does:

```
0x0C97C3CC LDRB R3,[R4,#4]       ; combined public mode
0x0C97C478 CMP R3,#4
0x0C97C47A BHI loc_C97C49C
...
0x0C97C49C SUBS R3,#5           ; modes5..9 -> base0..4
0x0C97C49E UXTB R3,R3
0x0C97C4A0 B loc_C97C480
0x0C97C480 CMP R3,#1
0x0C97C484 CMP R3,#2
0x0C97C488 CMP R3,#0
0x0C97C48C MOVNE R3,#0x90
0x0C97C48E MOVEQ R3,#0xD3
```

The final ordinary legacy branch at `0x1C060220` maps properties0xD3 to
HCI advertising type0, rather than relying on the enum's apparent name:

```
0x1C0606F8 CMP R3,#0xD2
0x1C0606FA BEQ ...type2
0x1C0606FE CMP R3,#0xD3
0x1C060700 ITE NE
0x1C060702 MOVNE.W R8,#3
0x1C060706 MOVEQ.W R8,#0
0x1C06070A B loc_1C060538
```

The following store at `0x1C060538` writes that value to the legacy command
parameter's advertising-type byte. This proves mode5 -> legacy ADV_IND type0.
Mode7 becomes properties0xD2 and legacy type2 (scannable, nonconnectable).

### Public block offsets actually consumed

`0x0CB8E6EC` copies the following from the public block; `0x0C97C398` then
uses the corresponding fields:

| Public offset | Width | Exact observed use |
| --- | --- | --- |
| +0 | u8 | combined type/mode enum |
| +1 | 6 bytes | peer address |
| +7 | u8 | peer address type |
| +8 | 6 bytes | own address |
| +14 | u8 | own address type (0 selects public,1 selects supplied random) |
| +15 | 1 byte | not copied by converter; alignment byte |
| +16 | u32 | interval, copied into both min/max interval |
| +20 | u8 | transmitted power value |
| +21 | u8 | channel selection:1->1,2->2,3->4,other->7 |
| +22 | u8 | advertising filter-policy byte forwarded |
| +23 | 1 byte | not copied by converter; alignment byte |
| +24 | u32 | final public word copied; platform converter uses low16 as enable-duration field |

Exact copy instructions include `0x0CB8E724 LDR R2,[R4,#0x10]`,
`0x0CB8E726 LDRB R3,[R4,#0xE]`,
`0x0CB8E72A LDRH.W R2,[R4,#0x15]`,
`0x0CB8E72E LDRB R1,[R4,#0x14]`,
`0x0CB8E736 LDR R3,[R4,#0x18]`.

Astrolink's zeroed block with mode5, interval320 at+16, power0xF6 at+20,
and all other bytes zero therefore follows the verified ordinary public
address/default channels/zero policy path. No extra connectable bit needs
setting. Duration units beyond this zero setting are outside this report.

## Public create/stop prototypes and wrapper ownership

```c
uintptr_t adv_create(uintptr_t client, const uint8_t params[28],
                     const uint8_t *data, uint32_t data_length,
                     const uint8_t *response, uint16_t response_length,
                     const struct AdvCallbacks *callbacks); // 0x1C00A908
void adv_stop(uintptr_t client, uintptr_t wrapper);           // 0x1C00A9AC
uintptr_t scan_create(uintptr_t client, const uint8_t settings[8],
                      const struct ScanCallbacks *callbacks); // 0x1C00AA40
uintptr_t scan_create_filtered(uintptr_t client, const uint8_t settings[8],
                               const uint8_t filter[16],
                               const struct ScanCallbacks *callbacks); // 0x1C00AAA4
void scan_stop(uintptr_t client, uintptr_t wrapper);          // 0x1C00AB28
```

The stop prototypes intentionally discard R0. Raw code returns different
incidental values on early-null versus successful IPC paths and does not
return a reliable stop-status enum. This matches the current P65 shims.

Advertising producer `0x1C00A908` allocates12 bytes with allocator
`0x1C166660`, saves callbacks at wrapper+4, copies the28-byte block and both
payloads into stack IPC storage, and emits67 using `0x1C00C494`. It accepts
lengths up to256, but Astrolink should keep its own legacy31-byte bound.
The returned service object is stored at wrapper+8. The callbacks table is
borrowed (the static module table must remain resident).

Scan producer `0x1C00AA40` allocates8 bytes, saves callbacks+4, copies8-byte
settings and sends212. Its service handle is wrapper+0. Filtered producer
`0x1C00AAA4` explicitly zeroes the612-byte IPC record, copies settings8 and
filter16, and emits213. Command212 does not consume a filter.

The exact stop argument reads are:

```
0x1C00A9B8 LDR R3,[R1,#8] ; advertising wrapper -> service handle
0x1C00A9BA MOVS R2,#0x44  ; command68
0x1C00A9C0 BL sub_1C00C494

0x1C00AB34 LDR R3,[R1]    ; scan wrapper -> service handle
0x1C00AB36 MOVS R2,#0xD6  ; command214
0x1C00AB3C BL sub_1C00C494
```

`0x1C00BF34` consumes67/68; `0x1C00ECF0` consumes212/213/214. The latter's
`0x1C00ED42..0x1C00ED50` sets `(server_cookie, IPC+48 settings, table)` and
calls service thunk `0x1C166EF8 -> 0x0CB8CB70`. Filtered case at
`0x1C00ED64..0x1C00ED74` additionally passes IPC+56 filter in R2, calls
`0x1C167BC0 -> 0x0CB8CAE0`.

## Filter16: UUID16 slots, enable flag, no evidence for +14/+15 flags

Service constructor `0x0CB8CAE0` zeroes its44-byte scanner object, sets
cookie+8 and callbacks+40, then:

```
0x0CB8CB16 CBZ R6,...          ; null filter
0x0CB8CB18 LDRB R3,[R6,#0xC]  ; filter+12 enable
0x0CB8CB1A CBNZ R3,0x0CB8CB42
...
0x0CB8CB42 MOV.W R2,#0x1F4    ;500
0x0CB8CB46 MOVW R3,#0x1388    ;5000
0x0CB8CB4A MOVS R1,#0
0x0CB8CB4C STRD.W R2,R3,[R6]  ; overwrites filter+0/+4
0x0CB8CB50 STRB R1,[R6,#0xD]  ; forces filter+13=0
0x0CB8CB52 MOVS R2,#0x10
0x0CB8CB56 ADD.W R0,R4,#0x18
0x0CB8CB5A BL sub_C8BE398     ; scanner+24 <-filter16
```

Report dispatcher `0x0CB8C3F0` reads scanner+36 (=filter+12) at
`0x0CB8C41C`. If zero it immediately delivers every report. Otherwise it
parses AD and uses matcher `0x0CB8C2B8` at `0x0CB8C46E` with scanner+24.
The matcher reads filter halfwords+8 and+10 at `0x0CB8C2C0/+2C4`.
AD parser `0x0CB8CC50` recognizes **AD type22 (0x16)** and stores that
record's little-endian UUID16 in parsed+2. This is not full128-bit service
UUID matching and not a general service-list filter.

Scanner+37 (=filter+13) is read at `0x0CB8C49C`: when zero, dispatches the
matching/cached report; when nonzero it applies elapsed-time and payload-cache
suppression using filter+0/+4 (which constructor normalizes500/5000).
The constructor forces this byte zero, so callers cannot enable that behavior
through the ordinary filtered-create path.

No reads of scanner+38/+39 (filter+14/+15) occur in the audited constructors,
scanner worker, report dispatcher, merge-settings routine, UUID matcher or
stop/destructor paths. The native filtered producer `0x0C65CBF4` initializes
its filter using whole words so these bytes are zero, and CLI
`0x0CB92450` zeroes the filter then sets UUID16+8 and enabled+12=1.
**Bounded conclusion:** +14/+15 are copied but unused/reserved in this exact
ordinary manager path; their purpose in unrelated code is not claimed.
They are not needed for Astrolink, whose current P65 shim uses command212.

The stock scanner worker initializes its settings to eight bytes representing
low word2/high word0 at `0x0CB8C7AE..0x0CB8C7B8`. The current P65 shim's
`[2,0,0,0,0,0,0,0]` matches those bytes. This report does not infer a general
active/passive scan enum from the literal alone.

## Scan report and terminal callback contracts

The public callback table is16 bytes:

```c
struct ScanCallbacks {
    uint32_t size;
    void (*report)(uintptr_t wrapper, const uint8_t *borrowed_report);
    void (*started)(uintptr_t wrapper, uint8_t status);
    void (*stopped)(uintptr_t wrapper);
};
```

Service callback table at `0x3C3D1190` is
`[16,0x1C00EC89,0x1C00EC45,0x1C00EC0D]`.
Report bridge `0x1C00EC88` copies12-byte header and places AD data from
report+11 at IPC+60. Client event316 consumer `0x1C00ED84` allocates
`length+12`, copies header from IPC+48, then copies payload from IPC+60 into
new_report+11 before calling the client's report slot and freeing the report:

```
0x1C00EDD4 ADD.W R1,R4,#0x30  ; IPC header
0x1C00EDD8 MOVS R2,#0xC
0x1C00EDDA BL sub_1C166990
0x1C00EDDE LDRB.W R2,[R4,#0x3A] ; AD length
0x1C00EDE2 ADD.W R1,R4,#0x3C    ; IPC payload
0x1C00EDE6 ADD.W R0,R5,#0xB     ; native payload+11
0x1C00EDEA BL sub_1C166990
0x1C00EDF4 LDR R3,[R3,#4]
0x1C00EDF6 BLX R3               ; report(wrapper,new_report)
0x1C00EDF8 MOV R0,R5
0x1C00EDFA BL sub_1C168AF0      ; borrowed report ends here
```

The independently stock consumer `0x0CB926BC` reads address0..5, RSSI+7,
address_type+8, packet_type+9, length+10 and AD+11. Existing ABI offsets match.
The service bridge drops zero-length reports instead of delivering event316.

Event317 loads wrapper from IPC+316 and status byte from+320, invokes started
slot+8, then reads the whole zero-initialized status word at+320 and frees the
client wrapper iff nonzero (`0x1C00EDA4..0x1C00EDC4`). The service-side cookie
is also freed after emitting failed-start event in `0x1C00EC44`.
Event318 invokes stopped slot+12 then frees client wrapper
(`0x1C00EE02..0x1C00EE16`), and `0x1C00EC0C` frees server cookie.
Do not explicitly free these scan wrappers in callbacks.

The manager has only **two scan registrations**, shown by its
`0x0CB8C7C2 LDRB ...count`, `0x0CB8C7C6 CMP R3,#2` and slot array of two.
Current module Session capacity4 must not be advertised as four independent
native P65 scans: stock consumers may already occupy either slot. Expect
asynchronous start failure status1/3/4 rather than assuming capacity.

## Advertising callbacks and failed-start leak

Client table layout matches the module:

```c
struct AdvCallbacks {
    uint32_t reserved; // common module uses0; client never reads this word
    void (*started)(uintptr_t wrapper, uint8_t advertising_id, uint8_t status);
    void (*stopped)(uintptr_t wrapper, uint8_t advertising_id);
};
```

Service constructor `0x1C059990` copies12 bytes from its callback table into
server advertiser+16. Worker `0x1C05979C` invokes started through advertiser+20
with R0=cookie (or advertiser if cookie null), R1=id, R2=status. Resource
allocation failures use id0 and status1/3. Successful controller notification
worker `0x1C0598A8` invokes the same slot with actual id and status0.
Termination unlinks the server advertiser, calls advertiser+24 with cookie
and id, then releases the server advertiser. Stopped's second byte is an ID,
not a status.

IPC bridge `0x1C00BEEC` emits254 with wrapper in IPC+44, id+48,status+49;
it frees its server cookie if status!=0. Bridge `0x1C00BEB4` emits255 with
wrapper+52,id+56 and always frees that cookie.

Client consumer `0x1C00BFB8` has these exact branches:

```
; event254
0x1C00BFE2 LDR R0,[R3,#0x2C]     ; wrapper
0x1C00BFE4 LDRB.W R2,[R3,#0x31]  ; status
0x1C00BFE8 LDR R1,[R0,#4]        ; callback table
0x1C00BFEA LDR R4,[R1,#4]        ; started slot
0x1C00BFEC LDRB.W R1,[R3,#0x30]  ; advertising ID
0x1C00BFF0 BLX R4
0x1C00BFF2 MOVS R0,#0
0x1C00BFF4 POP {R3,R4,R7,PC}     ; no wrapper free even on failure

; event255
0x1C00BFC6 LDR R4,[R3,#0x34]     ; wrapper
0x1C00BFC8 LDRB.W R1,[R3,#0x38]  ; advertising ID
0x1C00BFCC LDR R3,[R4,#4]
0x1C00BFCE MOV R0,R4
0x1C00BFD0 LDR R3,[R3,#8]        ; stopped slot
0x1C00BFD2 BLX R3
0x1C00BFD4 MOV R0,R4
0x1C00BFD6 BL sub_1C168AF0       ; frees wrapper after callback
```

**Concrete correction:** retain current successful-start/stopped behavior,
but provide a P65-specific advertising callback adapter to reclaim the12-byte
client wrapper exactly once on nonzero started status, after forwarding that
terminal result to the common module callback. The firmware event254 caller
does not dereference the wrapper after returning from that callback. Verify
allocator compatibility and follow-on callbacks as documented in the completed
follow-up below: both are now verified. The exact matching deallocator is
P65 `F_FREE = 0x0C50B25C`. Do not call native stop after failed start: server
advertiser/cookie have already been destroyed.

## Adapter state/registration

```c
uintptr_t adapter_register(uintptr_t client, const void *raw_callback_table);
    //0x1C008608
uint8_t adapter_get_state(uintptr_t client); //0x1C008720, IPC27
uint8_t adapter_enable(uintptr_t client);    //0x1C0086D0, IPC23
```

Register calls callback-list insertion `0x1C166C68(list,0,table)`, returning
its cookie. It uses client+104 for the list. On initial creation it emits49;
on subsequent registrations it only inserts another node. Dispatcher
`0x1C00B9C4` handles state event238 (0xEE), obtains registered table from
node+4, gets first function pointer table+0, and calls it with
`(node-or-owner, state_byte_at_IPC+44)`. It does not interpret a table size word.
The common `AdapterCallbacks { state, reserved[15] }` shape is compatible;
reserved entries remain null and stock callback tables remain intact.

`0x1C008720` emits27 and returns response byte, no output pointer.
Null client or transport failure returns0. `0x1C0086D0` emits23 and returns
response status or transport error; null client returns5. Boot initializer
`0x0CA3F3D8` stores its client at global **0x3C4942B0**, registers table
**0x3C342668**, calls state thunk `0x0C8BEB28 ->0x1C008720`, and checks4
before ready work. Independent Bluetooth owner initializer `0x0C657434`
checks the same state4 using a different client. No borrowed state-word
pointer should replace this public scalar query.

No change is recommended to the P65 adapter addresses, registration calling
convention, readiness constant, unfiltered scan shim, report offsets, existing
stop shims, or current mode5 advertising block. Required practical follow-up:
correct old service addresses in docs, document unused filter tail accurately,
handle the advertising failure-wrapper asymmetry, and then combine with the
separately audited loop/group capture before isolated runtime validation.


## Completed follow-up: matching allocator and no stopped after failure

The failure-wrapper cleanup is statically supported for this exact image:
forward `started(wrapper, id, status)` to the common callback, then if
`status != 0`, call the one-argument matching `F_FREE` on that wrapper once.
No extra free is needed for successful start or stopped; native event255
already frees successful wrappers after the stopped callback returns.

### Exact heap identity

```
0x1C166660 LDR.W PC,=(sub_C50B2A0+1)
0x1C166664 DCD 0x0C50B2A1
0x1C168AF0 LDR.W PC,=(sub_C50B25C+1)
0x1C168AF4 DCD 0x0C50B25D
```

These destinations are exactly the existing P65 ABI constants:
`F_ALLOC=0x0C50B2A0` and `F_FREE=0x0C50B25C`.
The public allocation/deallocation wrappers use the same process heap:

```
; malloc(size)
0x0C50B2A2 LDR R3,=0x3C3D7E9C
0x0C50B2A4 MOV R1,R0       ; size
0x0C50B2A8 LDR R0,[R3]     ; heap
0x0C50B2AA BL 0x0C50A83C  ; heap allocation

; free(pointer)
0x0C50B25E LDR R3,=0x3C3D7E9C
0x0C50B260 MOV R1,R0       ; pointer
0x0C50B264 LDR R0,[R3]     ; identical heap
0x0C50B266 BL 0x0C50A608  ; heap deallocation
```

The12-byte client wrapper is allocated by the first thunk in
`0x1C00A908`; no header/interior adjustment occurs before it is returned or
passed to the client's started callback. Freeing the callback's `wrapper`
argument directly through `F_FREE | 1` therefore matches allocation.

### All observed nonzero started statuses are terminal

The ordinary advertiser started-slot producers in the audited service are:

| Producer | Status | Membership and subsequent ownership |
| --- | --- | --- |
| 0x1C05979C, ID allocation failure |1| Never inserted into advertiser list; cookie freed by event254 bridge; advertiser freed by worker |
| 0x1C05979C, lower start call failure |3| Never inserted; releases any allocated advertising ID, frees advertiser/payloads |
| 0x1C059710, start timeout |2| Unlinks advertiser before failure callback; releases ID and frees advertiser afterward |
| 0x1C0598A8, controller start notification |0| Linked advertiser remains; its eventual stopped callback follows normal native ownership |

Timeout was explicitly checked at instruction level; its R1 argument is0,
and R2 is2, despite the decompiler omitting those arguments:

```
0x1C05973C LDR R0,[R4,#0x1C]
0x1C05973E BL ...timer_stop
0x1C059742 LDRD.W R0,R3,[R4]
0x1C059746 MOVS R1,#0
0x1C059748 STR R1,[R4,#0x1C]
0x1C05974A STR R0,[R3]          ; unlink prev/next
0x1C05974C LDRD.W R3,R0,[R4]
0x1C059750 STR R0,[R3,#4]
0x1C059752 LDR R0,[R4,#8]      ; IPC cookie
0x1C059754 STRD.W R1,R1,[R4]   ; clear links
0x1C05975E MOVS R2,#2
0x1C059760 LDR R3,[R4,#0x14]   ; started slot
0x1C059762 BLX R3             ; started(cookie,0,2)
...release advertising ID if present...
0x1C05977C BL sub_1C168AF0     ; free server advertiser
```

Immediate failure branches at `0x1C0597EE..0x1C059822` and
`0x1C059866..0x1C059892` do not reach successful insertion
`0x1C059826..0x1C059836` or timer creation. Therefore they cannot be found by
later notification workers.

Controller state worker `0x1C0598A8` searches the advertiser list for the ID
at `0x1C0598B6..0x1C0598CE`; it discards the notification if no linked entry
matches. Its state0 branch emits status0; state1 branch emits stopped only
for the matched live advertiser. Shutdown `0x1C059664` traverses the same
list. Explicit-stop worker `0x1C0595D4` likewise finds the list member first.
After any nonzero started outcome, none of those paths retains a reference
to the failed advertiser or its freed IPC cookie. A late controller event
may be dropped (or concern a newly registered advertiser after ID reuse),
but cannot produce stopped for the old failed wrapper through these paths.

### Client continuation after forwarding

`0x1C00BEEC` copies the wrapper value into event254, sends it, and frees its
server cookie on nonzero status. It does not dereference the client wrapper.
The client dispatcher loads the callback table before the callback and its
entire continuation is:

```
0x1C00BFF0 BLX R4   ; started(wrapper,id,status)
0x1C00BFF2 MOVS R0,#0
0x1C00BFF4 POP {R3,R4,R7,PC}
```

Thus freeing inside the P65 callback adapter after forwarding is safe against
both dispatcher post-callback reads and native double-free. Scope the
cleanup to this P65 ABI and nonzero started status; preserve the existing
handle match/terminal result handling and avoid any subsequent wrapper read.
This remains exact-image static proof, not a runtime failure-injection result.
