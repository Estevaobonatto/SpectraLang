#!/usr/bin/env python3
"""Capture and validate the real Phase 31 profiling evidence.

The official capture environment is Linux/WSL2. This module deliberately
never changes the Phase 31 baseline; it only writes profiling artifacts and
metadata under the selected output directory.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys
from typing import Any, Iterable

try:
    from scripts.phase31_contract import SCENARIOS
except ModuleNotFoundError:
    from phase31_contract import SCENARIOS  # type: ignore[no-redef]


PROFILE_SCENARIOS = (
    "cpu-loop-sum", "cpu-fibs", "cpu-string-build", "cpu-hashmap",
    "tensor-create", "tensor-elementwise", "tensor-reduce", "tensor-matmul",
)
LANGUAGES = ("spectra", "go", "rust")
REQUIRED_ARTIFACTS = (
    "spectra.flamegraph.svg", "spectra.perf-report.txt",
    "spectra.ir-before.txt", "spectra.ir-after.txt", "spectra.pipeline.txt",
    "go.perf-report.txt", "rust.perf-report.txt", "metadata.json",
)
DEFAULT_PROFILE_ROOT = Path("docs/performance/phase31-go-comparable/profiles")
SCHEMA = "spectra.phase31.profile.v1"


class ProfileError(RuntimeError):
    """A deterministic, user-facing profiling error."""


def run_command(command: list[str], cwd: Path, timeout: int = 60) -> tuple[int, str]:
    try:
        proc = subprocess.run(command, cwd=cwd, capture_output=True, text=True,
                              timeout=timeout, check=False)
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 127, str(exc)
    output = ((proc.stdout or "") + "\n" + (proc.stderr or "")).strip()
    return proc.returncode, output[-8000:]


def git_revision(root: Path) -> str:
    code, output = run_command(["git", "rev-parse", "HEAD"], root)
    if code != 0:
        raise ProfileError(f"unable to resolve Git revision: {output}")
    return output.splitlines()[-1].strip()


def tool_version(tool: str) -> str | None:
    executable = shutil.which(tool)
    if not executable:
        return None
    code, output = run_command([executable, "--version"], Path.cwd())
    return output.splitlines()[0] if code == 0 and output else executable


def required_tools(backend: str) -> tuple[str, ...]:
    if backend == "perf":
        return ("perf", "cargo", "go", "rustc", "stackcollapse-perf.pl", "flamegraph.pl")
    return ("valgrind", "callgrind_annotate", "cargo", "go", "rustc", "dot")


def preflight(backend: str) -> dict[str, Any]:
    missing = [tool for tool in required_tools(backend) if shutil.which(tool) is None]
    return {"platform": platform.platform(), "system": platform.system(),
            "backend": backend, "required_tools": list(required_tools(backend)),
            "missing_tools": missing,
            "ready": not missing and platform.system() == "Linux"}


def ensure_spectra_binary(binary: Path, profile: str) -> None:
    if not binary.is_file():
        raise ProfileError(f"Spectra binary does not exist: {binary}")
    normalized = str(binary).replace("\\", "/")
    if profile == "release" and "/debug/" in normalized:
        raise ProfileError("release profiling requires a release binary, not target/debug")
    if profile == "debug" and "/release/" in normalized:
        raise ProfileError("debug profiling requires a debug binary, not target/release")


def scenario_source(root: Path, scenario: str) -> Path:
    source = root / "benchmarks" / "cross-lang" / scenario / "spectra" / "bench.spectra"
    if not source.is_file():
        raise ProfileError(f"missing Spectra fixture for {scenario}: {source}")
    return source


def artifact_paths(root: Path, scenario: str) -> dict[str, Path]:
    directory = root / DEFAULT_PROFILE_ROOT / scenario
    return {name: directory / name for name in REQUIRED_ARTIFACTS}


def validate_artifacts(root: Path, scenarios: Iterable[str]) -> list[str]:
    errors: list[str] = []
    for scenario in scenarios:
        if scenario not in PROFILE_SCENARIOS or scenario not in SCENARIOS:
            errors.append(f"unsupported profiling scenario: {scenario}")
            continue
        paths = artifact_paths(root, scenario)
        for name, path in paths.items():
            if not path.is_file() or path.stat().st_size == 0:
                errors.append(f"{scenario}: missing or empty {name}")
        metadata = paths["metadata.json"]
        if metadata.is_file():
            try:
                payload = json.loads(metadata.read_text(encoding="utf-8"))
            except json.JSONDecodeError as exc:
                errors.append(f"{scenario}: invalid metadata JSON: {exc}")
            else:
                if payload.get("schema") != SCHEMA:
                    errors.append(f"{scenario}: metadata schema must be {SCHEMA}")
                if payload.get("scenario") != scenario:
                    errors.append(f"{scenario}: metadata scenario mismatch")
                if payload.get("baseline_modified") is not False:
                    errors.append(f"{scenario}: metadata must state baseline_modified=false")
    return errors


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")


def capture_perf_summary(tool: str, root: Path, output_dir: Path,
                         label: str, command: list[str]) -> tuple[list[str], str]:
    data_file = output_dir / f"{label}.perf.data"
    record_command = [tool, "record", "-F", "99", "-g", "-o", str(data_file), "--", *command]
    code, output = run_command(record_command, root, timeout=300)
    if code != 0:
        raise ProfileError(f"perf record failed for {label}: {output[-1000:]}")
    code, report = run_command([tool, "report", "--stdio", "-i", str(data_file)], root, timeout=120)
    if code != 0:
        raise ProfileError(f"perf report failed for {label}: {report[-1000:]}")
    return record_command, report


def capture_reference(root: Path, scenario: str, language: str,
                      output_dir: Path, perf: str) -> tuple[list[str], str]:
    source = root / "benchmarks" / "cross-lang" / scenario / language
    if language == "go":
        source_file = source / "bench.go"
        binary = output_dir / "go.bench"
        code, output = run_command(["go", "build", "-o", str(binary), str(source_file)], root, timeout=300)
    else:
        source_file = source / "bench.rs"
        binary = output_dir / "rust.bench"
        code, output = run_command(["rustc", "-O", "-o", str(binary), str(source_file)], root, timeout=300)
    if code != 0:
        raise ProfileError(f"{language} build failed for {scenario}: {output[-1000:]}")
    return capture_perf_summary(perf, root, output_dir, language, [str(binary)])


def capture_spectra(root: Path, binary: Path, scenario: str, profile: str, backend: str) -> dict[str, Any]:
    paths = artifact_paths(root, scenario)
    source = scenario_source(root, scenario)
    paths["metadata.json"].parent.mkdir(parents=True, exist_ok=True)
    if backend != "perf":
        raise ProfileError("capture currently requires backend=perf; Callgrind remains validation-compatible")
    tool = shutil.which("perf")
    collapse = shutil.which("stackcollapse-perf.pl")
    flamegraph = shutil.which("flamegraph.pl")
    if not tool or not collapse or not flamegraph:
        raise ProfileError("perf capture requires perf, stackcollapse-perf.pl, and flamegraph.pl")

    command = [str(binary), "run", str(source)]
    output_dir = paths["metadata.json"].parent
    perf_record, report = capture_perf_summary(tool, root, output_dir, "spectra", command)
    code, pipeline = run_command([str(binary), "compile", "--timings", str(source)], root, timeout=300)
    write_text(paths["spectra.pipeline.txt"], pipeline)
    if code != 0:
        raise ProfileError(f"pipeline timing failed for {scenario}: {pipeline[-1000:]}")
    write_text(paths["spectra.perf-report.txt"], report)
    code, folded = run_command([collapse, str(data_file)], root, timeout=120)
    write_text(folded_file, folded)
    if code != 0:
        raise ProfileError(f"stackcollapse failed for {scenario}: {folded[-1000:]}")
    code, svg = run_command([flamegraph, str(folded_file)], root, timeout=120)
    write_text(paths["spectra.flamegraph.svg"], svg)
    if code != 0:
        raise ProfileError(f"flamegraph generation failed for {scenario}: {svg[-1000:]}")

    ir_commands = {
        "spectra.ir-before.txt": [str(binary), "compile", "--dump-ir", "-O0", str(source)],
        "spectra.ir-after.txt": [str(binary), "compile", "--dump-ir", "-O3", str(source)],
    }
    for name, ir_command in ir_commands.items():
        code, ir = run_command(ir_command, root, timeout=300)
        write_text(paths[name], ir)
        if code != 0:
            raise ProfileError(f"IR dump failed for {scenario}: {ir[-1000:]}")
    reference_commands: dict[str, list[str]] = {}
    for language in ("go", "rust"):
        record_command, reference_report = capture_reference(root, scenario, language, output_dir, tool)
        reference_commands[language] = record_command
        write_text(paths[f"{language}.perf-report.txt"], reference_report)
    metadata = {
        "schema": SCHEMA, "scenario": scenario, "profile": profile,
        "binary": str(binary), "revision": git_revision(root),
        "host": platform.node(), "backend": backend,
        "tools": {name: tool_version(name) for name in required_tools(backend)},
        "commands": {"profile": perf_record, **reference_commands, **{name: cmd for name, cmd in ir_commands.items()}},
        "baseline_modified": False,
    }
    paths["metadata.json"].write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    return metadata


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("preflight", "capture", "validate"))
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--backend", choices=("perf", "callgrind"), default="perf")
    parser.add_argument("--profile", choices=("debug", "release"), default="release")
    parser.add_argument("--spectra-binary", type=Path)
    parser.add_argument("--scenarios", nargs="+", default=list(PROFILE_SCENARIOS))
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    root = args.root.resolve()
    if args.command == "preflight":
        state = preflight(args.backend)
        print(json.dumps(state, indent=2))
        return 0 if state["ready"] else 2
    if args.command == "validate":
        errors = validate_artifacts(root, args.scenarios)
        if errors:
            print("PROFILE VALIDATION FAILED")
            print("\n".join(f"- {error}" for error in errors))
            return 1
        print(f"PROFILE VALIDATION PASSED ({len(args.scenarios)} scenarios)")
        return 0
    if not args.spectra_binary:
        print("capture requires --spectra-binary", file=sys.stderr)
        return 2
    try:
        ensure_spectra_binary(args.spectra_binary, args.profile)
        state = preflight(args.backend)
        if not state["ready"]:
            raise ProfileError("profiling environment is not ready: " + ", ".join(state["missing_tools"]))
        for scenario in args.scenarios:
            capture_spectra(root, args.spectra_binary.resolve(), scenario, args.profile, args.backend)
            print(f"captured {scenario}")
    except ProfileError as exc:
        print(f"PROFILE BLOCKED: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
