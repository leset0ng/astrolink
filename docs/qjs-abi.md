# Q66 .155 QuickJS Feature binding

This binding targets only Q66 AP `user-4.100.155-cn-202609041500`, SHA-256
`ea0bdf1920cb30223d616432af00565ca67622e6468328f5eab155f8cdc2fb9f`.
Evidence was recovered from `analysis-q66-177/155/q66-ap.i64` using Hex-Rays
and checked against ARM instructions, particularly the placement of 64-bit
values. These are static ABI findings. The binding compiles for
`thumbv8m.main-none-eabi`; runtime import, RF operation and real-device cleanup
still require integration validation. Addresses below are even VAs; callable
Thumb pointers set bit zero.

## Registration and native import

`FeatureRegisterFeature` at `0x0C495F2C` accepts `(registry*, description*)`.
Its return is zero on failure and one on success. A registry is allocated per
application, rather than being a single global system registry. Registration
retains the description pointer. The module must therefore remain resident.

The stock QuickApp initializer `0x0C745FE0` creates its registry and registers
`locale` first at callsite `0x0C746460`, followed by the other builtins. Its
immutable locale name pointer is `0x2CB24EB4`. Astrolink installs a Supervisor
inline hook on `FeatureRegisterFeature`, calls the original registration, and
registers its own descriptor through the trampoline after this first builtin.
This preserves all stock registrations and does not change Bluetooth, packet,
pairing, or other modules' callbacks. Applications already running when the
module is activated must reopen to create a registry containing Astrolink.

The 28-byte description layout, verified against the builtin tables at
`0x2CA800D4..0x2CA80438`, is:

| Offset | Size | Use in binding / verified access |
| --- | --- | --- |
| 0 | 4 | Description version, builtin value 1 |
| 4 | 4 | NUL-terminated registry name `canopus.astrolink` |
| 8 | 4 | Interface/instance name; non-null gates `featureRequire` |
| 12 | 4 | Reserved, zero as in builtins; not otherwise interpreted |
| 16 | 4 | Pointer to lifecycle function table |
| 20 | 4 | Member count, zero for this binding |
| 24 | 4 | Member array, null for this binding |

Stock member entries are 12 bytes: type byte at +0, name pointer at +4,
member-specific description pointer at +8. `init_prototype` at `0x0C9151B8`
dispatches type 1 method, 2 property, 3 constant, and 4 event. Astrolink uses
zero stock members and attaches direct QuickJS native functions in
`onRequired`; it does not depend on the template FFI's type conversion ABI.

`AIOTJS::__app_require` at `0x0C73C944` strips the prefix `@app-module/`,
checks the resulting feature name against the application's manifest through
`0x0C737D34`, then calls the new FeatureManager before the old manager.
The manifest must contain `{ "name": "canopus.astrolink" }`. The source import
is `@canopus.astrolink` when the toolkit maps it to
`@app-module/canopus.astrolink`. The final compiler transformation is a
build/integration check, not something implied solely by the firmware's
compiler-only resolver at `0x0C721680`.

## Constructor and destruction ABI

The lifecycle table has six function pointers. Verified slots used here are:

| Offset | Signature | Evidence |
| --- | --- | --- |
| 0 | `void onRegister(const char *name)` | `0x0C495F2C` registration tail |
| 4 | `void onCreate(JSContext*, FeaturePrototype*)` | `0x0C4A0514` |
| 8 | `void onRequired(JSContext*, FeatureInstance*)` | `0x0C4A0A54` |
| 16 | `void onDestroy(JSContext*, FeaturePrototype*)` | `0x0C4A0B7C` |

Slots 12 and 20 are null; their precise semantics are not needed or claimed.
`onCreate` obtains a generation-safe owner token and stores the prototype's
manager pointer from +12. The prototype's JSValue slot at +56 is assigned only
**after** onCreate returns, so it cannot be used for property attachment there.
`onRequired` instead reads the already-initialized instance's weak JSValue at
+112, which `createJsInstance` (`0x0C4A0668`) sets before returning. Functions
are attached to that instance. They do not require a JS `this` object.

FeatureManager offsets used by the firmware itself include registry +4,
context-reference +8, task mutex +32, async handle +60 and task deque +80.
FeatureInstance holds its prototype at +28; the Feature manager constructor
passes the underlying JSContext through a 148-byte context table. No layout
from upstream QuickJS is substituted for these Feature objects.

