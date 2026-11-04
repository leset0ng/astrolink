# P65 native publication and loading regression

Scope: Redmi Watch 6 exact firmware `3.100.043`, emulator validation only.
The preceding Application slot27 correction is committed as `bde92f3` and
retained in `docs/p65-v46-runtime-validation.md`.

## Two distinct loading defects

The P65 pack bound `qjs_define_property_value_str` to `0x0C6D3320`, which is
actually a global-constructor installation helper. It takes the context global
as its definition target and uses the caller's object as the property value.
It then links constructor/prototype properties with fixed atoms and flags.
Its positive return was not evidence that the requested method was defined.

This explains the v46 observations precisely:

```text
expose_owner: global.subscribe = primitive_object
legacy_publish: global.__canopusAstrolinkNative = global
```

The native initializer created a real owner, a retained pipe endpoint, a plain
object (class1), and four native functions (class12, cproto1, magic0..3). Each
incorrect property helper returned1. JavaScript nevertheless saw system-global
keys on `__canopusAstrolinkNative` and an object-valued `subscribe`. The
notification/owner allocation was not the failure in this run.

The genuine string-name property wrapper is **`0x0C6D3210`**, callable
`0x0C6D3211`. Its prototype is unchanged:

```c
int define_property_value_str(void *ctx, uint64_t object,
    const char *name, uint64_t value, int flags);
```

It borrows the supplied object, interns/releases the name atom, passes the
supplied value/flags to the atom-based definition routine, and consumes the
value on both success and failure. The matching stock method constructor at
`0x0C9CC150` uses this exact wrapper at `0x0C9CC1A2` and `0x0C9CC1CA`.

Key instruction anchors:

- Correct wrapper `0x0C6D321C`: load name; `3222`: load value;
  `322A/322C`: retain supplied object registers.
- `3260/3264`: load supplied flags and OR0x2700; `3262/326C`: restore object;
  `3278/327E`: pass value/atom; `3280`: call engine define `0x0C6C7FB8`.
- Incorrect helper `0x0C6D3338`: load context global; `3388`: select global;
  `3396`: use caller object as value; `33A4`: define global[name].
  `33CA` and `3410`: hardcode prototype/constructor atoms0x3D/0x3E.
- Correct entry bytes16: `2de9f04f2ded028b8bb00aafd7f830b0`.

The source symbol and evidence live in the adjacent Canopus SDK target pack:
`targets/redmi-watch-6-3.100.043/symbols/` and
`EVID-ASTROLINK-P65-QJS-DEFINE-005`. Rust/C bindings were regenerated through
`canopus target generate redmi-watch-6-3.100.043`, and `--check` passes.
The symbol remains restricted/PENDING. No NativeProxy hooks or core export
semantics were changed.

Independently, the toolkit shadows `globalThis` with its wrapper argument.
The runtime diagnostic RPK measured:

| Expression | Runtime result before correction |
| --- | --- |
| `typeof globalThis.__canopusAstrolinkNative` | `undefined` |
| `typeof __canopusAstrolinkNative` | `object` |
| wrapper `global === globalThis` | `true` |
| `typeof __canopusAstrolinkNative.subscribe` | `object`, due to the wrong native definition helper |

`packages/astrolink/src/native.ts` now checks the unshadowed legacy identifier
with `typeof` before the existing Feature fallback. The facade cache remains
on the shared wrapper global. A compiled app/page regression with separate
engine and wrapper globals failed before the loader correction and passes
after it, including one subscription and proper disposal. Existing Feature
fallback behavior remains covered by the compiled tests.

## Empty ArrayBuffer rejection after loading was corrected

The v47 run passed loading, capabilities and the64-byte service-name boundary,
then returned `INVALID_ARGUMENT` while closing the boundary Session (3 checks).
The captured native request was correct: operation8, handle2, token1,
ArrayBuffer/tag-1, peer undefined/tag3, flag false/tag1.

