#!/usr/bin/env python3
"""Validate that the optimized APK actually retains every Rust-called Java method.

A Release APK once compiled and signed successfully but crashed immediately:
  java.lang.NoSuchMethodError: LegacyBridge.startupCheckpoint(...)
Debug APKs were unaffected. This checks the *APK Dex*, not the source tree.
"""
import argparse
import re
import sys
import zipfile
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("apk", type=Path)
args = parser.parse_args()

source = Path("src-tauri/src/android_bridge.rs").read_text(encoding="utf-8")
required = sorted(set(re.findall(r'call\("([A-Za-z0-9_]+)"', source)))
if "startupCheckpoint" not in required or "readCredential" not in required:
    sys.exit(f"Failed to discover required JNI bridge methods from Rust: {required}")

with zipfile.ZipFile(args.apk) as archive:
    dex_files = [
        name for name in archive.namelist()
        if re.fullmatch(r"classes\d*\.dex", name)
    ]
    if not dex_files:
        sys.exit(f"No classes.dex in APK {args.apk}")
    data = b"".join(archive.read(name) for name in dex_files)

missing = [method for method in required if method.encode() not in data]
print(f"Checked {len(required)} JNI bridge method names in "
      f"{len(dex_files)} optimized APK Dex files: {args.apk.name}")
for method in required:
    print(f"  {'MISSING' if method in missing else 'OK'} {method}")
if missing:
    sys.exit("Release JNI bridge methods are absent from classes.dex: "
             + ", ".join(missing))
