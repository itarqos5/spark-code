#!/usr/bin/env python3
"""Statically check Spark Code PE icon/version resources (no external packages).

Usage: python assets/branding/verify_windows_resources.py path/to/application.exe
SPDX-License-Identifier: MIT
"""
import argparse
import json
from pathlib import Path
import struct


def inspect(path):
    data = path.read_bytes()
    if data[:2] != b"MZ":
        raise ValueError("Not a PE binary")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("Invalid PE signature")
    machine, count = struct.unpack_from("<HH", data, pe + 4)
    optional = pe + 24
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    magic = struct.unpack_from("<H", data, optional)[0]
    if magic not in (0x10B, 0x20B):
        raise ValueError("Unsupported optional header")
    directory = optional + (112 if magic == 0x20B else 96)
    resource_rva, _ = struct.unpack_from("<II", data, directory + 16)
    sections = []
    for index in range(count):
        position = optional + optional_size + 40 * index
        size, rva, raw_size, offset = struct.unpack_from("<IIII", data, position + 8)
        sections.append((rva, max(size, raw_size), offset))

    def resolve(rva):
        for start, size, offset in sections:
            if start <= rva < start + size:
                return offset + rva - start
        raise ValueError(f"Unmapped resource RVA {rva:#x}")

    root = resolve(resource_rva)
    resources = {}

    def walk(relative, ids=()):
        if len(ids) > 3:
            raise ValueError("Unexpected resource-directory depth")
        offset = root + relative
        named, numeric = struct.unpack_from("<HH", data, offset + 12)
        for index in range(named + numeric):
            key, target = struct.unpack_from("<II", data, offset + 16 + 8 * index)
            if target & 0x80000000:
                walk(target & 0x7FFFFFFF, ids + (key,))
            else:
                rva, size = struct.unpack_from("<II", data, root + target)
                start = resolve(rva)
                resources[ids + (key,)] = data[start:start + size]

    walk(0)
    groups = [value for key, value in resources.items() if key[0] == 14]
    icons = {key[1] for key in resources if key[0] == 3}
    if not groups or not icons:
        raise ValueError("Missing icon group or icon images")
    sizes = set()
    for group in groups:
        reserved, kind, entries = struct.unpack_from("<HHH", group)
        if (reserved, kind) != (0, 1):
            raise ValueError("Invalid icon-group header")
        for index in range(entries):
            width, height, _, _, _, _, _, resource_id = struct.unpack_from("<BBBBHHIH", group, 6 + 14 * index)
            if resource_id not in icons:
                raise ValueError("Icon group points to a missing image")
            sizes.add((width or 256, height or 256))
    expected = {(size, size) for size in (16, 24, 32, 48, 64, 128, 256)}
    if not expected.issubset(sizes):
        raise ValueError(f"Icon resolutions missing: {expected - sizes}")

    def version_strings(blob):
        values = {}

        def node(start, limit):
            length, value_length, kind = struct.unpack_from("<HHH", blob, start)
            end = start + length
            if length < 6 or end > limit:
                raise ValueError("Invalid version-resource node")
            pos = start + 6
            key_start = pos
            while blob[pos:pos + 2] != b"\0\0":
                pos += 2
                if pos >= end:
                    raise ValueError("Unterminated version-resource key")
            key = blob[key_start:pos].decode("utf-16le")
            pos = (pos + 2 + 3) & ~3
            value_bytes = value_length * 2 if kind == 1 else value_length
            if kind == 1 and value_length:
                values[key] = blob[pos:pos + value_bytes].decode("utf-16le").rstrip("\0")
            pos = (pos + value_bytes + 3) & ~3
            while pos + 6 <= end:
                child_length = node(pos, end)
                pos = (pos + child_length + 3) & ~3
            return length

        node(0, len(blob))
        return values

    versions = [version_strings(value) for key, value in resources.items() if key[0] == 16]
    if not versions or any(value.get("ProductName") != "Spark Code" for value in versions):
        raise ValueError("ProductName is not exactly Spark Code")
    return {"path": str(path), "machine": hex(machine),
            "subsystem": struct.unpack_from("<H", data, optional + 68)[0],
            "icon_sizes": sorted(width for width, _ in sizes), "version": versions[0]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    args = parser.parse_args()
    print(json.dumps(inspect(args.binary), indent=2))
