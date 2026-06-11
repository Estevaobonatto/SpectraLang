# R-1801 ONNX Import and Export

Status: complete for the current production baseline.

## Contract

R-1801 adds a checked ONNX `ModelProto` subset for model ecosystem
interoperability. The runtime writes binary protobuf `.onnx` files directly and
imports the same supported subset back into a validated summary.

Supported model kinds:

- `linear`: ONNX `Gemm`
- `conv`: ONNX `Conv`
- `activation`: ONNX `Relu`
- `normalization`: ONNX `LayerNormalization`
- `transformer`: ONNX `MatMul`, `Softmax`, `LayerNormalization`, and `Gelu`

All exported values carry ranked shapes and `float32` tensor type metadata.

Public API:

| API | Purpose |
|---|---|
| `ml.onnx_export(path, kind)` | Writes a supported binary ONNX `ModelProto` and returns the path |
| `ml.onnx_import_summary(path)` | Imports the supported subset and returns a JSON summary of graphs, ops, inputs, outputs, dtype, and shape status |
| `ml.onnx_validate(path)` | Returns `1` when the file is a valid supported subset model, otherwise `0` |
| `ml.onnx_roundtrip(input_path, output_path)` | Imports and validates the input model, then writes an equivalent supported ONNX artifact |

## Supported Scope

The current importer is intentionally subset-based. It accepts the ONNX fields
needed by the exported `ModelProto` files:

- `ModelProto.ir_version`
- `ModelProto.producer_name`
- `ModelProto.graph`
- `ModelProto.opset_import`
- `GraphProto.node`
- `GraphProto.input`
- `GraphProto.output`
- `NodeProto.input`
- `NodeProto.output`
- `NodeProto.name`
- `NodeProto.op_type`
- `ValueInfoProto.name`
- `ValueInfoProto.type.tensor_type.elem_type`
- `TensorShapeProto.dim.dim_value`

Unsupported ONNX extensions fail validation instead of being silently accepted.

## Validation

Required gate:

```powershell
python scripts\validate_r1801_onnx_import_export.py
```

The script runs:

- `cargo test -p spectra-runtime ml_phase18_onnx_subset_export_import_and_roundtrip`
- `cargo run -p spectra-cli -- run tests/validation/95_ml_phase18_onnx_import_export.spectra`
- `cargo run -p spectra-cli -- run examples/ai/onnx_transformer_export.spectra`

It also parses generated `.onnx` files independently and checks graph count,
opset import, inputs, outputs, and required operators for linear, convolutional,
activation, normalization, and transformer blocks.
