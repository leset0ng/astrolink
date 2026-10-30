#!/usr/bin/env python3
"""Drive the real Mirage companion C API; never simulate Astrolink or BLE.

Credentials are read from the selected device in-process, never passed in argv.
Evidence: redacted events.jsonl, summary.json, bounded guest log tails, screenshot.
"""
import argparse
import ctypes
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import tempfile
import threading
import time
import zipfile

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CTL = ROOT.parent.parent / 'Mirage/Packages/MirageKit/.build/debug/miragectl'
SENSITIVE = re.compile(r'auth.?key|link.?key|secret|credential|password|token', re.I)
HEX_KEY = re.compile(r'(?i)\b[0-9a-f]{32,128}\b')
CALLBACK = ctypes.CFUNCTYPE(None, ctypes.c_char_p, ctypes.c_void_p)


class Redactor:
    def __init__(self, values):
        self.values = sorted({v for v in values if isinstance(v, str) and len(v) >= 8}, key=len, reverse=True)

    def text(self, value):
        for secret in self.values:
            value = value.replace(secret, '[REDACTED]')
            value = value.replace(secret.upper(), '[REDACTED]')
        # Guest/bridge logs can contain keys unrelated to the selected binding.
        return HEX_KEY.sub('[HEX-REDACTED]', value)

    def object(self, value):
        if isinstance(value, dict):
            return {k: ('[REDACTED]' if SENSITIVE.search(k) else v if k == 'sha256' else self.object(v))
                    for k, v in value.items()}
        if isinstance(value, list):
            return [self.object(v) for v in value]
        return self.text(value) if isinstance(value, str) else value


def find_device(library, selector):
    matches = []
    for path in (library / 'Devices').glob('*/device.json'):
        record = json.loads(path.read_text())
        if record['name'] == selector or record['id'].lower().startswith(selector.lower()):
            matches.append((path.parent, record))
    if len(matches) != 1:
        raise ValueError(f'device selector must match exactly one device; matched {len(matches)}')
    return matches[0]


class Bridge:
    def __init__(self, path, redactor, evidence, timeout):
        self.redactor, self.evidence, self.timeout = redactor, evidence, timeout
        self.condition = threading.Condition()
        self.lock = threading.Lock()
        self.results = {}
        self.callback = CALLBACK(self.receive)  # retain for the lifetime of the native runtime
        self.native = ctypes.CDLL(str(path))
        self.native.mab_init.argtypes = [CALLBACK, ctypes.c_void_p]
        self.native.mab_call.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
        self.native.mab_call.restype = ctypes.c_uint64
        self.native.mab_init(self.callback, None)

    def emit(self, event):
        event = self.redactor.object(event)
        line = json.dumps({'at': datetime.datetime.now(datetime.timezone.utc).isoformat(), **event}, ensure_ascii=False)
        with self.lock:
            self.evidence.write(line + '\n')
            self.evidence.flush()
            if event.get('type') in ('request', 'result', 'error', 'evidence'):
                print(line, flush=True)

    def receive(self, raw, _):
        try:
            event = json.loads(raw.decode('utf-8', errors='replace'))
            self.emit(event)
            if event.get('type') == 'result':
                with self.condition:
                    self.results[event['id']] = event
                    self.condition.notify_all()
        except Exception as error:
            self.emit({'type': 'error', 'message': str(error)})

    def call(self, method, params=None):
        params = params or {}
        self.emit({'type': 'request', 'method': method, 'params': params})
        request = self.native.mab_call(method.encode(), json.dumps(params).encode())
        if not request:
            raise RuntimeError(f'{method}: bridge did not queue request')
        deadline = time.monotonic() + self.timeout
        with self.condition:
            while request not in self.results:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise TimeoutError(f'{method}: no response within {self.timeout:g}s')
                self.condition.wait(remaining)
            result = self.results.pop(request)
        if not result.get('ok'):
            raise RuntimeError(f'{method}: {self.redactor.text(str(result.get("error", "unknown error")))}')
        return result.get('result')


def artifact(path):
    path = path.expanduser().resolve(strict=True)
    if not path.is_file():
        raise ValueError(f'not a file: {path}')
    return {'path': str(path), 'bytes': path.stat().st_size, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}


def capture_logs(device_dir, output, redactor, limit):
    captured = []
    for path in sorted((device_dir / 'Logs').glob('*.log')):
        with path.open('rb') as stream:
            size = path.stat().st_size
            stream.seek(max(0, size - limit))
            text = stream.read(limit).decode('utf-8', errors='replace')
        dest = output / 'guest-logs' / path.name
        dest.parent.mkdir(exist_ok=True)
        dest.write_text(redactor.text(text))
        captured.append({'source': path.name, 'sourceBytes': size, 'tailLimitBytes': limit, 'output': str(dest)})
    return captured


