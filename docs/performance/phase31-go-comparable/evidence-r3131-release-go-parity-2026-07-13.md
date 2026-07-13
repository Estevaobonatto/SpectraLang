# R-3131 release/Go parity evidence — 2026-07-13

## Finding

The prior failure came from comparing `target/debug/spectralang.exe` with an
optimized Go binary. That profile mismatch is fixed: the official runner now
uses `target/release/spectralang.exe`. One earlier focused release run was
within the target, but the final controlled runs below did not reproduce that
parity.

## Reproduction

```powershell
cargo build --release -p spectra-cli --bin spectralang
python scripts\diagnose_async_echo.py `
  --binary target\release\spectralang.exe `
  --profile release `
  --out target\phase31\async-echo-diagnostics\r3131-release.json
```

Measured diagnostic comparison:

```json
{
  "reference_runtime": "go",
  "spectra_median_ns": 27310000,
  "go_median_ns": 28180300,
  "gap_to_go_pct": -3.086,
  "spectra_stddev_pct": 4.97
}
```

Final focused release diagnostics:

| report | Spectra median | Go median | ratio | Spectra stddev |
|---|---:|---:|---:|---:|
| `r3131-final-focused.json` | 30.013 ms | 32.999 ms | 0.910 | 7.882% |
| `r3131-final-focused-2.json` | 29.483 ms | 32.770 ms | 0.900 | 6.724% |

The complete Phase 31 report `target/phase31/cross-lang-report.json` recorded
three release attempts at 27.694, 27.733, and 29.532 ms against Go at 30.104,
30.547, and 31.504 ms, yielding `gap_to_go=0.908`. The validator correctly
rejected this because the contract is bilateral +/-5%, not merely “Spectra is
not slower”.

Runtime counters for the full Spectra variant showed 10,000 fused Fast ABI
calls, 1,000 reset calls, 11,014 registry locks, 1 slot created outside the
fused path, and 10,000 counted tasks. Fused pairs do not allocate observable
slots.

## Decision

R-3131's official performance contract uses the optimized release binary and
Go as reference. The profile mismatch is resolved, but the current evidence
does not satisfy the +/-5% and <=5% variance gates. No compiler/runtime change
is justified by this evidence, and the historical Spectra baseline remains
unchanged. R-3131 remains `in_progress`.
