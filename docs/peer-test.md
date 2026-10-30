# External LE peer test equipment

`scripts/ble-peer-test.py` is a Python 3 standard-library HCI host for the
external peer-controller H4 TCP endpoint provided by Mirage/QEMU. It is separate
from the Astrolink module and npm package. It establishes a real controller LE
link before transmitting L2CAP packets; it does not call native Astrolink APIs,
fabricate their events, inject ACL before connection, or fall back to BR/EDR.

The current original QEMU checkout does not implement LE connection creation.
The separately implemented LE controller model must pass its controller tests
before this script can exercise the firmware path. An HCI Command Status of
success alone never satisfies this script's connection requirement. A run needs
an actual LE Connection Complete or LE Enhanced Connection Complete event with
the expected role and selected peer identity.

## Recorded integration

Module version 36 targets exact Q66 `4.100.155` and passes incoming and outgoing
CoC over the isolated Mirage controller model. Each role records LE establishment,
CoC, matching HELLO, 0/1/511/512-byte firmware SDUs, verified 511/512-byte replies
and successful cleanup. The actual example also records receipt of empty and
one-byte SDUs. `native-validation.md` contains exact artifacts and regressions.
These are simulated host/controller integration results; RF, phone/Audio
coexistence and hardware stability remain separate physical-device work.

## P67 integration

P67 `.043` module v42 also passes both roles using the same external peer
implementation. The accepted results are
`/tmp/astrolink-p67-v42-peer-central.json` (firmware incoming) and
`/tmp/astrolink-p67-v42-peer-outgoing.json` (firmware outgoing). Both verify real
LE establishment, matching HELLO, 0/1/32/511/512-byte firmware SDUs, byte-verified
511/512-byte replies, fully drained queues and successful cleanup. Actual
QuickApp logs independently confirm consumption of empty and one-byte SDUs.
The same Session is reused across disconnect/reconnect with new CIDs; a fresh
18/18 lifecycle probe passes after traffic. See `p67-runtime-validation.md` and
`evidence/p67-043-v42-coc.json`. P67 `.036` has its own 18/18 lifecycle result,
but no CoC peer acceptance is inherited from `.043`.

## Endpoint ownership and setup

Only one external host can own the controller endpoint. Finish the companion
connection used to install/launch the QuickApp before starting this script.
Do not run it concurrently with `mirage-smoke.py`, the Mirage companion UI, or
another external H4 host. Do not connect two controller UART endpoints together.

Use the running isolated device's `runtime.json` `bluetoothPort` value. The
script resets **its peer controller**, reads its public address and LE ACL buffer
capacity, and initializes event masks with reserved bits zero. In the inspected
QEMU configuration, the peer public address is `C0:FF:EE:00:00:01`; the AP
default is `C0:FF:EE:15:03:01`, and Mirage may configure the AP differently. The
script uses the controller's actual Read BD_ADDR result and does not invent or
reassign its public address. It does not reset the AP's controller,
install software, or operate the watch UI. Launch the built Astrolink QuickApp
through the normal companion installation/launch path first; then disconnect
the companion and operate the real QuickApp using Mirage touch input.

Offline checks require no controller:

```sh
python3 scripts/ble-peer-test.py --self-test
python3 -m py_compile scripts/ble-peer-test.py
```

The embedded 11 tests cover incremental H4 parsing, concatenated events/ACL,
interleaved handles, interrupted/orphan ACL fragments, Basic L2CAP lengths,
empty and 1/511/512-byte SDUs across MPS 23/64/128/512, receive-limit errors,
unequal local/remote CIDs for credit indications, credit overflow, request and
disconnect signaling, and the distinction between Command Status and a real LE
connection event. They test parsers and framing, not firmware execution.

## Exercise the QuickApp's scan/connect path

Start the equipment as a peripheral, substituting the actual port:

```sh
python3 scripts/ble-peer-test.py \
  --host 127.0.0.1 --port 64317 --role peripheral \
  --service chat --spsm 0x80 --duration 120 \
  --output /tmp/astrolink-peer-peripheral.json
```

The peer advertises the exact standard Service Data layout in
`src/protocol.rs`: Flags, AD type `0x21`, UUID
`9c43e0a1-f2d8-4b5c-8e67-1a623fb094d5`, version 1, SPSM, literal UTF-8 service
name, and one user byte `a5`. The default service is `chat`, the listening SPSM
is `0x80`, receive MTU is 512, receive MPS is 128, and initial receive credits
are 16. The advertised public address appears in `controller_ready` and
`ready_for_quickapp` output.

On the actual QuickApp:

1. Start scanning and select the displayed peer.
2. Connect. The firmware must create its LE ACL and issue a legacy LE Credit
   Based Connection Request. The equipment answers on signaling CID `0x0005`.
3. Wait for `astrolink_ready`; both directions must exchange the exact HELLO
   `01 04 63 68 61 74` for `chat` before application data is accepted.
4. Run Ping / binary boundary actions. The equipment responds to the example's
   `ALQ`, version 1, request type 1 envelope by changing only byte 4 to response
   type 2. Responses never echo again.
