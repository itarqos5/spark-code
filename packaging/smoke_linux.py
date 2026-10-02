#!/usr/bin/env python3
"""No-account launch smoke test; run under xvfb-run. SPDX-License-Identifier: MIT"""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary-dir', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=Path('dist/linux-smoke.json'))
    args = parser.parse_args()
    binaries = args.binary_dir.resolve()
    report = {
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
            runtime = Path(scratch) / 'runtime'
            runtime.mkdir(mode=0o700)
            env['XDG_RUNTIME_DIR'] = str(runtime)
            result = subprocess.run([str(binaries / 'spark-code'), '--help'], cwd=scratch, env=env, text=True, capture_output=True, timeout=15, check=True)
            if 'spark-code 1.0.0' not in result.stdout:
                raise RuntimeError('CLI help did not identify the expected 1.0.0 release')
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
