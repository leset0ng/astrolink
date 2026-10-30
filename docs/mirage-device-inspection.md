# Inspecting a Mirage device

How to read a running firmware's memory, run code in it, and move files onto it.
Everything here was verified on the p67 device (`probe-p67`, firmware 3.101.043)
on 2026-10-30; the q66 devices behave differently in two places, noted below.

## What each device exposes

A booted device has a runtime directory (`<library>/Devices/<uuid>/runtime.json`)
listing its endpoints: `qmp`, `qmpAux`, `console`, `vnc`, and `gdb` — but **`gdb`
only exists when the device booted in shell mode**, because QEMU is started with
`-S -gdb` only then (`LaunchPlan.swift`: `if device.options.shell`). Shell mode
also means the CPU starts halted and Mirage's NSH injector is what continues it.

`options.shell` lives in `device.json` and can be flipped while the device is
down:

```sh
miragectl shutdown probe-p67
python3 - <<'PY'
import json
p = '/tmp/astrolink-mirage-validation/Devices/<uuid>/device.json'
d = json.load(open(p)); d['options']['shell'] = True; json.dump(d, open(p, 'w'), indent=2)
PY
miragectl boot probe-p67        # prints "shell: nsh pid 9", creates gdb.sock
```

## Reading memory

QMP's `human-monitor-command` runs HMP, and HMP's `xp` reads guest physical
memory. A minimal client:

```python
send({'execute': 'human-monitor-command', 'arguments': {'command-line': 'xp /2048xw 0x20000000'}})
```

`xp /Nxw` returns 2048 words (8 KB) per call in well under a millisecond, so a
4 MB scan is ~500 calls. Address space on the `best1503` boards:

| Range | Contents |
| --- | --- |
| `0x20000000 .. 0x20160000` | internal SRAM (1.375 MB) — everything the firmware allocates |
| `0x1c000000`, `0x30000000` | further readable windows (PSRAM/flash windows) |
| `0x0c0c0000 +` | AP flash, XIP |
| `xip + 0x20000000` | the same flash through the cached alias (`0x2c0c0000 +`) |

Inside the guest, code executes from **both** aliases, so a static address has to
be tried in both forms (`0xc278294` and `0x2c278294` are the same bytes).

## Running code in the guest

q66 has no usable NSH: `miragectl sh` answers "The shell is not running" and
booting with `--no-shell` leaves none at all. **p67 does have one**, with the
builtin set including `xd`, `hexdump`, `dd`, `ps`, `free`, `insmod` / `lsmod` /
`rmmod`, `qjs`, `aiotjsc`, `dumpstack`, `log_service`. That is a much better
execution surface than the Lua watchface bootstrap q66 needs.

Backgrounding works, which matters because a foreground job holds the shell:

```sh
miragectl sh probe-p67 "qjs /data/probe.js &"
miragectl sh probe-p67 ps
```

The `qjs` on p67 is a **restricted parser**: `for(;;)`, C-style `for`, `while`
and even `if` are syntax errors; `do {} while (1)` and function declarations
work. A script that looks broken may just be using unsupported syntax.

## Moving files onto the guest

Writing through the *live* volume goes through NSH, which eats `;` and newlines —
a JS file arrives as one run-together line. Write to the partition while the
device is **down** instead:

```sh
miragectl shutdown probe-p67
miragectl fs put probe-p67 nand_data ./probe.js /probe.js   # lands at /data/probe.js
miragectl boot probe-p67
```

Verify by reading it back (`miragectl fs get probe-p67 live /data/probe.js out.js`)
and diffing; that check is what exposed the mangling.

## What this is for

Static analysis cannot recover values the firmware computes at run time — a
QuickJS class table, for instance, is built by registration calls, so its ids
have no static address. The device can answer those directly: create an object
with a recognisable payload, snapshot RAM once, find the payload, and read the
object header around the pointer to it. Use one snapshot per question, because
the heap moves between reads.