def capture_ramlog(runtime, output, redactor, base, size):
    """Read one explicitly selected bounded RAM window; retain only log lines."""
    endpoint = runtime.get('endpoints', {}).get('qmpAux', '')
    if not endpoint.startswith('unix:'):
        raise ValueError('bounded RAM log capture requires the device QMP Unix endpoint')
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(10)
        connection.connect(endpoint[5:])
        with connection.makefile('rwb') as stream:
            json.loads(stream.readline())

            def command(name, arguments=None):
                stream.write(json.dumps({'execute': name, 'arguments': arguments or {}}).encode() + b'\n')
                stream.flush()
                while True:
                    response = json.loads(stream.readline())
                    if 'return' in response:
                        return response['return']
                    if 'error' in response:
                        raise RuntimeError(response['error'])

            command('qmp_capabilities')
            descriptor, temporary = tempfile.mkstemp(prefix='astrolink-log-window-')
            os.close(descriptor)
            try:
                command('pmemsave', {'val': base, 'size': size, 'filename': temporary})
                data = Path(temporary).read_bytes()
            finally:
                Path(temporary).unlink(missing_ok=True)
    lines = []
    for match in re.finditer(rb'\[\d\d/\d\d \d\d:\d\d:\d\d\]', data):
        end = data.find(b'\n', match.start())
        end = end + 1 if end >= 0 else len(data)
        raw = data[match.start():min(end, match.start() + 1024)].split(b'\0', 1)[0]
        text = redactor.text(raw.decode('utf-8', errors='replace'))
        text = ''.join(char for char in text if char in '\n\t' or ord(char) >= 32)
        lines.append(f'{base + match.start():08x} {text.rstrip()}\n')
    destination = output / 'guest-logs' / 'ram-syslog.log'
    destination.parent.mkdir(exist_ok=True)
    destination.write_text(''.join(lines))
    return {'base': hex(base), 'windowBytes': size, 'timestampLines': len(lines), 'output': str(destination)}


def capture_nsh_log(device, output, redactor, miragectl, library, limit):
    """Capture the firmware log through a working NSH, as on p67."""
    result = subprocess.run(
        [str(miragectl.expanduser().resolve()), 'sh', device, 'dmesg'],
        env=dict(os.environ, MIRAGE_LIBRARY=str(library.expanduser().resolve())),
        capture_output=True, text=True, timeout=20,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr or result.stdout or 'NSH dmesg failed')
    destination = output / 'guest-logs' / 'nsh-syslog.log'
    destination.parent.mkdir(exist_ok=True)
    tail = result.stdout.encode('utf-8')[-limit:].decode('utf-8', errors='replace')
    destination.write_text(redactor.text(tail))
    return {'output': str(destination), 'tailLimitBytes': limit}


def probe_result(log_path):
    """Require an actual terminal probe event, not only launch acknowledgement."""
    events = []
    for line in log_path.read_text().splitlines():
        marker = '[AstrolinkProbe] '
        if marker not in line:
            continue
        try:
            event = json.loads(line.split(marker, 1)[1])
        except json.JSONDecodeError:
            continue
        if event.get('event') == 'page-init':
            events.clear()
        events.append(event)
    failures = [event for event in events if event.get('event') in ('failed', 'runner-error')
                or (event.get('event') == 'cleanup' and event.get('result') == 'failed')]
    complete = [event for event in events if event.get('event') == 'complete']
    return {'passed': bool(complete) and not failures,
            'checksPassed': max((event.get('passed', 0) for event in events), default=0),
            'failures': failures,
            'completed': complete[-1] if complete else None}


def parser():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--library', type=Path, required=True)
    p.add_argument('--device', required=True, help='exact device name or UUID prefix')
    p.add_argument('--bridge', type=Path, required=True, help='dylib exporting mab_init/mab_call')
    p.add_argument('--miragectl', type=Path, default=DEFAULT_CTL)
    p.add_argument('--output', type=Path, help='evidence directory (default: new /tmp/astrolink-smoke-* directory)')
    p.add_argument('--timeout', type=float, default=60)
    p.add_argument('--settle', type=float, default=2, help='seconds before final screenshot/log capture')
    p.add_argument('--log-bytes', type=int, default=262144, help='maximum tail bytes per existing guest log')
    p.add_argument('--probe-log-source', choices=['ram', 'nsh'], default='ram',
                   help='probe terminal events from a measured RAM window or a working NSH dmesg')
    p.add_argument('--ram-log-base', type=lambda value: int(value, 0), help='optional exact RAM log window address, e.g. 0x20100000 on current Q66 .155')
    p.add_argument('--ram-log-bytes', type=int, default=65536, help='RAM log window bytes (1..262144), only with --ram-log-base')
    commands = p.add_subparsers(dest='command', required=True)
    commands.add_parser('status')
    install = commands.add_parser('install', help='install supplied artifact(s) through real companion transport')
    install.add_argument('--module', type=Path, help='verified .canopus artifact; no implicit activation')
    install.add_argument('--quickapp', type=Path, help='built .rpk artifact')
    launch = commands.add_parser('launch')
    launch.add_argument('package')
    launch.add_argument('--page', default='')
    remove = commands.add_parser('remove')
    remove.add_argument('package')
    probe = commands.add_parser('probe', help='query native residency, optionally install RPK, then launch and hold one session')
    probe.add_argument('--quickapp', type=Path)
    probe.add_argument('--package', default='com.canopus.astrolink.probe')
    commands.add_parser('capture', help='capture screenshot and existing logs without connecting companion')
    canopus = commands.add_parser('canopus')
    canopus.add_argument('operation', choices=['query', 'enable', 'disable', 'scan', 'activate', 'exit-safe-mode'])
    canopus.add_argument('--module', help='module ID for query/enable/disable')
    canopus.add_argument('--safe-mode-action', choices=['keep', 'disableAll', 'rollbackAll'], default='keep')
    return p


