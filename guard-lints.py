#!/usr/bin/env python3
"""Fail unless every workspace member inherits the workspace lints."""
import pathlib, tomllib, sys

root = tomllib.loads(pathlib.Path("Cargo.toml").read_text())
members = root.get("workspace", {}).get("members", [])
failures = []
for pattern in members:
    for manifest in pathlib.Path(".").glob(f"{pattern.removesuffix('*')}*/Cargo.toml"):
        if manifest == pathlib.Path("Cargo.toml"):
            continue
        data = tomllib.loads(manifest.read_text())
        if "workspace" not in data.get("lints", {}):
            failures.append(str(manifest))
if failures:
    print("lints guard FAIL:", ", ".join(failures), file=sys.stderr)
    sys.exit(1)
print("lints guard: every member inherits workspace lints")
