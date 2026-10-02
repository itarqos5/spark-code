#!/usr/bin/env python3
"""Package native Linux x86-64 binaries and license notices. SPDX-License-Identifier: MIT"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import tarfile
import tempfile

from package_windows import dependency_notices


def validate_elf(path):
    with path.open('rb') as stream:
        header = stream.read(64)
    if len(header) < 64 or header[:6] != b'\x7fELF\x02\x01':
        raise ValueError(f'Not a little-endian 64-bit ELF binary: {path}')
    if struct.unpack_from('<H', header, 18)[0] != 62:
        raise ValueError(f'Not an x86-64 ELF binary: {path}')
    if struct.unpack_from('<H', header, 16)[0] not in (2, 3):
        raise ValueError(f'Not an executable ELF binary: {path}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary-dir', type=Path, default=Path('target/release'))
    parser.add_argument('--output-dir', type=Path, default=Path('dist'))
    parser.add_argument('--cargo-metadata', type=Path, required=True)
    args = parser.parse_args()
    repository = Path(__file__).resolve().parent.parent
    version = re.search(r'^version\s*=\s*"([^"]+)"', (repository / 'Cargo.toml').read_text(), re.MULTILINE).group(1)
    if not re.fullmatch(r'\d+\.\d+\.\d+', version):
        raise ValueError('Release packaging requires a numeric major.minor.patch version')
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    basename = f'spark-code-{version}-linux-x64'
    archive = output / f'{basename}.tar.gz'
    with tempfile.TemporaryDirectory(prefix='.spark-linux-stage-', dir=output) as staging:
        payload = Path(staging) / basename
        payload.mkdir()
        for name in ('spark-code', 'spark-code-desktop'):
            source = args.binary_dir / name
            validate_elf(source)
            shutil.copy2(source, payload / name)
            (payload / name).chmod(0o755)
        for source, name in (
            (repository / 'LICENSE', 'LICENSE'),
            (repository / 'packaging' / 'INSTALL-linux.md', 'INSTALL.md'),
            (repository / 'docs' / 'FEATURES.md', 'FEATURES.md'),
            (repository / 'docs' / f'RELEASE_NOTES_v{version}.md', 'RELEASE_NOTES.md'),
        ):
            shutil.copy2(source, payload / name)
        dependency_notices(args.cargo_metadata, payload)
        manifest = {
            'application': 'spark-code', 'version': version, 'architecture': 'x86_64',
            'target_platform': 'Linux x86-64; built and smoke-tested on Ubuntu 24.04 (glibc 2.39)',
            'runtime_compatibility': 'Requires X11 or XWayland and system libraries described in INSTALL.md; not a universal static Linux binary.',
            'files': {p.relative_to(payload).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(payload.rglob('*')) if p.is_file()},
        }
        (payload / 'package-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
        with tarfile.open(archive, 'w:gz', compresslevel=9) as tar:
            tar.add(payload, arcname=basename)
    sums = output / f'{basename}-SHA256SUMS.txt'
    sums.write_text(f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n', encoding='ascii')
    for path in (archive, sums):
        print(f'Created {path} ({path.stat().st_size:,} bytes)')


if __name__ == '__main__':
    main()
