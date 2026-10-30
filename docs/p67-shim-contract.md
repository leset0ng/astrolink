# P67 native integration contracts

Exact image: Xiaomi Smart Band 10 Pro `3.101.043`. The integration uses the
firmware's generic GAP, LE CoC and native Feature interfaces. Earlier notes
confused those interfaces with the MiWear convenience wrappers and raw stack
receive handlers; those notes are superseded by the contracts below.

## Register in each real application's registry

The sixteen-byte tuple supplied to `feature_registry_create` is a borrowed
constructor argument, not an independently usable application host.
`0x0C8660E4(host)` returns a **Feature manager**, whose raw registry is at `+4`.
Creating an unrelated manager with a zero-filled host does not publish a Feature
in an existing or future QuickApp.

The real ferry framework singleton lives at `0x2013CFE0`. Its FrameworkExt
constructor installs the cached vtable pointer `0x2CDA8CD8`; the live isolated
Mirage device independently confirmed this pointer before module activation.
The framework's implementation pointer is at `+4`.

The module preserves the real factory and initialization behavior, adding its
registration checkpoint through RAM virtual-table pointers:

| Item | Measured value |
| --- | --- |
| Framework primary vtable | `0x2CDA8CD8`, 13 slots plus two prefix words |
| Framework factory slot | 8, stock entry `0x0C8E54AC` |
| Implementation app-list mutex | `+348` |
| Implementation app vector begin/end | `+460` / `+464` |
| Application primary vtable | `0x2CDA7C80`, 50 slots plus two prefix words |
| Application initialization slot | 2, stock entry `0x0C8DB1F8` |
| Application runtime | `+100` |
| Runtime Feature manager | `+164` |
| Manager registry | `+4` |

Activation checks the live vtable and holds the framework's own recursive
app-list mutex while checking that no QuickApp is active and installing the
replacement dispatcher. Copies retain both Itanium prefix words and all stock
primary slots. Secondary Application vtables remain stock.

After the stock application initializer constructs its runtime and manager,
`feature_register_feature(registry, descriptor)` at `0x0C862898` receives the
module's resident descriptor. Its success result is exactly `1`; the ferry
bridge preserves the stock initializer's return convention.

The Supervisor hook engine does **not** implement FPB patching. A direct hook of
the Flash manager-creator thunk at `0x0C86236C` is therefore not a usable
production integration. The module uses actual RAM dispatch pointers.

## Native Feature lifecycle and JavaScript methods

The descriptor is 28 bytes: version, name, interface name, reserved word,
lifecycle pointer, member count and members pointer. The lifecycle contains six
function pointers. The relevant callbacks receive native C++ object pointers:

- `onCreate(JSContext*, FeaturePrototype*)`, lifecycle `+4`.
- `onRequired(JSContext*, FeatureInstance*)`, lifecycle `+8`.
- `onDestroy(JSContext*, FeaturePrototype*)`, lifecycle `+16`.

`prototype+12` points to the manager. `instance+112` holds its eight-byte
JavaScript object value. The prototype's JavaScript value at `+56` is assigned
**after** `onCreate` returns. The runtime-allocated opaque class identifier is
used by the stock JavaScript method dispatcher; lifecycle arguments themselves
are not JavaScript objects.

These measured native layouts permit shared owner tracking and JS method/value
conversion. The ferry registration path remains separate from Band 11's host
entry point.

## Event notification and lifetime

The manager's stock async pointer is at `+60`; the async handle's loop pointer
is at `+4`. On this firmware an async handle occupies **112 bytes**, with busy
word 6 and pending word 27. A 64-byte Band 11 allocation would overwrite the
following storage.

The bridge initializes its own correctly sized async on the owning JS task,
retains the loop's kernel pipe file for cross-task notification, and closes the
handle on the JS loop. Firmware `uv_close` is `0x0C2DB874`. Mutex wrappers are
`uv_mutex_init 0x0C2DCCCC`, lock `0x0C2DCDA8`, unlock `0x0C2DCDC4`.

The firmware's ordinary Feature task-post API at `0x0C86231C` takes three
arguments: native instance, callback, argument. It pins the instance and calls
`callback(active, argument)` with active `1` during normal drain or `0` during
destruction. Its ordinary descriptor-based libuv wake is not a substitute for a
pinned kernel file across NuttX task groups.

## Generic Bluetooth interfaces

The generic advertising producer `0x0C3B118C` takes the adapter, a **28-byte
parameter block**, advertising bytes, scan-response bytes and callbacks. Mode
is at parameter `+6`, power at `+15`, interval at `+16`. The high-level MiWear
routine `0x0C70ED94(flags)` constructs that block before calling the producer;
its flags are not the generic producer's ABI.

The generic scan producer is `0x0C3B1374`; scan stop is `0x0C3B140C` and
advertising stop is `0x0C3B125C`. The process-lifetime MiWear IPC client pointer
is at `0x20126738`; its business loop is at client `+56`. Generic IPC operations
must run in that business context with its retained wake resource.

`bt_queue_external 0x0C7EB9CC` takes **five** arguments: owner, run callback,
cancel callback, argument and flag. Error event `221` belongs to the cancellation
path. Omitting the cancellation callback shifts the remaining arguments and
cannot be treated as a valid alternative queue contract.

The generic CoC layer must be distinguished from the lower stack's receive
nodes. Verified callback identifiers include assigned `2`, open `3`, PSM
registered `9` and credits `17`. The ordinary accept handler is `0x0C80279C`,
which reads CID at `+0`, status at `+4`, detail at `+6`, and configuration at
`+52/+60/+62`. The handler at `0x0C802F18` applies disconnect/reject policy;
it is not the ordinary accept handler.

## QuickJS value APIs

Several earlier records described compositions where direct engine functions
exist. Measured entries include:

| Operation | Entry |
| --- | --- |
| `JS_NewStringLen(ctx, bytes, len)` | `0x0C294B50` |
| `JS_ToCStringLen2(ctx, len, value, cesu8)` | `0x0C29460C` |
| `JS_NewArrayBufferCopy(ctx, bytes, len)` | `0x0C2D34C8` |
| `JS_GetArrayBuffer(ctx, len, value)` | `0x0C2D362C` |
| `JS_GetTypedArrayBuffer(ctx, value, offset, len, element_size)` | `0x0C2D368C` |

ArrayBuffer objects have class 19 or 20, with their opaque pointer at object
`+32`. The opaque structure stores **length at `+0`, detached at `+4`, data at
`+8`**. The copy constructor allocates and copies its payload; it does not adopt
an external buffer. Strings returned by `JS_NewStringLen` are header pointers
with tag `-7`, and the function handles UTF-8 input.

Firmware-image analysis establishes ABI evidence. The integration is only
reported as emulator-verified after a real module activation and fresh native
QuickApp probe complete. Physical radio verification is a separate gate.
