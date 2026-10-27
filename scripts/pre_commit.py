"""Format captured index paths and verify their crates and reverse dependents."""

import json
import os
import pathlib
import subprocess
import sys

# shortcut: packages-local hook selection, replace when core tooling exposes a reusable entry point.


def run(command, **kwargs):
    """Run a check without suppressing a failure."""
    return subprocess.run(command, check=True, **kwargs)


def selection(metadata, paths):
    """Select staged owners and transitive reverse path dependents in metadata order."""
    members = [p for p in metadata["packages"] if p["id"] in metadata["workspace_members"]]
    roots = {pathlib.Path(p["manifest_path"]).parent: p["name"] for p in members}
    changed = set()
    for path in paths:
        absolute = pathlib.Path(metadata["workspace_root"]) / path
        owner = next((roots[parent] for parent in absolute.parents if parent in roots), None)
        if owner is None or absolute.name == "Cargo.toml":
            return ["--workspace"]
        changed.add(owner)
    if not paths:
        return ["--workspace"]
    pending = list(changed)
    while pending:
        dependency = pending.pop()
        for package in members:
            if package["name"] not in changed and any(
                roots.get(pathlib.Path(dep["path"])) == dependency
                for dep in package["dependencies"] if dep.get("path")
            ):
                changed.add(package["name"])
                pending.append(package["name"])
    return [arg for p in members if p["name"] in changed for arg in ("-p", p["name"])]


def format_and_restage(root):
    """Capture filenames before formatting and stage only surviving captured paths."""
    captured = subprocess.check_output(
        ["git", "diff", "--cached", "--name-only", "--no-renames", "-z"], cwd=root
    )
    paths = [os.fsdecode(path) for path in captured.split(b"\0") if path]
    run(["cargo", "fmt", "--all"], cwd=root)
    surviving = [path for path in paths if (root / path).is_file()]
    if surviving:
        run(["git", "add", "--", *surviving], cwd=root)
    return paths


def main():
    """Run the staged-workspace check recipe after formatting."""
    root = pathlib.Path.cwd()
    paths = format_and_restage(root)
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=root
    ))
    arguments = selection(metadata, paths)
    print("Selected crates:", " ".join(arguments), flush=True)
    run(["just", "_check", *arguments], cwd=root)
    print("All pre-commit checks passed!", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (subprocess.CalledProcessError, OSError, ValueError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