def main():
    p = parser()
    args = p.parse_args()
    if args.command == 'probe' and args.probe_log_source == 'ram' and args.ram_log_base is None:
        # The native backend is pinned to Q66 .155; use its observed live ring.
        args.ram_log_base = 0x20100000
        args.ram_log_bytes = 65536
    if args.timeout <= 0 or not 0 <= args.settle <= 60 or args.log_bytes < 1:
        p.error('timeout and log-bytes must be positive; settle must be between 0 and 60')
    if not 1 <= args.ram_log_bytes <= 262144 or (args.ram_log_base is not None and args.ram_log_base < 0):
        p.error('RAM log bytes must be 1..262144 and base must be nonnegative')
    if args.command == 'install' and not (args.module or args.quickapp):
        p.error('install requires --module and/or --quickapp')
    if args.command == 'canopus' and args.operation in ('enable', 'disable', 'activate') and not args.module:
        p.error('canopus enable/disable requires --module')
    output = args.output.expanduser().resolve() if args.output else Path(tempfile.mkdtemp(prefix='astrolink-smoke-'))
    output.mkdir(parents=True, exist_ok=True)
    device_dir, record = find_device(args.library.expanduser().resolve(), args.device)
    runtime = json.loads((device_dir / 'runtime.json').read_text())
    key = record['binding']['authKey']
    redactor = Redactor([key])
    summary = {'command': args.command, 'device': record['name'], 'firmware': record['firmwareID'], 'ok': False,
               'artifacts': [], 'evidence': str(output)}
    bridge = None
    settled = False
    events = (output / 'events.jsonl').open('w')
    try:
        if args.command != 'capture':
            bridge = Bridge(args.bridge.expanduser().resolve(strict=True), redactor, events, args.timeout)
            bridge.call('connect', {'host': '127.0.0.1', 'port': runtime['bluetoothPort'], 'authKey': key,
                                   'stateDir': str(device_dir / 'AstroBox'), 'timeoutSecs': max(1, int(args.timeout - 2)),
                                   'bandAddr': record['identity']['bluetoothAddress']})
        if args.command == 'status':
            for method in ('deviceInfo', 'canopusDeviceInfo', 'canopusModules', 'quickApps'):
                summary[method] = bridge.call(method)
        elif args.command == 'install':
            # Validate all requested files before making an installation request.
            for kind, file in (('canopus', args.module), ('quickapp', args.quickapp)):
                if file:
                    metadata = artifact(file)
                    if kind == 'quickapp':
                        with zipfile.ZipFile(metadata['path']) as package:
                            metadata['package'] = json.loads(package.read('manifest.json'))['package']
                    summary['artifacts'].append({'kind': kind, **metadata})
            for entry in summary['artifacts']:
                entry['result'] = bridge.call('install', {'path': entry['path'], 'kind': entry['kind']})
            if args.module:
                summary['canopusModules'] = bridge.call('canopusModules')
            if args.quickapp:
                summary['quickApps'] = bridge.call('quickApps')
        elif args.command == 'probe':
            summary['module'] = bridge.call('canopusQueryModule', {'module': 'astrolink'})
            if summary['module'].get('state') not in (9, 13):
                raise RuntimeError(f'Astrolink is not active: {summary["module"]}')
            if args.quickapp:
                metadata = artifact(args.quickapp)
                with zipfile.ZipFile(metadata['path']) as package:
                    metadata['package'] = json.loads(package.read('manifest.json'))['package']
                summary['artifacts'].append({'kind': 'quickapp', **metadata})
                summary['install'] = bridge.call('install', {'path': metadata['path'], 'kind': 'quickapp'})
            summary['launch'] = bridge.call('launchQuickApp', {'packageName': args.package, 'page': ''})
            if args.settle:
                time.sleep(args.settle)
            settled = True
        elif args.command == 'launch':
            summary['launch'] = bridge.call('launchQuickApp', {'packageName': args.package, 'page': args.page})
            # Xiaomi launch only acknowledges enqueue. Keep the real link alive
            # while the companion sends its queued packet before disconnecting.
            if args.settle:
                time.sleep(args.settle)
            settled = True
        elif args.command == 'remove':
            summary['remove'] = bridge.call('removeQuickApp', {'packageName': args.package})
        elif args.command == 'canopus':
            if args.operation == 'activate':
                summary['operation'] = bridge.call('canopusActivate', {'module': args.module})
                summary['note'] = 'Official CPC2 ACTIVATE through the real Supervisor; no UI or transport substitution.'
            elif args.operation in ('enable', 'disable'):
                summary['operation'] = bridge.call('canopusSetEnabled', {'module': args.module, 'enabled': args.operation == 'enable'})
                summary['note'] = 'ENABLE/DISABLE record next-boot intent; this is not immediate ACTIVATE.'
            elif args.operation == 'scan':
                summary['operation'] = bridge.call('canopusScan')
            elif args.operation == 'exit-safe-mode':
                summary['operation'] = bridge.call('canopusExitSafeMode', {'action': args.safe_mode_action})
                summary['device'] = bridge.call('canopusDeviceInfo')
            summary['modules'] = bridge.call('canopusQueryModule', {'module': args.module}) if args.module else bridge.call('canopusModules')
        summary['ok'] = True
    except Exception as error:
        summary['error'] = redactor.text(str(error))
        print(summary['error'], file=sys.stderr)
    finally:
        if bridge:
            try:
                bridge.call('disconnect')
            except Exception as error:
                summary['disconnectError'] = redactor.text(str(error))
        # Keep the ctypes callback alive until process exit: native workers can still emit logs.
        if args.settle and not settled:
            time.sleep(args.settle)
        env = dict(os.environ, MIRAGE_LIBRARY=str(args.library.expanduser().resolve()))
        try:
            screenshot = output / 'screen.png'
            result = subprocess.run([str(args.miragectl.expanduser().resolve()), 'screenshot', record['name'], str(screenshot)],
                                    env=env, capture_output=True, text=True, timeout=15)
            summary['screenshot'] = str(screenshot) if result.returncode == 0 else {'error': redactor.text(result.stderr or result.stdout)}
        except Exception as error:
            summary['screenshot'] = {'error': redactor.text(str(error))}
        try:
            summary['guestLogs'] = capture_logs(device_dir, output, redactor, args.log_bytes)
        except Exception as error:
            summary['guestLogError'] = redactor.text(str(error))
        if args.ram_log_base is not None:
            try:
                summary['ramLog'] = capture_ramlog(runtime, output, redactor, args.ram_log_base, args.ram_log_bytes)
                if args.command == 'probe' and args.probe_log_source == 'ram':
                    summary['probe'] = probe_result(Path(summary['ramLog']['output']))
                    if not summary['probe']['passed']:
                        summary['ok'] = False
                        summary['error'] = 'Native probe failed or no terminal completion event was captured'
            except Exception as error:
                summary['ramLogError'] = redactor.text(str(error))
                if args.command == 'probe' and args.probe_log_source == 'ram':
                    summary['ok'] = False
                    summary['error'] = 'Native probe result could not be captured'
        if args.command == 'probe' and args.probe_log_source == 'nsh':
            try:
                summary['nshLog'] = capture_nsh_log(
                    record['name'], output, redactor, args.miragectl, args.library, args.log_bytes)
                summary['probe'] = probe_result(Path(summary['nshLog']['output']))
                if not summary['probe']['passed']:
                    summary['ok'] = False
                    summary['error'] = 'Native probe failed or no terminal completion event was captured'
            except Exception as error:
                summary['ok'] = False
                summary['error'] = 'Native probe result could not be captured'
                summary['nshLogError'] = redactor.text(str(error))
        # Preserve file digests in summary for reproducibility; logs conservatively redact long hex strings.
        (output / 'summary.json').write_text(json.dumps(redactor.object(summary), ensure_ascii=False, indent=2) + '\n')
        print(json.dumps({'ok': summary['ok'], 'evidence': str(output), 'screenshot': summary.get('screenshot')}, ensure_ascii=False))
    # Do not close callback log sink early; daemon runtime threads live until interpreter exit.
    if bridge:
        globals()['_live_bridge'] = bridge
    return 0 if summary['ok'] else 1


if __name__ == '__main__':
    try:
        sys.exit(main())
    except Exception as error:
        print(f'mirage-smoke: {type(error).__name__}: setup failed (check paths and selected device)', file=sys.stderr)
        sys.exit(1)
