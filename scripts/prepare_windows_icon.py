#!/usr/bin/env python3
"""Build a multiresolution Windows ICO from the exact source artwork used by Android.

The committed 16px ICO is the canonical source for *both* platforms. The Android
CI resizes it into launcher PNGs. This script enhances the Windows packaging
resource in the CI workspace only, without introducing a second independent logo.
"""
from pathlib import Path
from PIL import Image

icon_path = Path("src-tauri/icons/icon.ico")
with Image.open(icon_path) as src:
    if src.format != "ICO":
        raise SystemExit("Expected the shared source ICO")
    sizes = src.ico.sizes()
    if (16, 16) not in sizes:
        raise SystemExit("The shared artwork must contain the original 16px icon")
    original = src.ico.getimage((16, 16)).convert("RGBA")

sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (256, 256)]
# The 16px original is the same source that Android resizes with LANCZOS.
# Upscaling cannot invent genuine extra detail; a vector source is a future
# artwork-only upgrade, not a reason to ship two inconsistent app identities.
original.resize((256, 256), Image.Resampling.LANCZOS).save(
    icon_path, format="ICO", sizes=sizes
)
with Image.open(icon_path) as output:
    generated = output.ico.sizes()
    missing = [size for size in sizes if size not in generated]
    if missing:
        raise SystemExit(f"Missing Windows ICO image sizes: {missing}")
    print(f"Windows ICO: {sorted(generated)} (shared Android artwork)")
