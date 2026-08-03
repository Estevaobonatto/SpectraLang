import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from scripts.validate_r2505_postgres import contains_sensitive_value, scrub


ROOT = Path(__file__).resolve().parents[1]
VALIDATOR = ROOT / "scripts" / "validate_r2505_postgres.py"


class R2505ValidatorTests(unittest.TestCase):
    def test_strong_standalone_password_is_detected_and_scrubbed(self) -> None:
        url = "postgres://spectra:ultra-secret-2505@localhost:5432/test"
        raw = "authentication failed for value ultra-secret-2505"
        self.assertTrue(contains_sensitive_value(raw, url))
        self.assertNotIn("ultra-secret-2505", str(scrub(raw, url)))

    def run_validator(self, require_database: bool) -> tuple[subprocess.CompletedProcess[str], dict]:
        with tempfile.TemporaryDirectory(prefix="r2505-validator-") as temporary:
            report = Path(temporary) / "report.json"
            command = [
                sys.executable,
                str(VALIDATOR),
                "--report",
                str(report),
            ]
            if require_database:
                command.append("--require-database")
            environment = os.environ.copy()
            environment.pop("SPECTRA_POSTGRES_URL", None)
            completed = subprocess.run(
                command,
                cwd=ROOT,
                env=environment,
                text=True,
                capture_output=True,
            )
            return completed, json.loads(report.read_text(encoding="utf-8"))

    def test_optional_local_lane_is_explicitly_skipped(self) -> None:
        completed, report = self.run_validator(require_database=False)
        self.assertEqual(completed.returncode, 0)
        self.assertEqual(report["status"], "skipped_environment")
        self.assertFalse(report["environment"]["configured"])

    def test_required_lane_fails_closed_without_database(self) -> None:
        completed, report = self.run_validator(require_database=True)
        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(report["status"], "failed")
        self.assertTrue(report["failures"])


if __name__ == "__main__":
    unittest.main()
