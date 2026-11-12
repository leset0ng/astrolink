# Astrolink

Astrolink is a resident Canopus module that exposes peer BLE discovery and
bidirectional LE L2CAP Credit Based Connections to QuickApps. Every firmware it
supports is described by one exact-target ABI module under `src/abi/`; today
**Xiaomi Smart Band 11, firmware 4.100.155** is the reference for recovered
and validated CoC integration. P67 **3.101.043** passes both CoC connection
directions and the 18-check native lifecycle probe in Mirage. P67 **3.101.036**
independently passes the 18-check native lifecycle probe through command-line
installation and activation. See `docs/p67-runtime-validation.md`.
Other firmware versions need their own GAP, CoC, task and QuickJS recovery
recorded in `../Canopus-Private/targets/<target>/symbols`; those that have not
been recovered are listed with `abi_unsupported!()` and never call firmware.
`docs/target-support.md` holds the matrix and the requirement list.

The module uses the existing firmware Bluetooth host. It registers independent
scan/advertisement handles and dynamic LE SPSMs. It does not replace the HCI
receiver, the phone connection, the Bluetooth adapter, or pairing callbacks.

## Validated build

Version code **43** moves the message buffers out of each Link into one shared
40-slot pool and raises the device link table to 16 entries; it passes the
18-check native lifecycle probe on `.155`. Version code **37** is the same Q66
`.155` behaviour with the per-target ABI
layout; it passes the 18-check native lifecycle probe. Version code **36** is
the build that also passed both LE CoC connection directions in the isolated
Mirage environment. The example
exercised complete 0/1/511/512-byte SDUs, matching service HELLO, binary
request/reply checks, oversize rejection and CoC/LE cleanup. The final native
runs use the firmware Bluelet host and a standard external LE peer.
See `docs/native-validation.md` for artifacts, evidence and remaining hardware
acceptance limits.

Installable outputs:

- Native module: `dist/astrolink-0.1.0.canopus` (version code 44).
- npm package: `packages/astrolink/dist/lesetong-astrolink-0.1.1.tgz`.
- Example: `examples/quickapp/dist/astrolink-example-source.rpk` or
  `astrolink-example-bytecode.rpk` in that directory.
- Standalone lifecycle probe: `examples/quickapp/dist/astrolink-probe-source.rpk`.

```sh
npm install /absolute/path/to/lesetong-astrolink-0.1.1.tgz
```

## QuickApp API

The TypeScript package `@lesetong/astrolink` wraps the firmware native Feature
`canopus.astrolink`. Declare the Feature in the application's manifest:

```json
{"features":[{"name":"canopus.astrolink"}]}
```

The package provides Session and Link objects, cancellable subscriptions, and
Promises for asynchronous operations. The example under `examples/quickapp/`
shows discovery, advertising, incoming/outgoing connections, binary Ping/echo,
message boundaries and cleanup. See the package's README for installation and
its generated type declarations for the full API.

```ts
import { loadAstrolink } from '@lesetong/astrolink'

const astrolink = loadAstrolink()
const session = astrolink.createSession({ serviceId: 'chat' })
session.onConnection(link => {
  link.onData(data => console.log(new Uint8Array(data)))
})
session.onAdvertisement(async peer => {
  const link = await session.connect(peer.peerId)
  link.onData(data => console.log(new Uint8Array(data)))
  await link.send(new Uint8Array([1, 2, 3]))
})
await session.startAdvertising({ data: new Uint8Array([1]) })
await session.startScan()
// On page/application shutdown:
// await session.close()
```

Register data listeners on each Link before sending application data. The
example adds its own request/reply encoding solely to verify delivery; the
native module adds no application ACK or message-fragment protocol.

## Contract and limits

- `serviceId` is required, up to **64 UTF-8 bytes**, and stays local: the air
  carries only a 128-bit UUID derived from it (FNV-1a). Two Sessions on one
  device may not share a derived identity; a byte-exact duplicate name returns
  `BUSY`.
- One legacy advertising packet carries Flags, the derived 128-bit service UUID,
  frame version 2, the dynamic SPSM and up to **8 bytes** of user data. The
  service name is never transmitted, so it does not consume air time. Overlength
  input is rejected.
- The link handshake exchanges `[version][uuid]` and closes the Link with
  `INVALID_ARGUMENT` when the peer's identity differs.
- Device connection capacity is **16 Links**, shared across at most **4 Sessions**.
  Host tests cover allocating, closing and reusing all 16 Link entries. Each
  Session permits one in-flight outbound connect, so establish its Links serially.
