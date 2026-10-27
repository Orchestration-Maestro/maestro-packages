"""Regression checks for staged-file formatting and package selection."""

import pathlib
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import pre_commit


class PreCommitTests(unittest.TestCase):
    """Keep unrelated work outside the commit and check reverse dependents."""

    def setUp(self):
        """Describe a chain and an unrelated workspace member."""
        self.metadata = {
            "workspace_root": "/repo",
            "workspace_members": ["a", "b", "c", "other"],
            "packages": [
                {
                    "id": name,
                    "name": name,
                    "manifest_path": f"/repo/crates/{name}/Cargo.toml",
                    "dependencies": [{"path": f"/repo/crates/{dep}"} for dep in deps],
                }
                for name, deps in [("a", []), ("b", ["a"]), ("c", ["b"]), ("other", [])]
            ],
        }

    def test_selects_transitive_reverse_dependencies_only(self):
        """A changed leaf selects its direct and indirect consumers."""
        self.assertEqual(
            pre_commit.selection(self.metadata, ["crates/a/src/lib.rs"]),
            ["-p", "a", "-p", "b", "-p", "c"],
        )

    def test_workspace_inputs_and_empty_index_check_every_member(self):
        """Manifest, root-file and empty changes cannot narrow the checks."""
        for paths in [[], ["Cargo.lock"], ["crates/a/Cargo.toml"], ["README.md"]]:
            with self.subTest(paths=paths):
                self.assertEqual(pre_commit.selection(self.metadata, paths), ["--workspace"])

    def test_renamed_paths_select_both_owners(self):
        """A rename must check both the old and new crate."""
        self.assertEqual(
            pre_commit.selection(self.metadata, ["crates/c/old.rs", "crates/other/new.rs"]),
            ["-p", "c", "-p", "other"],
        )

    def test_formatting_restages_only_captured_surviving_paths(self):
        """Use a real index to prove unrelated formatter edits stay unstaged."""
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            subprocess.run(["git", "init", "-q", directory], check=True)
            for name in ["staged space.rs", "unstaged.rs", "deleted.rs"]:
                (root / name).write_text("original\n")
            subprocess.run(["git", "-C", directory, "add", "staged space.rs", "deleted.rs"], check=True)

            def format_files(command, **kwargs):
                """Stand in for rustfmt without running a local Rust command."""
                if command[:2] == ["cargo", "fmt"]:
                    (root / "staged space.rs").write_text("formatted\n")
                    (root / "unstaged.rs").write_text("formatted unrelated\n")
                    (root / "deleted.rs").unlink()
                    return subprocess.CompletedProcess(command, 0)
                return subprocess.run(command, **kwargs, check=True)

            with patch("pre_commit.run", side_effect=format_files):
                pre_commit.format_and_restage(root)
            staged = subprocess.check_output(["git", "-C", directory, "diff", "--cached", "--name-only"])
            self.assertNotIn(b"unstaged.rs", staged)
            content = subprocess.check_output(["git", "-C", directory, "show", ":staged space.rs"])
            self.assertEqual(content, b"formatted\n")

    def test_formatter_failure_prevents_restaging(self):
        """A failing formatter cannot reach git add or later checks."""
        with patch("pre_commit.subprocess.check_output", return_value=b"a.rs\0"), patch(
            "pre_commit.run", side_effect=subprocess.CalledProcessError(1, ["cargo", "fmt"])
        ) as command:
            with self.assertRaises(subprocess.CalledProcessError):
                pre_commit.format_and_restage(pathlib.Path("/repo"))
            self.assertEqual(command.call_count, 1)


if __name__ == "__main__":
    unittest.main()
