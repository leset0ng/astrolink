# P65 3.100.043 CoC callback and SDU audit

Scope: static, read-only analysis of `/Users/lesetong/develop/mifw/analysis-p65-043/vela_ap.bin.i64` and its matching 11,793,540-byte AP. Image SHA-256 was independently checked as `a19b601477569765103eaf468b937bd8026b4d68ffa5c896bde222a79e475858`. No source or target-pack changes, runtime tests, or sibling-firmware evidence were used. Addresses below are exact P65 VAs; unnamed `sub_` functions are analysis labels.

## Decision

P65's ordinary channel uses the same event IDs and field offsets already declared in its ABI module for events 3, 6, 7, 8 and 17. The P65 `cfg` exclusions around OPEN, combined DATA/CREDITS and LOCAL_FLOW in `src/backend/bluelet.rs` are obsolete and prevent correct connection establishment and credit notification. Remove those exclusions and the duplicate P65-only DATA match arm; retain a single default arm and the final payload free. Keep event8 as an ignored/free-only notification, not a send completion or remote-consumption acknowledgement.

A second concrete discrepancy is in the empty-SDU shim: the manually built fragment retains metadata byte `buffer+8 = 3`, whereas P65's own ordinary LE fragment producer explicitly writes **1**. Set that byte to 1 only for the empty fragment; the original nonempty SDU buffer keeps allocator-default 3. First/last node flag `0x81`, node allocation/payload offset 16/8, buffer end/offset 18/16, and two zero SDU-length bytes at buffer+20 are verified correct.

## Event contracts

| Event | Producer | Block | Layout relevant to Astrolink | Ownership |
|---|---|---:|---|---|
| 3 OPEN | `0x1C067710` | 120 bytes | local CID u16 +108; remote receive MTU u16 +72 | callback owns block |
| 6 CLOSED | `0x1C0668BC` | 12 bytes | local CID u16 +0; reason u16 +2; peer identity bytes +4..11 | callback owns block; channel is destroyed after callback |
| 7 DATA | `0x1C069198`, ordinary LE completion | variable SDU buffer | end u16 +0; offset u16 +2; local CID low u16 +4; complete SDU at +4+offset, length end-offset | completed buffer transferred to callback |
| 8 LOCAL_FLOW | `0x1C065D24` | 6 bytes | CID u16 +0; copied input u16 +2; constant u16 1 at +4 | callback owns block; input separately freed by producer |
| 17 CREDITS | `0x1C068618` | 4 bytes | local CID u16 +0; received credit increment u16 +2 | callback owns block; native credit accounting/drain already occurred |

All scalar fields above are little endian. No payload is the channel pointer. Callback signature is `callback(u32 event, void *payload)`, loaded from channel+20.

## Allocation and freeing

Callback block allocator thunk `0x1C166DA0` jumps to `0x0C9A2400`, which calls `0x0C50B2A0` (the ABI's `F_ALLOC`). Free thunk `0x1C167678` jumps to `0x0C9A240C`, which calls `0x0C50B25C` (the ABI's `F_FREE`). These are the same heap family, so the backend's final `free(p)` is correct for all five events.

Independent stock consumer: `0x0C119040` switches on the same channel callback event numbers. Its event3 branch reads `payload[54]` for CID (+108), then `payload[36]` for MTU (+72); instruction `0x0C1191BE` is `LDRH.W R1,[R5,#0x48]`. Its event6 branch reads `payload[0]`/`payload[1]`. Its event7 branch reads end/offset, CID and `payload+4+offset`. Every non-forwarding/default path calls `0x0C25743C -> 0x0C9A240C -> 0x0C50B25C`, corroborating callback-owned payloads independently of producer return behavior. That stock callback services a different upper protocol; it is used only as a format/ownership consumer, not as proof of LE runtime operation.

## OPEN / event3

`0x1C067710(channel)`:

```c
p = malloc(120); memset(p, 0, 120);
memcpy(p + 100, *(void **)channel + 16, 8);
*(u16 *)(p + 108) = *(u16 *)(channel + 116);
*(u16 *)(p + 110) = *(u16 *)(channel + 118);
*(u16 *)(p + 112) = *(u16 *)(channel + 112);
p[116] = (*(u16 *)(channel + 114) & 0x100) != 0;
memcpy(p,      channel + 24, 44);  // local config
memcpy(p + 44, channel + 68, 44);  // remote config
sub_1C067660(channel);            // opens channel, allocates credit state
channel->callback(3, p);          // no subsequent free(p)
```

Exact instruction anchors: allocate `0x1C067718`; CID write `0x1C06773C`; local/remote config copies `0x1C067760`/`0x1C06776C`; state preparation `0x1C067772`; callback `R0=3,R1=p; BLX` at `0x1C067776..0x1C06777C`.