5. The example's binary/length action also sends an empty SDU and one byte
   `a5`, after its regular echo checks. These actual messages complete the raw
   receive-size observations. Ordinary example Ping envelopes
   cannot encode these lengths because their header is nine bytes. If an app
   does not send them, the test reports an incomplete boundary run and exits
   unsuccessfully. It never substitutes a larger envelope and calls it a zero-
   or one-byte message.

By default, after HELLO the equipment itself queues genuine SDUs of lengths
0, 1, 511 and 512 toward the firmware. The first two are empty and `a5`; the last
two use distinct deterministic example request sequences and verify every byte
of the real firmware replies. The application currently logs custom messages
for raw 0/1-byte receives. Those app logs/counters provide the additional
peer-to-firmware JS-consumption evidence; host/controller submission alone does
not prove JS consumed these two messages.

`--listen-only` disables the equipment's own boundary requests and only answers
actual QuickApp requests. `--hold` continues servicing traffic for the full
`--duration` after the required observations pass.

## Exercise the QuickApp's advertise/incoming path

Start the equipment as central:

```sh
python3 scripts/ble-peer-test.py \
  --host 127.0.0.1 --port 64317 --role central \
  --service chat --duration 120 \
  --output /tmp/astrolink-peer-central.json
```

Start advertising in the real QuickApp. The equipment performs active LE
scanning, parses complete standard advertisements, selects a connectable peer
whose literal service is `chat`, and takes the SPSM from that advertisement.
It stops scanning, issues LE Create Connection, waits for actual LE connection
completion, then opens CoC and verifies HELLO. `--peer AA:BB:CC:DD:EE:FF` limits
selection to one AP address. `--extended` uses LE Extended Create Connection
with the 1M initiating PHY instead of the legacy command.

Both modes use the same CoC and SDU implementation. They do not select a
classic ACL as an alternative.

## Wire and ownership details

- H4 packet type `0x01` carries HCI commands, `0x04` events and `0x02` ACL.
- The host respects the controller's LE ACL packet capacity. A CoC K-frame can
  span multiple HCI ACL fragments; PB first/continuation flags and Basic L2CAP
  lengths reassemble independently per connection handle.
- LE CoC signaling uses CID 5, request `0x14`, response `0x15` and Flow Control
  Credit indication `0x16`. The script listens on local CID `0x0040`, accepts
  the peer's independently allocated dynamic CID, and rejects mismatched
  credit/disconnect endpoints.
- A credit indication's CID is the **grant sender's local CID**. The recipient
  matches its remote endpoint CID. Linux v6.14 confirms this with
  `l2cap_chan_le_send_credits` assigning `pkt.cid = chan->scid`, and
  `l2cap_le_credits` looking up `l2cap_get_chan_by_dcid`.
- Each outgoing SDU becomes one or more K-frames, with a two-byte SDU length
  only in its first K-frame. An empty SDU is a real two-byte `00 00` first/last
  K-frame. Every K-frame consumes one channel credit. Incoming credits and
  receive MTU/MPS are checked independently; the script replenishes its granted
  credits before its bounded receive window reaches zero.
- The script handles L2CAP Disconnection Request/Response before issuing HCI
  Disconnect. Establishment and cleanup waits are bounded. Optional connection
  parameter updates are explicitly rejected rather than falsely acknowledged
  as applied. SMP pairing is outside this unauthenticated profile and produces
  a clear failure if the firmware requests it.

## Results and limits

JSON evidence is written to `--output`, including timestamped HCI packets,
advertisements, controller connection events, CoC parameters, HELLO, SDU lengths
and hashes, byte-verified example replies, and cleanup errors. It includes the
script hash so evidence can be associated with an implementation revision.

`summary.ok=true` requires genuine LE establishment, a matching HELLO, complete
firmware-to-peer reception of every required size, verified 511/512-byte
example replies in the default active run, fully submitted outgoing queues,
and successful cleanup. Any missing required size or protocol failure returns
nonzero. `host_queued_sdu_sizes` describes requests queued by this host;
`firmware_to_peer_sdu_sizes` describes actual complete SDUs received from the
firmware. These fields deliberately state different observations.

The implementation's first live integration runs are now recorded in
`native-validation.md`, including both successful v36 roles and earlier failures.
A successful run validates firmware protocol integration over the tested
controller model. It does not establish physical RF range/timing, hardware
buffer limits, phone or Audio coexistence, power behavior, or
two-physical-device acceptance.

Sources checked on 2026-10-29:

- Local `src/protocol.rs`: advertisement and HELLO wire contracts.
- Local `examples/quickapp/src/ts/probe.ts`: example-only binary request/response.
- Bluetooth Core 6.1, Vol 3 Part A, LE Credit Based Flow Control sections:
  <https://www.bluetooth.com/wp-content/uploads/Files/Specification/HTML/Core-61/out/en/host/logical-link-control-and-adaptation-protocol-specification.html>.
- Linux v6.14 L2CAP implementation, `l2cap_chan_le_send_credits`,
  `l2cap_le_credits`, `l2cap_le_connect_req`:
  <https://kernel.googlesource.com/pub/scm/linux/kernel/git/torvalds/linux/+/v6.14/net/bluetooth/l2cap_core.c>.