`onDestroy` removes the owner slot while holding the binding's mutex, then
calls `Adapter.close_owner(owner)` and releases retained JS callbacks on the
owner task. Late queued work carries the owner token, not a JSContext pointer.
A reused slot receives a fresh nonwrapping token and drops old work.

## Owner-task dispatch

The existing Feature manager queue is the dispatch mechanism:

- `0x0C491CD0`: enqueue a copied 24-byte descriptor under the manager's mutex;
  call `uv_async_send` (`0x0C3F5700`) on manager+60 after releasing it.
- `0x0C49296E`: libuv async callback invokes the manager drain with active=1.
- `0x0C4928B0`: swap the queue under mutex, then invoke callbacks outside it.
- `0x0C49794E`: manager destructor invokes the same drain with active=0.

The descriptor is `{ pointer reference; pointer callback; pointer user;
pointer reserved; uint8_t simple; uint8_t padding[7]; }`, size 24 on ARM.
For simple=0 the dispatcher calls `callback(int active, void *user)`; for
simple=1 it calls a single-argument callback. The optional reference can be
null (`0x0C4928A4` checks before release). The binding sets reference=0 and
uses its integer owner generation as user data, so cancellation does not need
a live Feature instance or JS object. Descriptors are copied before enqueue
returns; stack storage is therefore valid for the enqueue call.

`qjs::notify(owner)` coalesces notifications to one queued drain per owner.
It holds the binding mutex through enqueue, ensuring onDestroy cannot remove
and free the manager during a post. The core/runtime lock must be released
before calling notify. Cancellation callbacks never access JavaScript. A
normal drain consumes at most 16 events in one turn and schedules another turn
if needed. No timer, file polling, UI heartbeat, or application virtual-call
ABI is used for message delivery. The separate module deadline timer may still
be used by the core for connection timeouts.

The binding's mutex uses the firmware libuv initializer/lock/unlock at
`0x0C3F4B64`, `0x0C3F4BF8`, and `0x0C3F4C14`. The initializer calls
`pthread_mutex_init` (`0x0C354DC0`); fixed, immovable 32-byte aligned storage
covers the fields accessed by that implementation. There are no spin loops
that can deadlock a higher-priority Bluetooth task behind a preempted JS task.

## QuickJS value and callable ABI

This firmware's `JSValue` is a **64-bit integer scalar**, passed and returned
using AAPCS integer-pair rules. It must not be represented by a Rust C struct,
which would change the return ABI. Context goes in r0, a following JSValue
aligns to r2/r3, and JSValue results return in r0/r1. Observed tags are int=0,
bool=1, null=2, undefined=3, exception=6, string=-7 and object=-1. Reference
counting applies to unsigned high words above `0xFFFFFFF4`.

| VA | Recovered signature / behavior |
| --- | --- |
| `0x0C3B982C` | `JSValue JS_NewObject(ctx)` |
| `0x0C3B98D0` | `JSValue JS_NewCFunction2(ctx,fn,name,length,cproto,magic)` |
| `0x0C3BF280` | `int JS_IsFunction(ctx, JSValue)` |
| `0x0C3CCB00` | `JSValue JS_Call(ctx,fn,this,argc,argv)`; arguments borrowed |
| `0x0C3C3C46` | `int JS_DefinePropertyValueStr(ctx,obj,name,value,flags)`; consumes value |
| `0x0C3B69E8` | `JSValue JS_NewStringLen(ctx,bytes,len)`; UTF-8 |
| `0x0C3B883C` | `char *JS_ToCStringLen2(ctx,len*,value,cesu8)` |
| `0x0C3B89F2` | `void JS_FreeCString(ctx,string)` |
| `0x0C3B813C` | `void JS_FreeValue(ctx,value)` |
| `0x0C3B793E` | `JSValue JS_GetException(ctx)`; takes exception and resets runtime slot |
| `0x0C3E7674` | `JSValue JS_NewArrayBufferCopy(ctx,bytes,len)` |
| `0x0C3E7F9C` | `uint8_t *JS_GetArrayBuffer(ctx,len*,value)` |
| `0x0C3E848C` | `JSValue JS_GetTypedArrayBuffer(ctx,value,offset*,len*,elementSize*)` |

