# P65 Application-vtable startup regression

Scope: exact Redmi Watch 6 firmware `3.100.043`. This is emulator evidence;
physical-radio and bidirectional CoC acceptance remain separate.

## Confirmed v45 failure

The legacy initializer replaces the Application primary vptr with
`APPLICATION_DISPATCH + VTABLE_PREFIX_WORDS`. The old P65 count of 29 words
included two Itanium ABI prefix words, leaving only primary slots 0..26.
The firmware reads slot27 at vptr+0x6c and can call it indirectly.

The independent, unmodified-v45 diagnosis recorded:

| Item | Observed value |
| --- | --- |
| Module base | `0x3d4ad020` |
| Application | `0x3c753be0` |
| Original vptr | `0x3c3275e0` |
| Copied storage including prefix | `0x3d4be610` |
| Published copied vptr | `0x3d4be618` |
| Copy end, exclusive | `0x3d4be684` |
| Slot27 address in replacement | `0x3d4be684`, immediately outside the copy |
| Original slot27 | `0x0c9fc601` |
| Replacement slot27 | `0` |
| Native initialization detail | `203`, after publication returned |
| Saved r3 / PC | `0` / `0` |
| Saved r2 / LR | `0x0c9fc601` / `0x0c9fd55b` |
| Saved xPSR / EXC_RETURN | `0x801f0000` / `0xffffffed` |
| CFSR | `0x00020000`, INVSTATE |

The firmware consumer is:

```text
0x0c9fd412  LDR R3, [R0]
0x0c9fd414  LDR R2, =0x0c9fc601
0x0c9fd416  LDR R3, [R3,#0x6c]
0x0c9fd41a  CMP R3, R2
0x0c9fd41e  BNE 0x0c9fd558
...
0x0c9fd558  BLX R3
```

This agrees with the saved null call target. Both extension export/init
callbacks had valid Thumb targets and returned. The shared exception entry
`0x0c50dad8` is not the invalid-call site.

Raw independent diagnostic evidence remains in
`/tmp/canopus-p65-diag-boundary/{report.md,stops.jsonl,vtable-at-fault.json}`.
The original stopped library and paused diagnostic clone are preserved.

## Correction

`src/abi/redmi_watch_6_3_100_043.rs` now copies 30 words: both prefix words and
all 28 primary slots, including slot27. The existing destruction override at
slot19 and the shared legacy initializer are unchanged.

The default `SUPPORTED=false` gate remains. Only the isolated v46 package was
built with the gate temporarily enabled; the gate and regular gated build
output were restored before runtime validation.

The enabled v46 ELF passes unchanged Canopus verification: 71,369 resident
bytes, zero undefined symbols, 1,227 relocations. SHA-256:
`5c597c19fd5939f73ee17105580f286050d70e0403c6e4a7b75e02a816877262`.
The single-target package passes package verification. SHA-256:
`9b12687ba93649eb3c8da8945c931e21c61aee31204f1d3fbdccf1ffaba74974`.

The existing 42 host tests, format check, and enabled P65/Q66 ARM Clippy pass.
The regression was checked at the actual firmware slot27 consumer rather than
with a test that only repeats the table-length constant.

## Corrected runtime result

A copy-on-write clone of the stopped original library was made at
`/tmp/astrolink-p65-v46-validation`. Its host-side device UUID is
`B65E0046-DCCD-4804-A89C-94E75D515805`, name `astrolink-p65-v46`, Bluetooth port
59021. This gives it separate runtime sockets from the paused v45 diagnosis.
Firmware identity and binding were retained. Production stage1/stage2 and
Supervisor were unchanged and loaded once per boot after MiWear initialized.

An initial upgrade attempt returned `rebootRequired=true`, but the post-reboot
query still showed version45; offline readback confirmed the old ELF. That
attempt is not counted as a corrected run. Only in the stopped new clone, the
old Astrolink `.cmi/.ko` generations and single-module `registry.bin` were
removed. A fresh install through the same real encrypted companion channel
returned version46/state3 with no reboot required. ENABLE returned state5;
ACTIVATE returned version46/state13 (`BOOT_RESIDENT`)/activationError0.

The existing source-probe RPK was retained unchanged. Hardware breakpoints at
`0x0c9fd416` and `0x0c9fd418` captured the real firmware slot27 load:

| Item | Corrected runtime value |
| --- | --- |
| Application | `0x3c6ed010` |
| Published copied vptr | `0x3c6e8758`, distinct from the original |
| Copied slot27 | `0x0c9fc601` |
| Original slot27 | `0x0c9fc601` |
| r3 after slot27 load | `0x0c9fc601`, equal to r2 comparator |
| Destruction override slot19 | `0x3c6ddc29`, still installed |

The UsageFault-handler breakpoint remained armed for 75 seconds after the last
slot27 stop and was not hit. The observer then removed the remaining breakpoint
and detached normally. The JavaScript page ran and emitted actual probe events;
the original null virtual call and immediate startup reboot did not recur.

The lifecycle probe still fails before any of its 18 checks:

```text
{"event":"page-init"}
{"sequence":1,"event":"start","step":"加载原生 Feature","passed":0}
{"sequence":2,"event":"failed","step":"加载原生 Feature","passed":0,"error":"cannot read property 'subscribe' of null"}
```

This is a later native-interface loading failure, not lifecycle acceptance.
Its cause has not been established by this virtual-table correction. No GAP or
bidirectional CoC result is claimed and the default support gate remains false.

Evidence directories:

- `/tmp/astrolink-p65-v46-clean-install/`: fresh v46 installation.
- `/tmp/astrolink-p65-v46-clean-enable/`: enabled intent.
- `/tmp/astrolink-p65-v46-clean-activate/`: actual v46 residency.
- `/tmp/astrolink-p65-v46-boundary/stops.jsonl`: slot27 reads and bounded
  UsageFault observation.
- `/tmp/astrolink-p65-v46-native-probe/`: failed lifecycle summary and actual
  JavaScript events in `guest-logs/nsh-syslog.log`.
- `/tmp/astrolink-p65-v46-post-probe/`: a fresh real companion query still
  returned version46/state13/activationError0 after the observation window.

The new validation clone was then stopped. Offline readback of
`/canopus/inbox/astrolink.ko` is byte-identical to the enabled v46 ELF above.
The original library remains stopped and the independent diagnostic process
PID379 was left intact; no bootstrap was repeated in that paused instance.

Machine-readable retained results: `docs/evidence/p65-v46-validation.json`.
