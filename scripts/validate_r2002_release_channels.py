#!/usr/bin/env python3
"""Validate R-2002 production release channel metadata."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
from pathlib import Path


def run_command(args: list[str], cwd: Path, timeout: int = 20) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
    )


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def write_project(root: Path) -> None:
    (root / "src").mkdir(parents=True)
    (root / "src" / "main.spectra").write_text(
        "module release_channel_demo;\n\npub fn main() -> int {\n    return 0;\n}\n",
        encoding="utf-8",
    )
    (root / "spectra.toml").write_text(
        "\n".join(
            [
                "[project]",
                'name = "release_channel_demo"',
                'version = "0.2.0-beta.1"',
                'entry = "src/main.spectra"',
                'src_dirs = ["src"]',
                "",
                "[release]",
                'channel = "beta"',
                'compatibility = "spectralang-0.1"',
                'deprecated_since = "0.3.0"',
                'migration = "Use release_channel_demo_v2 before stable promotion."',
                "",
                "[dependencies]",
                "",
            ]
        ),
        encoding="utf-8",
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--root", default=".")
    args = parser.parse_args()

    repo = Path(args.root).resolve()
    binary = Path(args.binary).resolve()
    work = repo / "target" / "r2002-release-channels"
    project = work / "package"
    registry = work / "registry"
    scaffold = work / "scaffold"

    if work.exists():
        shutil.rmtree(work)
    project.mkdir(parents=True)
    registry.mkdir(parents=True)
    write_project(project)

    info = run_command([str(binary), "release-info", "--json", "--root", str(project)], repo)
    require(info.returncode == 0, f"release-info failed:\n{info.stdout}")
    require(
        "warning[release-deprecated]" in info.stdout
        and "release_channel_demo_v2" in info.stdout,
        f"missing deprecation warning and migration guidance:\n{info.stdout}",
    )
    payload_start = info.stdout.find("{")
    require(payload_start >= 0, f"missing release-info JSON:\n{info.stdout}")
    payload = json.loads(info.stdout[payload_start:])
    require(payload["schema"] == "spectralang.release-info.v1", "bad release-info schema")
    require(payload["cli"]["channel"] in {"nightly", "beta", "stable"}, "bad CLI channel")
    require(payload["cli"]["compatibility"] == "spectralang-0.1", "bad CLI compatibility")
    package = payload["packages"][0]
    require(package["channel"] == "beta", "package channel missing from release-info")
    require(package["compatibility"] == "spectralang-0.1", "package compatibility missing")
    require(package["deprecated_since"] == "0.3.0", "package deprecation missing")
    require("release_channel_demo_v2" in package["migration"], "migration missing")

    lock = run_command([str(binary), "package", "lock", "--root", str(project)], repo)
    require(lock.returncode == 0, f"package lock failed:\n{lock.stdout}")
    require("warning[release-deprecated]" in lock.stdout, "package lock did not warn")
    lock_text = (project / "spectra.lock").read_text(encoding="utf-8")
    require('channel = "beta"' in lock_text, "lockfile missing channel")
    require('compatibility = "spectralang-0.1"' in lock_text, "lockfile missing compatibility")
    require('deprecated_since = "0.3.0"' in lock_text, "lockfile missing deprecation")
    require("release_channel_demo_v2" in lock_text, "lockfile missing migration")

    publish = run_command(
        [str(binary), "package", "publish", "--root", str(project), "--registry", str(registry)],
        repo,
    )
    require(publish.returncode == 0, f"package publish failed:\n{publish.stdout}")
    metadata = registry / "release_channel_demo" / "0.2.0-beta.1" / "package.toml"
    metadata_text = metadata.read_text(encoding="utf-8")
    require('channel = "beta"' in metadata_text, "registry metadata missing channel")
    require(
        'compatibility = "spectralang-0.1"' in metadata_text,
        "registry metadata missing compatibility",
    )

    new_project = run_command([str(binary), "new", str(scaffold)], repo)
    require(new_project.returncode == 0, f"new project failed:\n{new_project.stdout}")
    scaffold_manifest = (scaffold / "spectra.toml").read_text(encoding="utf-8")
    require("[release]" in scaffold_manifest, "scaffold manifest missing release section")
    require('channel = "nightly"' in scaffold_manifest, "scaffold missing nightly channel")

    shutil.rmtree(work, ignore_errors=True)
    print("validated R-2002 production release channels")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
