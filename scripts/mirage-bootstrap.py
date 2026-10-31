#!/usr/bin/env python3
"""Bring up the production Canopus Supervisor on a fresh Mirage device, headless.

The q66/p67 firmware exposes no usable NSH: `miragectl boot` injects `nsh pid 9`
and then every `miragectl sh` command returns no output at all, so the loader
cannot be started from a shell. The only in-firmware execution surface is an
installed watchface's Lua VM, which is why this script drives the installer once
per boot. It is the whole manual procedure, scripted:

  1. import the exact firmware, create the device (profile from the target family)
  2. pair through the real companion bridge, confirming the band's dialog
  3. install the production installer watchface and read its id back
  4. shut down, copy that target's loader/stage/supervisor/boot assets in
  5. boot, press Run, and wait for the Supervisor and the module to be ACTIVE
  6. optionally install a .canopus package, reboot, repeat 5, and run the probe

Signature of the last step: the registry is only trusted when the module reports
CANOPUS_STATE_BOOT_RESIDENT (13). INSTALLED (3) and ENABLED (5) come first, so
"the Supervisor answers" is not "the module is loaded".

Usage:
  mirage-bootstrap.py --library DIR --target TARGET_ID --device NAME --bridge DYLIB \\
      [--firmware NAME] [--module dist/x.canopus] [--probe probes/x.rpk]

Everything after the Supervisor is up is ordinary CLI: `miragectl` for
boot/screenshot and `mirage-smoke.py install|probe` for the module.
"""
import argparse
import importlib.util
import json
import subprocess
import sys
import tarfile
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('smoke', HERE / 'mirage-smoke.py')
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)

ROOT = HERE.parents[0]
CANOPUS = ROOT.parent / 'Canopus-Private'
DEFAULT_CTL = ROOT.parent.parent / 'Mirage' / 'Packages' / 'MirageKit' / '.build' / 'debug' / 'miragectl'
PROFILES = {'xiaomi-band-11': 'q66', 'xiaomi-band-10-pro': 'p67', 'xiaomi-band-9': 'n66',
            'xiaomi-band-9-pro': 'n67', 'xiaomi-band-10': 'o66', 'redmi-watch-6': 'p65'}
ASSETS = {'xiaomi-band-11': 'xiaomi-band-11', 'xiaomi-band-9': 'xiaomi-band-9',
          'xiaomi-band-9-pro': 'xiaomi-band-9-pro', 'xiaomi-band-10': 'xiaomi-band-10',
          'xiaomi-band-10-pro': 'xiaomi-band-10-pro', 'redmi-watch-6': 'redmi-watch'}
ASSET_SUFFIXES = ['loader_profile', 'stage1', 'stage2', 'supervisor', 'boot']
# The installer watchface draws these on its 212x520 panel.
RUN_BUTTON = (106, 383)
WAKE = (106, 300)
ACTIVE = 13  # CANOPUS_STATE_BOOT_RESIDENT
CTL = DEFAULT_CTL


def ctl(*arguments, check=True):
    result = subprocess.run([str(CTL), *map(str, arguments)], capture_output=True, text=True)
    if check and result.returncode != 0:
        raise SystemExit(f'miragectl {" ".join(map(str, arguments))}: '
                         f'{result.stderr.strip() or result.stdout.strip()}')
    return result.stdout.strip()


def pack(target):
    path = CANOPUS / 'targets' / target / 'target.toml'
    if not path.is_file():
        raise SystemExit(f'no target pack for {target} at {path}')
    lines = path.read_text().splitlines()
    family = next(line.split('=')[1].strip().strip('"') for line in lines
                  if line.startswith('family'))
    version = next(line.split('=', 1)[1].strip().strip('"') for line in lines
                   if line.startswith('firmware_version'))
    return family, version


def package_version(path):
    """versionCode from a .canopus package, used to spot a staged-only install."""
    with tarfile.open(path) as archive:
        return json.loads(archive.extractfile('manifest.json').read())['version_code']


def ensure_firmware(target, version, explicit):
    """Mirage firmware id for a target, importing the local package if needed.

    The pack records the exact firmware version but not the archive name, and a
    library can hold several builds of the same version, so this accepts an
    unambiguous match only; anything else has to be named with --firmware.
    """
    listing = ctl('firmware', 'list')
    if explicit:
        if explicit in listing:
            return explicit
        wanted = explicit
    else:
        # Only the archive lines start at column 0; their indented description
        # repeats the version, so matching every line would double-count.
        matches = [line.split()[0] for line in listing.splitlines()
                   if version in line and not line.startswith(' ')]
        if len(matches) == 1:
            return matches[0]
        if len(matches) > 1:
            raise SystemExit(f'{len(matches)} library firmwares match {version}; pass --firmware')
        wanted = version
    name = wanted if wanted.endswith('.bin') else f'{wanted}.bin'
    archive = Path.home() / 'develop' / 'mifw' / name
    if not archive.is_file():
        raise SystemExit(f'firmware package not found: {archive}')
    ctl('firmware', 'import', archive)
    return name[:-4]


