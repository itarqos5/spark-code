#!/usr/bin/env python3
"""Fail closed on corrupt release payloads and failed native checks. SPDX-License-Identifier: MIT"""
import argparse
import hashlib
import json
from pathlib import Path
import re


def read_report(root, filename):
    report = json.loads((root / filename).read_text(encoding='utf-8-sig'))
    if not isinstance(report, dict) or report.get('error'):
        raise ValueError(f'Invalid or failed report: {filename}')
    return report


def verify(root, version='1.0.0', expected_commit=None):
    # The default preserves validation for the immutable 1.0.0 release script.
    if not re.fullmatch(r'\d+\.\d+\.\d+', version):
        raise ValueError('Expected a numeric major.minor.patch version')
    if expected_commit is not None and not re.fullmatch(r'[0-9a-f]{40}', expected_commit):
        raise ValueError('Expected a full lowercase Git commit SHA')
    groups = {
        f'spark-code-{version}-windows-x64-SHA256SUMS.txt': {f'spark-code-{version}-windows-x64-setup.exe', f'spark-code-{version}-windows-x64-portable.zip'},
        f'spark-code-{version}-linux-x64-SHA256SUMS.txt': {f'spark-code-{version}-linux-x64.tar.gz'},
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
            payload = root / name
            if not payload.is_file() or payload.stat().st_size == 0:
                raise ValueError(f'Missing or empty asset: {name}')
            if hashlib.sha256(payload.read_bytes()).hexdigest() != digest:
                raise ValueError(f'SHA-256 mismatch: {name}')
        if seen != required:
            raise ValueError(f'Missing checksum in {manifest}')
    for filename, flags in {
        'windows-smoke.json': ('cliHelpPassed', 'guiOpened', 'guiClosedCleanly'),
        'linux-smoke.json': ('cliHelpPassed', 'guiOpened', 'guiStayedAlive'),
    }.items():
        report = read_report(root, filename)
        if not all(report.get(flag) is True for flag in flags):
            raise ValueError(f'Platform launch checks did not pass: {filename}')
    if tuple(map(int, version.split('.'))) >= (1, 1, 0):
        commits = set()
        for platform in ('linux', 'windows'):
            filename = f'{platform}-build.json'
            report = read_report(root, filename)
            commit = report.get('commit', '')
            if report.get('version') != version or report.get('platform') != platform or not re.fullmatch(r'[0-9a-f]{40}', commit):
                raise ValueError(f'Build version/platform/commit mismatch: {filename}')
            if report.get('cliVersionPassed') is not True:
                raise ValueError(f'Exact CLI version was not verified: {filename}')
            if expected_commit is not None and commit != expected_commit:
                raise ValueError(f'Build commit does not match release commit: {filename}')
            commits.add(commit)
        if len(commits) != 1:
            raise ValueError('Platform builds came from different commits')
        linux = read_report(root, 'linux-smoke.json')
        if linux.get('expectedVersion') != version:
            raise ValueError('Linux smoke version does not match release')
        chrome = read_report(root, 'windows-chrome.json')
        flags = ('guiOpened', 'nativeWindowStylesPassed', 'nativeSystemMenuPresent', 'restoredBoundsPassed', 'guiClosedCleanly')
        if not all(chrome.get(flag) is True for flag in flags):
            raise ValueError('Native Windows chrome checks did not pass')
        states = ['restored'] + ['minimized', 'restored', 'maximized', 'restored'] * 2
        transitions = chrome.get('transitions', [])
        if [event.get('state') for event in transitions] != states or not all(event.get('passed') is True for event in transitions):
            raise ValueError('Native Windows state transitions did not pass')
        for binary, subsystem in [('desktop', 2), ('cli', 3)]:
            report = read_report(root, f'windows-{binary}-resources.json')
            strings = report.get('version', {})
            if report.get('machine') != '0x8664' or report.get('subsystem') != subsystem:
                raise ValueError(f'Unexpected Windows {binary} architecture/subsystem')
            if strings.get('ProductName') != 'Spark Code' or any(strings.get(key) != version for key in ('ProductVersion', 'FileVersion')):
                raise ValueError(f'Windows {binary} branding/version mismatch')
            if not {16, 24, 32, 48, 64, 128, 256}.issubset(set(report.get('icon_sizes', []))):
                raise ValueError(f'Windows {binary} icon sizes missing')
    print(f'Both platform checksums and native reports verified for {version}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('--version', default='1.0.0')
    parser.add_argument('--expected-commit')
    args = parser.parse_args()
    verify(args.root, args.version, args.expected_commit)