P65 `ARRAY_BUFFER_DATA` was8, so the bridge treated a JSObject bookkeeping
pointer as its JSArrayBuffer payload. On the actual empty object at
`0x3C648430`, object+8 was `0x3C6CDA78`; byte4 of that unrelated storage was
0x38, making the bridge reject the attached buffer as detached. Object+40 was
`0x3D5E27D0`, whose byte_length and detached byte were both0.

Exact accessor `0x0C71725C` confirms the correction:

```text
0x0C71726A LDRH class,[object,#6] ; accepts ArrayBuffer19/SharedArrayBuffer20
0x0C717274 LDR payload,[object,#0x28]
0x0C717278 LDRB detached,[payload,#4]
0x0C71727C LDR length,[payload]
0x0C717282 LDR data,[payload,#8]
```

The copy constructor `0x0C7170D8` writes payload to object+40 at
`0x0C71719E`. Typed-array accessor `0x0C7172C4` also uses object+40,
returns an owned backing-buffer reference, and supplies byte offset/length.
ArrayBuffer class19, Uint8Array class23, class offset6, detached offset4 and
both accessor addresses were already correct. Only the P65 object payload
offset changes to40. Captured request evidence is
`/tmp/astrolink-p65-v47-request-diagnostic/stops.jsonl`; v47 installed ELF
readback is byte-identical to its enabled build.

## Recreated Session stalls on the wrong Bluelet queue

With property definition, loading and buffer access corrected, v48 passed15
checks, including scan/advertising start/stop and chat Session close. The first
recreated Session close timed out. A diagnostic RPK logged matching close
operation/session/token and eventually received COMPLETE/error0 about31 seconds
later, coinciding with unrelated companion traffic.

Timed hardware breakpoints separated execution from delivery:

| Close | Request time | `run_work` time | Delay |
| --- | --- | --- | --- |
| boundary Session7 | 1793788943.132949 | 1793788943.994099 | 0.861s |
| chat Session8 | 1793788946.065544 | 1793788946.849630 | 0.784s |
| recreated Session9 | 1793788947.131514 | 1793788977.972137 | 30.841s |

JS notification followed each actual backend completion immediately. Backend
retirement fields were vacant afterward; the completion was delayed, not lost.
Raw timing: `/tmp/astrolink-p65-v48-close-timing/stops.jsonl`; request/event
pairs: `/tmp/astrolink-p65-v48-event-diagnostic-probe/guest-logs/nsh-syslog.log`.

The pack's `bt_queue_external` entry `0x1C1676C8 -> 0x0C98FD8C` was owner-local
list insertion. It links a20-byte record without locking or waking Bluelet and
returns the record pointer. Cross-task insertion is incorrect on this path.

The actual cross-task API is `0x1C167D40 -> 0x0C98FDD0`, callable
`0x1C167D41`. It keeps the existing five arguments and event byte1. It locks
mutex `*0x3C492754`, writes the complete record, and publishes its ring write
index. On an empty-to-nonempty transition, `0x0C98FEB8` posts semaphore
`*0x3C492750` through `0x0C9A252C -> 0x0C4C7178`. The idle Bluelet consumer
waits on this same semaphore at `0x0C98FD66`.

Consumer `0x0C98FC1E` invokes `run(owner,event,argument)`. The queue retains
argument for run/cancel and never frees it. An unavailable queue invokes
`cancel(0,221,argument)` synchronously. The return is mutex-unlock semaphore
status or cancel result; callback ownership remains authoritative.

The corrected binding and regenerated artifacts carry evidence
`EVID-ASTROLINK-P65-BLUELET-QUEUE-006`, still restricted/PENDING. No ad-hoc
semaphore post, timeout extension, JS polling or owner-state shortcut is added.

## Build and validation artifacts

- SDK tests: 11 passed.
- Compiled bundle tests: 4 passed, including the new separated-global case.
- Source probe typechecking/build: passed.
- Enabled v47 P65 ARM Clippy: passed with `-D warnings`.
- Canopus ELF and package verification: passed;71369 resident bytes,
  1227 relocations, zero undefined symbols.
- Enabled v47 ELF SHA-256:
  `6fded027cf1eebb022ff22125da1a1907abc13fa758cbd87578acf9bcde4e611`.
