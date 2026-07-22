#!/usr/bin/env python3
"""Validate R-905 with local Git repositories and catalog fixtures."""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BINARY = ROOT / "target" / "debug" / (
    "spectralang.exe" if os.name == "nt" else "spectralang"
)
COMPATIBILITY = "spectralang-0.1"


def fail(message: str) -> None:
    raise RuntimeError(f"R-905 validation failed: {message}")


def run(command: list[str], cwd: Path = ROOT, timeout: int = 180, expect: int = 0) -> str:
    proc = subprocess.run(command, cwd=cwd, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT, timeout=timeout, check=False)
    if proc.returncode != expect:
        fail(f"{' '.join(command)} returned {proc.returncode}, expected {expect}:\n{proc.stdout}")
    return proc.stdout


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")


def git(command: list[str], cwd: Path) -> str:
    return run(["git", *command], cwd=cwd)


def init_repo(path: Path, name: str, version: str, compatibility: str = COMPATIBILITY,
              dependencies: str = "") -> dict[str, str]:
    path.mkdir(parents=True)
    write(path / "spectra.toml", "\n".join([
        "[project]", f'name = "{name}"', f'version = "{version}"',
        'entry = "src/main.spectra"', 'src_dirs = ["src"]', "",
        "[release]", 'channel = "stable"', f'compatibility = "{compatibility}"', "",
        "[dependencies]", dependencies, "",
    ]))
    write(path / "src/main.spectra", "module main;\n\npub fn main() -> int { return 0; }\n")
    git(["init", "-q"], path)
    git(["config", "user.email", "spectra@example.local"], path)
    git(["config", "user.name", "Spectra R905"], path)
    git(["add", "."], path)
    git(["commit", "-q", "-m", f"release {name} {version}"], path)
    tag = f"v{version}"
    git(["tag", tag], path)
    revision = git(["rev-parse", "HEAD"], path).strip()
    return {"name": name, "version": version, "git": path.as_posix(),
            "tag": tag, "resolved_rev": revision,
            "checksum": directory_checksum(path)}


