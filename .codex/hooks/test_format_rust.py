#!/usr/bin/env python3
"""Focused tests for the PostToolUse Rust formatter hook."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock


MODULE_PATH = Path(__file__).with_name("format_rust.py")
SPEC = importlib.util.spec_from_file_location("format_rust", MODULE_PATH)
assert SPEC and SPEC.loader
FORMAT_RUST = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FORMAT_RUST)


class FormatRustTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.outside_tempdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tempdir.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        (self.root / "src").mkdir()
        (self.root / "src" / "one.rs").write_text("fn one() {}\n")
        (self.root / "src" / "two.rs").write_text("fn two() {}\n")

    def tearDown(self) -> None:
        self.outside_tempdir.cleanup()
        self.tempdir.cleanup()

    def test_selects_existing_rust_add_update_and_move_markers_once(self) -> None:
        patch = """*** Begin Patch
*** Update File: src/one.rs
*** Move to: src/two.rs
*** Add File: src/two.rs
*** Update File: src/one.rs
*** Update File: Cargo.toml
*** Update File: missing.rs
*** End Patch
"""
        self.assertEqual(
            FORMAT_RUST.changed_rust_files(patch, self.root),
            [
                (self.root / "src" / "one.rs").resolve(),
                (self.root / "src" / "two.rs").resolve(),
            ],
        )

    def test_rejects_traversal_and_symlink_escapes(self) -> None:
        outside = Path(self.outside_tempdir.name) / "outside.rs"
        outside.write_text("fn outside() {}\n")
        with self.assertRaises(FORMAT_RUST.HookError):
            FORMAT_RUST.changed_rust_files("*** Update File: ../outside.rs\n", self.root)
        with self.assertRaises(FORMAT_RUST.HookError):
            FORMAT_RUST.changed_rust_files("*** Update File: ../missing.rs\n", self.root)

        (self.root / "escape").symlink_to(outside.parent, target_is_directory=True)
        with self.assertRaises(FORMAT_RUST.HookError):
            FORMAT_RUST.changed_rust_files("*** Update File: escape/outside.rs\n", self.root)

    def test_invokes_rustfmt_without_a_shell_and_reports_failure(self) -> None:
        runner = Mock(
            return_value=subprocess.CompletedProcess(
                [], 1, stdout="", stderr="syntax error\nignored"
            )
        )
        result = FORMAT_RUST.run_rustfmt([self.root / "src" / "one.rs"], runner)
        self.assertEqual(result, "rustfmt failed (exit 1): syntax error")
        args, kwargs = runner.call_args
        self.assertEqual(args[0][:3], ["rustfmt", "--edition", "2024"])
        self.assertFalse(kwargs.get("shell", False))

    def test_main_stays_silent_when_no_rust_marker_exists(self) -> None:
        payload = {
            "cwd": str(self.root),
            "tool_input": {"command": "*** Update File: Cargo.toml\n"},
        }
        self.assertIsNone(FORMAT_RUST.main(payload))

    def test_main_returns_valid_post_tool_use_feedback(self) -> None:
        outside = Path(self.outside_tempdir.name) / "outside-main.rs"
        outside.write_text("fn outside_main() {}\n")
        payload = {
            "cwd": str(self.root),
            "tool_input": {"command": f"*** Update File: {outside}\n"},
        }
        result = FORMAT_RUST.main(payload)
        self.assertIsNotNone(result)
        decoded = json.loads(result)
        self.assertEqual(decoded["hookSpecificOutput"]["hookEventName"], "PostToolUse")


if __name__ == "__main__":
    unittest.main()
