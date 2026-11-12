# @lesetong/astrolink

TypeScript / JavaScript Session/Link API for the Canopus Astrolink BLE native Feature. The package contains compiled ES2015 ESM and CommonJS JavaScript plus TypeScript declarations, with **zero runtime npm dependencies**. ESM uses explicit `.mjs` entries and CommonJS uses `.cjs`, so aiot-toolkit’s JavaScript loaders cannot misclassify one module format as the other. It uses native events; no polling, file IPC or alternate Bluetooth transport is included.

The matching Astrolink native module must be installed and activated on a supported exact firmware target. Installing this npm package does not install the firmware module or add BLE support to Node.js or a browser.

## Install

Build the installable tarball from this repository:

```sh
cd packages/astrolink
pnpm install --frozen-lockfile
pnpm test
pnpm pack:npm
```

The resulting artifact is `dist/lesetong-astrolink-0.1.1.tgz`. Install that file in a QuickApp:

```sh
npm install /absolute/path/to/lesetong-astrolink-0.1.1.tgz
```

Declare the native Feature in the QuickApp `src/manifest.json`:

```json
{ "features": [{ "name": "canopus.astrolink" }] }
```

The package can be bundled by aiot-toolkit as an ordinary npm dependency. Do not configure it as a native external: the package wraps the lower-level Feature itself. No registry publication is implied or performed by these commands.

## Use

```ts
import astrolink, { type Link } from "@lesetong/astrolink"

const caps = astrolink.getCapabilities()
if (!caps.supported) throw new Error("Unsupported firmware")
const session = astrolink.createSession({ serviceId: "chat" })

function useLink(link: Link) {
  const offData = link.onData(data => console.log(new Uint8Array(data)))
  const offDisconnect = link.onDisconnect(reason => {
    console.log(reason)
    offData()
    offDisconnect()
  })
  return link.send(new Uint8Array([0, 128, 255]))
}

session.onConnection(link => { void useLink(link) })
session.onAdvertisement(event => console.log(event.peerId, event.rssi))
await session.startAdvertising({ data: new Uint8Array([161]) })
await session.startScan()
// For an advertised peer: const link = await session.connect(peerId)
// On application destruction: await session.close()
```

Importing the package is lazy: it does not immediately require the native Feature. To catch missing-module errors explicitly:

```ts
import { loadAstrolink } from "@lesetong/astrolink"

try {
  const astrolink = loadAstrolink()
  const caps = astrolink.getCapabilities()
  console.log(caps.supported)
} catch (error) {
  // Display a module installation / firmware compatibility error in your app.
  console.error(error)
}
```

The public typed API is exported as `Astrolink`, `Session`, `Link`, `Capabilities`, `Advertisement`, and `Unsubscribe`. `AstrolinkError` exposes `code`, `nativeCode`, and `backendError`. A send resolves when the native backend takes local submission responsibility; it does not promise the peer application consumed the message.

Current native limits are a 64-byte UTF-8 serviceId (transmitted only as its
128-bit derived UUID), 8-byte advertising data, up to 512-byte messages and
**16 Links** shared across at most **4 Sessions**.
Each Link allows four pending sends and four buffered receives; all Links share
one fixed pool of **40 messages** for both directions. Queue or pool exhaustion
rejects sends with `BUSY`, or closes only the receiving Link with
`RECEIVE_OVERFLOW`, without truncation or silent drops. Each Session permits only
one in-flight outbound connect, so establish its Links serially. Always use
`getCapabilities()` and `link.maxMessageBytes`; oversized arguments reject with
`INVALID_ARGUMENT` without truncation.

## Lifecycle and events

The confirmed native Feature is loaded through `$app_require$("@app-module/canopus.astrolink")`. Its primitive ABI is `getCapabilities`, `createSession(string) -> number`, `request(op, handle, token, data, peerId, flag) -> number`, and `subscribe(callback) -> number`. This package supplies the public objects, binary view handling, token-to-Promise completion, per-session routing, error mapping and cancellation.

All imports within one QuickApp global context share one façade and one primitive subscription, including separate app/page bundles. Use returned unsubscribe functions to release listeners and `session.close()` to release Session resources. `disposeAstrolink()` closes all package Sessions, waits for close completions, then unregisters the primitive callback. Call it during app destruction when this package is the application's sole Astrolink owner; it also prepares clean initialization on the next app entry.

Before a Link receives its first `onData` subscriber, up to four SDUs are retained in order. Overflow closes that Link with `RECEIVE_OVERFLOW` and requests native disconnection. Native/module limits remain authoritative; the JavaScript layer never silently drops a message and continues an apparently healthy Link.

`AstrolinkFacade`, `PrimitiveFeature`, `NativeEvent`, `OP`, and `EVENT` are named exports for native-boundary integration tests. They do not implement or simulate Bluetooth.

## Checks

```sh
pnpm test          # primitive ABI event/Promise/cleanup tests
pnpm test:install  # npm pack, isolated npm install, CJS/ESM imports and TS consumer
```

The install smoke test creates a temporary consumer outside this repository and installs the actual `.tgz` with npm offline. Tests use a numeric primitive fixture only at the JavaScript boundary; they do not validate firmware ABI execution or real BLE radio behavior. The full watch sample is in `../../examples/quickapp`.
