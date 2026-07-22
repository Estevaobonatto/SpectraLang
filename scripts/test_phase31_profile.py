from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from scripts.phase31_profile import (
    DEFAULT_PROFILE_ROOT, PROFILE_SCENARIOS, REQUIRED_ARTIFACTS, SCHEMA,
    validate_artifacts,
)


class Phase31ProfileTests(unittest.TestCase):
    def test_profile_contract_has_exact_eight_scenarios(self) -> None:
        self.assertEqual(len(PROFILE_SCENARIOS), 8)
        self.assertEqual(len(set(PROFILE_SCENARIOS)), 8)
        self.assertTrue(all(name.startswith(("cpu-", "tensor-")) for name in PROFILE_SCENARIOS))

    def test_missing_artifacts_are_reported(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            errors = validate_artifacts(Path(temporary), [PROFILE_SCENARIOS[0]])
        self.assertEqual(len(errors), len(REQUIRED_ARTIFACTS))

    def test_metadata_and_artifacts_validate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = root / DEFAULT_PROFILE_ROOT / PROFILE_SCENARIOS[0]
            directory.mkdir(parents=True)
            for name in REQUIRED_ARTIFACTS:
                path = directory / name
                if name == "metadata.json":
                    path.write_text(json.dumps({"schema": SCHEMA, "scenario": PROFILE_SCENARIOS[0], "baseline_modified": False}), encoding="utf-8")
                else:
                    path.write_text("evidence\n", encoding="utf-8")
            self.assertEqual(validate_artifacts(root, [PROFILE_SCENARIOS[0]]), [])

    def test_metadata_rejects_baseline_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = root / DEFAULT_PROFILE_ROOT / PROFILE_SCENARIOS[0]
            directory.mkdir(parents=True)
            for name in REQUIRED_ARTIFACTS:
                path = directory / name
                if name == "metadata.json":
                    path.write_text(json.dumps({"schema": SCHEMA, "scenario": PROFILE_SCENARIOS[0], "baseline_modified": True}), encoding="utf-8")
                else:
                    path.write_text("evidence\n", encoding="utf-8")
            self.assertTrue(any("baseline_modified" in error for error in validate_artifacts(root, [PROFILE_SCENARIOS[0]])))


if __name__ == "__main__":
    unittest.main()