Remote MTU is independently produced from incoming LE signaling: `0x1C068232..0x1C068236` reads request+8 and stores channel+96; response `0x1C068836..0x1C068838` reads response+6 and stores channel+96. Thus its event offset is `44 + (96 - 68) = 72`. Response acceptance at `0x1C068858..0x1C068866` calls the OPEN producer after validation. Some signaling function entries here are missing from IDA's function inventory; bounded raw instructions establish the stores without modifying the shared IDB.

`0x1C067660` sets channel state +120 to 9. On first ordinary LE setup it allocates/zeros a 16-byte queue state at channel+4, sets receive-credit count +12 from channel+60 and transmit credits +14 from channel+104. The function then submits queued pre-open SDUs. This preparation happens before event3, so backend opening/send-after-open logic is appropriate.

The current P65 exclusion discards event3, leaving backend `Channel.opened=false` and remote MTU zero. Merely enabling the ABI support flag cannot repair this.

## CREDITS / event17

`0x1C068618(peer, signaling)` walks live channels matching peer and peer CID (channel+118) against signaling+4. It reads newly granted credits from signaling+6, adds them to `*(u16 *)(channel->queue + 14)`, and detects overflow at 65536. Overflow disconnects/rejects/tears down according to channel state and emits no ordinary credit report.

Normal path at `0x1C06866E..0x1C068696`:

```c
queue->tx_credits += delta;
sub_1C067374(queue, channel); // native queued fragments drained first
p = malloc(4); memset(p, 0, 4);
*(u16 *)(p + 0) = *(u16 *)(channel + 116);
*(u16 *)(p + 2) = delta;
channel->callback(17, p);
```

Exact anchors: native drain call `0x1C068674`; allocate `0x1C06867A`; delta/CID stores `0x1C06868C`/`0x1C06868E`; callback `0x1C068690..0x1C068696`.

Enable the existing shared `Event::Credits` branch and retain post-match `resume_send(index, cid)`. Do not add delta to the firmware credit queue again: accounting and drain already occurred. A credit event is flow-control availability, not application delivery acknowledgement.

## LOCAL_FLOW / event8

`0x1C065D24(owner, ignored_event, input)` finds the channel by input CID at +0 (or a special fixed-channel registry case), reads callback+20 and allocates a separate six-byte notification. Raw disassembly is required because Hex-Rays incorrectly infers a one-argument callback:

```text
1C065D50 MOVS R0,#6
1C065D52 BL   sub_1C166DA0
1C065D56 MOV  R1,R0          ; payload kept in second callback arg
1C065D58 MOVS R2,#1
1C065D5A LDRH R3,[R4]
1C065D5C MOVS R0,#8
1C065D5E STRH R3,[R1]       ; CID
1C065D60 LDRH R3,[R4,#2]
1C065D62 STRH R2,[R1,#4]    ; constant 1
1C065D64 STRH R3,[R1,#2]    ; copied input value
1C065D66 BLX  R5
1C065D68 MOV  R0,R4
1C065D6A BL   sub_1C167678   ; frees input, not callback notification
```

The producer is queued by public work wrapper `0x0C15AFA8` / SRAM counterpart (literal to Thumb entry `0x1C065D25` at `0x0C15AFD0` / `0x1C06B20C`). The copied input value's higher-level interpretation is not necessary to ignore/free it and is intentionally not claimed as a send-completion count. The independent event17 path is what updates LE channel credits.

## DATA / event7

`0x1C069198` parses incoming L2CAP frames, finds an open channel (state9), accounts receive credits, strips the four-byte L2CAP header, and handles ordinary LE reassembly through channel+4 queue state:

- With no partial SDU it reads the two-byte announced SDU length at buffer+4+offset.
- With a partial SDU it allocates a combined buffer using `0x1C1663B0`, copies old and new K-frame bytes, and frees both originals.
- When `(announced_len + 2) == (end - offset)`, it ORs the local CID into buffer+4, advances offset by two (removing the SDU-length prefix), invokes callback7 and clears its local owner pointer before its common free path.
- Oversized/inconsistent fragments are dropped and partial state cleared by the firmware; an incomplete SDU remains in queue-state+8, without callback.

Raw completion anchors `0x1C069550..0x1C06956A` write offset+2, write CID and call `R0=7,R1=completed_buffer` before setting the producer-owned buffer register to zero. The fixed/unfragmented L2CAP path at `0x1C06946E..0x1C069486` similarly transfers a buffer after advancing offset by four; ordinary LE uses the reassembly branch.

Therefore the existing P65 DATA arm is semantically the same as the shared DATA arm; consolidating it is supported. Emit/copy all data synchronously before final `free(p)`; never retain a slice into this firmware allocation. Empty SDUs are represented by `end==offset` after stripping the zero-length prefix and should still emit a zero-length Data event.

## CLOSED / event6

`0x1C0668BC(channel, reason)` removes the channel from its registry and drains/frees its fragment queue, partial receive buffer and pre-open SDUs. It separately allocates and zeroes 12 bytes; copies peer identity to +4 when available; writes CID+0/reason+2; then invokes callback6. After the callback it frees the channel allocation at channel-8. Payload is not subsequently freed by the producer.

