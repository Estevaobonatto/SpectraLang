# `std.ml` Artifact Container

R-3003 provides the native Spectra Artifact Container v1 for checkpoints and
multi-array tensor archives. The container stores a canonical JSON manifest
and contiguous little-endian tensor payloads behind a fixed `SPARART1` header.
Every array and the complete container are protected by SHA-256 checksums.
The manifest records the exact container and tensor-encoding compatibility
contract so a future reader can reject incompatible representations.

The public handle API is exposed from `std.ml`:

- `artifact_new(name, model_version, kind)` accepts `checkpoint` or
  `multi_array`.
- `artifact_set_metadata`, `artifact_add_tensor`, and `artifact_save` build
  and atomically persist an artifact.
- `artifact_load`, `artifact_tensor`, `artifact_metadata`, and
  `artifact_validate` validate and read the artifact.
- `artifact_free` releases the artifact handle; tensor handles returned from a
  loaded artifact remain ordinary tensor handles and can be freed normally.

Version 1 currently accepts CPU contiguous `int` and `float` tensors with
physical `f64` storage. Unsupported dtypes, devices, dimensions, corrupted
checksums, overlapping ranges, unknown manifest fields, and incompatible
versions are rejected before any artifact is returned.

The executable contract is exercised by
`tests/validation/186_ml_artifact_container.spectra` and independently checked
by `scripts/validate_r3003_artifacts.py`.
