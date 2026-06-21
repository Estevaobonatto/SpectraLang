#!/usr/bin/env python3
"""Validate Git-backed package catalog, one-command add, and import flow."""

from __future__ import annotations

import argparse
import os
import shutil
import stat
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    print(f"R-914 validation failed: {message}", file=sys.stderr)
    sys.exit(1)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def run(args: list[str], cwd: Path = ROOT, timeout: int = 120, check: bool = True) -> str:
    completed = subprocess.run(
        args,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
        check=False,
    )
    if check and completed.returncode != 0:
        fail(f"command {' '.join(args)} failed:\n{completed.stdout}")
    return completed.stdout


def cargo_cmd() -> str:
    configured = os.environ.get("CARGO")
    if configured:
        return configured
    found = shutil.which("cargo")
    if found:
        return found
    windows_default = Path.home() / ".cargo" / "bin" / "cargo.exe"
    if windows_default.exists():
        return str(windows_default)
    return "cargo"


def git(args: list[str], cwd: Path) -> str:
    return run(["git", *args], cwd=cwd)


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def remove_tree(path: Path) -> None:
    def on_error(func, target, _exc_info):
        os.chmod(target, stat.S_IWRITE)
        func(target)

    if path.exists():
        shutil.rmtree(path, onerror=on_error)


def init_git_package(path: Path, name: str, version: str, sources: dict[str, str], deps: str = "") -> None:
    path.mkdir(parents=True)
    write(
        path / "spectra.toml",
        "\n".join(
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
                "[dependencies]",
                deps,
                "",
            ]
        ),
    )
    for rel, text in sources.items():
        write(path / rel, text)
    git(["init", "-q"], path)
    git(["config", "user.email", "spectra@example.local"], path)
    git(["config", "user.name", "Spectra Validator"], path)
    git(["add", "."], path)
    git(["commit", "-q", "-m", f"release {name} {version}"], path)
    git(["tag", f"v{version}"], path)


def validate(binary: Path) -> None:
    work = ROOT / "target" / "r914-package-catalog-git"
    remove_tree(work)
    repos = work / "repos"
    catalog = work / "catalog"
    consumer = work / "consumer"

    base_repo = repos / "gitbase"
    init_git_package(
        base_repo,
        "gitbase",
        "1.0.0",
        {
            "src/base.spectra": "module gitbase.base;\n\npub fn seed() -> int {\n    return 40;\n}\n",
        },
    )

    math_repo = repos / "gitmath"
    base_url = base_repo.as_posix()
    init_git_package(
        math_repo,
        "gitmath",
        "1.2.3",
        {
            "src/core.spectra": "module gitmath.core;\n\nimport { seed } from gitbase.base;\n\npub fn double_plus_seed(value: int) -> int {\n    return seed() + value * 2;\n}\n",
        },
        deps=f'gitbase = {{ version = "1.0.0", git = "{base_url}", tag = "v1.0.0" }}',
    )

    write(
        consumer / "spectra.toml",
        "\n".join(
            [
                "[project]",
                'name = "git_package_consumer"',
                'version = "0.1.0"',
                'entry = "src/main.spectra"',
                'src_dirs = ["src"]',
                "",
                "[package.catalogs]",
                f'local = "{catalog.as_posix()}"',
                "",
                "[dependencies]",
                "",
            ]
        ),
    )
    write(
        consumer / "src/main.spectra",
        "module consumer.main;\n\nimport { double_plus_seed } from gitmath.core;\n\npub fn main() -> int {\n    let result = double_plus_seed(1);\n    if result != 42 {\n        return result;\n    }\n    return 0;\n}\n",
    )

    run([str(binary), "package", "register", "--root", str(math_repo), "--git", math_repo.as_posix(), "--tag", "v1.2.3", "--catalog", str(catalog)])
    index_path = catalog / "package.index.toml"
    require(index_path.is_file(), "catalog index missing")
    index = tomllib.loads(index_path.read_text(encoding="utf-8"))
    require(index["packages"][0]["name"] == "gitmath", "registered package name wrong")
    require("gitmath.core" in index["packages"][0]["modules"], "catalog missing exported module")

    search = run([str(binary), "package", "search", "math", "--root", str(consumer)])
    require("gitmath 1.2.3" in search, "search did not find gitmath")
    info = run([str(binary), "package", "info", "gitmath", "--root", str(consumer)])
    require("git:" in info and "tag:v1.2.3" in info, "info missing git metadata")
    versions = run([str(binary), "package", "versions", "gitmath", "--root", str(consumer)])
    require("1.2.3" in versions, "versions missing 1.2.3")

    run([str(binary), "package", "add", "gitmath", "--root", str(consumer)])
    manifest_text = (consumer / "spectra.toml").read_text(encoding="utf-8")
    require('[dependencies.gitmath]' in manifest_text, "manifest missing gitmath dependency")
    require('git = ' in manifest_text and 'tag = "v1.2.3"' in manifest_text, "manifest missing git source")
    require('checksum = "' in manifest_text, "manifest missing checksum")

    lock = tomllib.loads((consumer / "spectra.lock").read_text(encoding="utf-8"))
    require(lock["version"] == 2, "lockfile must be version 2")
    packages = {pkg["name"]: pkg for pkg in lock["packages"]}
    require(packages["gitmath"]["source_kind"] == "git", "gitmath lock source must be git")
    require(len(packages["gitmath"]["resolved_rev"]) >= 40, "gitmath lock missing commit sha")
    require(len(packages["gitmath"]["checksum"]) == 64, "gitmath checksum must be SHA-256")
    require("gitbase" in packages, "transitive git dependency missing from lock")

    for command in ["check", "run", "test", "doc"]:
        run([str(binary), "package", command, "--root", str(consumer)], timeout=180)

    tree = run([str(binary), "package", "tree", "--root", str(consumer)])
    require("git_package_consumer" in tree and "gitmath" in tree and "gitbase" in tree, "tree missing dependency graph")
    run([str(binary), "package", "fetch", "--root", str(consumer), "--offline"])

    tampered = manifest_text.replace('checksum = "', 'checksum = "0000', 1)
    (consumer / "spectra.toml").write_text(tampered, encoding="utf-8")
    failed = run([str(binary), "package", "check", "--root", str(consumer)], check=False)
    require("checksum mismatch" in failed, "tampered checksum did not fail")


def main() -> None:
    parser = argparse.ArgumentParser()
    default_binary = ROOT / "target" / "debug" / (
        "spectralang.exe" if sys.platform.startswith("win") else "spectralang"
    )
    parser.add_argument("--binary", type=Path, default=default_binary)
    args = parser.parse_args()

    run([cargo_cmd(), "build", "-q", "-p", "spectra-cli"], timeout=180)
    validate(args.binary.resolve())
    print("validated R-914 package catalog Git flow")


if __name__ == "__main__":
    main()
