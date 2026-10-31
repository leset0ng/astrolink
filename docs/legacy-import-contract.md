# Legacy JavaScript import contract

N66 firmware `3.1.32`, AP SHA-256
`9c02dab4020b2cc9666ee7d34cf27d311b76aadcec519a38361bbcbd94c53264`.
Static recovery only; runtime acceptance is pending.

## Declaration and initialization are different phases

The extension callback at node +12 runs before the module initializes. It must
**declare** exports with `JS_AddModuleExport`, N66 `0x0C1EEDFA(ctx,module,name)`.
The callback at node +8 runs during module initialization and **sets** declared
exports with `JS_SetModuleExport`, N66 `0x0C1EEE3C(ctx,module,name,value)`.
The setter consumes the value on success and failure. Reversing these calls
leaves the native module's exports uninitialized.

For N66, `src/qjs/legacy.rs` declares `canopusAstrolink` in +12, creates its
owner/method object in +8, then publishes it as
`globalThis.__canopusAstrolinkNative`. Runtime probing showed the stock
`Object.entries(core)` route left `aiot.canopusAstrolink` null; the direct
global-object export avoids substituting a fake Feature. O66/P65 retain their
separate global-object path and remain disabled.

## N66 bootstrap evidence

Three firmware bytecode blobs were fully parsed against its reader and
prepopulated atom pool, not ordinary upstream atom serialization:

| Module | Cached address | Bytes |
| --- | --- | --- |
| `@aiot/feature` | `0x2C6BB59C` | 155 |
| `@aiot/bootstrap` | `0x2C6B7E0C` | 1,842 |
| `@aiot/framework` | `0x2C6BB637` | 70,281 |

The bootstrap imports namespace `core` from `@aiot/core` and `exportFeatures`
from `@aiot/feature`. `Object.entries(core)` is evaluated at bootstrap +0x146.
It skips reserved names, routes signals and stock features, and defines every
other export as a read-only property on `aiot` at +0x1B4..+0x224. Finally it
publishes `aiot` on globalThis at +0x431..+0x463. `canopusAstrolink` is outside
the reserved and stock-feature name lists.

The private `$app_require$` function is at framework code +0xC5B7, cached
`0x2C6C7BEE`. It matches `^@app-module\/(.+)$` and looks up a private table;
it does not search arbitrary core exports. Manifest parsing only accepts
`system.`/`service.` names and the population code also requires a hardcoded
wrapper factory. Therefore adding a core export cannot make
`$app_require$("@app-module/canopus.astrolink")` work unchanged. The loader
itself is non-configurable/non-writable after bootstrap.

`packages/astrolink/src/native.ts` first checks `aiot.canopusAstrolink`, then
the direct legacy primitive, then uses the existing Feature loader for
Q66/P67. Facade caching and disposal are unchanged. Tests cover all paths,
including precedence and subscription release.

## Manifest and validation boundary

The N66 core export path does not invoke the stock Feature loader and needs no
custom Feature declaration. Newer Q66/P67 still require
`{"name":"canopus.astrolink"}`. A shared manifest can retain that declaration,
but the old framework can log an invalid-feature diagnostic; it does not grant
legacy access. Other stock features retain their normal requirements.

The parser, input blobs and fully decoded records are retained under
`/tmp/astrolink-n66-import/`; this document preserves the behavior and addresses
needed to review the implementation after temporary artifacts expire.
Safe application association, destruction, loop notification and exact
QuickJS bindings must still be proved for each enabled firmware. No runtime
result is inferred from bytecode decoding or package-level tests.
