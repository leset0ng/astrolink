# Q66 .155 QuickJS Feature binding

This record describes the Q66 `.155` values of
`src/abi/xiaomi_band_11_4_100_155.rs` and the target-independent bridge in
`src/qjs/feature.rs`. It targets only Q66 AP
`user-4.100.155-cn-202609041500`, SHA-256
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

Registration uses a synchronous, writable RAM dispatch path. The global at
`0x200D972C` points to the heap `FrameworkExt` object (also published through
`0x200C73B0`). Its vptr is a writable word; the stock vtable is `0x2CA8065C`.
Astrolink copies its thirteen primary virtual slots and two ABI prefix words
into resident module data and uses the Supervisor's **pointer** hook to replace
that object's vptr. All slots remain stock except factory slot +32. No firmware
instruction or flash vtable is written.

The factory `0x0C74CA84` constructs a 440-byte Application through `0x0C73F47C`,
adds it to the framework's application vector, and returns it before its JS
thread starts. The wrapper replaces the new Application's primary vptr with a
resident copy of its fifty primary slots plus two ABI prefix words. It changes
only initialize slot +8. The secondary vptrs at Application+4 and +24 stay stock.
The init wrapper calls the original init, then registers Astrolink in the
newly-created registry using Application+100 -> RuntimeContext+116 ->
FeatureManager+4 -> registry. The exact thread entry `0x0C74E2DC` calls
Application virtual init +8 before virtual run +16; source loading/evaluation
happens in run. Registration therefore runs on the actual JS owner task before
any source/native import in that context, rather than racing require afterward.

Initial installation requires the framework to exist and QuickApps to be closed
with no launch overlapping module activation. It checks the framework's app
vector under its render/list mutex (+204 in the implementation object) and
returns BUSY if the vector is nonempty. It also returns BUSY if another extension
already owns a different FrameworkExt vtable. There is no periodic application
scan, no post-launch registration retry, and no alternate JS transport.

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
`onCreate` obtains a generation-safe owner token and reads FeatureManagerQjs
from FeaturePrototypeQjs +12. The +28 pointer belongs to the QJS class object
constructed at `0x0C4929C8`; it must not be substituted for the Feature
prototype passed to this lifecycle callback. The prototype's JSValue slot at +56 is assigned only
**after** onCreate returns, so it cannot be used for property attachment there.
`onRequired` instead reads the already-initialized instance's weak JSValue at
+112, which `createJsInstance` (`0x0C4A0668`) sets before returning. Functions
are attached to that instance. They do not require a JS `this` object.

FeatureManager offsets used by the firmware itself include registry +4,
context-reference +8, task mutex +32, async handle +60 and task deque +80.
FeaturePrototypeQjs retains FeatureManagerQjs at +12, confirmed by the live
onCreate callback registers and the QJS prototype layout. The QJS class
constructor at `0x0C4929C8` separately stores the class's prototype at +28 and
initializes the class's self-linked list at +8/+12; those are different objects.
The Feature manager constructor passes the underlying JSContext through a
148-byte context table. No layout
from upstream QuickJS is substituted for these Feature objects.

`onDestroy` removes the owner slot while holding the binding's mutex, then
calls `Adapter.close_owner(owner)` and releases retained JS callbacks on the
owner task. Late queued work carries the owner token, not a JSContext pointer.
A reused slot receives a fresh nonwrapping token and drops old work.

## Owner-task dispatch

The module owns a persistent 64-byte libuv async handle for each JS owner.
It initializes the handle on that JS task, using the loop read from the stock
manager's async pointer at manager+60, then handle+4. Its async callback drains
the module's existing bounded native event queue on the JS thread. No new
polling loop, manager deque copy, or replacement of stock callbacks is involved.

The stock Feature manager post at `0x0C491CD0` enqueues a copied Task24 under
its mutex, then calls fd-based `uv_async_send` on its async handle. Actual v32
execution proved that invoking it from `miwear_bluetooth` resolves JS wake fd
85 in the wrong task group's file table: the first failure was EBADF at
`0x0C3F5758` (`async.c:213`), with return address `0x0C491DA7`. Therefore it
is not the cross-task-group notification mechanism used by this binding.

Instead, while still on the JS owner, the binding retains the loop's pipe
write endpoint (loop+316) with `fs_getfilep`, duplicates it into a zeroed,
8-byte-aligned 32-byte owned file with `file_dup3`, and releases the source
reference. Producers use `file_write` on that owned file, independent of their
numeric fd table. This target uses a pipe and one-byte writes, not eventfd.

| VA | Exact operation |
| --- | --- |
| `0x0C332C90` | `int fs_getfilep(int fd, file **out)`, retains a reference |
| `0x0C334998` | `int fs_putfilep(file *)`, releases the retained source |
| `0x0C341C7A` | `int file_dup3(file *src,file *dst,int flags)`, owned destination |
| `0x0C33CC4C` | `ssize_t file_write(file *,const void *,size_t)`, negative errno |
| `0x0C340A24` | `int file_close(file *)`, closes the owned duplicate |
| `0x0C3EA6E4` | `void uv_close(handle *,close_callback)`, owner-loop close |

