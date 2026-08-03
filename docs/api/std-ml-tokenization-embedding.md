# Production Tokenization and Embedding

R-3005 adds production loaders on top of the R-3003 Artifact Container v1.

`std.ml.tokenizer_load(path)` accepts a validated WordPiece artifact with
`tokenizer_type = "wordpiece"`, `tokenizer_version = "v1"`, and canonical
`vocab_json` metadata. The returned handle supports deterministic
`tokenizer_encode` and `tokenizer_decode`, including special tokens, unknown
tokens, continuation pieces, and stable rejection of invalid IDs.

`std.ml.embedding_load(path, tensor_name)` accepts an artifact marked with
`artifact_role = "embedding_weights"`. It validates vocabulary size,
embedding dimension, dtype, rank, layout, precision, metadata, and checksums
before returning a real tensor handle. That handle is consumed by the existing
`std.ml.embedding_lookup` operation.

The older inline `tokenizer_wordpiece` and hash-based `text_embed` APIs remain
available for compatibility, but are documented and audited as baselines.
They are never used as fallbacks by the production loaders.