def ensure_device(device, firmware, profile):
    listing = ctl('list')
    if any(device in line for line in listing.splitlines()):
        return
    ctl('create', device, '--firmware', firmware, '--profile', profile)


def connect(library, device, bridge_path, timeout=120):
    device_dir, record = smoke.find_device(Path(library), device)
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
    # appears once the pairing request reaches the firmware: press it from a
    # helper thread while the blocking connect call waits.
    outcome = {}

    def attempt():
        try:
            outcome['ok'] = bridge.call('connect', params)
        except Exception as error:  # noqa: BLE001 - reported to the caller
            outcome['error'] = error

    worker = threading.Thread(target=attempt, daemon=True)
    worker.start()
    deadline = time.monotonic() + 25
    while worker.is_alive() and time.monotonic() < deadline:
        ctl('tap', device, 106, 470, check=False)
        time.sleep(0.7)
    worker.join(timeout)
    if 'error' in outcome:
        raise outcome['error']
    return bridge


def stage_assets(library, device, target, assets):
    """Install the installer watchface and copy this target's payloads into it."""
    line = next(row for row in ctl('list').splitlines() if device in row)
    if 'Booted' not in line:
        ctl('boot', device, '--no-shell')
    ctl('tap', device, *WAKE)
    bridge = connect(library, device, BRIDGE)
    try:
        faces = bridge.call('watchfaces') or []
        identifier = next((face['id'] for face in faces if face.get('name') == 'Canopus'), None)
        if identifier is None:
            identifier = bridge.call('install', {'path': str(assets / '未命名表盘.bin'),
                                                 'kind': 'watchface'})['watchfaceId']
            print(f'installer watchface {identifier} installed')
    finally:
        bridge.call('disconnect')
    ctl('shutdown', device)
    market = f'/app/watchface/market/{identifier}'
    for suffix in ASSET_SUFFIXES:
        local = assets / f'canopus_{suffix}-{target}.bin'
        if not local.is_file():
            if suffix == 'boot':
                # The Band 9 staged loader has no persistent boot patch.
                continue
            raise SystemExit(f'missing installer asset {local}')
        ctl('fs', 'put', device, 'nand_data', local, f'{market}/{local.name}')
    print(f'staged {target} assets into {market}')


def activate(device, target, library, seconds, require_module=True):
    """Boot, press Run, and wait for the module to be resident.

    A freshly installed module starts in INSTALLED and only loads when its
    intent is ENABLED and the Supervisor starts again, so one extra boot is
    part of the documented activation rather than a retry.

    With require_module=False this only waits for the Supervisor, which is what
    an install needs: the module registry is empty before the first install, so
    waiting for a module state there would never succeed.
    """
    pressed_enable = False
    retries = 0
    for _cycle in range(2):
        ctl('shutdown', device, check=False)
        ctl('boot', device, '--no-shell')
        deadline = time.monotonic() + seconds / 2
        presses = 3
        while time.monotonic() < deadline:
            ctl('tap', device, *WAKE, check=False)
            if presses:
                # The watchface needs a moment after boot and a tap that lands
                # before it is interactive is lost, so press Run on a few early
                # polls only; a running loader must not be restarted.
                ctl('tap', device, *RUN_BUTTON, check=False)
                presses -= 1
            bridge = connect(library, device, BRIDGE, timeout=30)
            try:
                # The Supervisor answers deviceInfo before its own loop serves
                # the module registry, so the two queries fail independently.
                try:
                    addressable = bridge.call('canopusDeviceInfo').get('targetId') == target
                except Exception:  # noqa: BLE001 - not up yet
                    addressable = False
                if addressable and not require_module:
                    print('Supervisor addressable')
                    return True
                if addressable:
                    try:
                        state = bridge.call('canopusQueryModule', {'module': 'astrolink'})
                    except Exception:  # noqa: BLE001 - registry not serving yet
                        state = None
                    if state is None:
                        print('Supervisor addressable, module registry not answering')
                    elif state.get('state') == ACTIVE:
                        print(f'module active: {json.dumps(state)}')
                        return True
                    elif state.get('state') == 15 and retries < 1:
                        bridge.call('canopusActivate', {'module': 'astrolink'})
                        retries += 1
                        print(f'retried activation in place after error {state.get("activationError")}')
                    elif state.get('state') in (3, 5) and not pressed_enable:
                        bridge.call('canopusSetEnabled', {'module': 'astrolink', 'enabled': True})
                        pressed_enable = True
                        print(f'module intent enabled (state {state.get("state")})')
                        break
                    else:
                        print(f'Supervisor up, module state {state.get("state")}')
            finally:
                try:
                    bridge.call('disconnect')
                except Exception:  # noqa: BLE001 - already gone
                    pass
            time.sleep(5)
    return False


