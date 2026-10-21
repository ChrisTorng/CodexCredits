#!/usr/bin/env python3
"""Fail Android CI if the Tauri mobile entry annotation targets the wrong function.

A previous build annotated a #[tauri::command] getter instead of run(), which
compiled into an APK but never started the actual Tauri application.
"""
from pathlib import Path
import re

source = Path("src-tauri/src/lib.rs").read_text(encoding="utf-8")
annotations = re.findall(
    r"(?m)^#\[cfg_attr\(mobile,\s*tauri::mobile_entry_point\)\]"
    r"\s*((?:#\[[^\n]*\]\s*)*)(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*\(",
    source,
)
assert len(annotations) == 1, (
    f"expected one mobile_entry_point annotation, found {annotations!r}"
)
attrs, name = annotations[0]
assert name == "run", f"mobile entry point is {name!r}, expected 'run'"
assert "tauri::command" not in attrs, "mobile entry may not be a Tauri command"
print("OK: Tauri mobile entry point is uniquely attached to run()")
