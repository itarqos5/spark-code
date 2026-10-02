# SPDX-License-Identifier: MIT
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("package_windows", Path(__file__).with_name("package_windows.py"))
packaging = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packaging)


def make_pe(import_name=b"KERNEL32.dll", machine=0x8664):
    data = bytearray(1024)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HH", data, 0x84, machine, 1)
    struct.pack_into("<H", data, 0x94, 240)
    optional = 0x98
    struct.pack_into("<H", data, optional, 0x20B)
    struct.pack_into("<I", data, optional + 108, 16)
    struct.pack_into("<II", data, optional + 120, 0x1000, 40)
    struct.pack_into("<IIII", data, optional + 240 + 8, 512, 0x1000, 512, 512)
    struct.pack_into("<IIIII", data, 512, 0, 0, 0, 0x1040, 0)
    data[576:576 + len(import_name) + 1] = import_name + b"\0"
    return data


class PackagingTests(unittest.TestCase):
    def test_reads_x64_imports(self):
        with tempfile.TemporaryDirectory() as temp:
            binary = Path(temp) / "app.exe"
            binary.write_bytes(make_pe())
            self.assertEqual(packaging.pe_imports(binary), ["kernel32.dll"])

    def test_rejects_32_bit_binary(self):
        with tempfile.TemporaryDirectory() as temp:
            binary = Path(temp) / "app.exe"
            binary.write_bytes(make_pe(machine=0x14C))
            with self.assertRaisesRegex(ValueError, "Not an AMD64"):
                packaging.pe_imports(binary)

    def test_rejects_invalid_binary(self):
        with tempfile.TemporaryDirectory() as temp:
            binary = Path(temp) / "app.exe"
            binary.write_bytes(b"not executable")
            with self.assertRaisesRegex(ValueError, "Not a Windows PE"):
                packaging.pe_imports(binary)

    def test_rejects_path_import(self):
        with tempfile.TemporaryDirectory() as temp:
            binary = Path(temp) / "app.exe"
            binary.write_bytes(make_pe(b"../evil.dll"))
            with self.assertRaisesRegex(ValueError, "Unsafe DLL import"):
                packaging.pe_imports(binary)

    def test_system_allowlist_excludes_external_runtimes(self):
        self.assertTrue(packaging.system_library("combase.dll"))
        self.assertTrue(packaging.system_library("api-ms-win-core-synch-l1-2-0.dll"))
        self.assertFalse(packaging.system_library("libwinpthread-1.dll"))
        self.assertFalse(packaging.system_library("vcruntime140.dll"))

    def test_missing_runtime_fails_closed(self):
        with tempfile.TemporaryDirectory() as temp:
            payload = Path(temp)
            (payload / "app.exe").write_bytes(make_pe(b"external.dll"))
            with self.assertRaisesRegex(ValueError, "Missing runtime DLL external.dll"):
                packaging.copy_runtime_dependencies(payload, [payload])

    def test_resolves_recursive_runtime_imports(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            payload, runtimes = root / "payload", root / "runtime"
            payload.mkdir(); runtimes.mkdir()
            (payload / "app.exe").write_bytes(make_pe(b"first.dll"))
            (runtimes / "first.dll").write_bytes(make_pe(b"second.dll"))
            (runtimes / "second.dll").write_bytes(make_pe(b"kernel32.dll"))
            audit = packaging.copy_runtime_dependencies(payload, [runtimes])
            self.assertEqual(set(audit), {"app.exe", "first.dll", "second.dll"})
            self.assertFalse((payload / "kernel32.dll").exists())

    def test_exact_uninstall_manifest(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            payload = root / "payload"
            (payload / "licenses" / "crate").mkdir(parents=True)
            (payload / "spark-code.exe").write_bytes(b"test")
            (payload / "licenses" / "crate" / "LICENSE").write_text("license")
            destination = root / "payload.nsh"
            packaging.write_payload_include(payload, destination)
            script = destination.read_text()
            self.assertIn('Delete "$INSTDIR\\spark-code.exe"', script)
            self.assertIn('Delete "$INSTDIR\\licenses\\crate\\LICENSE"', script)
            self.assertIn('RMDir "$INSTDIR\\licenses\\crate"', script)
            self.assertNotIn("RMDir /r", script)
            self.assertNotIn("Delete /", script)
            self.assertNotIn("*", script)

    def test_nsis_escapes_literal_dollar(self):
        self.assertEqual(packaging.nsis_string("C:/User/$name"), "C:/User/$$name")


if __name__ == "__main__":
    unittest.main()
