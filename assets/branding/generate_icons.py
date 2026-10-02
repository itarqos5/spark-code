#!/usr/bin/env python3
"""Render the checked-in original SVG with Inkscape, then encode PNG/ICO.

Requirements: Inkscape and Pillow. Both are asset-authoring dependencies only;
normal Cargo and packaging builds use the checked-in generated files.
SPDX-License-Identifier: MIT
"""
from pathlib import Path
import shutil
import subprocess
import tempfile

from PIL import Image

HERE = Path(__file__).resolve().parent
SIZES = (16, 24, 32, 48, 64, 128, 256)


def main():
    inkscape = shutil.which("inkscape")
    if not inkscape:
        raise SystemExit("Inkscape is required to render the SVG source")
    with tempfile.TemporaryDirectory(prefix="spark-icons-") as temporary:
        rendered = Path(temporary) / "source.png"
        subprocess.run([
            inkscape, str(HERE / "spark-code.svg"), "--export-type=png",
            f"--export-filename={rendered}", "--export-width=1024",
            "--export-height=1024",
        ], check=True)
        with Image.open(rendered) as image:
            source = image.convert("RGBA")
            frames = [source.resize((size, size), Image.Resampling.LANCZOS) for size in SIZES]
        for size, frame in zip(SIZES, frames):
            frame.save(HERE / f"spark-code-{size}.png", optimize=True)
        frames[-1].save(HERE / "spark-code.png", optimize=True)
        frames[-1].save(HERE / "spark-code.ico", format="ICO",
                        sizes=[(size, size) for size in SIZES], append_images=frames[:-1])
    with Image.open(HERE / "spark-code.ico") as icon:
        assert icon.ico.sizes() == {(size, size) for size in SIZES}
    print("Generated Spark Code PNGs and seven-resolution Windows icon")


if __name__ == "__main__":
    main()
