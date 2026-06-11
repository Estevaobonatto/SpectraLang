# R-1803 Tokenization, Embeddings, and RAG Toolkit

Status: complete for the current production baseline.

## Contract

R-1803 adds a deterministic NLP/RAG toolkit to `std.ml`. The baseline is
runtime-backed and file-persistable, so examples can run without Python glue.

Public API:

| API | Purpose |
|---|---|
| `ml.tokenizer_wordpiece(vocab_spec)` | Creates a WordPiece-style tokenizer from newline-separated `token:id` entries |
| `ml.tokenizer_encode(tokenizer, text)` | Encodes text into an int tensor of token IDs using greedy longest-match WordPiece segmentation |
| `ml.tokenizer_decode(tokenizer, ids)` | Decodes token IDs back to text, merging `##suffix` pieces |
| `ml.text_embed(text, dim)` | Creates a deterministic normalized hash embedding |
| `ml.vector_index_new(dim)` | Creates a cosine-similarity vector index |
| `ml.vector_index_insert(index, id, vector)` | Inserts or replaces a vector by string ID |
| `ml.vector_index_query(index, vector, top_k)` | Returns JSON ranked retrieval results |
| `ml.vector_index_persist(index, path)` | Writes a JSON vector-index artifact |
| `ml.vector_index_load(path)` | Loads a persisted vector index |
| `ml.rag_chunk_text(text, max_chars, overlap)` | Returns JSON chunks with deterministic IDs |
| `ml.rag_build_prompt(context, question)` | Builds a deterministic prompt from retrieved context and a question |
| `ml.rag_evaluate_answer(answer, expected)` | Returns token-overlap F1 in permille (`0..1000`) for RAG answer evaluation |

## Validation

Required gate:

```powershell
python scripts\validate_r1803_rag_toolkit.py
```

The script runs:

- `cargo test -p spectra-runtime ml_phase18_rag_tokenizer_vector_index_and_prompt_eval`
- `cargo run -p spectra-cli -- run tests/validation/97_ml_phase18_rag_toolkit.spectra`
- `cargo run -p spectra-cli -- run examples/ai/rag_retrieval_pipeline.spectra`

It also parses the persisted vector index and checks the generated RAG report.
