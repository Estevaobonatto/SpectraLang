#!/usr/bin/env python3
"""Generate deterministic O0/O3 IR evidence for Phase 31 tasks.

The original R-3103 entry point remains compatible; ``--task r3104`` only
changes the manifest schema and output label so the same deterministic
generator can be reused by the next optimization task.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from typing import Any

try:
    from scripts.phase31_contract import SCENARIOS
except ModuleNotFoundError:  # pragma: no cover - direct script execution
    from phase31_contract import SCENARIOS  # type: ignore[no-redef]


ROOT = Path(__file__).resolve().parents[1]
MANIFEST_SCHEMA = "spectra.phase31.r3103_ir_manifest.v1"
IR_OPTIONS = {
    "o0": ["compile", "--dump-ir", "-O0"],
    "o3": ["compile", "--dump-ir", "-O3"],
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def file_record(path: Path, relative_path: str | None = None) -> dict[str, Any]:
    return {
        "path": relative_path or path.name,
        "sha256": sha256_file(path),
        "bytes": path.stat().st_size,
    }


def manifest_payload(
    *,
    root: Path,
    binary: Path,
    output_root: Path,
    revision: str,
    files: dict[str, dict[str, dict[str, Any]]],
    task_id: str = "r3103",
) -> dict[str, Any]:
    binary_relative = binary.resolve().relative_to(root.resolve()).as_posix()
    return {
        "schema": f"spectra.phase31.{task_id}_ir_manifest.v1",
        "git_revision": revision,
        "profile": "release",
        "binary": binary_relative,
        "binary_sha256": sha256_file(binary),
        "benchmark_languages": ["spectra", "go", "rust"],
        "java_excluded": True,
        "options": IR_OPTIONS,
        "scenario_count": len(SCENARIOS),
        "scenarios": list(SCENARIOS),
        "files": files,
    }


def run_ir(binary: Path, source: Path, options: list[str], root: Path) -> str:
    command = [str(binary), *options, str(source)]
    try:
        result = subprocess.run(
            command,
            cwd=root,
            capture_output=True,
            text=True,
            timeout=300,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise RuntimeError(f"IR command failed for {source}: {exc}") from exc
    output = (result.stdout or "")
    if result.stderr:
        output += result.stderr
    if result.returncode != 0:
        raise RuntimeError(
            f"IR command failed for {source} ({' '.join(options)}), "
            f"exit={result.returncode}: {output[-2000:]}"
        )
    if not output.strip():
        raise RuntimeError(f"IR command returned empty output for {source} ({' '.join(options)})")
    return output


def generate(
    *, root: Path, binary: Path, output_root: Path, task_id: str = "r3103"
) -> dict[str, Any]:
    binary = binary.resolve()
    output_root = output_root.resolve()
    if not binary.is_file():
        raise RuntimeError(f"release binary does not exist: {binary}")
    normalized_binary = binary.as_posix().lower()
    if "/target/release/spectralang.exe" not in normalized_binary:
        raise RuntimeError("R-3103 IR generation requires target/release/spectralang.exe")

    revision_result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    if revision_result.returncode != 0 or not revision_result.stdout.strip():
        raise RuntimeError("unable to resolve current Git revision")
    revision = revision_result.stdout.strip()

    generated: dict[str, dict[str, dict[str, Any]]] = {}
    for scenario in SCENARIOS:
        source = root / "benchmarks" / "cross-lang" / scenario / "spectra" / "bench.spectra"
        if not source.is_file():
            raise RuntimeError(f"missing Spectra fixture for {scenario}: {source}")
        scenario_root = output_root / scenario
        scenario_root.mkdir(parents=True, exist_ok=True)
        scenario_files: dict[str, dict[str, Any]] = {}
        for level, options in IR_OPTIONS.items():
            output_path = scenario_root / f"{level}.txt"
            output_path.write_text(run_ir(binary, source, options, root), encoding="utf-8", newline="\n")
            scenario_files[level] = file_record(
                output_path,
                f"{scenario}/{level}.txt",
            )
        generated[scenario] = scenario_files

    manifest = manifest_payload(
        root=root,
        binary=binary,
        output_root=output_root,
        revision=revision,
        files=generated,
        task_id=task_id,
    )
    manifest_path = output_root / "manifest.json"
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8", newline="\n")
    return manifest


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=Path("target/phase31/r3103-ir"))
    parser.add_argument("--task", choices=("r3103", "r3104"), default="r3103")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        manifest = generate(
            root=ROOT,
            binary=args.binary,
            output_root=args.out,
            task_id=args.task,
        )
    except (OSError, RuntimeError, ValueError) as exc:
        print(f"{args.task.upper()} IR generation: BLOCKED: {exc}", file=sys.stderr)
        return 1
    print(
        f"{args.task.upper()} IR generated: {manifest['scenario_count']} scenarios, "
        f"revision {manifest['git_revision']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
