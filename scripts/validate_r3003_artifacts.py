"""Independent validation gate for Spectra Artifact Container v1."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import struct
import subprocess
import tempfile
from pathlib import Path

MAGIC = b"SPARART1"
VERSION = 1
HEADER = struct.Struct("<8sIQQ")
DIGEST_SIZE = 32


def canonical_container(descriptor: dict) -> bytes:
    payload = bytearray()
    arrays = []
    for array in descriptor["arrays"]:
        values = array["values"]
        if array["dtype"] == "int":
            data = b"".join(struct.pack("<q", int(value)) for value in values)
        else:
            data = b"".join(struct.pack("<d", float(value)) for value in values)
        offset = len(payload)
        payload.extend(data)
        arrays.append(
            {
                "name": array["name"],
                "dtype": array["dtype"],
                "precision": "f64",
                "shape": array["shape"],
                "layout": "contiguous",
                "offset": offset,
                "length": len(data),
                "checksum": hashlib.sha256(data).hexdigest(),
            }
        )
    manifest = {
        "schema": "spectralang.artifact.v1",
        "format_version": VERSION,
        "kind": descriptor["kind"],
        "name": descriptor["name"],
        "model_version": descriptor["model_version"],
        "compatibility": {
            "container": "spectralang.artifact.v1",
            "tensor_encoding": "little-endian-f64-slots",
        },
        "metadata": descriptor["metadata"],
        "arrays": arrays,
    }
    manifest_bytes = json.dumps(manifest, separators=(",", ":"), ensure_ascii=False).encode()
    body = HEADER.pack(MAGIC, VERSION, len(manifest_bytes), len(payload)) + manifest_bytes + payload
    return body + hashlib.sha256(body).digest()


def parse_container(data: bytes) -> dict:
    if len(data) < HEADER.size + DIGEST_SIZE:
        raise ValueError("truncated container")
    magic, version, manifest_len, payload_len = HEADER.unpack_from(data)
    if magic != MAGIC or version != VERSION:
        raise ValueError("magic/version mismatch")
    expected = HEADER.size + manifest_len + payload_len + DIGEST_SIZE
    if expected != len(data):
        raise ValueError("length mismatch")
    if hashlib.sha256(data[:-DIGEST_SIZE]).digest() != data[-DIGEST_SIZE:]:
        raise ValueError("global checksum mismatch")
    start = HEADER.size
    manifest = json.loads(data[start : start + manifest_len])
    if manifest.get("schema") != "spectralang.artifact.v1" or manifest.get("format_version") != VERSION:
        raise ValueError("manifest schema/version mismatch")
    if manifest.get("kind") not in {"checkpoint", "multi_array"}:
        raise ValueError("invalid kind")
    if not isinstance(manifest.get("name"), str) or not manifest["name"] or not isinstance(manifest.get("model_version"), str) or not manifest["model_version"]:
        raise ValueError("invalid artifact identity")
    if manifest.get("compatibility") != {"container": "spectralang.artifact.v1", "tensor_encoding": "little-endian-f64-slots"}:
        raise ValueError("unsupported compatibility contract")
    if not isinstance(manifest.get("metadata"), dict) or any(not isinstance(key, str) or not isinstance(value, str) for key, value in manifest["metadata"].items()):
        raise ValueError("invalid metadata")
    arrays = manifest.get("arrays")
    if not isinstance(arrays, list) or not arrays:
        raise ValueError("arrays missing")
    payload = data[start + manifest_len : start + manifest_len + payload_len]
    ranges = []
    names = set()
    for array in arrays:
        required = {"name", "dtype", "precision", "shape", "layout", "offset", "length", "checksum"}
        if set(array) != required:
            raise ValueError("array schema mismatch")
        if not array["name"] or array["name"] in names:
            raise ValueError("duplicate/empty array name")
        names.add(array["name"])
        if array["dtype"] not in {"int", "float"} or array["precision"] != "f64" or array["layout"] != "contiguous":
            raise ValueError("unsupported array representation")
        shape = array["shape"]
        if not shape or any(not isinstance(dim, int) or dim <= 0 for dim in shape):
            raise ValueError("invalid shape")
        offset, length = array["offset"], array["length"]
        if not isinstance(offset, int) or not isinstance(length, int) or offset < 0 or length < 0 or offset + length > len(payload):
            raise ValueError("invalid range")
        if length != 8 * __import__("functools").reduce(lambda a, b: a * b, shape, 1):
            raise ValueError("shape/length mismatch")
        chunk = payload[offset : offset + length]
        if hashlib.sha256(chunk).hexdigest() != array["checksum"]:
            raise ValueError("array checksum mismatch")
        ranges.append((offset, offset + length))
    if any(left[1] > right[0] for left, right in zip(sorted(ranges), sorted(ranges)[1:])):
        raise ValueError("overlapping ranges")
    return manifest


def corrupt(data: bytes, case: str) -> bytes:
    raw = bytearray(data)
    if case == "global_checksum":
        raw[-1] ^= 1
    elif case == "truncated":
        return bytes(raw[:-3])
    elif case == "unsupported_version":
        raw[8:12] = struct.pack("<I", 99)
    else:
        manifest_len = HEADER.unpack_from(raw)[2]
        start = HEADER.size
        manifest = json.loads(raw[start : start + manifest_len])
        if case == "array_checksum":
            manifest["arrays"][0]["checksum"] = "0" * 64
        elif case == "shape_mismatch":
            manifest["arrays"][0]["shape"] = [999]
        elif case == "dtype_unsupported":
            manifest["arrays"][0]["dtype"] = "u128"
        elif case == "metadata_invalid":
            manifest["metadata"] = ["invalid"]
        elif case == "overlapping_offsets":
            manifest["arrays"][1]["offset"] = manifest["arrays"][0]["offset"]
        else:
            raise ValueError(f"unknown corruption case: {case}")
        encoded = json.dumps(manifest, separators=(",", ":"), ensure_ascii=False).encode()
        payload_start = start + manifest_len
        payload = raw[payload_start : payload_start + HEADER.unpack_from(raw)[3]]
        body = HEADER.pack(MAGIC, VERSION, len(encoded), len(payload)) + encoded + payload
        return body + hashlib.sha256(body).digest()
    return bytes(raw)


def run_cli(binary: Path, fixture: Path, cwd: Path) -> tuple[bool, str]:
    result = subprocess.run([str(binary), "run", str(fixture)], cwd=cwd, capture_output=True, text=True, timeout=30)
    return result.returncode == 0, (result.stdout + result.stderr)[-2000:]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--fixture", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--fixtures-dir", type=Path, default=Path("tests/fixtures/r3003"))
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    report = {
        "schema": "spectralang.r3003_artifacts.v1",
        "format_version": VERSION,
        "valid_fixtures": [],
        "rejected_corrupt_fixtures": [],
        "round_trip_results": [],
        "determinism_results": [],
        "atomic_write_results": [],
        "failures": [],
    }
    try:
        descriptors = [json.loads(path.read_text(encoding="utf-8")) for path in sorted(args.fixtures_dir.glob("*-valid.json"))]
        with tempfile.TemporaryDirectory(prefix="spectra-r3003-") as temp:
            temp_path = Path(temp)
            for descriptor in descriptors:
                first = canonical_container(descriptor)
                second = canonical_container(descriptor)
                name = descriptor["name"]
                parse_container(first)
                report["valid_fixtures"].append(name)
                report["determinism_results"].append({"fixture": name, "deterministic": first == second})
                target = temp_path / f"{name}.spar"
                target.write_bytes(first)
                parse_container(target.read_bytes())
                report["round_trip_results"].append({"fixture": name, "passed": True})
            atomic_target = temp_path / "atomic.spar"
            atomic_target.write_bytes(canonical_container(descriptors[0]))
            leftovers = list(temp_path.glob("atomic.tmp-*"))
            report["atomic_write_results"].append({"target": str(atomic_target), "passed": not leftovers})
            if leftovers:
                report["failures"].append("atomic write temporary file remained")
            cases = json.loads((args.fixtures_dir / "corruption-cases.json").read_text(encoding="utf-8"))["cases"]
            base = canonical_container(descriptors[0])
            for case in cases:
                try:
                    parse_container(corrupt(base, case))
                except ValueError:
                    report["rejected_corrupt_fixtures"].append(case)
                else:
                    report["failures"].append(f"corruption case accepted: {case}")
        ok, output = run_cli(args.binary.resolve(), args.fixture.resolve(), root)
        report["round_trip_results"].append({"fixture": str(args.fixture), "cli": ok, "output": output})
        cli_target = root / "target" / "r3003-artifacts" / "cli-checkpoint.spar"
        leftovers = list(cli_target.parent.glob(f"{cli_target.stem}.tmp-*")) if cli_target.parent.exists() else []
        report["atomic_write_results"].append({"target": str(cli_target), "passed": ok and cli_target.exists() and not leftovers})
        if not ok or not cli_target.exists() or leftovers:
            report["failures"].append("CLI atomic write evidence failed")
        if not ok:
            report["failures"].append("CLI fixture failed")
    except Exception as exc:  # the report is also the gate's diagnostic artifact
        report["failures"].append(str(exc))
    report["status"] = "passed" if not report["failures"] and len(report["rejected_corrupt_fixtures"]) == 8 else "failed"
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"status": report["status"], "failures": report["failures"]}, sort_keys=True))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
