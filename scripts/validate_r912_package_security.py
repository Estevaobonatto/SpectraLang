#!/usr/bin/env python3
"""Validate R-912 package cache, path, host, checksum, and lockfile security."""

from __future__ import annotations

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


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")


def run(
    binary: Path,
    args: list[str],
    cwd: Path,
    expect: int = 0,
    env: dict[str, str] | None = None,
) -> str:
    merged = os.environ.copy()
    if env:
        merged.update(env)
    result = subprocess.run(
        [str(binary), *args],
        cwd=cwd,
        env=merged,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=180,
        check=False,
    )
    if result.returncode != expect:
        raise RuntimeError(
            f"{' '.join(args)} returned {result.returncode}, expected {expect}:\n"
            f"{result.stdout}"
        )
    return result.stdout


def git(args: list[str], cwd: Path) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=60,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} failed:\n{result.stdout}")
    return result.stdout.strip()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def manifest(name: str, version: str = "1.0.0") -> str:
    return "\n".join(
        [
            "[project]",
            f'name = "{name}"',
            f'version = "{version}"',
            'entry = "src/main.spectra"',
            'src_dirs = ["src"]',
            "",
            "[release]",
            'channel = "stable"',
            'compatibility = "spectralang-0.1"',
            "",
        ]
    )


def init_git_package(path: Path, name: str, symlink: bool = False) -> None:
    path.mkdir(parents=True)
    write(path / "spectra.toml", manifest(name))
    write(path / "src/main.spectra", "module package.main;\npub fn secure_value() -> int { return 0; }\n")
    if symlink:
        outside = path.parent / "outside.txt"
        write(outside, "outside payload\n")
        try:
            os.symlink(outside, path / "src/outside-link.txt")
        except OSError as exc:
            raise RuntimeError(f"symlink fixture could not be created: {exc}") from exc
    git(["init", "-q"], path)
    git(["config", "user.email", "spectra@example.local"], path)
    git(["config", "user.name", "Spectra R912"], path)
    git(["add", "."], path)
    git(["commit", "-q", "-m", f"release {name}"], path)
    git(["tag", "v1.0.0"], path)


def consumer(path: Path) -> None:
    write(path / "spectra.toml", manifest("consumer"))
    write(path / "src/main.spectra", "module consumer.main;\npub fn main() -> int { return 0; }\n")


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def staging_entries(root: Path) -> list[Path]:
    spectra = root / ".spectra"
    if not spectra.exists():
        return []
    return [
        path
        for path in spectra.rglob("*")
        if path.name.startswith(".") and (".git" not in path.parts)
    ]