- One `send` is one complete CoC SDU, including empty SDUs. Sending is limited
  to the smaller of the remote receive MTU and **512 bytes**. Local receive
  MTU is independently configured as 512 bytes.
- Each Link has four pending sends and four buffered received messages. All
  Links share a fixed pool of **40 messages** for both directions; the global
  budget can be exhausted before a Link reaches its per-direction bound.
  Transmit queue or pool saturation returns `BUSY`; receive queue or pool
  overflow closes only that Link with `RECEIVE_OVERFLOW`, without truncation
  or silent drops. A pending send retains its slot until host submission is
  confirmed, so `BUSY` retries keep the complete SDU. Reading a received message
  or closing its Link releases the corresponding slots.
- `send` resolves when the firmware host takes responsibility for the SDU.
  It does not assert that remote JavaScript or the remote application consumed it.
- Scan reports are filtered by service and deduplicated by peer/data/SPSM;
  `allowDuplicates` delivers repeated matching reports. The peer cache has eight
  entries. `peerId` preserves the address and address type as an opaque string.
- Connections have a 15-second establishment/HELLO deadline. Both peers exchange
  version and service name before the Link is exposed to JavaScript.
- Stopping advertising rejects new incoming channels while established Links
  continue. Controller termination of connectable advertising releases its set
  without revoking listener intent for the peer's ensuing CoC request. Explicit
  stop or Session close revokes admission even after that automatic termination.
  Closing a Session closes its own resources. Destroying the owning QuickApp
  context cancels its work and releases JS references on its owner task.
- The native Feature and callbacks remain resident until reboot. Disabling or
  replacing the module requires reboot; closing a Session does not unload code.

## Build

Keep this repository next to `Canopus-Private`. Install Rust's
`thumbv8m.main-none-eabi` target and LLVM/LLD. Use the SDK revision containing the
Astrolink exact-target allowlist and Rust linking fixes described in
`docs/build-verification.md`.

```sh
rustup target add thumbv8m.main-none-eabi
export CANOPUS_ROOT="$(cd ../Canopus-Private && pwd)"
export PATH="/opt/homebrew/opt/llvm/bin:/opt/homebrew/opt/lld/bin:$PATH"
cargo build --manifest-path "$CANOPUS_ROOT/Cargo.toml" -p canopus-cli
"$CANOPUS_ROOT/target/debug/canopus" build --target xiaomi-band-11-4.100.155
"$CANOPUS_ROOT/target/debug/canopus" package
```

Device ELF output is under `build/xiaomi-band-11-4.100.155/`; installation
packages are under `dist/`. Verify the resulting `.canopus` using
`canopus package verify PATH` before installing it through the companion API.
After activation, reopen a QuickApp so its fresh Feature registry includes
Astrolink.

Tests need one exact-firmware feature, because the generated address crate only
builds for the target it was generated from:

```sh
cargo test --features target-xiaomi-band-11-4-100-155
cargo clippy --all-targets --features target-xiaomi-band-11-4-100-155 -- -D warnings
cargo fmt --check
```

## Implementation and evidence

- `src/core.rs`: allocation-free Session/Link state, bounded queues, connection
  handshake, timeout, resource state and generation-safe routing.
- `src/protocol.rs`: standard legacy advertisement and HELLO formats.
- `src/abi/`: one module per firmware, holding the recovered callable and data
  addresses (from the target pack) plus every structure offset the backend uses.
- `src/backend/`: target-independent GAP and Bluelet CoC logic, owner-task
  submission and explicit firmware-buffer ownership.
- `src/qjs/`: per-application Feature registration, native binary argument
  handling, QJS task dispatch and context destruction.
- `src/runtime.rs`: asynchronous operation completion and cross-task integration.
- `docs/ble-abi.md`, `docs/qjs-abi.md`: recovered ABI and ownership evidence.
- `docs/simulator-validation.md`: reproducible isolated Mirage environment and
  actual observations.

Host tests and emulator tests do not establish two-device radio behavior. The
isolated QEMU worktree implements standard LE connection creation, ACL routing,
disconnect/reconnect and Classic coexistence; its 19 controller regressions pass
(see `docs/qemu-le-validation.md`). On top of that controller, the native module
and example pass incoming and outgoing CoC/HELLO/SDU integration, with actual
QuickApp binary-consumption logs. Two physical devices are still required for
radio acceptance, phone/Audio coexistence, throughput, heap stability and
screen-off testing. See `docs/native-validation.md` for the exact tested scope.
