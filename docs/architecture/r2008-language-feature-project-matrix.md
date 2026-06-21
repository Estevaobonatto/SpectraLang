# R-2008 Language Feature Project Matrix

Source of truth: `docs/architecture/r2008-language-feature-project-matrix.toml`.

This matrix defines the complete project plan for post-baseline integrated
validation. `R-2009` owns the basic language projects. `R-2010` owns the AI
Support projects. `R-2011` will execute these entries, and `R-2012` requires
every real failure found during execution to be fixed or promoted into a new
roadmap item before certification.

| Project | Roadmap | Command | Owner | Feature Coverage |
|---|---|---|---|---|
| `basic_components_package` | `R-2009` | `spectralang package test` | `tooling` | modules, functions, structs/classes, traits, generics, closures, control flow, stdlib |
| `basic_runtime_run` | `R-2009` | `spectralang run` | `tooling` | modules, functions, structs/classes, control flow, stdlib |
| `basic_package_check` | `R-2009` | `spectralang package check` | `tooling` | modules, traits, generics, closures, stdlib |
| `ai_tensor_autodiff_run` | `R-2010` | `spectralang run` | `ml` | modules, functions, tensors, autodiff, graph/fusion, stdlib |
| `ai_data_experiment_package` | `R-2010` | `spectralang package test` | `ml` | modules, traits, tensors, data, experiment, evaluation, monitoring |
| `ai_model_ecosystem_check` | `R-2010` | `spectralang package check` | `ml` | generics, closures, ONNX, RAG, serving, evaluation, monitoring |
| `ai_serving_guardrails_run` | `R-2010` | `spectralang run` | `ml` | serving, evaluation, monitoring, control flow, stdlib |

Required coverage: modules, functions, structs/classes, traits, generics,
closures, control flow, stdlib, tensors, autodiff, graph/fusion, data,
experiment, ONNX, RAG, serving, evaluation, and monitoring.

Validation command:

```powershell
python scripts\validate_r2008_language_feature_matrix.py
```

The validator checks that every required feature is covered, every row declares
one supported CLI/package command, every owner and roadmap item is valid, and
the follow-on gap items `R-2009` through `R-2013` exist before the release
candidate gate.
