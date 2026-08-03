#!/usr/bin/env python3
"""Validate deterministic catalog synchronization and cache management for R-911."""

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
    raise RuntimeError(f"R-911 validation failed: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def run(binary: Path, args: list[str], cwd: Path, expect: int = 0) -> str:
    result = subprocess.run(
        [str(binary), *args], cwd=cwd, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        timeout=180, check=False,
    )
    if result.returncode != expect:
        fail(f"{' '.join(args)} returned {result.returncode}, expected {expect}:\n{result.stdout}")
    return result.stdout


def git(args: list[str], cwd: Path) -> str:
    result = subprocess.run(
        ["git", *args], cwd=cwd, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        timeout=60, check=False,
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


def init_git(path: Path) -> None:
    git(["init", "-q"], path)
    git(["config", "user.email", "spectra@example.local"], path)
    git(["config", "user.name", "Spectra Validator"], path)
    git(["add", "."], path)
    git(["commit", "-q", "-m", "fixture"], path)


def package_repo(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    write(path / "spectra.toml", """[project]
name = "catalog_math"
version = "1.0.0"
entry = "src/main.spectra"
src_dirs = ["src"]

[release]
channel = "stable"
compatibility = "spectralang-0.1"
""")
    write(path / "src/main.spectra", 'module catalog_math.core\n\npublic func answer() returns int { return 42\n }\n')
    init_git(path)
    git(["tag", "v1.0.0"], path)


def catalog_index(package_path: Path, description: str = "math helpers") -> str:
    return f"""schema = "spectra-package-catalog-v1"

[[packages]]
name = "catalog_math"
version = "1.0.0"
git = "{package_path.as_posix()}"
tag = "v1.0.0"
description = "{description}"
keywords = ["math"]
compatibility = "spectralang-0.1"
license = "MIT"
modules = ["catalog_math.core"]
owner = "validator"
"""


def catalog_repo(path: Path, package_path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    write(path / "package.index.toml", catalog_index(package_path))
    init_git(path)


def validate(binary: Path) -> None:
    work = ROOT / "target" / f"r911-catalog-sync-{os.getpid()}"
    remove_tree(work)
    repos = work / "repos"
    consumer = work / "consumer"
    package = repos / "catalog_math"
    catalog = repos / "catalog"
    bad_catalog = repos / "bad-catalog"
    package_repo(package)
    catalog_repo(catalog, package)

    write(consumer / "spectra.toml", """[project]
name = "catalog_consumer"
version = "0.1.0"
entry = "src/main.spectra"
src_dirs = ["src"]
""")
    write(consumer / "src/main.spectra", 'module catalog_consumer.main\n\npublic func main() returns int { return 0\n }\n')

    catalog_source = catalog.as_posix()
    run(binary, ["package", "catalog", "add", "primary", catalog_source, "--root", str(consumer)], consumer)
    listed = run(binary, ["package", "catalog", "list", "--root", str(consumer)], consumer)
    require("primary" in listed and "missing" in listed, "catalog list did not report missing cache")

    run(binary, ["package", "catalog", "sync", "--root", str(consumer)], consumer)
    state = consumer / ".spectra" / "catalogs" / "catalogs.lock"
    cache_index = consumer / ".spectra" / "catalogs" / "primary" / "package.index.toml"
    require(state.is_file() and cache_index.is_file(), "catalog sync did not publish cache/state")
    listed = run(binary, ["package", "catalog", "list", "--root", str(consumer)], consumer)
    require("ready" in listed and "local" not in listed.splitlines()[0], "catalog list missed Git revision")

    for command, expected in [("search", "catalog_math"), ("info", "catalog_math"), ("versions", "1.0.0")]:
        query = "math" if command == "search" else "catalog_math"
        output = run(binary, ["package", command, query, "--root", str(consumer)], consumer)
        require(expected in output, f"cached catalog was not used by package {command}")

    run(binary, ["package", "add", "catalog_math", "--root", str(consumer)], consumer)
    require("catalog_math" in (consumer / "spectra.toml").read_text(encoding="utf-8"), "catalog package was not added")

    before = state.read_bytes()
    write(catalog / "package.index.toml", catalog_index(package, "updated description"))
    git(["add", "package.index.toml"], catalog)
    git(["commit", "-q", "-m", "update catalog"], catalog)
    run(binary, ["package", "catalog", "sync", "--root", str(consumer)], consumer)
    require(state.read_bytes() != before, "catalog revision update was not recorded")

    bad_catalog.mkdir(parents=True, exist_ok=True)
    write(bad_catalog / "package.index.toml", "schema = \"wrong-schema\"\n")
    init_git(bad_catalog)
    run(binary, ["package", "catalog", "add", "broken", bad_catalog.as_posix(), "--root", str(consumer)], consumer)
    previous_cache = cache_index.read_bytes()
    failed = run(binary, ["package", "catalog", "sync", "--root", str(consumer)], consumer, expect=74)
    require("catalog" in failed.lower() and "schema" in failed.lower(), "invalid catalog diagnostic missing context")
    require(cache_index.read_bytes() == previous_cache, "valid catalog cache was lost after sync failure")

    run(binary, ["package", "catalog", "remove", "broken", "--root", str(consumer)], consumer)
    remove_tree(catalog)
    run(binary, ["package", "catalog", "sync", "--offline", "--locked", "--root", str(consumer)], consumer)
    remove_tree(consumer / ".spectra" / "catalogs" / "primary")
    missing = run(binary, ["package", "catalog", "sync", "--offline", "--root", str(consumer)], consumer, expect=74)
    require("cache" in missing.lower(), "offline missing catalog cache was accepted")

    print("validated R-911 deterministic catalog sync")


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
