"""Generate deterministic, checked-in R-3005 Artifact Container fixtures."""

from __future__ import annotations

import copy
import hashlib
import json
import struct
from pathlib import Path

MAGIC = b"SPARART1"
VERSION = 1
HEADER = struct.Struct("<8sIQQ")


def make_container(manifest: dict, arrays: list[tuple[str, str, list[int], list[int | float]]]) -> bytes:
    payload = bytearray()
    entries = []
    for name, dtype, shape, values in arrays:
        if dtype == "int":
            raw = b"".join(struct.pack("<q", int(value)) for value in values)
        else:
            raw = b"".join(struct.pack("<d", float(value)) for value in values)
        offset = len(payload)
        payload.extend(raw)
        entries.append({
            "name": name,
            "dtype": dtype,
            "precision": "f64",
            "shape": shape,
            "layout": "contiguous",
            "offset": offset,
            "length": len(raw),
            "checksum": hashlib.sha256(raw).hexdigest(),
        })
    manifest = {
        "schema": "spectralang.artifact.v1",
        "format_version": VERSION,
        "kind": manifest["kind"],
        "name": manifest["name"],
        "model_version": manifest["model_version"],
        "compatibility": {
            "container": "spectralang.artifact.v1",
            "tensor_encoding": "little-endian-f64-slots",
        },
        "metadata": manifest["metadata"],
        "arrays": entries,
    }
    encoded = json.dumps(manifest, separators=(",", ":"), ensure_ascii=False).encode()
    body = HEADER.pack(MAGIC, VERSION, len(encoded), len(payload)) + encoded + payload
    return body + hashlib.sha256(body).digest()


def rewrite_manifest(data: bytes, edit) -> bytes:
    magic, version, manifest_len, payload_len = HEADER.unpack_from(data)
    start = HEADER.size
    manifest = json.loads(data[start : start + manifest_len])
    edit(manifest)
    payload = data[start + manifest_len : start + manifest_len + payload_len]
    encoded = json.dumps(manifest, separators=(",", ":"), ensure_ascii=False).encode()
    body = HEADER.pack(magic, version, len(encoded), len(payload)) + encoded + payload
    return body + hashlib.sha256(body).digest()


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    out = root / "tests" / "fixtures" / "r3005"
    out.mkdir(parents=True, exist_ok=True)
    tokens = [
        {"id": 0, "token": "[UNK]"},
        {"id": 1, "token": "hello"},
        {"id": 2, "token": "world"},
        {"id": 3, "token": "[CLS]"},
        {"id": 4, "token": "[SEP]"},
        {"id": 5, "token": "[PAD]"},
        {"id": 6, "token": "##s"},
    ]
    vocab = {"tokens": tokens, "special_tokens": {"unk": 0, "cls": 3, "sep": 4, "pad": 5}, "lowercase": True, "continuation_prefix": "##"}
    tokenizer_manifest = {
        "name": "r3005-tokenizer",
        "model_version": "tokenizer-v1",
        "kind": "multi_array",
        "metadata": {"tokenizer_type": "wordpiece", "tokenizer_version": "v1", "vocab_json": json.dumps(vocab, separators=(",", ":"))},
    }
    tokenizer = make_container(tokenizer_manifest, [("token_ids", "int", [7], list(range(7)))])
    (out / "tokenizer-valid.spar").write_bytes(tokenizer)

    embedding_manifest = {
        "name": "r3005-embedding",
        "model_version": "embedding-v1",
        "kind": "checkpoint",
        "metadata": {"artifact_role": "embedding_weights", "vocab_size": "7", "embedding_dim": "4", "tokenizer_version": "v1"},
    }
    weights = [
        0.0, 0.0, 0.0, 0.0,
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
        0.5, 0.5, 0.0, 0.0,
        0.0, 0.5, 0.5, 0.0,
    ]
    embedding = make_container(embedding_manifest, [("embedding.weight", "float", [7, 4], weights)])
    (out / "embedding-valid.spar").write_bytes(embedding)

    (out / "tokenizer-duplicate-token.spar").write_bytes(rewrite_manifest(tokenizer, lambda m: m["metadata"].update({"vocab_json": json.dumps({**vocab, "tokens": tokens + [{"id": 7, "token": "hello"}]}, separators=(",", ":"))})))
    (out / "tokenizer-noncontiguous-id.spar").write_bytes(rewrite_manifest(tokenizer, lambda m: m["metadata"].update({"vocab_json": json.dumps({**vocab, "tokens": [{**token, "id": token["id"] + 1} for token in tokens]}, separators=(",", ":"))})))
    (out / "tokenizer-missing-unk.spar").write_bytes(rewrite_manifest(tokenizer, lambda m: m["metadata"].update({"vocab_json": json.dumps({**vocab, "special_tokens": {"cls": 3, "sep": 4}}, separators=(",", ":"))})))
    (out / "tokenizer-special-missing.spar").write_bytes(rewrite_manifest(tokenizer, lambda m: m["metadata"].update({"vocab_json": json.dumps({**vocab, "special_tokens": {**vocab["special_tokens"], "sep": 99}}, separators=(",", ":"))})))
    (out / "tokenizer-invalid-json.spar").write_bytes(rewrite_manifest(tokenizer, lambda m: m["metadata"].update({"vocab_json": "{"})))

    (out / "embedding-shape-invalid.spar").write_bytes(rewrite_manifest(embedding, lambda m: m["arrays"][0].update({"shape": [6, 4]})))
    (out / "embedding-dtype-invalid.spar").write_bytes(rewrite_manifest(embedding, lambda m: m["arrays"][0].update({"dtype": "int"})))
    (out / "embedding-metadata-invalid.spar").write_bytes(rewrite_manifest(embedding, lambda m: m["metadata"].update({"artifact_role": "checkpoint"})))
    corrupted_array = bytearray(embedding)
    corrupted_array[-33] ^= 1
    (out / "embedding-array-checksum-invalid.spar").write_bytes(corrupted_array)
    corrupted_global = bytearray(embedding)
    corrupted_global[-1] ^= 1
    (out / "embedding-global-checksum-invalid.spar").write_bytes(corrupted_global)
    (out / "embedding-truncated.spar").write_bytes(embedding[:-3])
    unsupported = bytearray(embedding)
    unsupported[8:12] = struct.pack("<I", 99)
    (out / "embedding-version-invalid.spar").write_bytes(unsupported)


if __name__ == "__main__":
    main()
