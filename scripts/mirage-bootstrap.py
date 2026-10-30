#!/usr/bin/env python3
"""Bootstrap the production Canopus Supervisor on a fresh Mirage device.

The q66/p67 firmware exposes no usable NSH: `miragectl boot` injects `nsh pid 9`
and then every `miragectl sh` command returns no output at all, so the loader
cannot be started from a shell. The only in-firmware execution surface is an
installed watchface's Lua VM, so this script does exactly what the manual
procedure does, without hand-driven taps:

  1. import the exact firmware and create the device (profile from the family)
  2. pair through the real companion bridge, confirming the band's dialog
  3. install the production installer watchface and read its id back
  4. shut down, copy that target's loader/stage/supervisor/boot assets in
  5. boot, open the installer, press Run, and wait for the Supervisor to answer

Everything after this is normal CLI: `miragectl` for boot/screenshot and
`mirage-smoke.py install|probe` for the module.

Usage:
  mirage-bootstrap.py --library DIR --target TARGET_ID --device NAME \
      --bridge DYLIB [--firmware NAME] [--assets DIR] [--run]

--run also presses the watchface's Run button. Pass --no-run to stop after the
assets are staged.
"""
import argparse
import json
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import importlib.util

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('smoke', HERE / 'mirage-smoke.py')
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)

ROOT = HERE.parents[0]
CANOPUS = ROOT.parent / 'Canopus-Private'
DEFAULT_CTL = ROOT.parent.parent / 'Mirage' / 'Packages' / 'MirageKit' / '.build' / 'debug' / 'miragectl'
PROFILES = {'xiaomi-band-11': 'q66', 'xiaomi-band-10-pro': 'p67', 'xiaomi-band-9': 'n66',
            'xiaomi-band-9-pro': 'n67', 'xiaomi-band-10': 'o66', 'redmi-watch-6': 'p65'}
ASSETS = {
    'xiaomi-band-11': 'xiaomi-band-11',
    'xiaomi-band-9': 'xiaomi-band-9',
    'xiaomi-band-9-pro': 'xiaomi-band-9-pro',
    'xiaomi-band-10': 'xiaomi-band-10',
    'xiaomi-band-10-pro': 'xiaomi-band-10-pro',
    'redmi-watch-6': 'redmi-watch',
}
ASSET_SUFFIXES = ['loader_profile', 'stage1', 'stage2', 'supervisor', 'boot']
# The installer watchface draws Run at this point of its 212x520 panel.
RUN_BUTTON = (106, 383)
WAKE = (106, 300)


CTL = DEFAULT_CTL


def ctl(miragectl, *arguments, check=True):
    result = subprocess.run([str(miragectl), *map(str, arguments)], capture_output=True, text=True)
    if check and result.returncode != 0:
        raise SystemExit(f'miragectl {" ".join(map(str, arguments))} failed: {result.stderr.strip() or result.stdout.strip()}')
    return result.stdout.strip()


def pack(target):
    path = CANOPUS / 'targets' / target / 'target.toml'
    if not path.is_file():
        raise SystemExit(f'no target pack for {target} at {path}')
    text = path.read_text()
    family = next(line.split('=')[1].strip().strip('"') for line in text.splitlines()
                  if line.startswith('family'))
    source = next((line.split('=', 1)[1].strip().strip('"') for line in text.splitlines()
                   if line.startswith('source_repo')), None)
    return family, source


def ensure_firmware(miragectl, target, source, explicit):
    """Firmware id in the Mirage library, importing the local package if needed."""
    wanted = explicit or (source or '')
    listing = ctl(miragectl, 'firmware', 'list')
    if wanted and wanted in listing:
        return wanted
    if not wanted:
        raise SystemExit(f'cannot infer the firmware for {target}; pass --firmware')
    name = wanted if wanted.endswith('.bin') else f'{wanted}.bin'
    archive = Path.home() / 'develop' / 'mifw' / name
    if not archive.is_file():
        raise SystemExit(f'firmware package not found: {archive}')
    ctl(miragectl, 'firmware', 'import', archive)
    return name[:-4]


def ensure_device(miragectl, library, device, firmware, profile):
    listing = ctl(miragectl, 'list')
    if any(device in line for line in listing.splitlines()):
        return
    ctl(miragectl, 'create', device, '--firmware', firmware, '--profile', profile)