def install(device, library, package, target):
    ctl('tap', device, *WAKE, check=False)
    bridge = connect(library, device, BRIDGE)
    try:
        state = bridge.call('canopusQueryModule', {'module': 'astrolink'})
        if bridge.call('canopusDeviceInfo').get('safeMode'):
            bridge.call('canopusExitSafeMode', {'action': 'keep'})
            print('exiting safe mode with keep; rebooting before install')
            bridge.call('disconnect')
            if not activate(device, target, library, 60, require_module=False):
                raise SystemExit('Supervisor did not return after safe-mode exit')
            bridge = connect(library, device, BRIDGE)
            state = bridge.call('canopusQueryModule', {'module': 'astrolink'})
        if state and state.get('version') == package_version(package):
            if state.get('state') == 13:
                print(f'already resident at version {state.get("version")}; skipping same-version install')
                return
            bridge.call('canopusRemove', {'module': 'astrolink'})
            bridge.call('disconnect')
            if not activate(device, target, library, 60, require_module=False):
                raise SystemExit('Supervisor did not return before same-version reinstall')
            bridge = connect(library, device, BRIDGE)
            state = bridge.call('canopusQueryModule', {'module': 'astrolink'})
        if state and state.get('state') in (15, 20):
            # REMOVE is a boot intent on this Supervisor. Apply it before
            # installing the replacement generation, rather than reinstalling
            # into the failed slot in the same boot.
            if state.get('state') != 20:
                bridge.call('canopusRemove', {'module': 'astrolink'})
            print('removing old generation; rebooting before reinstall')
            bridge.call('disconnect')
            if not activate(device, target, library, 60, require_module=False):
                raise SystemExit('Supervisor did not return after failed-generation removal')
            bridge = connect(library, device, BRIDGE)
        print(f'module installed: {json.dumps(bridge.call("install", {"path": str(package), "kind": "canopus"}))}')
    finally:
        bridge.call('disconnect')


def remove(device, library):
    ctl('tap', device, *WAKE, check=False)
    bridge = connect(library, device, BRIDGE)
    try:
        bridge.call('canopusRemove', {'module': 'astrolink'})
        print('module removed')
    finally:
        bridge.call('disconnect')


def resident_state(device, target, library):
    bridge = connect(library, device, BRIDGE, timeout=30)
    try:
        return bridge.call('canopusQueryModule', {'module': 'astrolink'})
    finally:
        try:
            bridge.call('disconnect')
        except Exception:  # noqa: BLE001 - already gone
            pass


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
    parser.add_argument('--module', type=Path, help='install this .canopus package')
    parser.add_argument('--probe', type=Path, help='run the native probe with this .rpk')
    parser.add_argument('--stage-only', action='store_true',
                        help='only install the installer watchface and stage assets')
    parser.add_argument('--wait', type=float, default=240, help='seconds to wait for activation')
    args = parser.parse_args()

    global CTL, BRIDGE
    CTL = args.miragectl
    BRIDGE = args.bridge
    family, version = pack(args.target)
    assets = args.assets or (CANOPUS / 'watchfaces' / 'canopus-installer-prod' / ASSETS[family])
    firmware = ensure_firmware(args.target, version, args.firmware)
    ensure_device(args.device, firmware, PROFILES[family])
    print(f'device {args.device} on {firmware} ({PROFILES[family]})')
    stage_assets(args.library, args.device, args.target, assets)
    if args.stage_only:
        return
    if not activate(args.device, args.target, args.library, args.wait,
                    require_module=not args.module):
        raise SystemExit(f'Supervisor did not come up for {args.target}')

    if args.module:
        expected = package_version(args.module)
        install(args.device, args.library, args.module, args.target)

        # INSTALL of a resident module stages a new generation. Always boot it
        # before judging activation; a safe-mode fallback must not be retried
        # against the old resident image.
        ctl('shutdown', args.device, check=False)

        def resident():
            if not activate(args.device, args.target, args.library, args.wait):
                raise SystemExit('module did not become resident after the install')
            return resident_state(args.device, args.target, args.library)

        state = resident()
        if state.get('version') != expected:
            # Installing a version code that is already present only stages
            # `/canopus/inbox/astrolink.ko.next`; the registry keeps reporting
            # the previous image (or none) forever. Removing first makes the
            # install replace `.ko` instead of queueing a next generation.
            print(f'resident version {state.get("version")} != package {expected}; '
                  'removing and reinstalling')
            remove(args.device, args.library)
            install(args.device, args.library, args.module, args.target)
            state = resident()
            if state.get('version') != expected:
                raise SystemExit(f'resident module is version {state.get("version")}, '
                                 f'package is {expected}')
        print(f'resident version {expected} confirmed')

    if not args.probe:
        return
    subprocess.run([sys.executable, str(HERE / 'mirage-smoke.py'),
                    '--library', str(args.library), '--device', args.device,
                    '--bridge', str(args.bridge),
                    '--output', f'/tmp/astrolink-{args.device}-probe',
                    'probe', '--quickapp', str(args.probe)], check=False)
    summary = json.loads((Path(f'/tmp/astrolink-{args.device}-probe') / 'summary.json').read_text())
    print(json.dumps({'ok': summary.get('ok'), 'module': summary.get('module'),
                      'probe': summary.get('probe')}))


if __name__ == '__main__':
    main()
