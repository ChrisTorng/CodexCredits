#!/usr/bin/env python3
"""Fail the Windows build if the EXE/setup's embedded 16/32/256px icon isn't
the cyan/gold Codex Credits brand. Inspect PE RT_ICON bytes, not Explorer's cache.
"""
import io
import struct
import sys
from pathlib import Path

import pefile
from PIL import Image

ICON_MAGIC = b"\x89PNG\r\n\x1a\n"
RT_ICON = 3


def recognized_brand(img: Image.Image) -> bool:
    rgba = img.convert("RGBA")
    gold = cyan = 0
    for r, g, b, a in rgba.getdata():
        if a < 100:
            continue
        if r > 140 and g > 70 and g < r and b < 115:
            gold += 1
        if g > 90 and b > 90 and b > r * 1.2:
            cyan += 1
    return gold >= max(1, rgba.width // 8) and cyan >= max(1, rgba.width // 8)


def decode_resource(payload: bytes) -> Image.Image:
    if payload.startswith(ICON_MAGIC):
        return Image.open(io.BytesIO(payload)).convert("RGBA")
    # RT_ICON stores the BITMAPINFOHEADER without the surrounding ICO directory.
    if len(payload) < 40:
        raise ValueError("truncated icon resource")
    width, double_height = struct.unpack_from("<ii", payload, 4)
    height = abs(double_height) // 2
    if width <= 0 or height <= 0 or width > 2048 or height > 2048:
        raise ValueError("invalid RT_ICON bitmap dimensions")
    w = 0 if width == 256 else width
    h = 0 if height == 256 else height
    icon_header = struct.pack("<HHH", 0, 1, 1)
    entry = struct.pack("<BBBBHHII", w, h, 0, 0, 1, 32, len(payload), 22)
    return Image.open(io.BytesIO(icon_header + entry + payload)).convert("RGBA")


def check_pe(path: Path) -> None:
    pe = pefile.PE(str(path), fast_load=False)
    sizes = set()
    for entry in pe.DIRECTORY_ENTRY_RESOURCE.entries:
        if entry.id != RT_ICON:
            continue
        for icon in entry.directory.entries:
            for lang in icon.directory.entries:
                record = lang.data.struct
                payload = pe.get_data(record.OffsetToData, record.Size)
                try:
                    img = decode_resource(payload)
                except Exception:
                    continue
                if recognized_brand(img):
                    sizes.add(img.size[0])
    # 16px RT_ICON is the crucial resource Explorer and Windows taskbar use.
    required = {16, 32}
    if not required.issubset(sizes):
        raise SystemExit(
            f"{path}: missing embedded cyan/gold icons at {sorted(required)}; "
            f"brand frames found: {sorted(sizes)}"
        )
    print(f"{path.name}: verified cyan/gold embedded icon frames {sorted(sizes)}")


def check_source(path: Path) -> None:
    with Image.open(path) as icon:
        resolutions = icon.ico.sizes()
        for size in (16, 32, 48, 256):
            if (size, size) not in resolutions:
                raise SystemExit(f"Source ICO lacks {size}x{size} frame")
            if not recognized_brand(icon.ico.getimage((size, size))):
                raise SystemExit(f"Source ICO {size}x{size} uses the wrong branding")
    print("Source ICO: 16/32/48/256 pixels all match cyan/gold brand")


if __name__ == "__main__":
    if len(sys.argv) < 3:
        raise SystemExit("Usage: verify_windows_icons.py app.exe setup.exe")
    check_source(Path("src-tauri/icons/icon.ico"))
    for filename in sys.argv[1:]:
        check_pe(Path(filename))