The native callable uses cproto=1, the generic magic convention:
`JSValue fn(JSContext*, JSValue this, int argc, JSValue *argv, int magic)`.
The `JS_DefinePropertyValueStr` signature is especially significant: object
is r2/r3, name is stack+0, value is stack+8 and flags stack+16. Treating this
address as the shorter `JS_SetPropertyStr` ABI corrupts the call.

Class 19 is ArrayBuffer, class 23 is Uint8Array, and the backing buffer's
payload+4 is the detached flag. Uint8Array offset/length and backing buffer
bounds are checked before copying, including empty buffers. Other typed arrays
are rejected. Borrowed JS buffers are copied before Adapter.request returns;
the core/backend must not retain the borrowed byte slice.

## Core integration contract

`src/qjs/mod.rs` defines `Adapter`, `Capabilities`, `Event` and constants.
The module runtime supplies one static Adapter to
`unsafe qjs::install(ctx, &ADAPTER)`. Installation must precede opening the
example application, and the module stays resident afterward.

Native methods are:

```ts
getCapabilities(): { supported: boolean, maxLinks: number,
  maxServiceIdBytes: number, maxAdvertisingDataBytes: number }
createSession(serviceId: string): number
request(op: number, handle: number, token: number,
  data?: ArrayBuffer | Uint8Array, peerId?: string, flag?: boolean): number
subscribe(callback?: (event: NativeEvent) => void): number
```

A nonnegative return indicates synchronous success; negative values encode
errors. Owner tokens are internal to the binding. Adapter errors use positive
codes 1 UNSUPPORTED, 2 INVALID_ARGUMENT, 3 BUSY, 4 TIMEOUT,
5 DISCONNECTED, 6 RECEIVE_OVERFLOW and 7 BACKEND_ERROR.

Request operations are 1 startScan, 2 stopScan, 3 startAdvertising,
4 stopAdvertising, 5 connect, 6 send, 7 disconnect and 8 close. The flag is
allowDuplicates for startScan. A successful asynchronous request must eventually
enqueue an EVENT_COMPLETE carrying the supplied token, including failures
occurring after acceptance. Tokens must be positive 31-bit integers; handles
must also fit positive 31-bit integers. The JavaScript facade supplies the
public Promise/Session/Link API; Promise resolvers run inside the native event
callback on the JS owner task, through this same stock feature dispatch.

Event kinds are 1 complete, 2 advertisement, 3 inbound connection,
4 data and 5 disconnect. Each object contains `kind`, `token`, `session`,
`link`, `peerId`, `maxMessageBytes`, `rssi`, `error`, `backendError`, and `data`
(an ArrayBuffer). The facade knows the Session serviceId and supplies it in
public advertisement callbacks. Outbound connect returns its Link in the
completion event and does not emit an inbound-connection notification.

The binding owns four owner slots and one retained subscription per owner;
the runtime owns bounded session/link/event queues. JavaScript API callbacks
can replace/unsubscribe reentrantly: a drain holds a temporary function
reference, notices changed subscription, and schedules remaining work for the
new subscriber. Exceptions thrown by subscribers are taken and released.
Failure allocating an event object closes that owner's resources instead of
silently consuming additional messages. Adapter.next_event must provide valid
lengths, and must copy data into the fixed Event before returning.

## Validation boundaries

An isolated crate importing this binding passed `cargo check --features device
--target thumbv8m.main-none-eabi`. This checks Rust types and actual ARM target
layout. Firmware functions were renamed and annotated in the IDA database.
It does not prove that the Supervisor hook engine relocates this prologue on
the emulator, that app import executes successfully, or that Promise callbacks
and app exit are correct at runtime. Those require the module's simulator
integration probe and cannot be inferred from a successful static build.

## Independent instruction audit

A second pass checked the binary interfaces directly against instructions and
corrected an initial class-ID mistake: **Uint8Array is class 23 (`0x17`)**;
class 22 is Int8Array. The implementation now accepts class 19 ArrayBuffer and
class 23 Uint8Array only. This is established by the following linked evidence,
not by assuming upstream class numbering:

1. Runtime creation `0x0C3B94EC` starts builtin atoms at atom 1 with the packed
   NUL-separated table at `0x2CA19C9C` (`null`). Instructions `0x0C3B9572` and
   `0x0C3B9598..9E` initialize and increment the atom number in table order.