def main() -> int:
    binary = DEFAULT_BINARY
    if len(sys.argv) > 2 and sys.argv[1] == "--binary":
        binary = Path(sys.argv[2])
    binary = binary.resolve()
    if not binary.is_file():
        print(f"binary does not exist: {binary}", file=sys.stderr)
        return 1

    try:
        with tempfile.TemporaryDirectory(prefix="spectralang-r912-") as raw:
            work = Path(raw)
            git_repo = work / "git-package"
            init_git_package(git_repo, "securepkg")
            project = work / "project"
            consumer(project)

            run(
                binary,
                ["package", "add", "securepkg", "--root", str(project), "--git", git_repo.as_posix(), "--tag", "v1.0.0"],
                project,
            )
            vendor = project / ".spectra/packages/securepkg-1.0.0"
            require(vendor.is_dir(), "Git package was not installed")
            require(not staging_entries(project), "staging entries remained after Git install")
            run(binary, ["package", "check", "--root", str(project), "--locked"], project)

            lock = project / "spectra.lock"
            lock_text = lock.read_text(encoding="utf-8")
            write(lock, lock_text.replace('version = "1.0.0"', 'version = "1.0.1"', 1))
            output = run(binary, ["package", "check", "--root", str(project), "--locked"], project, expect=74)
            require("lockfile" in output and "package" in output, "lockfile tamper diagnostic missing")
            write(lock, lock_text)

            lock.unlink()
            output = run(binary, ["package", "check", "--root", str(project), "--locked"], project, expect=74)
            require("does not exist" in output, "missing lockfile diagnostic missing")
            run(binary, ["package", "check", "--root", str(project)], project)

            manifest_text = (project / "spectra.toml").read_text(encoding="utf-8")
            tampered_manifest = manifest_text.replace('checksum = "', 'checksum = "0000', 1)
            write(project / "spectra.toml", tampered_manifest)
            output = run(binary, ["package", "check", "--root", str(project)], project, expect=74)
            require("checksum mismatch" in output, "Git checksum mismatch was not rejected")
            require((vendor / "src/main.spectra").is_file(), "valid cache was lost after checksum failure")
            write(project / "spectra.toml", manifest_text)

            registry_source = work / "registry-source"
            init_git_package(registry_source, "registry_pkg")
            registry = work / "registry"
            run(binary, ["package", "publish", "--root", str(registry_source), "--registry", str(registry)], registry_source)
            registry_payload = registry / "registry_pkg/1.0.0/package/src/main.spectra"
            original_payload = registry_payload.read_text(encoding="utf-8")
            write(registry_payload, original_payload + "\n// tampered\n")
            registry_consumer = work / "registry-consumer"
            consumer(registry_consumer)
            output = run(
                binary,
                ["package", "add", "registry_pkg", "--version", "1.0.0", "--root", str(registry_consumer), "--registry", str(registry)],
                registry_consumer,
                expect=74,
            )
            require("checksum mismatch" in output, f"registry checksum mismatch was not rejected: {output}")
            require(not staging_entries(registry_consumer), "staging remained after registry failure")

            traversal = work / "traversal"
            consumer(traversal)
            output = run(
                binary,
                ["package", "add", "../escape", "--root", str(traversal), "--registry", str(registry)],
                traversal,
                expect=74,
            )
            require("unsafe package path" in output, "registry path traversal was not rejected")

            allowlist_project = work / "allowlist"
            consumer(allowlist_project)
            output = run(
                binary,
                ["package", "add", "securepkg", "--root", str(allowlist_project), "--git", "https://blocked.example/securepkg.git", "--tag", "v1.0.0"],
                allowlist_project,
                expect=74,
                env={"SPECTRA_PACKAGE_ALLOWED_HOSTS": "github.com,gitlab.com"},
            )
            require("not allowed" in output and "blocked.example" in output, "host allowlist diagnostic missing")
            local_project = work / "local-allowlist"
            consumer(local_project)
            run(
                binary,
                ["package", "add", "securepkg", "--root", str(local_project), "--git", git_repo.as_posix(), "--tag", "v1.0.0"],
                local_project,
                env={"SPECTRA_PACKAGE_ALLOWED_HOSTS": "github.com"},
            )

            symlink_source = work / "symlink-source"
            init_git_package(symlink_source, "symlinkpkg")
            symlink_registry = work / "symlink-registry"
            run(binary, ["package", "publish", "--root", str(symlink_source), "--registry", str(symlink_registry)], symlink_source)
            outside = work / "symlink-outside.txt"
            write(outside, "outside payload\n")
            try:
                os.symlink(outside, symlink_registry / "symlinkpkg/1.0.0/package/src/outside-link.txt")
            except OSError as exc:
                raise RuntimeError(f"symlink fixture could not be created: {exc}") from exc
            symlink_project = work / "symlink-consumer"
            consumer(symlink_project)
            output = run(
                binary,
                ["package", "add", "symlinkpkg", "--version", "1.0.0", "--root", str(symlink_project), "--registry", str(symlink_registry)],
                symlink_project,
                expect=74,
            )
            require("symbolic links" in output, "symlink payload was not rejected")
            require(not staging_entries(symlink_project), "staging remained after symlink rejection")

            branch_output = run(
                binary,
                ["package", "register", "--root", str(git_repo), "--git", git_repo.as_posix(), "--branch", "main", "--catalog", str(work / "branch-catalog")],
                git_repo,
                expect=74,
            )
            require("branch" in branch_output.lower(), "mutable branch publication was not rejected")

        print("validated R-912 package security")
        return 0
    except (OSError, subprocess.SubprocessError, RuntimeError) as exc:
        print(exc, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
