#!/usr/bin/env python3
"""Validate R-906 package-aware imports and semantic package boundaries."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BINARY = ROOT / "target" / "debug" / (
    "spectralang.exe" if os.name == "nt" else "spectralang"
)


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")


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
        raise RuntimeError(
            f"{' '.join(args)} returned {result.returncode}, expected {expect}:\n"
            f"{result.stdout}"
        )
    return result.stdout


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def package_manifest(name: str, members: list[str] | None = None) -> str:
    lines = [
        "[project]",
        f'name = "{name}"',
        'version = "0.1.0"',
        'entry = "src/main.spectra"',
        'src_dirs = ["src"]',
    ]
    if members:
        lines.extend(["", "[workspace]", "members = [" + ", ".join(f'"{m}"' for m in members) + "]"])
    return "\n".join(lines) + "\n"


def make_workspace(root: Path) -> None:
    write(root / "spectra.toml", package_manifest("consumer", ["lib", "base"]))
    write(
        root / "src/main.spectra",
        """module consumer.main;
import { public_value } from lib.core;

pub fn main() -> int {
    return public_value();
}
""",
    )
    write(
        root / "lib/spectra.toml",
        package_manifest("lib"),
    )
    write(
        root / "lib/src/core.spectra",
        """module lib.core;
import { base_value } from base.core;

internal fn secret() -> int {
    return base_value();
}

pub fn public_value() -> int {
    return secret();
}
""",
    )
    write(
        root / "lib/src/helper.spectra",
        """module lib.helper;
import { secret } from lib.core;

pub fn same_package_value() -> int {
    return secret();
}
""",
    )
    write(root / "base/spectra.toml", package_manifest("base"))
    write(root / "base/src/core.spectra", "module base.core;\npub fn base_value() -> int { return 41; }\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file():
        print(f"binary does not exist: {binary}", file=sys.stderr)
        return 1

    try:
        with tempfile.TemporaryDirectory(prefix="spectralang-r906-") as raw:
            work = Path(raw)
            workspace = work / "workspace"
            make_workspace(workspace)

            output = run(binary, ["package", "check", "--root", str(workspace)], workspace)
            require("lib.core" in output and "base.core" in output, "package check did not compile transitive modules")

            missing = work / "missing"
            shutil.copytree(workspace, missing)
            write(
                missing / "src/main.spectra",
                "module consumer.main;\nimport lib.missing;\n\nfn main() -> int { return 0; }\n",
            )
            output = run(binary, ["package", "check", "--root", str(missing)], missing, expect=74)
            require("lib.missing" in output, "missing import diagnostic omitted module")
            require("package 'lib' source:" in output, "missing import diagnostic omitted package source")
            require("package 'consumer'" in output, "missing import diagnostic omitted importer package")

            unknown = work / "unknown"
            shutil.copytree(workspace, unknown)
            write(
                unknown / "src/main.spectra",
                "module consumer.main;\nimport nowhere.missing;\n\nfn main() -> int { return 0; }\n",
            )
            output = run(binary, ["package", "check", "--root", str(unknown)], unknown, expect=74)
            require("nowhere.missing" in output, "unknown import diagnostic omitted module")

            duplicate = work / "duplicate"
            write(duplicate / "spectra.toml", package_manifest("consumer", ["left", "right"]))
            write(duplicate / "src/main.spectra", "module consumer.main;\n")
            write(duplicate / "left/spectra.toml", package_manifest("left"))
            write(duplicate / "left/src/shared.spectra", "module shared.same;\n")
            write(duplicate / "right/spectra.toml", package_manifest("right"))
            write(duplicate / "right/src/shared.spectra", "module shared.same;\n")
            output = run(binary, ["package", "check", "--root", str(duplicate)], duplicate, expect=74)
            require("shared.same" in output, "duplicate diagnostic omitted module")
            require("left" in output and "right" in output, "duplicate diagnostic omitted package names")
            require("root" in output, "duplicate diagnostic omitted package roots")

            cross = work / "cross"
            shutil.copytree(workspace, cross)
            write(
                cross / "src/main.spectra",
                """module consumer.main;
import { secret } from lib.core;

fn main() -> int { return secret(); }
""",
            )
            output = run(binary, ["package", "check", "--root", str(cross)], cross, expect=65)
            require("internal" in output and "different package" in output, "cross-package internal diagnostic missing")

        print("validated R-906 package imports")
        return 0
    except (OSError, subprocess.SubprocessError, RuntimeError) as exc:
        print(exc, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
