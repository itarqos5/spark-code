# SPDX-License-Identifier: MIT
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest

from package_linux import validate_elf
from verify_release import verify


class LinuxPackagingTests(unittest.TestCase):
    def test_accepts_x64_elf_executable(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'binary'
            header = bytearray(64)
            header[:6] = b'\x7fELF\x02\x01'
            struct.pack_into('<HH', header, 16, 3, 62)
            path.write_bytes(header)
            validate_elf(path)

    def test_rejects_wrong_architecture(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'binary'
            header = bytearray(64)
            header[:6] = b'\x7fELF\x02\x01'
            struct.pack_into('<HH', header, 16, 3, 183)
            path.write_bytes(header)
            with self.assertRaisesRegex(ValueError, 'Not an x86-64'):
                validate_elf(path)

    def test_rejects_non_elf(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'binary'
            path.write_bytes(b'not a native binary')
            with self.assertRaises(ValueError):
                validate_elf(path)


class ReleaseVerificationTests(unittest.TestCase):
    def stage(self, root, version='1.0.0', commit='a' * 40):
        for platform, suffixes in [('windows', ['-setup.exe', '-portable.zip']), ('linux', ['.tar.gz'])]:
            names = [f'spark-code-{version}-{platform}-x64{suffix}' for suffix in suffixes]
            for name in names:
                (root / name).write_bytes(b'fixture')
            digest = hashlib.sha256(b'fixture').hexdigest()
            (root / f'spark-code-{version}-{platform}-x64-SHA256SUMS.txt').write_text(''.join(f'{digest}  {name}\n' for name in names))
        (root / 'windows-smoke.json').write_text(json.dumps(dict(cliHelpPassed=True, guiOpened=True, guiClosedCleanly=True)))
        (root / 'linux-smoke.json').write_text(json.dumps(dict(cliHelpPassed=True, guiOpened=True, guiStayedAlive=True, expectedVersion=version)))

        if version == '1.1.0':
            for platform in ('windows', 'linux'):
                (root / f'{platform}-build.json').write_text(json.dumps(dict(platform=platform, version=version, commit=commit, cliVersionPassed=True)))
            states = ['restored'] + ['minimized', 'restored', 'maximized', 'restored'] * 2
            chrome = dict(guiOpened=True, nativeWindowStylesPassed=True, nativeSystemMenuPresent=True, restoredBoundsPassed=True, guiClosedCleanly=True,
                          transitions=[dict(state=state, passed=True) for state in states])
            (root / 'windows-chrome.json').write_text(json.dumps(chrome))
            for binary, subsystem in [('desktop', 2), ('cli', 3)]:
                resource = dict(machine='0x8664', subsystem=subsystem, icon_sizes=[16, 24, 32, 48, 64, 128, 256],
                                version=dict(ProductName='Spark Code', ProductVersion=version, FileVersion=version))
                (root / f'windows-{binary}-resources.json').write_text(json.dumps(resource))

    def test_verifies_1_1_exact_commit_and_native_reports(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.stage(root, '1.1.0')
            verify(root, '1.1.0', 'a' * 40)

    def test_rejects_wrong_release_commit(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.stage(root, '1.1.0')
            with self.assertRaisesRegex(ValueError, 'release commit'):
                verify(root, '1.1.0', 'b' * 40)

    def test_rejects_mismatched_platform_commits(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.stage(root, '1.1.0')
            path = root / 'linux-build.json'
            report = json.loads(path.read_text())
            report['commit'] = 'b' * 40
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError, 'different commits'):
                verify(root, '1.1.0')

    def test_rejects_stale_binary_branding_or_version(self):
        for key, value in [('ProductName', 'spark-code'), ('ProductVersion', '1.0.0'), ('FileVersion', '1.0.0')]:
            with self.subTest(key=key), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                self.stage(root, '1.1.0')
                path = root / 'windows-desktop-resources.json'
                report = json.loads(path.read_text())
                report['version'][key] = value
                path.write_text(json.dumps(report))
                with self.assertRaisesRegex(ValueError, 'branding/version mismatch'):
                    verify(root, '1.1.0')

    def test_rejects_missing_native_chrome_flag(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.stage(root, '1.1.0')
            path = root / 'windows-chrome.json'
            report = json.loads(path.read_text())
            report['nativeWindowStylesPassed'] = False
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError, 'chrome checks'):
                verify(root, '1.1.0')

    def test_rejects_incomplete_native_transition_cycles(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.stage(root, '1.1.0')
            path = root / 'windows-chrome.json'
            report = json.loads(path.read_text())
            report['transitions'].pop()
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError, 'state transitions'):
                verify(root, '1.1.0')

    def test_rejects_linux_smoke_version_mismatch(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.stage(root, '1.1.0')
            path = root / 'linux-smoke.json'
            report = json.loads(path.read_text())
            report['expectedVersion'] = '1.0.0'
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError, 'Linux smoke version'):
                verify(root, '1.1.0')

    def test_rejects_wrong_architecture_or_subsystem(self):
        for key, value in [('machine', '0xaa64'), ('subsystem', 2)]:
            with self.subTest(key=key), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                self.stage(root, '1.1.0')
                path = root / 'windows-cli-resources.json'
                report = json.loads(path.read_text())
                report[key] = value
                path.write_text(json.dumps(report))
                with self.assertRaisesRegex(ValueError, 'architecture/subsystem'):
                    verify(root, '1.1.0')

    def test_rejects_invalid_version(self):
        with self.assertRaisesRegex(ValueError, 'numeric'):
            verify(Path('.'), '../1.1.0')

    def test_verifies_both_platforms(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.stage(root)
            verify(root)

    def test_rejects_corrupted_payload(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.stage(root)
            (root / 'spark-code-1.0.0-linux-x64.tar.gz').write_bytes(b'corrupt')
            with self.assertRaisesRegex(ValueError, 'SHA-256 mismatch'):
                verify(root)

    def test_rejects_missing_checksum(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.stage(root)
            (root / 'spark-code-1.0.0-linux-x64-SHA256SUMS.txt').write_text('')
            with self.assertRaisesRegex(ValueError, 'Missing checksum'):
                verify(root)

    def test_rejects_unsuccessful_gui(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.stage(root)
            (root / 'linux-smoke.json').write_text('{}')
            with self.assertRaisesRegex(ValueError, 'launch checks did not pass'):
                verify(root)


if __name__ == '__main__':
    unittest.main()
