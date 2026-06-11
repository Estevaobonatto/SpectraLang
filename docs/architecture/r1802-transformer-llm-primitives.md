# R-1802 Transformer and LLM Runtime Primitives

Status: complete for the current production baseline.

## Contract

R-1802 adds runtime-backed transformer and LLM primitives to `std.ml`. The
functions operate on real `std.tensor` handles and validate dtype/shape before
execution.

Public API:

| API | Purpose |
|---|---|
| `ml.embedding_lookup(table, ids)` | Gathers rows from a `[vocab, dim]` float embedding table using an int tensor of token IDs |
| `ml.positional_encoding(seq_len, dim)` | Creates deterministic sinusoidal `[seq_len, dim]` positional encodings |
| `ml.layer_norm(input, scale, bias, eps)` | Normalizes the last dimension and applies scale/bias |
| `ml.gelu(input)` | Applies approximate GELU elementwise |
| `ml.swiglu(input, gate)` | Applies `input * sigmoid(gate)` elementwise |
| `ml.attention(query, key, value)` | Computes scaled dot-product attention for 2D tensors |
| `ml.kv_cache_new(max_tokens, dim)` | Creates a KV cache handle |
| `ml.kv_cache_append(cache, key, value)` | Appends `[tokens, dim]` key/value tensors and returns cache length |
| `ml.kv_cache_keys(cache)` / `ml.kv_cache_values(cache)` | Materializes cached keys/values as tensors |
| `ml.kv_cache_len(cache)` | Returns cached token count |
| `ml.logits_sample(logits, temperature)` | Samples an index from softmax(logits / temperature) using the runtime RNG |

## Execution Semantics

The baseline execution path is deterministic CPU/fallback materialization over
tensor handles. This keeps the API valid for CPU tensors and for tensors that
were explicitly routed through available accelerator placement APIs. When an
accelerator backend is available, the primitive contract requires equivalent
observable outputs within `std.tensor` numerical tolerance; unsupported
accelerator-specific kernels fall back to materialized tensor values rather
than returning an internal runtime error.

## Shape Contract

- `embedding_lookup`: table rank 2, IDs rank 1 or any materialized int tensor.
- `positional_encoding`: positive `seq_len` and `dim`.
- `layer_norm`: `scale` and `bias` are rank 1 with length equal to the last input dimension.
- `swiglu`: input and gate shapes must match.
- `attention`: query/key/value are rank 2; query and key hidden dimensions match; key/value token counts match.
- `kv_cache_append`: key and value rank 2 shapes match cache `dim` and remaining capacity.
- `logits_sample`: logits are finite and temperature is positive.

## Validation

Required gate:

```powershell
python scripts\validate_r1802_transformer_primitives.py
```

The script runs:

- `cargo test -p spectra-runtime ml_phase18_transformer_primitives_and_sampling`
- `cargo run -p spectra-cli -- run tests/validation/96_ml_phase18_transformer_primitives.spectra`
- `cargo run -p spectra-cli -- run examples/ai/toy_transformer_inference.spectra`

The toy transformer example now uses real `std.ml` transformer primitives
instead of placeholder dot/matmul arithmetic.
