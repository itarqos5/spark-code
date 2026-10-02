#!/usr/bin/env python3
"""No-account launch smoke test; run under xvfb-run. SPDX-License-Identifier: MIT"""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import re
import struct
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary-dir', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=Path('dist/linux-smoke.json'))
    parser.add_argument('--expected-version', default='1.1.0')
    parser.add_argument('--capture', type=Path, help='Optional native PNG screenshot destination')
    args = parser.parse_args()
    if not re.fullmatch(r'\d+\.\d+\.\d+', args.expected_version):
        parser.error('--expected-version must be numeric major.minor.patch')
    capture = args.capture or (Path(os.environ['SPARK_CODE_CAPTURE']) if os.environ.get('SPARK_CODE_CAPTURE') else None)
    if capture is not None:
        capture = capture.resolve()
        capture.parent.mkdir(parents=True, exist_ok=True)
        if capture.exists():
            parser.error('Capture destination already exists; choose a fresh path')
    binaries = args.binary_dir.resolve()
    report = {
        'expectedVersion': args.expected_version,
        'os': platform.platform(), 'measuredAtUtc': datetime.now(timezone.utc).isoformat(),
        'scenario': 'Packaged native GUI launch under Xvfb/X11 and CLI help; no provider or account used',
        'cliHelpPassed': False, 'guiOpened': False, 'guiStayedAlive': False,
        'shutdown': 'Smoke harness terminates only its own GUI process; interactive close is not measured',
    }
    process = None
    try:
        with tempfile.TemporaryDirectory(prefix='spark-linux-smoke-') as scratch:
            env = dict(os.environ, SPARK_CODE_DATA_DIR=scratch, WINIT_UNIX_BACKEND='x11')
            env.pop('WAYLAND_DISPLAY', None)
            if capture is not None:
                env['SPARK_CODE_CAPTURE'] = str(capture)
            runtime = Path(scratch) / 'runtime'
            runtime.mkdir(mode=0o700)
            env['XDG_RUNTIME_DIR'] = str(runtime)
            result = subprocess.run([str(binaries / 'spark-code'), '--help'], cwd=scratch, env=env, text=True, capture_output=True, timeout=15, check=True)
            if result.stdout.splitlines()[0:1] != [f'spark-code {args.expected_version}']:
                raise RuntimeError(f'CLI help did not identify the expected {args.expected_version} release')
            report['cliHelpPassed'] = True
            with (Path(scratch) / 'gui.log').open('w+') as log:
                process = subprocess.Popen([str(binaries / 'spark-code-desktop')], cwd=scratch, env=env, stdout=log, stderr=log)
                try:
                    deadline = time.monotonic() + 15
                    while time.monotonic() < deadline:
                        if process.poll() is not None:
                            raise RuntimeError(f'GUI exited early with status {process.returncode}')
                        windows = subprocess.run(['xdotool', 'search', '--onlyvisible', '--pid', str(process.pid)], env=env, text=True, capture_output=True, timeout=3)
                        if windows.returncode == 0 and windows.stdout.strip():
                            report['guiOpened'] = True
                            break
                        time.sleep(0.25)
                    if not report['guiOpened']:
                        raise RuntimeError('GUI process ran but no visible X11 window appeared')
                    time.sleep(3)
                    if process.poll() is not None:
                        raise RuntimeError(f'GUI exited after opening: {process.returncode}')
                    report['guiStayedAlive'] = True
                    if capture is not None:
                        deadline = time.monotonic() + 10
                        while not capture.is_file() and time.monotonic() < deadline:
                            if process.poll() is not None:
                                raise RuntimeError('GUI exited before native capture')
                            time.sleep(0.1)
                        image = capture.read_bytes()
                        if len(image) < 24 or image[:8] != b'\x89PNG\r\n\x1a\n' or image[12:16] != b'IHDR':
                            raise RuntimeError('Native capture was not a PNG')
                        width, height = struct.unpack('>II', image[16:24])
                        if width < 640 or height < 400:
                            raise RuntimeError('Native capture was unexpectedly small')
                        report['nativeCapture'] = {'filename': capture.name, 'width': width, 'height': height}
                    status = Path(f'/proc/{process.pid}/status').read_text()
                    report['processMemoryStatus'] = [line for line in status.splitlines() if line.startswith(('VmRSS:', 'VmSize:'))]
                finally:
                    if process.poll() is None:
                        process.terminate()
                        try:
                            process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait(timeout=5)
                    log.seek(0)
                    report['guiLog'] = log.read()[-16000:]
    except Exception as error:
        report['error'] = str(error)
        raise
    finally:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print('Packaged CLI help and visible native Linux GUI launch passed.')


if __name__ == '__main__':
    main()
