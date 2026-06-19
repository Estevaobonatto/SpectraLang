#!/usr/bin/env python3
"""Validate Phase 13 book/example wiring.

This is intentionally lightweight: it checks that the adoption book exists,
that required chapters are present, and that every AI reference example is
mentioned in the book so users can discover runnable programs from docs alone.
"""

from __future__ import annotations

import argparse
from pathlib import Path


REQUIRED_CHAPTERS = [
    "README.md",
    "01-language-basics.md",
    "02-numerics.md",
    "03-tensors.md",
    "04-autodiff.md",
    "05-model-authoring.md",
    "06-deployment-export.md",
    "07-stdlib-runtime-packages.md",
    "08-benchmarks-and-comparisons.md",
    "09-hello-http.md",
]

REQUIRED_EXAMPLES = [
    "linear_regression_train_export.spectra",
    "logistic_regression_train_export.spectra",
    "mlp_training_serving.spectra",
    "cnn_image_classifier.spectra",
    "toy_transformer_inference.spectra",
    "data_preprocessing_pipeline.spectra",
]

REQUIRED_API_EXAMPLES = [
    "00_hello_http.spectra",
]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".", help="repository root")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    book_dir = root / "docs" / "book"
    examples_dir = root / "examples" / "ai"
    api_examples_dir = root / "examples" / "api"

    errors: list[str] = []
    for chapter in REQUIRED_CHAPTERS:
        path = book_dir / chapter
        if not path.is_file():
            errors.append(f"missing book chapter: {path}")

    book_text = ""
    if book_dir.is_dir():
        for path in sorted(book_dir.glob("*.md")):
            book_text += path.read_text(encoding="utf-8") + "\n"

    for example in REQUIRED_EXAMPLES:
        path = examples_dir / example
        if not path.is_file():
            errors.append(f"missing AI example: {path}")
        if example not in book_text:
            errors.append(f"AI example is not referenced by docs/book: {example}")

    for example in REQUIRED_API_EXAMPLES:
        path = api_examples_dir / example
        if not path.is_file():
            errors.append(f"missing API example: {path}")
        if example not in book_text:
            errors.append(f"API example is not referenced by docs/book: {example}")

    api_index = root / "docs" / "api" / "README.md"
    if not api_index.is_file():
        errors.append(f"missing API reference index: {api_index}")
    else:
        api_index_text = api_index.read_text(encoding="utf-8")
        if "../book/09-hello-http.md" not in api_index_text:
            errors.append("docs/api/README.md does not link to Hello HTTP")

    if "run_tests.ps1" not in book_text:
        errors.append("book does not mention run_tests.ps1 validation")

    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1

    print(
        f"validated {len(REQUIRED_CHAPTERS)} book chapters and "
        f"{len(REQUIRED_EXAMPLES)} AI examples and "
        f"{len(REQUIRED_API_EXAMPLES)} API examples"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
