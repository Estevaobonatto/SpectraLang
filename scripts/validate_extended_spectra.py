"""Execute the extended algorithmic Spectra fixture suite.

The broad PowerShell runner compiles every file under tests/validation.  This
focused gate complements that compile coverage by executing the 50 new
algorithmic fixtures and checking their process exit status.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_NAMES = (
    "203_base_fibonacci_iterative.spectra",
    "204_base_gcd_lcm_extended.spectra",
    "205_base_prime_sieve.spectra",
    "206_base_binary_search_bounds.spectra",
    "207_base_insertion_sort_checksum.spectra",
    "208_base_prefix_difference.spectra",
    "209_base_matrix_integer_multiply.spectra",
    "210_base_string_frequency.spectra",
    "211_base_parentheses_parser.spectra",
    "212_base_ring_queue.spectra",
    "213_base_bfs_line_graph.spectra",
    "214_base_dijkstra_small.spectra",
    "215_base_knapsack_dynamic_programming.spectra",
    "216_base_edit_distance.spectra",
    "217_base_enum_state_machine.spectra",
    "218_base_closure_pipeline.spectra",
    "219_base_trait_struct_score.spectra",
    "220_base_recursive_tree_sum.spectra",
    "221_tensor_elementwise_pipeline.spectra",
    "222_tensor_flatten_slice_pipeline.spectra",
    "223_tensor_index_mutation.spectra",
    "224_tensor_extrema_argmax.spectra",
    "225_tensor_transpose_shape.spectra",
    "226_tensor_concat_stack.spectra",
    "227_tensor_batched_matmul.spectra",
    "228_tensor_activation_chain.spectra",
    "229_tensor_exp_log_sqrt.spectra",
    "230_tensor_rng_reproducibility.spectra",
    "231_tensor_statistics_lifecycle.spectra",
    "232_tensor_cpu_placement.spectra",
    "233_tensor_precision_conversion.spectra",
    "234_tensor_autodiff_vector.spectra",
    "235_tensor_autodiff_matmul.spectra",
    "236_tensor_autodiff_dot.spectra",
    "237_tensor_grad_mode_zeroing.spectra",
    "238_tensor_diff_block_gradient.spectra",
    "239_tensor_pool_reuse.spectra",
    "240_tensor_deterministic_mode.spectra",
    "241_tensor_static_shape_literals.spectra",
    "242_tensor_refill_validity.spectra",
    "243_ml_linear_inference.spectra",
    "244_ml_linear_training_step.spectra",
    "245_ml_cnn_pool_dropout.spectra",
    "246_ml_loss_families.spectra",
    "247_ml_optimizer_scheduler.spectra",
    "248_ml_dataset_split_loader.spectra",
    "249_ml_dataframe_feature_pipeline.spectra",
    "250_ml_transformer_primitives.spectra",
    "251_ml_tokenizer_rag_pipeline.spectra",
    "252_ml_metrics_report.spectra",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--binary",
        default=str(ROOT / "target" / "debug" / "spectralang.exe"),
        help="path to the SpectraLang CLI binary",
    )
    parser.add_argument(
        "--report",
        default=str(ROOT / "target" / "extended-spectra" / "report.json"),
        help="JSON report path",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=float,
        default=60.0,
        help="per-fixture execution timeout",
    )
    return parser.parse_args()


def resolve_binary(value: str) -> Path:
    binary = Path(value)
    if not binary.is_absolute():
        binary = ROOT / binary
    return binary.resolve()


def run_fixture(binary: Path, fixture: Path, timeout_seconds: float) -> dict[str, object]:
    try:
        completed = subprocess.run(
            [str(binary), "run", str(fixture)],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        return {
            "fixture": fixture.relative_to(ROOT).as_posix(),
            "status": "timeout",
            "exit_code": 124,
            "detail": str(error),
        }

    output = (completed.stdout + "\n" + completed.stderr).strip()
    result: dict[str, object] = {
        "fixture": fixture.relative_to(ROOT).as_posix(),
        "status": "passed" if completed.returncode == 0 else "failed",
        "exit_code": completed.returncode,
    }
    if output:
        result["detail"] = output[-2000:]
    return result


def main() -> int:
    args = parse_args()
    binary = resolve_binary(args.binary)
    if not binary.is_file():
        print(f"binary not found: {binary}", file=sys.stderr)
        return 2
    if len(FIXTURE_NAMES) != 50:
        print(f"fixture catalog must contain exactly 50 entries, found {len(FIXTURE_NAMES)}", file=sys.stderr)
        return 2

    fixtures = [ROOT / "tests" / "validation" / name for name in FIXTURE_NAMES]
    missing = [path.relative_to(ROOT).as_posix() for path in fixtures if not path.is_file()]
    if missing:
        print("missing fixtures:", *missing, sep="\n  ", file=sys.stderr)
        return 2

    cases = [run_fixture(binary, fixture, args.timeout_seconds) for fixture in fixtures]
    failed = [case for case in cases if case["status"] != "passed"]
    report = {
        "schema": "spectralang.extended_spectra.v1",
        "fixture_count": len(cases),
        "passed": len(cases) - len(failed),
        "failed": len(failed),
        "cases": cases,
    }
    report_path = Path(args.report)
    if not report_path.is_absolute():
        report_path = ROOT / report_path
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    print(f"extended Spectra fixtures: {report['passed']}/{report['fixture_count']} passed")
    if failed:
        for case in failed:
            print(f"FAIL {case['fixture']} (exit={case['exit_code']})", file=sys.stderr)
        return 1
    print(f"report: {report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
