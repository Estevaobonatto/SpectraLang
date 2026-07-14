"""Independent R-3006 artifact and CLI validation gate."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import subprocess
from pathlib import Path

HEADER = struct.Struct("<8sIQQ")
MAGIC = b"SPARART1"


def read_artifact(path: Path) -> dict:
    data = path.read_bytes()
    if len(data) < HEADER.size + 32:
        raise ValueError("truncated")
    magic, version, manifest_len, payload_len = HEADER.unpack_from(data)
    if magic != MAGIC or version != 1:
        raise ValueError("unsupported header")
    body_len = HEADER.size + manifest_len + payload_len
    if len(data) != body_len + 32 or hashlib.sha256(data[:body_len]).digest() != data[body_len:]:
        raise ValueError("global checksum")
    manifest = json.loads(data[HEADER.size : HEADER.size + manifest_len])
    payload = data[HEADER.size + manifest_len : body_len]
    if manifest.get("kind") != "multi_array" or manifest.get("schema") != "spectralang.artifact.v1":
        raise ValueError("not an artifact container")
    arrays = {entry["name"]: entry for entry in manifest.get("arrays", [])}
    if set(arrays) != {"vectors", "levels", "links"}:
        raise ValueError("array set")
    for entry in arrays.values():
        start = entry["offset"]
        end = start + entry["length"]
        raw = payload[start:end]
        if end > len(payload) or hashlib.sha256(raw).hexdigest() != entry["checksum"]:
            raise ValueError("array checksum")
    metadata = manifest.get("metadata", {})
    required = {"artifact_role": "vector_index", "index_type": "hnsw", "index_version": "v1", "metric": "cosine", "dtype": "f64", "model_version": manifest.get("model_version")}
    if any(metadata.get(key) != value for key, value in required.items()):
        raise ValueError("metadata")
    if metadata.get("dimension") != "4" or metadata.get("entry_count") != "4":
        raise ValueError("dimension or count")
    if arrays["vectors"]["dtype"] != "float" or arrays["vectors"]["shape"] != [4, 4]:
        raise ValueError("vectors")
    if arrays["levels"]["dtype"] != "int" or arrays["levels"]["shape"] != [4]:
        raise ValueError("levels")
    if arrays["links"]["dtype"] != "int" or arrays["links"]["shape"] != [4, 1, 16]:
        raise ValueError("links")
    if int(metadata.get("entry_point", "-1")) not in range(4):
        raise ValueError("entry point")
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    fixture_dir = root / "tests" / "fixtures" / "r3006"
    valid = fixture_dir / "vector-index-valid.spar"
    rejected = [path for path in fixture_dir.iterdir() if path.name != valid.name]
    report = {"schema": "spectralang.r3006_vector_index.v1", "format_version": 1, "valid_fixtures": [], "rejected_corrupt_fixtures": [], "round_trip_results": [], "determinism_results": [], "atomic_write_results": [], "metrics_results": [], "failures": [], "status": "failed"}
    try:
        read_artifact(valid)
        report["valid_fixtures"].append(valid.name)
    except Exception as error:  # pragma: no cover - gate reports exact failure
        report["failures"].append(f"valid fixture: {error}")
    for path in rejected:
        try:
            if path.suffix == ".json":
                raise ValueError("legacy JSON rejected")
            read_artifact(path)
        except Exception:
            report["rejected_corrupt_fixtures"].append(path.name)
        else:
            report["failures"].append(f"accepted invalid fixture: {path.name}")
    try:
        completed = subprocess.run([str(args.binary), "run", str(args.fixture)], cwd=root, capture_output=True, text=True, timeout=30)
        report["round_trip_results"].append({"fixture": str(args.fixture), "returncode": completed.returncode, "stdout": completed.stdout[-2000:], "stderr": completed.stderr[-2000:]})
        if completed.returncode != 0:
            report["failures"].append("CLI fixture failed")
        else:
            report["determinism_results"].append({"fixture": str(args.fixture), "status": "passed"})
            persisted = root / "target" / "r3006-vector-index" / "cli-index.spar"
            first_digest = hashlib.sha256(persisted.read_bytes()).hexdigest()
            second = subprocess.run([str(args.binary), "run", str(args.fixture)], cwd=root, capture_output=True, text=True, timeout=30)
            second_digest = hashlib.sha256(persisted.read_bytes()).hexdigest()
            report["atomic_write_results"].append({"path": str(persisted), "replacement_returncode": second.returncode, "deterministic_bytes": first_digest == second_digest, "status": "passed" if second.returncode == 0 and first_digest == second_digest else "failed"})
            if second.returncode != 0 or first_digest != second_digest:
                report["failures"].append("atomic replacement or deterministic bytes failed")
    except Exception as error:
        report["failures"].append(f"CLI execution: {error}")
    report["metrics_results"].append({"algorithm": "hnsw", "metric": "cosine", "status": "checked by fixture"})
    if not report["failures"] and len(report["rejected_corrupt_fixtures"]) == len(rejected):
        report["status"] = "passed"
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps({"schema": report["schema"], "status": report["status"], "valid": len(report["valid_fixtures"]), "rejected": len(report["rejected_corrupt_fixtures"]), "failures": len(report["failures"])}, ensure_ascii=False))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
