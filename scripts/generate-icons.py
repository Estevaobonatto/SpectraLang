#!/usr/bin/env python3
"""Generate derivative icon assets from the canonical SVG in assets/icon.svg.

Outputs:
  - assets/icon.png                         (512x512, reference PNG)
  - tools/vscode-extension/assets/spectra-icon.png       (128x128)
  - tools/vscode-extension/assets/spectra-file-light.png (24x24)
  - tools/vscode-extension/assets/spectra-file-dark.png  (24x24)
  - installer/spectra-icon.ico                           (multi-res ICO)
  - tools/spectra-cli/assets/icon.ico                    (multi-res ICO)
"""

from pathlib import Path
import io
import struct

import cairosvg
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SVG = ROOT / "assets" / "icon.svg"

ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]


def render_svg(size: int) -> Image.Image:
    png_bytes = cairosvg.svg2png(url=str(SVG), output_width=size, output_height=size)
    return Image.open(io.BytesIO(png_bytes))


def save_ico(path: Path, sizes: list[int]) -> None:
    """Write a Windows ICO file embedding PNG data for each requested size."""
    path.parent.mkdir(parents=True, exist_ok=True)
    pngs = [
        cairosvg.svg2png(url=str(SVG), output_width=size, output_height=size)
        for size in sizes
    ]

    count = len(pngs)
    header = struct.pack("<HHH", 0, 1, count)  # Reserved, Type=1 (icon), Count
    entries = b""
    data = b""
    offset = 6 + 16 * count

    for size, png in zip(sizes, pngs):
        # ICO directory stores 0 for dimensions >= 256.
        dim = size if size < 256 else 0
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(png), offset)
        data += png
        offset += len(png)

    path.write_bytes(header + entries + data)


def save_png(path: Path, size: int) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    img = render_svg(size)
    img.save(path, format="PNG")


def main() -> None:
    save_png(ROOT / "assets" / "icon.png", 512)
    save_png(ROOT / "tools" / "vscode-extension" / "assets" / "spectra-icon.png", 128)
    save_png(ROOT / "tools" / "vscode-extension" / "assets" / "spectra-file-light.png", 24)
    save_png(ROOT / "tools" / "vscode-extension" / "assets" / "spectra-file-dark.png", 24)
    save_ico(ROOT / "installer" / "spectra-icon.ico", ICO_SIZES)
    save_ico(ROOT / "tools" / "spectra-cli" / "assets" / "icon.ico", ICO_SIZES)

    print("Icons generated from", SVG)
    for path in [
        ROOT / "assets" / "icon.png",
        ROOT / "tools" / "vscode-extension" / "assets" / "spectra-icon.png",
        ROOT / "tools" / "vscode-extension" / "assets" / "spectra-file-light.png",
        ROOT / "tools" / "vscode-extension" / "assets" / "spectra-file-dark.png",
        ROOT / "installer" / "spectra-icon.ico",
        ROOT / "tools" / "spectra-cli" / "assets" / "icon.ico",
    ]:
        print("  -", path.relative_to(ROOT))


if __name__ == "__main__":
    main()
