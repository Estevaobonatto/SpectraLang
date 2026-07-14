"""Generate independent Spectra Artifact Container v1 fixtures for R-3006."""

from __future__ import annotations

import hashlib
import json
import struct
from pathlib import Path

HEADER = struct.Struct("<8sIQQ")
MAGIC = b"SPARART1"


def container(metadata: dict[str, str], arrays: list[tuple[str, str, list[int], list[int | float]]], *, name="r3006-index", version="r3006-fixture-v1") -> bytes:
    payload = bytearray()
    table = []
    for name_, dtype, shape, values in arrays:
        raw = b"".join(struct.pack("<q" if dtype == "int" else "<d", value) for value in values)
        offset = len(payload)
        payload.extend(raw)
        table.append({"name": name_, "dtype": dtype, "precision": "f64", "shape": shape, "layout": "contiguous", "offset": offset, "length": len(raw), "checksum": hashlib.sha256(raw).hexdigest()})
    manifest = {"schema": "spectralang.artifact.v1", "format_version": 1, "kind": "multi_array", "name": name, "model_version": version, "compatibility": {"container": "spectralang.artifact.v1", "tensor_encoding": "little-endian-f64-slots"}, "metadata": metadata, "arrays": table}
    encoded = json.dumps(manifest, separators=(",", ":"), ensure_ascii=False).encode()
    body = HEADER.pack(MAGIC, 1, len(encoded), len(payload)) + encoded + payload
    return body + hashlib.sha256(body).digest()


def rewrite(data: bytes, edit) -> bytes:
    magic, version, manifest_len, payload_len = HEADER.unpack_from(data)
    start = HEADER.size
    manifest = json.loads(data[start : start + manifest_len])
    edit(manifest)
    payload = data[start + manifest_len : start + manifest_len + payload_len]
    encoded = json.dumps(manifest, separators=(",", ":"), ensure_ascii=False).encode()
    body = HEADER.pack(magic, version, len(encoded), len(payload)) + encoded + payload
    return body + hashlib.sha256(body).digest()


def main() -> None:
    out = Path(__file__).resolve().parents[1] / "tests" / "fixtures" / "r3006"
    out.mkdir(parents=True, exist_ok=True)
    ids = ["alpha", "beta", "gamma", "delta"]
    metadata = {"artifact_role": "vector_index", "index_type": "hnsw", "index_version": "v1", "metric": "cosine", "dtype": "f64", "m": "16", "ef_construction": "200", "ef_search": "64", "seed": "0", "dimension": "4", "entry_count": "4", "max_level": "0", "entry_point": "0", "model_version": "r3006-fixture-v1", "ids_json": json.dumps(ids, separators=(",", ":"))}
    vectors = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
    levels = [0, 0, 0, 0]
    links = []
    neighbors = [[1, 2, 3], [0, 2, 3], [0, 1, 3], [0, 1, 2]]
    for row in neighbors:
        links.extend(row + [-1] * 13)
    valid = container(metadata, [("vectors", "float", [4, 4], vectors), ("levels", "int", [4], levels), ("links", "int", [4, 1, 16], links)])
    (out / "vector-index-valid.spar").write_bytes(valid)
    (out / "vector-index-array-checksum-invalid.spar").write_bytes(valid[:-33] + bytes([valid[-33] ^ 1]) + valid[-32:])
    (out / "vector-index-global-checksum-invalid.spar").write_bytes(valid[:-1] + bytes([valid[-1] ^ 1]))
    (out / "vector-index-truncated.spar").write_bytes(valid[:-5])
    (out / "vector-index-dimension-invalid.spar").write_bytes(rewrite(valid, lambda m: m["metadata"].update({"dimension": "3"})))
    (out / "vector-index-dtype-invalid.spar").write_bytes(rewrite(valid, lambda m: m["arrays"][0].update({"dtype": "int"})))
    (out / "vector-index-metadata-invalid.spar").write_bytes(rewrite(valid, lambda m: m["metadata"].update({"artifact_role": "checkpoint"})))
    (out / "vector-index-link-invalid.spar").write_bytes(rewrite(valid, lambda m: m["metadata"].update({"entry_point": "99"})))
    (out / "vector-index-legacy.json").write_text('{"schema":"spectra.ml.vector_index.v1","dim":4,"entries":[]}', encoding="utf-8")


if __name__ == "__main__":
    main()
