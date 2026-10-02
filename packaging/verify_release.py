#!/usr/bin/env python3
"""Fail closed on missing, unexpected, or corrupt release payloads. SPDX-License-Identifier: MIT"""
import hashlib
import json
from pathlib import Path
import re
import sys


def verify(root):
    groups = {
        'spark-code-1.0.0-windows-x64-SHA256SUMS.txt': {'spark-code-1.0.0-windows-x64-setup.exe', 'spark-code-1.0.0-windows-x64-portable.zip'},
        'spark-code-1.0.0-linux-x64-SHA256SUMS.txt': {'spark-code-1.0.0-linux-x64.tar.gz'},
    }
    for manifest, required in groups.items():
        seen = set()
        for line in (root / manifest).read_text(encoding='ascii').splitlines():
            match = re.fullmatch(r'([0-9a-f]{64})  ([A-Za-z0-9._-]+)', line)
            if not match:
                raise ValueError(f'Malformed checksum in {manifest}')
            digest, name = match.groups()
            if name not in required or name in seen:
                raise ValueError(f'Unexpected or duplicate asset in {manifest}: {name}')
            seen.add(name)
            if hashlib.sha256((root / name).read_bytes()).hexdigest() != digest:
                raise ValueError(f'SHA-256 mismatch: {name}')
        if seen != required:
            raise ValueError(f'Missing checksum in {manifest}')
    for filename, flags in {
        'windows-smoke.json': ('cliHelpPassed', 'guiOpened', 'guiClosedCleanly'),
        'linux-smoke.json': ('cliHelpPassed', 'guiOpened', 'guiStayedAlive'),
    }.items():
        report = json.loads((root / filename).read_text(encoding='utf-8-sig'))
        if report.get('error') or not all(report.get(flag) is True for flag in flags):
            raise ValueError(f'Platform launch checks did not pass: {filename}')
    print('Both platform checksums and launch reports verified.')


if __name__ == '__main__':
    verify(Path(sys.argv[1]))
