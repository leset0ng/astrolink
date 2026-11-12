# Astrolink

Direct Bluetooth LE messaging between QuickApps on Xiaomi wearables.

Two watches running the same QuickApp can find each other and exchange binary
messages over Bluetooth, with no phone and no network in between. Astrolink
is a resident Canopus native module that adds a `canopus.astrolink` Feature to
the firmware, plus a TypeScript package, `@lesetong/astrolink`, that wraps it
in a Session/Link API.

> **Status:** early (0.1). All runtime validation so far was done in the Mirage
> emulator. Testing between two physical devices is still to do.

## How it works

```
        Watch A                                    Watch B
┌───────────────────────┐                  ┌───────────────────────┐
│ QuickApp              │                  │ QuickApp              │
│ @lesetong/astrolink   │                  │ @lesetong/astrolink   │
├───────────────────────┤    advertise     ├───────────────────────┤
│ Astrolink module      │ ◄──── scan ────► │ Astrolink module      │
│ canopus.astrolink     │                  │ canopus.astrolink     │
├───────────────────────┤    LE CoC link   ├───────────────────────┤
│ Firmware BLE host     │ ◄──────────────► │ Firmware BLE host     │
└───────────────────────┘                  └───────────────────────┘
```

1. **Discover.** A Session advertises a 128-bit UUID derived from its
   `serviceId`, plus up to 8 bytes of your own data. A scanner only reports
   peers with the same `serviceId`. The name itself is never sent.
2. **Connect.** Peers open an LE L2CAP Credit-Based Connection on a dynamic
   SPSM, then exchange a short handshake. A peer with a different service is
   rejected before any data reaches JavaScript.
3. **Exchange.** Each `send` arrives as exactly one message on the other side,
   in order, from 0 to 512 bytes.

Astrolink uses the firmware's own Bluetooth host. It adds its own scan and
advertising handles and leaves the phone connection, pairing and the HCI layer
alone.

## Quick start

Declare the Feature in the QuickApp's `src/manifest.json`:

```json
{ "features": [{ "name": "canopus.astrolink" }] }
```

Then use the package:

```ts
import { loadAstrolink, type Link } from '@lesetong/astrolink'

const astrolink = loadAstrolink() // throws if the native module is missing
if (!astrolink.getCapabilities().supported) {
  throw new Error('Astrolink does not support this firmware')
}

const session = astrolink.createSession({ serviceId: 'chat' })

function attach(link: Link) {
  link.onData(data => console.log('received', new Uint8Array(data)))
  link.onDisconnect(reason => console.log('closed', reason))
}

// Watch A: advertise and accept incoming links
session.onConnection(attach)
await session.startAdvertising()

// Watch B: scan, connect, send
session.onAdvertisement(async peer => {
  await session.stopScan()
  const link = await session.connect(peer.peerId)
  attach(link)
  await link.send(new Uint8Array([1, 2, 3]))
})
await session.startScan()

// When the app exits: await session.close()
```

The full two-device demo lives in `examples/quickapp/`, and the package's own
README covers the complete API.

## Supported devices

Firmware internals differ from build to build, so support is per **exact**
firmware version. A firmware that is not adapted still loads the module, but
`getCapabilities().supported` is `false` and the firmware is never touched.

| Device | Firmware | Status |
| --- | --- | --- |
| Xiaomi Smart Band 11 | 4.100.155 | Validated: lifecycle and connections in both directions (reference target) |
| Xiaomi Smart Band 10 Pro | 3.101.043 | Validated: lifecycle and connections in both directions |
| Xiaomi Smart Band 11 | 4.100.179 | Lifecycle validated, connections not yet tested |
| Xiaomi Smart Band 10 Pro | 3.101.036 | Lifecycle validated, connections not yet tested |
| Xiaomi Smart Band 9 | 3.1.32 | Lifecycle validated, connections not yet tested |
| Redmi Watch 6 | 3.100.043 | Lifecycle validated, disabled by default |
| Xiaomi Smart Band 11 | 4.100.139 | Builds, no runtime result |
| Xiaomi Smart Band 10 | 3.1.28 | Disabled, static recovery only |
| Xiaomi Smart Band 9 Pro | 3.1.190 | Not supported |