2. Table atom 160 is `ArrayBuffer` (`0x2CA1A186`), 161 SharedArrayBuffer,
   162 Uint8ClampedArray (`0x2CA1A1A4`), 163 Int8Array (`0x2CA1A1B6`) and
   164 Uint8Array (`0x2CA1A1C0`).
3. Context initialization `0x0C3B98F0` starts the typed-array class loop at
   class 21 (`MOV.W R8,#0x15`, `0x0C3BA520`). It obtains each constructor's
   atom as **class + 141** (`ADD.W R2,R8,#0x8D`, `0x0C3BA582`) and passes
   that same class as constructor magic (`STR.W R8,[SP,#4]`, `0x0C3BA59C`).
   Thus atom 164 Uint8Array maps to class `164-141=23`.
4. Constructor `0x0C3E7888` passes that magic to object creation as its class,
   then creates the backing-buffer view. `JS_GetTypedArrayBuffer` reads view
   offset/byte length at payload+16/+20 (`0x0C3E84B4..BE`) and checks the
   backing buffer's detached flag first (`0x0C3E84A0..AC`).

Undefined and integer zero are also independently distinct:

```text
0x0C492A38 MOVS R2,#0
0x0C492A3A MOVS R3,#3
0x0C492A3C STRD R2,R3,[R0,#0x60]
0x0C492A40 STRD R2,R3,[R0,#0x68]
0x0C492A44 STRD R2,R3,[R0,#0x70]
```

These Feature instance slots are initialized to canonical
`0x0000000300000000` undefined. Integer boxing at `0x0C48EACA` instead stores
its integer into the low word and zero into the high word (`0x0C48EACA..D0`),
so numeric zero is the all-zero u64. The binding checks the full canonical
undefined scalar and returns boxed integer zero for successful primitive calls.

The instance weak value used by `onRequired` is initialized first:

```text
0x0C4A08FE STRD R10,R11,[R5,#0x70]  ; instance+112 = JS instance value
0x0C4A0908 B return_createJsInstance
...
0x0C4A0B14 BL createJsInstance
0x0C4A0B18 ...
0x0C4A0B42 MOV R1,R5               ; same FeatureInstance pointer
0x0C4A0B46 LDR R0,[R11]            ; JSContext
0x0C4A0B4A BLX lifecycle_onRequired
```

The JSValue at +112 is borrowed: property definition uses it during
onRequired without increment/decrement. `createJsInstance` still owns the
new object's reference until the require return value is handed to the caller.

Registration success is explicit `MOVS R0,#1` at `0x0C4960EC`. Null registry
or null description name branch to `MOVS R0,#0` at `0x0C495F48`. The stock
caller tests zero and skips further builtin registration on failure at
`0x0C746464..66`; this confirms the hook's nonzero-success convention.
The stock literal at `0x0C746624` contains descriptor pointer `0x2CA80438`.
Its name field at +4 is the actual mapped pointer **`0x2CB24EB4`**, whose
bytes are `6c 6f 63 61 6c 65 00` (`locale\0`). Identical descriptor bytes at
flash aliases `0x0CA80438`, `0x28A80438` and `0x2CA80438` still retain this
same `0x2CB24EB4` name pointer. The hook compares that stored pointer and does
not invent a different alias or dereference the constant.

The module's complete absolute firmware code reference set is:

```text
0x0C3B69E8  0x0C3B793E  0x0C3B813C  0x0C3B883C  0x0C3B89F2
0x0C3B982C  0x0C3B98D0  0x0C3BF280  0x0C3C3C46  0x0C3CCB00
0x0C3E7674  0x0C3E7F9C  0x0C3E848C  0x0C3F4B64  0x0C3F4BF8
0x0C3F4C14  0x0C491CD0  0x0C495F2C
```

Calls set Thumb bit zero; `0x0C495F2D` is also the Supervisor inline-hook
target, whose original runs through the Supervisor-generated trampoline.
The sole absolute data identity reference is `0x2CB24EB4` (locale name, pointer
comparison only). The binding does not directly read fixed RAM globals.
Descriptor, manager, prototype, context and JS object addresses come from the
firmware's callback arguments or dynamically allocated objects.
