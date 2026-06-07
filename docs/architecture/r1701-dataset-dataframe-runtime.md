# R-1701 Dataset and DataFrame Runtime

Status: complete for the current production baseline.

## Contract

R-1701 adds file-backed numerical datasets and minimal dataframe handles to `std.ml`.
All data loaded by this phase is materialized as existing `std.tensor` float tensors,
so the established training, dataloader, optimizer, and autodiff APIs continue to
operate without a separate data execution engine.

Supported loaders:

| API | Format |
|---|---|
| `ml.dataset_from_csv(path, label_col, has_header)` | Numeric CSV; one column is selected as label and all remaining columns become features |
| `ml.dataset_from_jsonl(path)` | JSONL rows with `features: [float...]` and `label: float` |
| `ml.dataset_from_npy(features_path, labels_path, rows)` | NumPy `.npy` v1.0 little-endian f64 one-dimensional arrays |
| `ml.dataset_from_directory(path)` | Directory containing `features.csv` and `labels.csv`, both numeric CSV with headers |
| `ml.dataframe_from_csv(path, has_header)` | Numeric CSV dataframe handle |

The `.npy` support intentionally matches the existing interop baseline:
NumPy v1.0, little-endian `f64`, one-dimensional, C-order arrays.

## Dataset Operations

Datasets are immutable handles. Operations return new dataset handles:

- `ml.dataset_map_features(dataset, scale, bias)` applies `value * scale + bias`.
- `ml.dataset_filter_label_min(dataset, min_label)` keeps rows whose first label value is greater than or equal to `min_label`.
- `ml.dataset_train_split(dataset, train_len)` returns the first `train_len` rows.
- `ml.dataset_test_split(dataset, train_len)` returns the remaining rows.

Dataloaders remain deterministic:

- `ml.dataloader_new(dataset, batch_size, seed)`
- `ml.dataloader_batch_count(loader)`
- `ml.dataloader_batch_features(loader, batch_index)`
- `ml.dataloader_batch_labels(loader, batch_index)`

The seed controls the existing deterministic per-batch ordering. A seed of `0`
keeps natural row order.

## DataFrame Operations

DataFrames are numeric table handles:

- `ml.dataframe_rows(frame)`
- `ml.dataframe_cols(frame)`
- `ml.dataframe_column(frame, col)` returns a float tensor for the selected column.

Dataframes are intentionally lightweight in this phase. Training uses datasets;
dataframes provide inspection and column extraction for tabular preprocessing.

## Validation

Required gate:

```powershell
python scripts\validate_r1701_data_runtime.py
```

The script runs:

- `cargo test -p spectra-runtime ml_phase17_dataset_dataframe_file_loaders_transforms_and_splits`
- `cargo run -p spectra-cli -- run tests/validation/92_ml_phase17_data_runtime.spectra`
- `cargo run -p spectra-cli -- run examples/ai/tabular_dataset_training.spectra`

Checked-in fixtures:

- `tests/fixtures/r1701/tabular.csv`
- `tests/fixtures/r1701/rows.jsonl`
- `tests/fixtures/r1701/directory_dataset/features.csv`
- `tests/fixtures/r1701/directory_dataset/labels.csv`

The Rust runtime test also generates temporary `.npy` fixtures and validates the
NPY loader without Python glue.