Exact anchors: allocate `0x1C06695E` (or null-peer branch `0x1C0669CE`); identity copy `0x1C066974`; CID/reason stores `0x1C066978..0x1C066980`; callback `0x1C066982..0x1C066988`; channel allocation free `0x1C06698A..0x1C06698E`.

The existing shared CLOSED handler's payload fields and final free are correct. Do not dereference the native channel after the close callback has returned.

## Original SDUs and empty fragments

### Nonempty original SDU

`0x1C1673B0 -> 0x0C98F0D0` allocates a list node as `malloc(payload_size+8)`, zeroes it and returns allocation+8. `0x1C166688 -> 0x0C98F0F8` inserts this payload pointer and uses links at payload-8/-4. `0x1C168598 -> 0x0C98F194` dequeues payload pointers. Thus a fragment payload of 8 bytes has a 16-byte total allocation; Astrolink's `FRAGMENT_NODE_SIZE=16`, `FRAGMENT_NODE_PAYLOAD=8` is right.

Buffer allocator `0x1C1663B0 -> 0x0C98F480(payload_len, headroom)` sets offset=headroom+4, end=offset+payload_len, allocates end+6 bytes, zeroes the allocation, and sets buffer+8=3 when offset>7. Astrolink's original buffer offset16/end16+len, alloc end+6, CID word+4 and source bytes+20 accurately reproduce allocator(headroom12).

`0x1C067408(owner,event,original_buffer)` finds the channel by CID low16 at buffer+4. A nonopen channel queues the original as-is; an open ordinary LE channel reads remaining length=end-offset and channel MPS+106. It creates its own eight-byte fragment payload nodes, copies payload from buffer+4+offset, adds the two-byte full SDU length only to the first fragment, and sets first bit1 / final bit0x80. It eventually frees the original buffer at `0x1C067622` before calling the credit drain at `0x1C06762C`. Caller must never free or retry the same original after submission.

For an empty original, branch around `0x1C06745E..0x1C067478` skips fragment creation and reaches that free/drain path: sending it normally silently emits no zero-length SDU. The explicit empty-fragment path is necessary.

### Empty SDU correction

The following current construction is evidenced:

```c
buffer = malloc(24); memset(buffer,0,24);
buffer->end = 18; buffer->offset = 16;
*(u16 *)(buffer + 20) = 0; // two-byte SDU length, not app payload
buffer[8] = 1;           // REQUIRED stock fragment metadata, not original SDU default3
allocation = malloc(16); memset(allocation,0,16);
node = allocation + 8;
*(void **)(node + 0) = buffer;
node[4] = 0x81;          // first + last
queue_push(channel->queue, node);
credit_flush(channel->queue, channel);
```

P65 stock nonempty fragment producer writes node flag at `0x1C0674D4`, calls buffer allocator(headroom12) at `0x1C0674DA`, then **overwrites metadata+8 to1** at `0x1C0674DE..0x1C0674E2`. That override must also be made when synthesizing the empty fragment. Metadata is consumed by ACL scheduling (`0x0C99838C` loads buffer+8 and compares queue-priority ranks; `0x0C9980B8` also loads it for host packet behavior). No claim is made that value3 necessarily corrupts every empty packet; it fails to reproduce the measured native fragment contract and can change scheduling.

Credit drain `0x1C067374` dequeues while queue+14>0; writes four-byte L2CAP header immediately before payload (`0x1C06739E..0x1C0673B4`), reduces buffer offset by4, copies node flags into buffer+10 (`0x1C0673BE..0x1C0673C0`), transfers buffer to ACL queue (`0x1C0673D0`), decrements one credit, and frees only node allocation-8 (`0x1C0673D4..0x1C0673DE`). With zero credit both the fragment buffer and node remain owned by the firmware queue until a later grant or channel teardown. A module must not free them after queue_push/flush, including when no packet was immediately transmitted.

## Implementation recommendations and remaining gates

1. Remove the P65 exclusions on OPEN, combined DATA/CREDITS and LOCAL_FLOW. Delete the duplicate P65 DATA arm and duplicate target-specific default; preserve shared final free and credit resume.
2. Update LOCAL_FLOW comment to include exact P65 producer `0x1C065D24`; do not retain the sibling-only address as its P65 evidence.
3. Override `buffer[SDU_BUFFER_META]=1` in P65's empty branch after the nonempty early return. Keep original-buffer default3 for nonempty SDUs. Keep empty prefix/node flag/layout as-is.
4. Record this exact-image evidence with the P65 target, then build/verify and run lifecycle plus both LE CoC roles. Static recovery alone does not justify changing runtime support status to verified.
5. The callback payload layouts/ownership are no longer a static blocker. Runtime acceptance, GAP/foreign-group loop work handled by other investigations, and any host-specific regression remain separate gates. This pass did not audit all incoming/assigned/registration paths or all enhanced CoC events; the module requests ordinary LE CoC, and no support for enhanced event14 is implied.