"Lifecycle" is an 18-check probe covering Feature loading, scanning,
advertising and Session teardown. Details are in
[docs/target-support.md](docs/target-support.md).

## Limits

| | |
| --- | --- |
| `serviceId` | up to 64 UTF-8 bytes |
| Advertising data | up to 8 bytes |
| Message size | up to 512 bytes, or the peer's MTU if smaller |
| Links | 16 per device, shared by up to 4 Sessions |
| Outbound connects | one at a time per Session |
| Connect timeout | 15 s, including the handshake |
| Queues | 4 pending sends and 4 unread messages per Link, 40 messages per device |
| Peer cache | 8 entries |

A few things worth knowing:

- **Nothing is dropped or truncated silently.** A full send queue rejects with
  `BUSY` and keeps the message, so retrying is safe. A full receive queue
  closes only that Link, with `RECEIVE_OVERFLOW`.
- **`send` resolves on hand-off, not delivery.** It means the firmware
  Bluetooth host accepted the message, not that the remote app read it. Add your
  own acknowledgement if you need one.
- **Stopping advertising** blocks new incoming Links. Existing Links stay open.
- **The module stays loaded until reboot.** Closing a Session frees its
  resources but does not unload the code; disabling or replacing the module
  needs a reboot.

## Installing

Astrolink has two parts, and a QuickApp needs both:

1. **Native module** (`.canopus`). Build it for your exact firmware (see below),
   install it through the Canopus companion and activate it. Reopen any QuickApp
   afterwards so it picks up the new Feature.
2. **npm package.** Build the tarball and add it to your QuickApp:

   ```sh
   cd packages/astrolink
   pnpm install --frozen-lockfile
   pnpm pack:npm
   # then, in your QuickApp:
   npm install /absolute/path/to/lesetong-astrolink-0.1.1.tgz
   ```

## Building from source

You need a `Canopus-Private` checkout next to this repository, the
`thumbv8m.main-none-eabi` Rust target and LLVM/LLD. The required SDK revision
is described in [docs/build-verification.md](docs/build-verification.md).

```sh
rustup target add thumbv8m.main-none-eabi
export CANOPUS_ROOT="$(cd ../Canopus-Private && pwd)"
export PATH="/opt/homebrew/opt/llvm/bin:/opt/homebrew/opt/lld/bin:$PATH"

cargo build --manifest-path "$CANOPUS_ROOT/Cargo.toml" -p canopus-cli
"$CANOPUS_ROOT/target/debug/canopus" build --target xiaomi-band-11-4.100.155
"$CANOPUS_ROOT/target/debug/canopus" package
"$CANOPUS_ROOT/target/debug/canopus" package verify dist/astrolink-0.1.0.canopus
```

The device ELF ends up in `build/<target>/` and the installable package in
`dist/`.

Tests need one exact-firmware feature, because the generated address crate
only builds for its own target:

```sh
cargo test --features target-xiaomi-band-11-4-100-155
cargo clippy --all-targets --features target-xiaomi-band-11-4-100-155 -- -D warnings
cargo fmt --check
```

## Project layout

```
src/
  core.rs        Session/Link state, bounded queues, handshake and timeouts
  protocol.rs    advertisement and handshake encoding
  abi/           one file per firmware: recovered addresses and struct offsets
  backend/       GAP scanning/advertising and LE CoC on the firmware host
  qjs/           QuickJS Feature registration and task dispatch
  runtime.rs     async completion across firmware tasks
packages/astrolink/   TypeScript package (@lesetong/astrolink)
examples/quickapp/    two-device demo app and lifecycle probe
docs/                 protocol, firmware ABI notes and validation records
```

## Further reading

- [docs/wire-format.md](docs/wire-format.md): advertising packet and handshake format
- [docs/target-support.md](docs/target-support.md): device matrix and what adapting a firmware requires
- [docs/porting-playbook.md](docs/porting-playbook.md): adding a new firmware
- [docs/native-validation.md](docs/native-validation.md): validation evidence and what is still untested
- [docs/ble-abi.md](docs/ble-abi.md), [docs/qjs-abi.md](docs/qjs-abi.md): recovered Bluetooth and QuickJS firmware interfaces

## License

MIT. See `LICENSE`.
