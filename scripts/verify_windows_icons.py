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
    icon_resources = {}
    groups = []
    for entry in pe.DIRECTORY_ENTRY_RESOURCE.entries:
        if entry.id not in (RT_ICON, 14):
            continue
        for item in entry.directory.entries:
            for lang in item.directory.entries:
                record = lang.data.struct
                payload = pe.get_data(record.OffsetToData, record.Size)
                if entry.id == RT_ICON:
                    icon_resources[item.id] = payload
                else:
                    groups.append(payload)
    if not groups:
        raise SystemExit(f"{path}: PE has no RT_GROUP_ICON resources")
    # Explorer selects a GROUP_ICON and then refers to its icon IDs. Merely
    # finding the brand elsewhere among unused RT_ICON resources is not enough.
    for group_number, group in enumerate(groups, 1):
        if len(group) < 6:
            raise SystemExit(f"{path}: truncated icon group")
        count = struct.unpack_from("<H", group, 4)[0]
        sizes = set()
        for index in range(count):
            offset = 6 + index * 14
            if offset + 14 > len(group):
                raise SystemExit(f"{path}: corrupt icon group entry")
            width, height, _, _, _, _, _, resource_id = struct.unpack_from(
                "<BBBBHHIH", group, offset
            )
            width = width or 256
            height = height or 256
            payload = icon_resources.get(resource_id)
            if not payload:
                continue
            try:
                img = decode_resource(payload)
            except Exception as exc:
                raise SystemExit(f"{path}: cannot decode group icon {resource_id}: {exc}")
            if img.size != (width, height):
                raise SystemExit(f"{path}: group icon {resource_id} resolution mismatch")
            if recognized_brand(img):
                sizes.add(width)
            elif width in (16, 32):
                raise SystemExit(
                    f"{path}: embedded group {group_number} {width}px icon "
                    "is not the cyan/gold brand (possibly stale C)"
                )
        if not {16, 32}.issubset(sizes):
            raise SystemExit(
                f"{path}: group {group_number} lacks branded 16/32px icons; "
                f"verified: {sorted(sizes)}"
            )
        print(f"{path.name} group {group_number}: verified {sorted(sizes)}")
    print(f"{path.name}: all {len(groups)} embedded icon groups validated")


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
