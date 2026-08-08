#!/usr/bin/env python3
"""Format in-repository Rust files named by Codex apply-patch markers."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Callable, Iterable


PATCH_FILE_MARKER = re.compile(
    r"^\*\*\* (?:(?:Add|Update) File|Move to): (.+)$", re.MULTILINE
)


class HookError(Exception):
    """A hook input cannot safely be formatted."""


def git_root(cwd: str) -> Path:
    try:
        result = subprocess.run(
            ["git", "-C", cwd, "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise HookError("could not resolve the repository root") from error
    return Path(result.stdout.strip()).resolve(strict=True)


def changed_rust_files(patch: str, root: Path) -> list[Path]:
    """Return existing, non-escaping Rust paths named by add, update, or move markers."""
    root = root.resolve(strict=True)
    resolved: list[Path] = []
    seen: set[Path] = set()
    for marker in PATCH_FILE_MARKER.findall(patch):
        candidate = Path(marker)
        if candidate.suffix != ".rs":
            continue
        path = candidate if candidate.is_absolute() else root / candidate
        try:
            path = path.resolve(strict=False)
            path.relative_to(root)
        except (OSError, RuntimeError, ValueError) as error:
            raise HookError("refused a Rust path outside the repository root") from error
        if path.is_file() and path not in seen:
            seen.add(path)
            resolved.append(path)
    return resolved


def run_rustfmt(
    paths: Iterable[Path],
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> str | None:
    files = [str(path) for path in paths]
    if not files:
        return None
    try:
        result = runner(
            ["rustfmt", "--edition", "2024", *files],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError:
        return "rustfmt could not be started"
    if result.returncode == 0:
        return None
    detail = (result.stderr or result.stdout).strip().splitlines()
    suffix = f": {detail[0]}" if detail else ""
    return f"rustfmt failed (exit {result.returncode}){suffix}"


def feedback(message: str) -> str:
    return json.dumps(
        {
            "hookSpecificOutput": {
                "hookEventName": "PostToolUse",
                "additionalContext": message,
            }
        },
        separators=(",", ":"),
    )


def main(payload: object) -> str | None:
    if not isinstance(payload, dict):
        return feedback("Rust formatter hook received invalid apply-patch input.")
    tool_input = payload.get("tool_input")
    if not isinstance(tool_input, dict) or not isinstance(tool_input.get("command"), str):
        return feedback("Rust formatter hook could not read the apply-patch command.")
    try:
        root = git_root(str(payload.get("cwd") or os.getcwd()))
        paths = changed_rust_files(tool_input["command"], root)
        error = run_rustfmt(paths)
    except HookError as error:
        return feedback(f"Rust formatter hook: {error}.")
    if error:
        return feedback(f"Rust formatter hook: {error}.")
    return None


if __name__ == "__main__":
    try:
        payload = json.load(sys.stdin)
    except json.JSONDecodeError:
        print(feedback("Rust formatter hook received invalid JSON input."))
    else:
        output = main(payload)
        if output:
            print(output)
