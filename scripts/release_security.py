#!/usr/bin/env python3
"""Release supply-chain helpers for SpectraLang.

This script creates and verifies release evidence:

- SHA-256 checksums for release artifacts.
- A minimal CycloneDX-compatible SBOM derived from Cargo.lock and npm lockfiles.
- Provenance metadata for traceability.
- HMAC-SHA256 signatures over the manifest.

Production release signing must provide SPECTRA_RELEASE_SIGNING_KEY. Local tests
may pass --allow-dev-key to use a deterministic non-secret test key.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import hmac
import json
import os
import platform
import re
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEV_SIGNING_KEY = b"spectralang-local-release-validation-key"


@dataclass(frozen=True)
class Artifact:
    path: Path
    sha256: str
    size: int


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def stable_json(data: Any) -> bytes:
    return json.dumps(data, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode(
        "utf-8"
    )


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def discover_artifacts(paths: list[Path]) -> list[Artifact]:
    artifacts: list[Artifact] = []
    for input_path in paths:
        path = input_path.resolve()
        if not path.exists():
            raise SystemExit(f"artifact path does not exist: {path}")
        files = [path] if path.is_file() else sorted(p for p in path.rglob("*") if p.is_file())
        for file_path in files:
            if file_path.name.endswith((".sha256", ".sig", ".provenance.json", ".sbom.json")):
                continue
            artifacts.append(
                Artifact(
                    path=file_path,
                    sha256=sha256_file(file_path),
                    size=file_path.stat().st_size,
                )
            )
    if not artifacts:
        raise SystemExit("no release artifacts found")
    return artifacts


def git_value(args: list[str], default: str = "unknown") -> str:
    try:
        completed = subprocess.run(
            ["git", *args],
            cwd=repo_root(),
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        value = completed.stdout.strip()
        return value or default
    except Exception:
        return default


def parse_cargo_lock(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    packages: list[dict[str, Any]] = []
    current: dict[str, str] = {}
    in_package = False
    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if stripped == "[[package]]":
            if current.get("name") and current.get("version"):
                packages.append(
                    {
                        "type": "library",
                        "name": current["name"],
                        "version": current["version"],
                        "purl": f"pkg:cargo/{current['name']}@{current['version']}",
                    }
                )
            current = {}
            in_package = True
            continue
        if not in_package or "=" not in stripped:
            continue
        key, value = stripped.split("=", 1)
        key = key.strip()
        value = value.strip().strip('"')
        if key in {"name", "version"}:
            current[key] = value
    if current.get("name") and current.get("version"):
        packages.append(
            {
                "type": "library",
                "name": current["name"],
                "version": current["version"],
                "purl": f"pkg:cargo/{current['name']}@{current['version']}",
            }
        )
    return packages


def parse_npm_lock(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    data = json.loads(path.read_text(encoding="utf-8"))
    packages = data.get("packages", {})
    components: list[dict[str, Any]] = []
    for package_path, meta in sorted(packages.items()):
        if not package_path.startswith("node_modules/"):
            continue
        name = package_path.removeprefix("node_modules/")
        version = meta.get("version")
        if not version:
            continue
        components.append(
            {
                "type": "library",
                "name": name,
                "version": version,
                "purl": f"pkg:npm/{name}@{version}",
            }
        )
    return components


def build_sbom() -> dict[str, Any]:
    root = repo_root()
    components = parse_cargo_lock(root / "Cargo.lock")
    components.extend(parse_npm_lock(root / "tools" / "vscode-extension" / "package-lock.json"))
    unique: dict[tuple[str, str, str], dict[str, Any]] = {}
    for component in components:
        unique[(component["purl"], component["name"], component["version"])] = component
    return {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:{uuid.uuid5(uuid.NAMESPACE_URL, 'spectralang-sbom')}",
        "version": 1,
        "metadata": {
            "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "component": {"type": "application", "name": "SpectraLang"},
        },
        "components": list(unique.values()),
    }


def signing_key(allow_dev_key: bool) -> bytes:
    env_key = os.environ.get("SPECTRA_RELEASE_SIGNING_KEY")
    if env_key:
        return env_key.encode("utf-8")
    if allow_dev_key:
        return DEV_SIGNING_KEY
    raise SystemExit(
        "SPECTRA_RELEASE_SIGNING_KEY is required for release signing "
        "(use --allow-dev-key only for local validation)"
    )


def sign_payload(payload: bytes, key: bytes) -> str:
    return base64.b64encode(hmac.new(key, payload, hashlib.sha256).digest()).decode("ascii")


def create(args: argparse.Namespace) -> int:
    root = repo_root()
    artifact_paths = [Path(p) for p in args.artifact]
    artifacts = discover_artifacts(artifact_paths)
    out_dir = Path(args.out).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    version = args.version or git_value(["describe", "--tags", "--always"])
    commit = git_value(["rev-parse", "HEAD"])
    dirty = git_value(["status", "--porcelain"], default="")
    manifest = {
        "schema": "spectralang.release-manifest.v1",
        "version": version,
        "commit": commit,
        "source_dirty": bool(dirty),
        "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "builder": {
            "os": platform.platform(),
            "python": platform.python_version(),
        },
        "artifacts": [
            {
                "path": str(artifact.path.relative_to(root) if artifact.path.is_relative_to(root) else artifact.path),
                "sha256": artifact.sha256,
                "size": artifact.size,
            }
            for artifact in artifacts
        ],
    }

    manifest_payload = stable_json(manifest)
    signature = {
        "schema": "spectralang.release-signature.v1",
        "algorithm": "HMAC-SHA256",
        "key_id": "env:SPECTRA_RELEASE_SIGNING_KEY"
        if os.environ.get("SPECTRA_RELEASE_SIGNING_KEY")
        else "dev-local-validation",
        "manifest_sha256": hashlib.sha256(manifest_payload).hexdigest(),
        "signature": sign_payload(manifest_payload, signing_key(args.allow_dev_key)),
    }
    provenance = {
        "schema": "spectralang.provenance.v1",
        "subject": manifest["artifacts"],
        "builder": manifest["builder"],
        "commit": commit,
        "version": version,
        "workflow": os.environ.get("GITHUB_WORKFLOW", "local"),
        "run_id": os.environ.get("GITHUB_RUN_ID", "local"),
        "run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT", "local"),
    }
    sbom = build_sbom()

    (out_dir / "release-manifest.json").write_bytes(manifest_payload + b"\n")
    (out_dir / "release-manifest.json.sig").write_text(
        json.dumps(signature, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (out_dir / "release-provenance.json").write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (out_dir / "release-sbom.cdx.json").write_text(
        json.dumps(sbom, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    checksum_lines = [
        f"{artifact.sha256}  {artifact.path.name}" for artifact in sorted(artifacts, key=lambda a: str(a.path))
    ]
    (out_dir / "SHA256SUMS").write_text("\n".join(checksum_lines) + "\n", encoding="utf-8")
    print(f"release evidence written to {out_dir}")
    print(f"artifacts: {len(artifacts)}")
    print(f"sbom components: {len(sbom['components'])}")
    return 0


def verify(args: argparse.Namespace) -> int:
    evidence = Path(args.evidence).resolve()
    manifest_path = evidence / "release-manifest.json"
    signature_path = evidence / "release-manifest.json.sig"
    checksums_path = evidence / "SHA256SUMS"
    sbom_path = evidence / "release-sbom.cdx.json"
    provenance_path = evidence / "release-provenance.json"
    for path in [manifest_path, signature_path, checksums_path, sbom_path, provenance_path]:
        if not path.exists():
            raise SystemExit(f"missing evidence file: {path}")

    manifest_payload = manifest_path.read_bytes().rstrip(b"\n")
    manifest = json.loads(manifest_payload.decode("utf-8"))
    signature = json.loads(signature_path.read_text(encoding="utf-8"))
    expected_manifest_sha = hashlib.sha256(stable_json(manifest)).hexdigest()
    if signature.get("manifest_sha256") != expected_manifest_sha:
        raise SystemExit("signature manifest_sha256 does not match manifest")
    expected_sig = sign_payload(stable_json(manifest), signing_key(args.allow_dev_key))
    if not hmac.compare_digest(signature.get("signature", ""), expected_sig):
        raise SystemExit("release manifest signature verification failed")

    checksum_map: dict[str, str] = {}
    for line in checksums_path.read_text(encoding="utf-8").splitlines():
        match = re.match(r"^([0-9a-f]{64})\s+(.+)$", line.strip())
        if not match:
            raise SystemExit(f"invalid SHA256SUMS line: {line}")
        checksum_map[match.group(2)] = match.group(1)
    for artifact in manifest.get("artifacts", []):
        name = Path(artifact["path"]).name
        if checksum_map.get(name) != artifact["sha256"]:
            raise SystemExit(f"checksum mismatch for {name}")

    sbom = json.loads(sbom_path.read_text(encoding="utf-8"))
    if sbom.get("bomFormat") != "CycloneDX" or not sbom.get("components"):
        raise SystemExit("SBOM is missing CycloneDX metadata or components")
    provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
    if provenance.get("commit") != manifest.get("commit"):
        raise SystemExit("provenance commit does not match manifest")
    print("release evidence verification passed")
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="SpectraLang release security tooling")
    sub = parser.add_subparsers(dest="command", required=True)

    create_parser = sub.add_parser("create", help="create release evidence")
    create_parser.add_argument("--artifact", action="append", required=True)
    create_parser.add_argument("--out", required=True)
    create_parser.add_argument("--version")
    create_parser.add_argument("--allow-dev-key", action="store_true")
    create_parser.set_defaults(func=create)

    verify_parser = sub.add_parser("verify", help="verify release evidence")
    verify_parser.add_argument("--evidence", required=True)
    verify_parser.add_argument("--allow-dev-key", action="store_true")
    verify_parser.set_defaults(func=verify)

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
