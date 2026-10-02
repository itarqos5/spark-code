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
    def stage(self, root):
        for platform, suffixes in [('windows', ['-setup.exe', '-portable.zip']), ('linux', ['.tar.gz'])]:
            names = [f'spark-code-1.0.0-{platform}-x64{suffix}' for suffix in suffixes]
            for name in names:
                (root / name).write_bytes(b'fixture')
            digest = hashlib.sha256(b'fixture').hexdigest()
            (root / f'spark-code-1.0.0-{platform}-x64-SHA256SUMS.txt').write_text(''.join(f'{digest}  {name}\n' for name in names))
        (root / 'windows-smoke.json').write_text(json.dumps(dict(cliHelpPassed=True, guiOpened=True, guiClosedCleanly=True)))
        (root / 'linux-smoke.json').write_text(json.dumps(dict(cliHelpPassed=True, guiOpened=True, guiStayedAlive=True)))

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
