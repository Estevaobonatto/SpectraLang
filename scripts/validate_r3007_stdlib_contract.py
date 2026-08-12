#!/usr/bin/env python3
"""Executable production-contract audit for SpectraLang stdlib surfaces."""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import subprocess
import sys
import tomllib
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
REPORT_SCHEMA = "spectralang.r3007_stdlib_contract.v1"
ALLOWED_CLASSIFICATIONS = {"production", "baseline", "simulation", "unsupported", "incomplete"}
ALLOWED_OWNERS = {"frontend", "semantic", "midend", "backend", "runtime", "numerics", "ml", "web", "db", "tooling", "ecosystem"}
SOURCE_KEYS = ("semantic", "runtime", "api_runtime", "lowering", "backend")
SYMBOL_RE = re.compile(r"(?:std|spectra\.std|spectra\.api)\.[A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)*")
FULL_DECL_RE = re.compile(r'\(\s*"((?:std|spectra\.std|spectra\.api)\.[A-Za-z0-9_.]+)"\s*,\s*"([^"]+)"')
STRING_PATH_RE = re.compile(r'"((?:std|spectra\.std|spectra\.api)\.[A-Za-z0-9_.]+)"')
SIGNAL_RE = re.compile(r"\b(?:TODO|FIXME|unimplemented|placeholder|mock|stub|simulat(?:e|ed|ion)|reserved but not implemented)\b", re.IGNORECASE)


@dataclass
class SymbolEvidence:
    kind: str = "function"
    paths: set[str] = field(default_factory=set)
    sources: set[str] = field(default_factory=set)
    semantic_declared: bool = False
    runtime_registered: bool = False
    lowering_modes: set[str] = field(default_factory=set)
    backend_special_path: bool = False
    documentation_refs: set[str] = field(default_factory=set)
    probe_refs: set[str] = field(default_factory=set)


@dataclass
class SourceInventory:
    symbols: dict[str, SymbolEvidence]
    files: dict[str, list[str]]
    signals: list[dict[str, Any]]
    generic_lowering: bool = False
    api_lowering: bool = False


def canonical_symbol(value: str) -> str:
    value = value.rstrip(".,;:()[]{}\"'")
    if value.startswith("spectra.std."):
        return "std." + value[len("spectra.std.") :]
    if value.startswith("spectra.api."):
        return "std.api." + value[len("spectra.api.") :]
    return value


def strip_rust_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    return re.sub(r"//.*", "", text)


def source_paths(root: Path, configured: list[str]) -> list[Path]:
    paths: list[Path] = []
    for raw in configured:
        path = root / raw
        if path.is_dir():
            paths.extend(sorted(path.rglob("*.rs")))
        elif path.is_file():
            paths.append(path)
    return paths


def add_symbol(inventory: dict[str, SymbolEvidence], raw: str, source: str, *, kind: str = "function", mode: str | None = None) -> None:
    symbol = canonical_symbol(raw)
    if symbol.count(".") < 2:
        return
    evidence = inventory.setdefault(symbol, SymbolEvidence(kind=kind))
    if evidence.kind == "function" and kind != "function":
        evidence.kind = kind
    evidence.sources.add(source)
    evidence.paths.add(symbol)
    if source == "semantic":
        evidence.semantic_declared = True
    elif source in {"runtime", "api_runtime"}:
        evidence.runtime_registered = True
    if mode:
        evidence.lowering_modes.add(mode)
    if source == "backend":
        evidence.backend_special_path = True


