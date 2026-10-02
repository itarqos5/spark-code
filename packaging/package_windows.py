#!/usr/bin/env python3
"""Stage validated x64 PE binaries, resolve non-system DLLs, create NSIS + ZIP.

Uses only Python's standard library. Windows system libraries are never copied.
SPDX-License-Identifier: MIT
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tempfile
import zipfile

# Libraries included with the intended Windows 10 x64 platform. Unknown imports
# fail closed until explicitly supplied through --dll-dir (or this list reviewed).
SYSTEM_DLLS = frozenset("""
advapi32 avrt bcrypt bcryptprimitives cabinet cfgmgr32 clbcatq combase comctl32 comdlg32
crypt32 cryptbase cryptnet cryptsp d2d1 d3d11 d3d12 d3dcompiler_47 dcomp dbghelp
dinput8 dnsapi dsound dwmapi dwrite dxcore dxgi dxva2 gdi32 gdi32full hid imagehlp
imm32 iphlpapi kernel32 kernelbase mf mfplat mfreadwrite mfuuid msimg32 msvcrt
mswsock ncrypt netapi32 normaliz nsi ntdll ole32 oleacc oleaut32 opengl32 powrprof
profapi propsys psapi rpcrt4 runtimeobject secur32 setupapi shcore shell32 shlwapi
sspicli ucrtbase urlmon user32 userenv usp10 uxtheme version wevtapi windowscodecs
winhttp wininet winmm winscard winspool wintrust wldap32 ws2_32 wtsapi32
""".split())


def pe_imports(path):
    """Read standard and delay-load PE import tables without running the binary."""
    data = path.read_bytes()
    if data[:2] != b"MZ":
        raise ValueError(f"Not a Windows PE binary: {path}")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError(f"Invalid PE signature: {path}")
    machine, count = struct.unpack_from("<HH", data, pe + 4)
    if machine != 0x8664:
        raise ValueError(f"Not an AMD64 binary: {path} (machine {machine:#x})")
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    optional = pe + 24
    if struct.unpack_from("<H", data, optional)[0] != 0x20B:
        raise ValueError(f"Not a PE32+ binary: {path}")
    image_base = struct.unpack_from("<Q", data, optional + 24)[0]
    section_table = optional + optional_size
    sections = []
    for i in range(count):
        offset = section_table + 40 * i
        virtual_size, virtual_address, raw_size, raw_offset = struct.unpack_from("<IIII", data, offset + 8)
        sections.append((virtual_address, max(virtual_size, raw_size), raw_offset))

    def rva_offset(rva):
        for address, size, raw in sections:
            if address <= rva < address + size:
                result = raw + rva - address
                if result >= len(data):
                    break
                return result
        raise ValueError(f"Unmapped PE RVA {rva:#x} in {path}")

    def dll_name(rva):
        start = rva_offset(rva)
        stop = data.find(b"\0", start, start + 260)
        if stop < 0:
            raise ValueError(f"Malformed DLL name in {path}")
        name = data[start:stop].decode("ascii").lower()
        if not re.fullmatch(r"[a-z0-9_.+\-]+\.dll", name):
            raise ValueError(f"Unsafe DLL import {name!r} in {path}")
        return name

    imports = set()
    directory_count = struct.unpack_from("<I", data, optional + 108)[0]
    if directory_count > 1:
        rva, size = struct.unpack_from("<II", data, optional + 112 + 8)
        if rva:
            offset = rva_offset(rva)
            for pos in range(offset, offset + size, 20):
                fields = struct.unpack_from("<IIIII", data, pos)
                if not any(fields):
                    break
                imports.add(dll_name(fields[3]))
    if directory_count > 13:
        rva, size = struct.unpack_from("<II", data, optional + 112 + 13 * 8)
        if rva:
            offset = rva_offset(rva)
            for pos in range(offset, offset + size, 32):
                fields = struct.unpack_from("<IIIIIIII", data, pos)
                if not any(fields):
                    break
                imports.add(dll_name(fields[1] if fields[0] & 1 else fields[1] - image_base))
    return sorted(imports)


def system_library(name):
    return name.startswith(("api-ms-win-", "ext-ms-win-")) or name[:-4] in SYSTEM_DLLS


def copy_runtime_dependencies(payload, search_dirs):
    available = {}
    for directory in search_dirs:
        if not directory.is_dir():
            raise ValueError(f"DLL search directory does not exist: {directory}")
        for path in directory.iterdir():
            if path.is_file() and path.suffix.lower() == ".dll":
                available.setdefault(path.name.lower(), path)
    pending = list(payload.glob("*.exe"))
    audited = {}
    while pending:
        binary = pending.pop(0)
        if binary.name in audited:
            continue
        imports = pe_imports(binary)
        audited[binary.name] = imports
        for name in imports:
            if system_library(name):
                continue
            destination = payload / name
            if not destination.exists():
                if name not in available:
                    raise ValueError(f"Missing runtime DLL {name}, imported by {binary.name}. Supply its toolchain directory with --dll-dir; do not copy Windows system DLLs.")
                shutil.copy2(available[name], destination)
                pending.append(destination)
    return audited


def nsis_string(value):
    return str(value).replace("$", "$$").replace('"', '$\\"')


def write_payload_include(payload, destination):
    files = sorted(p for p in payload.rglob("*") if p.is_file())
    directories = sorted({p.parent.relative_to(payload) for p in files}, key=lambda p: (len(p.parts), str(p)))
    lines = ["; Generated exact-file manifest. Never recursively remove the install folder.", "!macro InstallPayload"]
    for directory in directories:
        suffix = "" if str(directory) == "." else "\\" + str(directory).replace("/", "\\")
        lines.append(f'  SetOutPath "$INSTDIR{nsis_string(suffix)}"')
        for path in files:
            if path.parent.relative_to(payload) == directory:
                lines.append(f'  File "{nsis_string(path)}"')
    lines += ['  SetOutPath "$INSTDIR"', "!macroend", "!macro UninstallPayload"]
    for path in files:
        relative = str(path.relative_to(payload)).replace("/", "\\")
        lines.append(f'  Delete "$INSTDIR\\{nsis_string(relative)}"')
    for directory in reversed(directories):
        if str(directory) != ".":
            relative = str(directory).replace("/", "\\")
            lines.append(f'  RMDir "$INSTDIR\\{nsis_string(relative)}"')
    lines.append("!macroend")
    destination.write_text("\n".join(lines) + "\n", encoding="utf-8")


def dependency_notices(metadata_file, payload):
    """Preserve published crate license files and record dependency attribution."""
    metadata = json.loads(metadata_file.read_text(encoding="utf-8"))
    used = {node["id"] for node in metadata["resolve"]["nodes"]}
    notices = ["# Rust dependency notices", "", "Cargo.lock pins these dependency versions. This inventory may include build/test dependencies.", "License files below are copied from the downloaded crate sources, when provided.", ""]
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if package["id"] not in used or package["name"] == "spark-code":
            continue
        label = f'{package["name"]}-{package["version"]}'
        notices += [f'## {label}', f'License: {package.get("license") or "see bundled license file"}', f'Source: {package.get("repository") or "https://crates.io/crates/" + package["name"]}', ""]
        source = Path(package["manifest_path"]).parent
        candidates = [p for p in source.iterdir() if p.is_file() and p.name.lower().startswith(("license", "licence", "copying", "copyright", "notice"))]
        if package.get("license_file"):
            explicit = source / package["license_file"]
            if explicit.is_file():
                candidates.append(explicit)
        for path in set(candidates):
            destination = payload / "licenses" / label / path.name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, destination)
    (payload / "THIRD-PARTY-NOTICES.md").write_text("\n".join(notices), encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary-dir", required=True, type=Path)
    parser.add_argument("--output-dir", type=Path, default=Path("dist"))
    parser.add_argument("--version")
    parser.add_argument("--dll-dir", action="append", type=Path, default=[])
    parser.add_argument("--runtime-notice", action="append", type=Path, default=[], help="License/copyright notice for supplied runtime DLLs")
    parser.add_argument("--cargo-metadata", required=True, type=Path)
    parser.add_argument("--makensis", default="makensis")
    args = parser.parse_args()
    repository = Path(__file__).resolve().parent.parent
    version = args.version or re.search(r'^version\s*=\s*"([^"]+)"', (repository / "Cargo.toml").read_text(), re.MULTILINE).group(1)
    match = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)(?:-[a-zA-Z0-9.-]+)?", version)
    if not match or any(int(v) > 65535 for v in match.groups()):
        raise ValueError("Version must be a safe semantic version with numeric components <= 65535")
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    basename = f"spark-code-{version}-windows-x64"
    installer = output / f"{basename}-setup.exe"
    archive = output / f"{basename}-portable.zip"
    binary_dir = args.binary_dir.resolve()
    with tempfile.TemporaryDirectory(prefix=".spark-stage-", dir=output) as staging:
        stage = Path(staging)
        payload = stage / basename
        payload.mkdir()
        for name in ("spark-code.exe", "spark-code-desktop.exe"):
            source = binary_dir / name
            if not source.is_file():
                raise ValueError(f"Required release binary missing: {source}")
            shutil.copy2(source, payload / name)
        audited = copy_runtime_dependencies(payload, [binary_dir] + [p.resolve() for p in args.dll_dir])
        runtime_dlls = list(payload.glob("*.dll"))
        if runtime_dlls and not args.runtime_notice:
            raise ValueError("Bundled runtime DLLs require their license/copyright notice via --runtime-notice")
        for index, notice in enumerate(args.runtime_notice):
            shutil.copy2(notice, payload / f"RUNTIME-NOTICE-{index + 1}-{notice.name}")
        shutil.copy2(repository / "LICENSE", payload / "LICENSE")
        shutil.copy2(repository / "docs" / "INSTALL.md", payload / "INSTALL.md")
        shutil.copy2(repository / "docs" / "FEATURES.md", payload / "FEATURES.md")
        release_notes = repository / "docs" / f"RELEASE_NOTES_v{version}.md"
        if release_notes.is_file():
            shutil.copy2(release_notes, payload / "RELEASE_NOTES.md")
        shutil.copy2(repository / "packaging" / "manage-user-path.ps1", payload / "manage-user-path.ps1")
        dependency_notices(args.cargo_metadata, payload)
        (payload / "runtime-dependencies.json").write_text(json.dumps(audited, indent=2) + "\n", encoding="utf-8")
        manifest = {"application": "spark-code", "version": version, "architecture": "x86_64", "target_platform": "Windows 10/11 x64", "runtime_compatibility": "Requires actual Windows validation; packaging alone does not establish compatibility.", "files": {str(p.relative_to(payload)).replace("\\", "/"): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(payload.rglob("*")) if p.is_file()}}
        (payload / "package-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        include = stage / "payload.nsh"
        write_payload_include(payload, include)
        flag = "/D" if os.name == "nt" else "-D"
        command = [args.makensis, f"{flag}VERSION={version}", f'{flag}VERSION_QUAD={".".join(match.groups())}.0', f"{flag}OUTPUT_FILE={installer}", f"{flag}PAYLOAD_INCLUDE={include}", str(repository / "packaging" / "installer.nsi")]
        subprocess.run(command, check=True, cwd=repository)
        if not installer.is_file():
            raise RuntimeError("NSIS did not produce the installer")
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=9, strict_timestamps=False) as zipped:
            for path in sorted(payload.rglob("*")):
                if path.is_file():
                    zipped.write(path, path.relative_to(stage).as_posix())
    hashes = output / f"{basename}-SHA256SUMS.txt"
    hashes.write_text("".join(f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n" for p in (installer, archive)), encoding="ascii")
    for path in (installer, archive, hashes):
        print(f"Created {path} ({path.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()