The custom wake preserves libuv's atomic busy counter (+24) and pending flag
(+60), including pending exchange before a single-byte write, EINTR retry,
EAGAIN coalescing, and decrementing busy after the write. The receiver clears
pending before invoking the async callback. `qjs::notify` holds the binding
mutex across the wake, so owner withdrawal cannot race with a producer.
A normal drain processes at most 16 events per turn, then signals another turn
if needed. The existing deadline heartbeat also retries retained events after
an unexpected hard write error; it is not the normal delivery mechanism.

Feature onDestroy runs before the manager closes its own async. It removes the
owner from the producer registry under that same mutex, then calls uv_close on
the owned async. Only its close callback closes the owned file and frees handle
storage. The firmware's app shutdown runs uv_run(UV_RUN_NOWAIT) while draining
closing handles before destroying the loop. No callback after owner withdrawal
accesses its JS context, manager, or subscription. Owner tokens are nonwrapping.

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
example application; all QuickApps must be closed with no concurrent launch
during that one-time activation. The module stays resident afterward.

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
It does not prove that the RAM dispatch integration executes successfully on
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

Calls set Thumb bit zero. `0x0C495F2C` is a direct normal registration call;
it is no longer a firmware inline-hook target.
The absolute data references in the current implementation are the framework
singleton pointer at `0x200D972C`, FrameworkExt vtable with prefixes at
`0x2CA80654` (60 bytes), and Application vtable with prefixes at `0x2CA7C2CC`
(208 bytes). The locale name/descriptor addresses above are historical static
registration evidence and are no longer referenced by module code.

## Manifest initialization and hook-engine boundary

The application initialization path does **not** reject an unknown feature name
while collecting manifest declarations. In `Application::init` (`0x0C745FE0`),
`initDeclaredFeatures` starts at `0x0C7472B2`: it checks that `features` is an
array, iterates entries with a string `name`, and inserts each name into a
string tree. `0x0C74754E..54` logs `insert feature name:%s`. There is no Feature
registry lookup in that loop. Missing/wrong-shaped arrays take the informational
`No declared feature` path, then continue initialization at `0x0C7472DA`.
`0x0C737D34`, used later by `__app_require`, checks only this declared-name tree.
Thus the declaration check and implementation availability check are separate.

The available-feature query `Application::hasFeature` (`0x0C745254`, Application
vtable offset `0x88`) looks up the old registry and then the new per-app Feature
registry. It reports false for missing descriptions, including checks for dotted
member names. This is a capability-query path; the inspected app initialization
and app execution paths do not use it to reject every declared feature before
source evaluation. The stock Feature registry lookup's observed direct callers
are FeatureManager `findFeature` / `featureRequire` and this available-feature
query. This establishes the runtime behavior; it does not claim to exhaustively
prove every package-installer policy. A missing Astrolink implementation alone
has not been established as a reason to abort before the example's try/catch.
The manifest declaration must stay intact during real integration validation.

An initial attempt to inline-hook FeatureRegisterFeature was rejected by the
actual Supervisor with UNSUPPORTED. Its first eight bytes are copy-safe, but the
Supervisor's code view authorizes only module-owned executable memory and cannot
patch firmware Flash. This is a mechanism boundary; adding a callable address
to a binding allowlist cannot change it. That path was removed from the binding.
The replacement uses the existing RAM pointer-hook branch, which saves the
original pointer before assigning the replacement and does not call the code
view or instruction decoder.

The pointer hook target is the dynamic heap FrameworkExt object's first word.
The hook value is the module-resident vtable pointer, not a Thumb callable;
only the function pointers stored inside that table are callable. The SDK's
pointer-hook branch copies pointer values verbatim and accepts a word-aligned
SRAM/PSRAM slot. The original vtable and every untouched method are preserved.
The original Application initialize and factory methods are read from their
stock tables, making the two wrapper call signatures explicit:

```c
Application *createApplication(FrameworkExt *framework, const char *uri);
int initializeApplication(Application *application, RunOptions *options,
                          uv_loop_t *optional_loop);
```

The module's fixed data tables are never freed. Stock destructors replace their
own objects' vptrs as usual; the copied primary tables retain destructor slots,
ABI offset/type-info prefixes and every unrelated virtual method. The resident
lifecycle is required because any live Application can retain a copied vptr.
The root framework singleton is initialized once during the firmware's QuickApp
interface bootstrap (`0x0C7E88A0`), before ordinary installed QuickApps launch.

Proof sites for the replacement route:

| VA | Evidence |
| --- | --- |
| `0x0C7EA088..27C` | Creates FrameworkExt, stores `0x200D972C`, initializes it and publishes `0x200C73B0` |
| `0x2CA80654` | Two ABI prefix words followed by thirteen FrameworkExt virtual slots |
| `0x0C74CA84` | Factory allocates 440 bytes, calls Application constructor, and returns object |
| `0x0C73F47C` | Constructor installs primary vptr `0x2CA7C2D4`, secondary vptrs +4/+24, runtime initially zero |
| `0x2CA7C2CC` | Two ABI prefix words followed by fifty Application primary slots |
| `0x0C5849EC` | QuickApp frontend obtains/creates Application, then spawns JS thread |
| `0x0C74E2DC` | JS thread calls initialize slot +8, then run slot +16 |
| `0x0C745FE0` | Initialize creates RuntimeContext and per-app Feature manager/registry |
| `0x0C7479E8` | Run loads/evaluates app.js/app.jsc and starts user application execution |