def matching_brace(text: str, opening: int) -> int:
    depth = 0
    for index in range(opening, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return index
    return len(text)


def semantic_inventory(text: str, symbols: dict[str, SymbolEvidence]) -> None:
    clean = strip_rust_comments(text)
    for match in re.finditer(r'register_module\("((?:std|spectra\.std|spectra\.api)[^"]+)"', clean):
        add_symbol(symbols, match.group(1), "semantic", kind="module")
    for match in FULL_DECL_RE.finditer(clean):
        kind = "type" if match.group(2).startswith(("struct", "record", "enum", "type")) else "function"
        add_symbol(symbols, match.group(1), "semantic", kind=kind)
    for match in re.finditer(r"fn\s+make_std_([a-z0-9_]+)\s*\([^)]*\)\s*(?:->\s*[^{]+)?\{", clean):
        name = match.group(1)
        module = "std.api." + name[4:] if name.startswith("api_") else "std." + name
        body = clean[match.end() : matching_brace(clean, clean.find("{", match.start()))]
        for function in set(re.findall(r'\.functions\.insert\(\s*"([A-Za-z_][A-Za-z0-9_]*)"', body)):
            add_symbol(symbols, f"{module}.{function}", "semantic")
        for function in set(re.findall(r'"([A-Za-z_][A-Za-z0-9_]*)"\.to_string\(\),\s*pub_fn', body)):
            add_symbol(symbols, f"{module}.{function}", "semantic")
        for function in set(re.findall(r'\(\s*"([A-Za-z_][A-Za-z0-9_]*)"\s*,\s*vec!', body)):
            add_symbol(symbols, f"{module}.{function}", "semantic")
        for type_name in re.findall(r'exports\.types\.insert\(\s*"([A-Za-z_][A-Za-z0-9_]*)"', body):
            add_symbol(symbols, f"{module}.{type_name}", "semantic", kind="type")


def runtime_inventory(text: str, source: str, symbols: dict[str, SymbolEvidence]) -> None:
    clean = strip_rust_comments(text)
    constants: dict[str, str] = {}
    for name, path in re.findall(r'const\s+([A-Z][A-Z0-9_]*)\s*:\s*&str\s*=\s*"((?:spectra\.std|spectra\.api)\.[^"]+)"', clean):
        constants[name] = path
    registered = set(re.findall(r"register_host_function\(\s*([A-Z][A-Z0-9_]*)\s*,", clean))
    for name in registered:
        if name in constants:
            add_symbol(symbols, constants[name], source)
    for path in STRING_PATH_RE.findall(clean):
        if source == "api_runtime" and re.search(r"name\s*:\s*\"" + re.escape(path), clean):
            add_symbol(symbols, path, source)


def lowering_inventory(text: str, symbols: dict[str, SymbolEvidence]) -> tuple[bool, bool]:
    clean = strip_rust_comments(text)
    in_api = "fn lookup_std_api_host_function" in clean
    for path in STRING_PATH_RE.findall(clean):
        add_symbol(symbols, path, "lowering", mode="api_external_lowering" if path.startswith(("std.api.", "spectra.api.")) else "explicit_lowering")
    return "lookup_std_host_function" in clean, in_api


def backend_inventory(text: str, symbols: dict[str, SymbolEvidence]) -> None:
    for path in STRING_PATH_RE.findall(strip_rust_comments(text)):
        add_symbol(symbols, path, "backend")


def discover_sources(root: Path, manifest: dict[str, Any]) -> SourceInventory:
    symbols: dict[str, SymbolEvidence] = {}
    files: dict[str, list[str]] = {}
    signals: list[dict[str, Any]] = []
    generic_lowering = False
    api_lowering = False
    for category in SOURCE_KEYS:
        paths = source_paths(root, manifest.get("sources", {}).get(category, []))
        files[category] = [str(path.relative_to(root)) for path in paths]
        for path in paths:
            text = path.read_text(encoding="utf-8")
            if category == "semantic":
                semantic_inventory(text, symbols)
            elif category in {"runtime", "api_runtime"}:
                runtime_inventory(text, category, symbols)
            elif category == "lowering":
                generic, api = lowering_inventory(text, symbols)
                generic_lowering = generic_lowering or generic
                api_lowering = api_lowering or api
            else:
                backend_inventory(text, symbols)
            for line_number, line in enumerate(text.splitlines(), start=1):
                if SIGNAL_RE.search(line):
                    signals.append({"category": category, "path": str(path.relative_to(root)), "line": line_number, "symbols": sorted({canonical_symbol(x) for x in SYMBOL_RE.findall(line)}), "text": line.strip()})
    if generic_lowering:
        for symbol, evidence in symbols.items():
            if evidence.semantic_declared and evidence.kind == "function" and not evidence.lowering_modes:
                evidence.lowering_modes.add("generic_lowering")
    if api_lowering:
        for symbol, evidence in symbols.items():
            if symbol.startswith("std.api.") and evidence.semantic_declared and not evidence.lowering_modes:
                evidence.lowering_modes.add("api_external_lowering")
    return SourceInventory(symbols, files, signals, generic_lowering, api_lowering)


def load_roadmap_ids(root: Path) -> set[str]:
    data = tomllib.loads((root / "roadmap" / "roadmap.toml").read_text(encoding="utf-8"))
    return {item["id"] for item in data.get("items", []) if "id" in item}


def validate_manifest(root: Path, manifest: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if manifest.get("schema") != REPORT_SCHEMA:
        errors.append(f"manifest schema must be {REPORT_SCHEMA}")
    if not manifest.get("contract_version"):
        errors.append("manifest contract_version is required")
    if not manifest.get("namespace"):
        errors.append("manifest must declare namespaces")
    roadmap_ids = load_roadmap_ids(root)
    prefixes: list[str] = []
    for namespace in manifest.get("namespace", []):
        prefix = namespace.get("prefix", "")
        prefixes.append(prefix)
        if not prefix.startswith("std."):
            errors.append(f"invalid namespace prefix: {prefix}")
        if namespace.get("owner") not in ALLOWED_OWNERS:
            errors.append(f"namespace {prefix} has invalid owner")
        if namespace.get("classification") not in ALLOWED_CLASSIFICATIONS:
            errors.append(f"namespace {prefix} has invalid classification")
        if namespace.get("roadmap") not in roadmap_ids:
            errors.append(f"namespace {prefix} references missing roadmap item {namespace.get('roadmap')}")
        if namespace.get("classification") != "production" and not namespace.get("reason"):
            errors.append(f"non-production namespace {prefix} requires reason")
    if len(prefixes) != len(set(prefixes)):
        errors.append("manifest contains duplicate namespace prefixes")
    rule_prefixes: list[str] = []
    for rule in manifest.get("rule", []):
        prefix = rule.get("prefix", "")
        rule_prefixes.append(prefix)
        if not prefix.startswith("std.") or rule.get("classification") not in ALLOWED_CLASSIFICATIONS:
            errors.append(f"rule {prefix} has invalid prefix or classification")
        if rule.get("roadmap") not in roadmap_ids:
            errors.append(f"rule {prefix} references missing roadmap item {rule.get('roadmap')}")
        if rule.get("owner") and rule["owner"] not in ALLOWED_OWNERS:
            errors.append(f"rule {prefix} has invalid owner")
        if not rule.get("reason"):
            errors.append(f"rule {prefix} requires reason")
    if len(rule_prefixes) != len(set(rule_prefixes)):
        errors.append("manifest contains duplicate rule prefixes")
    for follow_up in manifest.get("follow_up", []):
        if follow_up.get("owner") not in ALLOWED_OWNERS:
            errors.append(f"follow-up {follow_up.get('prefix')} has invalid owner")
        if follow_up.get("roadmap") not in roadmap_ids:
            errors.append(f"follow-up {follow_up.get('prefix')} references missing roadmap item")
        if not follow_up.get("reason"):
            errors.append(f"follow-up {follow_up.get('prefix')} requires reason")
    for path in manifest.get("docs", []):
        if not (root / path).is_file():
            errors.append(f"documentation path is missing: {path}")
    for category in SOURCE_KEYS:
        if not source_paths(root, manifest.get("sources", {}).get(category, [])):
            errors.append(f"source category has no files: {category}")
    probes = manifest.get("probe", [])
    if not probes:
        errors.append("manifest must declare probes")
    probe_ids: list[str] = []
    for probe in probes:
        probe_ids.append(probe.get("id", ""))
        if not probe.get("id") or not probe.get("covers"):
            errors.append("probe requires id and covers")
        if probe.get("kind", "spectra") not in {"spectra", "external"}:
            errors.append(f"probe {probe.get('id')} has invalid kind")
        if probe.get("kind", "spectra") == "spectra" and not probe.get("path"):
            errors.append(f"probe {probe.get('id')} requires path")
        if probe.get("kind", "spectra") == "external" and not probe.get("command"):
            errors.append(f"external probe {probe.get('id')} requires command")
        if probe.get("path") and not (root / probe["path"]).is_file():
            errors.append(f"probe path is missing: {probe.get('path')}")
    if len(probe_ids) != len(set(probe_ids)):
        errors.append("manifest contains duplicate probe ids")
    return errors


def matching_contract(symbol: str, manifest: dict[str, Any]) -> dict[str, Any] | None:
    namespaces = [x for x in manifest.get("namespace", []) if symbol == x.get("prefix") or symbol.startswith(x.get("prefix", "") + ".")]
    if not namespaces:
        return None
    result = dict(max(namespaces, key=lambda x: len(x["prefix"])))
    rules = [x for x in manifest.get("rule", []) if symbol == x.get("prefix") or symbol.startswith(x.get("prefix", "") + ".")]
    if rules:
        result.update(max(rules, key=lambda x: len(x["prefix"])))
    return result


def classification_for(symbol: str, manifest: dict[str, Any]) -> dict[str, Any] | None:
    """Compatibility helper used by focused tests and downstream tooling."""
    return matching_contract(symbol, manifest)


def follow_up_for(symbol: str, manifest: dict[str, Any]) -> dict[str, Any] | None:
    matches = [x for x in manifest.get("follow_up", []) if symbol == x.get("prefix") or symbol.startswith(x.get("prefix", "") + ".")]
    return max(matches, key=lambda x: len(x["prefix"]), default=None)


def documentation_refs(root: Path, symbol: str, docs: list[str]) -> list[str]:
    aliases = {symbol, symbol.replace("std.api", "spectra.api")}
    namespace = ".".join(symbol.split(".")[:2])
    aliases.add(namespace)
    aliases.add(namespace.replace("std.api", "spectra.api"))
    refs: list[str] = []
    for raw in docs:
        text = (root / raw).read_text(encoding="utf-8")
        if any(alias in text for alias in aliases):
            refs.append(raw)
    return refs


def probe_matches(symbol: str, manifest: dict[str, Any]) -> list[dict[str, Any]]:
    return [probe for probe in manifest.get("probe", []) if any(fnmatch.fnmatch(symbol, pattern) for pattern in probe.get("covers", []))]


def run_probe(root: Path, binary: Path, probe: dict[str, Any], timeout_seconds: int) -> dict[str, Any]:
    if probe.get("kind", "spectra") == "external":
        command = [str(value) for value in probe["command"]]
    else:
        command = [str(binary), "run", probe["path"]]
    try:
        completed = subprocess.run(command, cwd=root, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout_seconds, check=False)
    except subprocess.TimeoutExpired as exc:
        output = exc.stdout if isinstance(exc.stdout, str) else ""
        return {"id": probe["id"], "kind": probe.get("kind", "spectra"), "path": probe.get("path"), "status": "timeout", "exit_code": None, "command": command, "output_tail": "\n".join(output.splitlines()[-20:])}
    return {"id": probe["id"], "kind": probe.get("kind", "spectra"), "path": probe.get("path"), "status": "passed" if completed.returncode == 0 else "failed", "exit_code": completed.returncode, "command": command, "output_tail": "\n".join((completed.stdout or "").splitlines()[-20:])}


def build_report(root: Path, manifest: dict[str, Any], inventory: SourceInventory, probe_results: list[dict[str, Any]]) -> dict[str, Any]:
    blockers: list[dict[str, Any]] = []
    warnings: list[dict[str, Any]] = []
    covered: dict[str, dict[str, Any]] = {}
    result_by_id = {result["id"]: result for result in probe_results}
    for symbol, evidence in sorted(inventory.symbols.items()):
        contract = matching_contract(symbol, manifest)
        if contract is None:
            blockers.append({"kind": "unclassified_symbol", "symbol": symbol, "sources": sorted(evidence.sources)})
            continue
        docs = documentation_refs(root, symbol, manifest.get("docs", []))
        probes = probe_matches(symbol, manifest)
        probe_ids = [probe["id"] for probe in probes]
        covered[symbol] = {
            "path": symbol,
            "kind": evidence.kind,
            "classification": contract["classification"],
            "owner": contract["owner"],
            "roadmap": contract["roadmap"],
            "sources": sorted(evidence.sources),
            "semantic_declared": evidence.semantic_declared,
            "runtime_registered": evidence.runtime_registered,
            "lowering_registered": bool(evidence.lowering_modes),
            "lowering_modes": sorted(evidence.lowering_modes),
            "backend_special_path": evidence.backend_special_path,
            "documentation_refs": docs,
            "probe_refs": probe_ids,
            "probe_paths": [probe.get("path") or " ".join(probe.get("command", [])) for probe in probes],
            "probe_status": {probe_id: result_by_id.get(probe_id, {}).get("status", "missing") for probe_id in probe_ids},
            "probe_coverage": bool(probes),
            "coverage_reason": contract.get("reason") if not probes else None,
            "reason": contract.get("reason"),
        }
        if contract["classification"] == "production" and not docs:
            blockers.append({"kind": "production_symbol_without_documentation", "symbol": symbol})
        if contract["classification"] == "production" and not probes:
            blockers.append({"kind": "production_symbol_without_probe", "symbol": symbol})
        if contract["classification"] == "production" and any(result_by_id.get(probe_id, {}).get("status") != "passed" for probe_id in probe_ids):
            blockers.append({"kind": "production_probe_failed", "symbol": symbol, "probes": probe_ids})
        if contract["classification"] == "production" and any(signal_symbol == symbol for signal in inventory.signals for signal_symbol in signal["symbols"]):
            blockers.append({"kind": "production_implementation_signal", "symbol": symbol})

    semantic = {symbol for symbol, evidence in inventory.symbols.items() if evidence.semantic_declared and evidence.kind == "function"}
    runtime = {symbol for symbol, evidence in inventory.symbols.items() if evidence.runtime_registered}
    lowered = {symbol for symbol, evidence in inventory.symbols.items() if evidence.lowering_modes}
    runtime_without_semantic = sorted(runtime - semantic)
    semantic_without_runtime = sorted(semantic - runtime)
    semantic_without_lowering = sorted(semantic - lowered)
    lowering_without_runtime = sorted(lowered - runtime)
    divergences = {
        "semantic_without_runtime": semantic_without_runtime,
        "runtime_without_semantic": runtime_without_semantic,
        "semantic_without_lowering": semantic_without_lowering,
        "lowering_without_runtime": lowering_without_runtime,
        "backend_special_paths": sorted(symbol for symbol, evidence in inventory.symbols.items() if evidence.backend_special_path),
    }
    follow_ups: list[dict[str, Any]] = []
    for kind, symbols in (
        ("semantic_without_runtime", semantic_without_runtime),
        ("runtime_without_semantic", runtime_without_semantic),
        ("semantic_without_lowering", semantic_without_lowering),
        ("lowering_without_runtime", lowering_without_runtime),
    ):
        for symbol in symbols:
            follow_up = follow_up_for(symbol, manifest)
            if follow_up is None:
                blockers.append({"kind": "divergence_without_follow_up", "divergence": kind, "symbol": symbol})
            else:
                follow_ups.append({"kind": kind, "symbol": symbol, "owner": follow_up["owner"], "roadmap": follow_up["roadmap"], "reason": follow_up["reason"]})
    for signal in inventory.signals:
        warnings.append({"kind": "implementation_signal", **signal})
    counts = Counter(item["classification"] for item in covered.values())
    coverage_by_namespace: dict[str, Any] = {}
    for namespace in manifest.get("namespace", []):
        prefix = namespace["prefix"]
        symbols = [item for item in covered if item == prefix or item.startswith(prefix + ".")]
        covered_count = sum(bool(covered[item]["probe_coverage"]) for item in symbols)
        coverage_by_namespace[prefix] = {"symbol_count": len(symbols), "covered_symbols": covered_count, "uncovered_symbols": len(symbols) - covered_count, "classification": namespace["classification"]}
    production_symbols = [item for item in covered.values() if item["classification"] == "production"]
    return {
        "schema": REPORT_SCHEMA,
        "contract_version": manifest["contract_version"],
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "git_revision": git_revision(root),
        "manifest": "scripts/stdlib_contract.toml",
        "status": "passed" if not blockers else "failed",
        "passed": not blockers,
        "counts": dict(sorted(counts.items())),
        "source_files": inventory.files,
        "symbols": covered,
        "source_reconciliation": divergences,
        "probe_coverage": {"covered_symbols": sum(bool(item["probe_coverage"]) for item in covered.values()), "uncovered_symbols": sum(not item["probe_coverage"] for item in covered.values()), "coverage_by_namespace": coverage_by_namespace},
        "signals": inventory.signals,
        "warnings": warnings,
        "blockers": blockers,
        "follow_up_tasks": follow_ups,
        "probe_results": probe_results,
        "exceptions_applied": manifest.get("rule", []),
        "production_claims": {
            "production_symbol_count": len(production_symbols),
            "unproven_symbols": [item["path"] for item in production_symbols if not item["probe_coverage"] or any(status != "passed" for status in item["probe_status"].values())],
            "contradictory_symbols": [item["path"] for item in production_symbols if item["reason"] and any(word in item["reason"].lower() for word in ("simulat", "placeholder", "alias", "baseline"))],
        },
    }


def git_revision(root: Path) -> str:
    result = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=root, text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=False)
    return result.stdout.strip() if result.returncode == 0 else "unknown"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", default="scripts/stdlib_contract.toml")
    parser.add_argument("--binary", default="target/debug/spectralang.exe")
    parser.add_argument("--report", default="target/r3007-stdlib-contract/report.json")
    parser.add_argument("--timeout-seconds", type=int, default=45)
    args = parser.parse_args()
    manifest_path = Path(args.manifest)
    manifest_path = manifest_path if manifest_path.is_absolute() else ROOT / manifest_path
    if not manifest_path.is_file():
        print(f"R-3007 failure: manifest not found: {manifest_path}", file=sys.stderr)
        return 2
    try:
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        print(f"R-3007 failure: invalid manifest: {exc}", file=sys.stderr)
        return 2
    errors = validate_manifest(ROOT, manifest)
    if errors:
        for error in errors:
            print(f"R-3007 manifest error: {error}", file=sys.stderr)
        return 2
    inventory = discover_sources(ROOT, manifest)
    binary = Path(args.binary)
    binary = binary if binary.is_absolute() else ROOT / binary
    probe_results = []
    for probe in manifest["probe"]:
        if binary.is_file() or probe.get("kind", "spectra") == "external":
            probe_results.append(run_probe(ROOT, binary, probe, args.timeout_seconds))
        else:
            probe_results.append({"id": probe["id"], "kind": "spectra", "path": probe.get("path"), "status": "binary_missing", "exit_code": None, "command": []})
    report = build_report(ROOT, manifest, inventory, probe_results)
    report_path = Path(args.report)
    report_path = report_path if report_path.is_absolute() else ROOT / report_path
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"[R-3007] discovered {len(inventory.symbols)} typed symbols across {len(inventory.files)} source categories")
    print(f"[R-3007] classification counts: {json.dumps(report['counts'], sort_keys=True)}")
    print(f"[R-3007] probes: {json.dumps({x['id']: x['status'] for x in probe_results}, sort_keys=True)}")
    print(f"[R-3007] tracked follow-ups: {len(report['follow_up_tasks'])}")
    print(f"[R-3007] blockers: {len(report['blockers'])}")
    print(f"[R-3007] wrote {report_path}")
    for blocker in report["blockers"][:20]:
        print(f"[R-3007] blocker: {json.dumps(blocker, sort_keys=True)}", file=sys.stderr)
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
