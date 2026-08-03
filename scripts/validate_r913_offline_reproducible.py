#!/usr/bin/env python3
"""Validate offline package resolution and locked reproducibility for R-913."""

from __future__ import annotations

import argparse
import os
import shutil
import stat
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    raise RuntimeError(f"R-913 validation failed: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def run(binary: Path, args: list[str], cwd: Path, expect: int = 0) -> str:
    result = subprocess.run(
        [str(binary), *args],
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=180,
        check=False,
    )
    if result.returncode != expect:
        fail(f"{' '.join(args)} returned {result.returncode}, expected {expect}:\n{result.stdout}")
    return result.stdout


def git(args: list[str], cwd: Path) -> str:
    result = subprocess.run(
        ["git", *args], cwd=cwd, text=True, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, timeout=60, check=False,
    )
    if result.returncode != 0:
        fail(f"git {' '.join(args)} failed:\n{result.stdout}")
    return result.stdout.strip()


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")


def remove_tree(path: Path) -> None:
    def on_error(func, target, _exc_info):
        os.chmod(target, stat.S_IRWXU)
        func(target)

    if path.exists():
        shutil.rmtree(path, onerror=on_error)


def init_repo(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    write(path / "spectra.toml", """[project]
name = "offline_math"
version = "1.0.0"
entry = "src/main.spectra"
src_dirs = ["src"]

[release]
channel = "stable"
compatibility = "spectralang-0.1"
""")
    write(path / "src/core.spectra", "module offline_math.core;\n\npub fn answer() -> int {\n    return 42;\n}\n")
    git(["init", "-q"], path)
    git(["config", "user.email", "spectra@example.local"], path)
    git(["config", "user.name", "Spectra Validator"], path)
    git(["add", "."], path)
    git(["commit", "-q", "-m", "offline fixture"], path)
    git(["tag", "v1.0.0"], path)


def validate(binary: Path) -> None:
    work = ROOT / "target" / f"r913-offline-reproducible-{os.getpid()}"
    repo = work / "repo"
    consumer = work / "consumer"
    restored = work / "restored"
    init_repo(repo)
    repo_url = repo.as_posix()

    write(consumer / "spectra.toml", f"""[project]
name = "offline_consumer"
version = "0.1.0"
entry = "src/main.spectra"
src_dirs = ["src"]

[dependencies.offline_math]
version = "1.0.0"
git = "{repo_url}"
tag = "v1.0.0"
""")
    write(consumer / "src/main.spectra", """module offline_consumer.main;

import { answer } from offline_math.core;

pub fn main() -> int {
    return answer();
}
""")

    run(binary, ["package", "fetch", "--root", str(consumer)], consumer)
    lock = consumer / "spectra.lock"
    git_cache = consumer / ".spectra" / "git"
    vendor_cache = consumer / ".spectra" / "packages"
    require(lock.is_file(), "online fetch did not create spectra.lock")
    require(git_cache.is_dir(), "Git cache was not populated")
    require(vendor_cache.is_dir(), "vendor cache was not populated")
    original_lock = lock.read_bytes()

    shutil.copytree(consumer, restored)
    remove_tree(repo)
    run(binary, ["package", "fetch", "--root", str(restored), "--offline", "--locked"], restored)
    run(binary, ["package", "check", "--root", str(restored), "--offline", "--locked"], restored)
    run(binary, ["package", "build", "--root", str(restored), "--offline", "--locked"], restored)
    require((restored / "spectra.lock").read_bytes() == original_lock, "offline locked fetch rewrote the lockfile")

    missing_cache = restored / ".spectra" / "git"
    hidden_cache = restored / ".spectra" / "git.disabled"
    missing_cache.rename(hidden_cache)
    require(not missing_cache.exists() and hidden_cache.is_dir(), "fixture failed to disable the Git cache")
    missing = run(binary, ["package", "fetch", "--root", str(restored), "--offline", "--locked"], restored, expect=74)
    require("cache" in missing.lower(), "missing offline cache diagnostic is not actionable")

    hidden_cache.rename(missing_cache)
    changed_lock = original_lock.replace(b'version = "1.0.0"', b'version = "1.0.1"', 1)
    require(changed_lock != original_lock, "fixture failed to alter lockfile")
    (restored / "spectra.lock").write_bytes(changed_lock)
    changed = run(binary, ["package", "check", "--root", str(restored), "--offline", "--locked"], restored, expect=74)
    require("lockfile" in changed.lower(), "changed lockfile was accepted")
    (restored / "spectra.lock").write_bytes(original_lock)

    tampered_repo = restored / ".spectra" / "git" / "offline_math"
    git_metadata = tampered_repo / ".git"
    hidden_metadata = tampered_repo / ".git.disabled"
    git_metadata.rename(hidden_metadata)
    tampered = run(binary, ["package", "fetch", "--root", str(restored), "--offline", "--locked"], restored, expect=74)
    hidden_metadata.rename(git_metadata)
    require("cache" in tampered.lower(), "corrupt Git cache was not rejected")

    print("validated R-913 offline and reproducible package flow")


def main() -> None:
    parser = argparse.ArgumentParser()
    default_binary = ROOT / "target" / "debug" / ("spectralang.exe" if os.name == "nt" else "spectralang")
    parser.add_argument("--binary", type=Path, default=default_binary)
    args = parser.parse_args()
    if not args.binary.is_file():
        fail(f"binary not found: {args.binary}")
    try:
        validate(args.binary.resolve())
    except (OSError, subprocess.SubprocessError, RuntimeError) as error:
        print(error, file=sys.stderr)
        return sys.exit(1)


if __name__ == "__main__":
    main()
