# 6. Deployment And Export

The current production deployment path is file-based export plus local serving.
This keeps examples deterministic and validates the runtime without requiring a
network stack.

## Export A Toy Model

AI examples write simple artifacts with `std.fs`:

```spectra
import std.fs as fs;

fs.fs_write(
    "target/ai-examples/linear_regression_model.txt",
    "spectralang linear regression model\nfeatures=4\noutputs=1\n"
);
```

Run:

```powershell
New-Item -ItemType Directory -Force -Path target\ai-examples | Out-Null
.\target\debug\spectralang.exe run examples\ai\linear_regression_train_export.spectra
Get-Content target\ai-examples\linear_regression_model.txt
```

## Serve A Local Model

`std.serve` provides deterministic in-process serving handles for validation and
local integration tests.

```spectra
import std.serve as serve;

let server = serve.local_create();
serve.local_load_model(server, 1, 42);
serve.local_warmup(server, 2);
let request = serve.local_submit(server, 1, 100);
let result = serve.local_result(server, request);
```

The validated MLP example trains a toy model boundary and checks local serving:

```powershell
.\target\debug\spectralang.exe run examples\ai\mlp_training_serving.spectra
```

## Transformer Inference

The transformer example validates inference-style tensor operations and local
serving together:

```powershell
.\target\debug\spectralang.exe run examples\ai\toy_transformer_inference.spectra
```

## Export Contract

- Export examples write into `target/ai-examples/`.
- Files are intentionally simple text until a production checkpoint format is
  added.
- `.npy` exchange is covered by the Phase 8 Python bridge, not by these book
  examples.
- Network serving remains beta/deferred; Phase 13 uses deterministic local
  serving so validation is portable on Windows and CI.