- v47 package SHA-256:
  `06faa8a5a2e4d6899375278f57e9534805c7cfeb5b943da75ffb75d446a1bf51`.
- Corrected source probe SHA-256:
  `7ca544316a9f0e019c870717f93364dbb50f8c6db2c1351ec7c6663646c53ea4`.

The existing isolated library `/tmp/astrolink-p65-v46-validation` and device
`astrolink-p65-v46` are reused for v47; these are host-side historical names,
not the module version. Only that stopped clone's old Astrolink files and
single-module registry were removed before the fresh v47 install. The
production staged bootstrap and Supervisor were retained. After building the
enabled validation artifact, `SUPPORTED=false` and its regular gated build
were restored before runtime validation.

Diagnostic evidence:

- `/tmp/astrolink-p65-v46-owner-diagnostic/stops.jsonl`: owner/pipe success and
  incorrect positive publication return.
- `/tmp/astrolink-p65-v46-value-diagnostic/stops.jsonl`: actual primitive and
  four function values before the incorrect property helper.
- `/tmp/astrolink-p65-v46-global-probe/`: wrapper/engine-global separation.
- `/tmp/astrolink-p65-v46-method-diagnostic-probe/`: object-valued methods and
  system-global key list.
- `/tmp/astrolink-p65-v46-loader-fix-probe/`: JS-only correction still fails
  with `not a function`; it is not counted as acceptance.

## Final v49 runtime result

The fresh v49 package includes the corrected property helper and external
queue bindings, ArrayBuffer payload40 and the guarded real-global loader.
The default support gate and normal gated build were restored before execution.

- Enabled ELF SHA-256:
  `741077f30c0a25990e8c5774201c9e65546a3ba73d0eff4240404a8021df464a`.
- Package SHA-256:
  `21659e510d42a1390a8d62cd37a4706e36d8531e1852c7fafc6a8d9a4d02e0a6`.
- Unchanged corrected source-probe SHA-256:
  `7ca544316a9f0e019c870717f93364dbb50f8c6db2c1351ec7c6663646c53ea4`.
- ELF/package verification and enabled P65 ARM Clippy pass. Resident size
  remains71369 bytes, with1227 relocations and zero undefined symbols.

Actual fresh install reports version49/state3 and rebootRequired=false;
enable reports state5; activate reports version49/state13/error0.

**The formal source lifecycle probe passes18/18 with no failures.** It loads
the native interface, reads capabilities, accepts64-byte and rejects65-byte
service names, closes the empty-buffer boundary Session, starts/stops scan,
starts/stops one-byte advertising, repeats stop/close operations, and completes
two Session recreation/release rounds. It runs without diagnostic RPK wrappers
or debugger breakpoints.

The formerly delayed recreated Session releases now complete in49ms and47ms;
chat Session close completes in50ms and boundary Session close in34ms. The
terminal event is sequence37, event `complete`, passed18. It occurs during the
connected observation window, before companion disconnect; the fix does not
rely on unrelated traffic waking Bluelet.

Evidence:

- `/tmp/astrolink-p65-v49-install/`
- `/tmp/astrolink-p65-v49-enable/`
- `/tmp/astrolink-p65-v49-activate/`
- `/tmp/astrolink-p65-v49-native-probe/summary.json`
- `/tmp/astrolink-p65-v49-native-probe/guest-logs/nsh-syslog.log`
- `/tmp/astrolink-p65-v49-post-probe/`: fresh companion query still returns
  version49/state13/activationError0 after the completed probe.

The clone was then stopped, and `/canopus/inbox/astrolink.ko` was read back to
`/tmp/astrolink-p65-v49-installed-check.elf`; it is byte-identical to the enabled
v49 ELF above. The original stopped library and independent diagnostic PID379
remain preserved.

This establishes isolated emulator lifecycle/GAP acceptance. Bidirectional
CoC, HELLO/SDU exchange and physical-radio acceptance remain unvalidated.
`SUPPORTED=false` and target-pack restricted/PENDING remain unchanged.
Machine-readable results are retained in `docs/evidence/p65-v49-validation.json`.
