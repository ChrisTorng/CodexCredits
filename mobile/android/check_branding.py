#!/usr/bin/env python3
"""Verify cross-platform display names and upgrade-compatible identities."""
import json
from pathlib import Path

root=Path(__file__).resolve().parents[2]
read=lambda p:(root/p).read_text(encoding="utf-8")
android=json.loads(read("src-tauri/tauri.android.conf.json"))
windows=json.loads(read("src-tauri/tauri.conf.json"))
assert android["productName"]=="Codex Credits"
assert android["identifier"]=="com.christorng.androidcodextools"
assert windows["identifier"]=="com.christorng.codexcredits"
assert windows["productName"]=="CodexCredits"
assert android["version"]==windows["version"]=="2.0.12"
assert android["bundle"]["android"]["versionCode"]==2000012
assert windows["app"]["windows"][0]["title"]=="Codex Credits"
assert windows["bundle"]["windows"]["nsis"]["shortcutName"]=="Codex Credits"
assert windows["bundle"]["windows"]["nsis"]["uninstallDisplayName"]=="Codex Credits"
assert "<h1>Codex Credits</h1>" in read("src/main.ts")
assert "<title>Codex Credits</title>" in read("index.html")
for path in ("src/main.ts","src-tauri/src/lib.rs","mobile/android/UsageHistory.java"):
    assert "Codex Tools" not in read(path), f"Old visible name: {path}"
assert 'app.set(A("label"),"Codex Credits")' in read("mobile/android/prepare.py")
print("Shared branding and application identities OK")
