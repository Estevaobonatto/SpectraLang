# R-1901 Model Evaluation and Metrics Suite

## Status

Complete for the current production AI evaluation baseline.

## Runtime Contract

`std.ml` exposes deterministic evaluation helpers that return JSON strings:

- `metrics_classification(labels, predictions)` reports accuracy, precision, recall, F1, ROC-AUC baseline, and confusion counts.
- `metrics_regression(expected, predicted)` reports MSE, MAE, and RMSE.
- `metrics_ranking(relevance, scores, top_k)` reports hit rate, MRR, and NDCG at `k`.
- `metrics_generation(output, reference)` reports exact match, token F1, and a deterministic perplexity proxy.
- `serving_metrics(latencies_ms, requests, errors)` reports request count, error rate, average latency, p95 latency, and throughput.
- `evaluation_report(path, name, ...)` writes a versioned JSON report and a human-readable companion text file.

The API intentionally uses existing tensor handles and strings so it integrates with the current `std.tensor` and `std.ml` host-call ABI without introducing new language-level record types.

## Validation

- Runtime unit test: `ml_phase19_evaluation_metrics_and_report`
- Public language validation: `tests/validation/98_ml_phase19_evaluation_metrics.spectra`
- AI reference example: `examples/ai/model_evaluation_report.spectra`
- Gate script: `scripts/validate_r1901_evaluation_metrics.py`
- Full suite integration: `run_tests.ps1` group `phase19-evaluation`