def directory_checksum(path: Path) -> str:
    digest = hashlib.sha256()
    files = sorted(p for p in path.rglob("*") if p.is_file() and ".git" not in p.parts)
    for file in files:
        digest.update(file.relative_to(path).as_posix().encode())
        digest.update(b"\0")
        digest.update(file.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def catalog_entry(metadata: dict[str, str], compatibility: str = COMPATIBILITY) -> str:
    return "\n".join([
        "[[packages]]", f'name = "{metadata["name"]}"',
        f'version = "{metadata["version"]}"', f'git = "{metadata["git"]}"',
        f'tag = "{metadata["tag"]}"', f'resolved_rev = "{metadata["resolved_rev"]}"',
        f'checksum = "{metadata["checksum"]}"', f'compatibility = "{compatibility}"',
        "modules = []", "",
    ])


def consumer(path: Path, catalog: Path) -> None:
    write(path / "spectra.toml", "\n".join([
        "[project]", 'name = "consumer"', 'version = "0.1.0"',
        'entry = "src/main.spectra"', 'src_dirs = ["src"]', "",
        "[package.catalogs]", f'local = "{catalog.as_posix()}"', "",
        "[dependencies]", "",
    ]))
    write(path / "src/main.spectra", "module consumer;\n\npub fn main() -> int { return 0; }\n")


def assert_contains(output: str, text: str, context: str) -> None:
    if text not in output:
        fail(f"{context}: expected {text!r}, got:\n{output}")


def validate(binary: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="spectralang-r905-") as raw:
        work = Path(raw)
        repos = work / "repos"
        catalog = work / "catalog"
        catalog.mkdir()

        demo10 = init_repo(repos / "demo-1.0.0", "demo", "1.0.0")
        demo11 = init_repo(repos / "demo-1.1.0", "demo", "1.1.0")
        demo20 = init_repo(repos / "demo-2.0.0", "demo", "2.0.0", "spectralang-0.2")
        pre = init_repo(repos / "pre-1.0.0-alpha.1", "pre", "1.0.0-alpha.1")
        entries = "schema = \"spectra-package-catalog-v1\"\n\n"
        entries += catalog_entry(demo10) + catalog_entry(demo11)
        entries += catalog_entry(demo11)  # identical duplicate must coalesce
        entries += catalog_entry(demo20, "spectralang-0.2")
        entries += catalog_entry(pre)
        write(catalog / "package.index.toml", entries)

        selected = work / "selected"
        consumer(selected, catalog)
        run([str(binary), "package", "add", "demo", "--root", str(selected)])
        manifest = tomllib.loads((selected / "spectra.toml").read_text(encoding="utf-8"))
        if manifest["dependencies"]["demo"]["version"] != "1.1.0":
            fail("highest compatible catalog version was not selected")
        first_lock = (selected / "spectra.lock").read_bytes()
        run([str(binary), "package", "lock", "--root", str(selected)])
        second_lock = (selected / "spectra.lock").read_bytes()
        if first_lock != second_lock:
            fail("repeated lockfile resolution is not deterministic")

        exact = work / "exact"
        consumer(exact, catalog)
        run([str(binary), "package", "add", "demo@1.0.0", "--root", str(exact)])
        exact_manifest = tomllib.loads((exact / "spectra.toml").read_text(encoding="utf-8"))
        if exact_manifest["dependencies"]["demo"]["version"] != "1.0.0":
            fail("exact catalog version was not selected")

        prerelease = work / "prerelease"
        consumer(prerelease, catalog)
        run([str(binary), "package", "add", "pre@1.0.0-alpha.1", "--root", str(prerelease)])

        incompatible = work / "incompatible"
        consumer(incompatible, catalog)
        output = run([str(binary), "package", "add", "demo@2.0.0", "--root", str(incompatible)], expect=74)
        assert_contains(output, "incompatible with CLI compatibility", "catalog compatibility")

        invalid_catalog = work / "invalid-catalog"
        invalid_catalog.mkdir()
        write(invalid_catalog / "package.index.toml", "schema = \"spectra-package-catalog-v1\"\n\n[[packages]]\nname = \"bad\"\nversion = \"1.0\"\ngit = \"C:/bad\"\ncompatibility = \"spectralang-0.1\"\n")
        invalid_consumer = work / "invalid"
        consumer(invalid_consumer, invalid_catalog)
        output = run([str(binary), "package", "add", "bad", "--root", str(invalid_consumer)], expect=74)
        assert_contains(output, "invalid semver", "invalid semver")

        conflict_catalog = work / "conflict-catalog"
        conflict_catalog.mkdir()
        conflict = catalog_entry(demo10).replace(demo10["git"], (repos / "other").as_posix())
        write(conflict_catalog / "package.index.toml", "schema = \"spectra-package-catalog-v1\"\n\n" + catalog_entry(demo10) + conflict)
        conflict_consumer = work / "conflict"
        consumer(conflict_consumer, conflict_catalog)
        output = run([str(binary), "package", "add", "demo", "--root", str(conflict_consumer)], expect=74)
        assert_contains(output, "catalog entries for 'demo' version '1.0.0' conflict", "catalog conflict")

        direct = work / "direct"
        consumer(direct, catalog)
        output = run([str(binary), "package", "add", "demo", "--root", str(direct), "--git", demo20["git"], "--tag", demo20["tag"]], expect=74)
        assert_contains(output, "incompatible with CLI compatibility", "direct Git compatibility")

        duplicate_root = work / "duplicate-root"
        write(duplicate_root / "spectra.toml", "\n".join([
            "[project]", 'name = "root"', 'version = "0.1.0"',
            'entry = "src/main.spectra"', 'src_dirs = ["src"]', "",
            "[workspace]", 'members = ["one", "two"]', "",
        ]))
        write(duplicate_root / "src/main.spectra", "module root;\n")
        for member in ("one", "two"):
            init_repo(duplicate_root / member, "same", "1.0.0")
        output = run([str(binary), "package", "lock", "--root", str(duplicate_root)], expect=74)
        assert_contains(output, "appears more than once", "duplicate package")
        assert_contains(output, "one", "duplicate first origin")
        assert_contains(output, "two", "duplicate second origin")

        cycle_root = work / "cycle-root"
        write(cycle_root / "spectra.toml", "\n".join([
            "[project]", 'name = "cycle-root"', 'version = "0.1.0"',
            'entry = "src/main.spectra"', 'src_dirs = ["src"]', "",
            "[workspace]", 'members = ["a", "b"]', "",
        ]))
        write(cycle_root / "src/main.spectra", "module root;\n")
        init_repo(cycle_root / "a", "a", "1.0.0", dependencies='b = { path = "../b" }')
        init_repo(cycle_root / "b", "b", "1.0.0", dependencies='a = { path = "../a" }')
        output = run([str(binary), "package", "lock", "--root", str(cycle_root)], expect=74)
        assert_contains(output, "cyclic package dependency detected: a -> b -> a", "cycle chain")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    args = parser.parse_args()
    if not args.binary.is_file():
        fail(f"binary does not exist: {args.binary}")
    try:
        validate(args.binary.resolve())
    except (OSError, subprocess.SubprocessError, RuntimeError) as exc:
        print(exc, file=sys.stderr)
        return 1
    print("validated R-905 package resolver")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