def connect(library, device, bridge_path, timeout=120):
    device_dir, record = smoke.find_device(library, device)
    runtime = json.loads((device_dir / 'runtime.json').read_text())
    key = record['binding']['authKey']
    evidence = Path('/tmp/astrolink-bootstrap')
    evidence.mkdir(exist_ok=True)
    handle = (evidence / 'bridge.jsonl').open('a')
    bridge = smoke.Bridge(bridge_path, smoke.Redactor([key]), handle, timeout)
    params = {'host': '127.0.0.1', 'port': runtime['bluetoothPort'], 'authKey': key,
              'stateDir': str(device_dir / 'AstroBox'), 'timeoutSecs': timeout - 5,
              'bandAddr': record['identity']['bluetoothAddress']}
    # A fresh binding waits for the band's confirmation dialog, which only
    # appears once the pairing request has reached the firmware. Press it from a
    # helper thread while the connect call blocks.
    outcome = {}

    def attempt():
        try:
            outcome['ok'] = bridge.call('connect', params)
        except Exception as error:  # noqa: BLE001 - reported to the caller below
            outcome['error'] = error

    worker = threading.Thread(target=attempt, daemon=True)
    worker.start()
    deadline = time.monotonic() + 25
    while worker.is_alive() and time.monotonic() < deadline:
        ctl(CTL, 'tap', device, 106, 470, check=False)
        time.sleep(0.7)
    worker.join(timeout)
    if 'error' in outcome:
        raise outcome['error']
    return bridge


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--library', type=Path, required=True)
    parser.add_argument('--target', required=True)
    parser.add_argument('--device', required=True)
    parser.add_argument('--bridge', type=Path, required=True)
    parser.add_argument('--miragectl', type=Path, default=DEFAULT_CTL)
    parser.add_argument('--firmware')
    parser.add_argument('--assets', type=Path)
    parser.add_argument('--run', dest='run', action='store_true', default=True)
    parser.add_argument('--no-run', dest='run', action='store_false')
    parser.add_argument('--stage-only', action='store_true')
    args = parser.parse_args()

    global CTL
    miragectl = CTL = args.miragectl
    family, source = pack(args.target)
    profile = PROFILES[family]
    assets = args.assets or (CANOPUS / 'watchfaces' / 'canopus-installer-prod' / ASSETS[family])
    firmware = ensure_firmware(miragectl, args.target, source, args.firmware)
    ensure_device(miragectl, args.library, args.device, firmware, profile)
    print(f'device {args.device} on {firmware} ({profile})')

    line = next(row for row in ctl(miragectl, 'list').splitlines() if args.device in row)
    if 'Booted' not in line:
        ctl(miragectl, 'boot', args.device, '--no-shell')
    ctl(miragectl, 'tap', args.device, *WAKE)
    bridge = connect(args.library, args.device, args.bridge)
    installed = False
    try:
        faces = bridge.call('watchfaces')
        installed = any(face.get('id') == '00000000000l' for face in (faces or []))
        identifier = '00000000000l'
        if not installed:
            result = bridge.call('install', {'path': str(assets / '未命名表盘.bin'), 'kind': 'watchface'})
            identifier = result['watchfaceId']
            print(f'installer watchface {identifier} installed')
    finally:
        bridge.call('disconnect')

    ctl(miragectl, 'shutdown', args.device)
    market = f'/app/watchface/market/{identifier}'
    for suffix in ASSET_SUFFIXES:
        name = f'canopus_{suffix}-{args.target}.bin'
        local = assets / name
        if not local.is_file():
            raise SystemExit(f'missing installer asset {local}')
        ctl(miragectl, 'fs', 'put', args.device, 'nand_data', local, f'{market}/{name}')
    print(f'staged {args.target} assets into {market}')
    if args.stage_only or not args.run:
        return

    ctl(miragectl, 'boot', args.device, '--no-shell')
    ctl(miragectl, 'tap', args.device, *WAKE)
    ctl(miragectl, 'tap', args.device, *RUN_BUTTON)
    deadline = time.monotonic() + 240
    while time.monotonic() < deadline:
        # The watchface needs a moment after boot; press Run on every poll so a
        # too-early tap cannot leave the installer idle.
        ctl(miragectl, 'tap', args.device, *WAKE, check=False)
        ctl(miragectl, 'tap', args.device, *RUN_BUTTON, check=False)
        bridge = connect(args.library, args.device, args.bridge, timeout=30)
        try:
            info = bridge.call('canopusDeviceInfo')
            if info.get('targetId') == args.target:
                print(f'Supervisor resident: {json.dumps(info)}')
                return
        except Exception:
            pass
        finally:
            try:
                bridge.call('disconnect')
            except Exception:
                pass
        time.sleep(5)
    raise SystemExit(f'Supervisor did not answer for {args.target}; inspect the installer screen')


if __name__ == '__main__':
    main()
