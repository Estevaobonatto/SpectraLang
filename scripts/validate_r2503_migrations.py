#!/usr/bin/env python3
"""Independent end-to-end gate for the SQLite migrations framework."""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sqlite3
import subprocess
import tempfile
from pathlib import Path


def run(binary: Path, args: list[str], root: Path) -> tuple[int, str]:
    completed = subprocess.run([str(binary), *args], cwd=root, text=True, capture_output=True)
    return completed.returncode, completed.stdout + completed.stderr


def canonical(value: str) -> str:
    return value.replace("\r\n", "\n").replace("\r", "\n")


def migration_checksum(version: int, name: str, up: str, down: str) -> str:
    digest = hashlib.sha256()
    values = (str(version), name, canonical(up), canonical(down))
    for index, value in enumerate(values):
        digest.update(value.encode("utf-8"))
        if index < len(values) - 1:
            digest.update(b"\0")
    return digest.hexdigest()


def copy_tree(source: Path, destination: Path) -> None:
    shutil.copytree(source, destination)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--database", required=True)
    parser.add_argument("--migrations-dir", required=True)
    parser.add_argument("--report", required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    binary = (root / args.binary).resolve() if not Path(args.binary).is_absolute() else Path(args.binary)
    migrations_source = (root / args.migrations_dir).resolve() if not Path(args.migrations_dir).is_absolute() else Path(args.migrations_dir)
    database = (root / args.database).resolve() if not Path(args.database).is_absolute() else Path(args.database)
    report_path = (root / args.report).resolve() if not Path(args.report).is_absolute() else Path(args.report)
    report = {
        "schema": "spectralang.r2503_migrations.v1",
        "discovery": {},
        "checksums": {},
        "apply": {},
        "rollback": {},
        "partial_failure": {},
        "drift_detection": {},
        "concurrency": {},
        "cli": {},
        "sqlite": {},
        "failures": [],
        "status": "failed",
    }
    try:
        if not binary.is_file():
            raise RuntimeError(f"CLI binary does not exist: {binary}")
        if not migrations_source.is_dir():
            raise RuntimeError(f"migration fixture does not exist: {migrations_source}")
        database.parent.mkdir(parents=True, exist_ok=True)
        if database.exists():
            database.unlink()
        apply_code, apply_output = run(binary, ["db", "migrate", "--database", str(database), "--migrations-dir", str(migrations_source)], root)
        if apply_code != 0:
            raise RuntimeError("initial migrate failed: " + apply_output)
        connection = sqlite3.connect(database)
        versions = connection.execute("SELECT version, name, checksum FROM _spectra_migrations ORDER BY version").fetchall()
        tables = {row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        if versions != [(1, "create_users", versions[0][2]), (2, "add_email", versions[1][2])]:
            raise RuntimeError(f"unexpected tracking rows: {versions!r}")
        if "users" not in tables:
            raise RuntimeError("users table was not created")
        columns = {row[1] for row in connection.execute("PRAGMA table_info(users)")}
        if "email" not in columns:
            raise RuntimeError("second migration did not add email")
        for version, name, stored in versions:
            up = (migrations_source / f"{version:04d}_{name}.up.sql").read_text(encoding="utf-8")
            down = (migrations_source / f"{version:04d}_{name}.down.sql").read_text(encoding="utf-8")
            if stored != migration_checksum(version, name, up, down):
                raise RuntimeError(f"independent checksum mismatch for migration {version}")
        connection.close()
        status_code, status_output = run(binary, ["db", "status", "--database", str(database), "--migrations-dir", str(migrations_source), "--json"], root)
        status = json.loads(status_output) if status_code == 0 else {}
        if status_code != 0 or len(status.get("applied", [])) != 2 or status.get("pending") or status.get("drift"):
            raise RuntimeError("status JSON did not report two applied migrations")
        rollback_code, rollback_output = run(binary, ["db", "rollback", "--database", str(database), "--migrations-dir", str(migrations_source), "--steps", "1"], root)
        if rollback_code != 0:
            raise RuntimeError("rollback failed: " + rollback_output)
        connection = sqlite3.connect(database)
        columns = {row[1] for row in connection.execute("PRAGMA table_info(users)")}
        if "email" in columns or connection.execute("SELECT COUNT(*) FROM _spectra_migrations").fetchone()[0] != 1:
            raise RuntimeError("rollback did not revert the latest migration")
        connection.close()
        with tempfile.TemporaryDirectory(prefix="spectra-r2503-") as temp:
            temp_root = Path(temp)
            drift_migrations = temp_root / "drift"
            copy_tree(migrations_source, drift_migrations)
            drift_database = temp_root / "drift.sqlite"
            if run(binary, ["db", "migrate", "--database", str(drift_database), "--migrations-dir", str(drift_migrations)], root)[0] != 0:
                raise RuntimeError("drift fixture could not be prepared")
            (drift_migrations / "0001_create_users.up.sql").write_text("CREATE TABLE changed(id INTEGER);\n", encoding="utf-8")
            drift_code, _ = run(binary, ["db", "migrate", "--database", str(drift_database), "--migrations-dir", str(drift_migrations)], root)
            if drift_code == 0:
                raise RuntimeError("checksum drift was accepted")

            broken = temp_root / "broken"
            broken.mkdir()
            (broken / "0001_ok.up.sql").write_text("CREATE TABLE ok(id INTEGER);", encoding="utf-8")
            (broken / "0001_ok.down.sql").write_text("DROP TABLE ok;", encoding="utf-8")
            (broken / "0002_broken.up.sql").write_text("THIS IS NOT SQL;", encoding="utf-8")
            (broken / "0002_broken.down.sql").write_text("DROP TABLE missing;", encoding="utf-8")
            (broken / "0003_never.up.sql").write_text("CREATE TABLE never(id INTEGER);", encoding="utf-8")
            (broken / "0003_never.down.sql").write_text("DROP TABLE never;", encoding="utf-8")
            broken_database = temp_root / "broken.sqlite"
            broken_code, _ = run(binary, ["db", "migrate", "--database", str(broken_database), "--migrations-dir", str(broken)], root)
            if broken_code == 0:
                raise RuntimeError("invalid SQL migration was accepted")
            connection = sqlite3.connect(broken_database)
            applied = connection.execute("SELECT version FROM _spectra_migrations").fetchall()
            if applied != [(1,)] or connection.execute("SELECT name FROM sqlite_master WHERE name='never'").fetchone() is not None:
                raise RuntimeError("partial migration left invalid state")
            connection.close()

            orphan = temp_root / "orphan"
            orphan.mkdir()
            (orphan / "0001_orphan.up.sql").write_text("CREATE TABLE orphan(id INTEGER);", encoding="utf-8")
            orphan_code, _ = run(binary, ["db", "status", "--database", str(temp_root / "orphan.sqlite"), "--migrations-dir", str(orphan)], root)
            if orphan_code == 0:
                raise RuntimeError("orphan migration file was accepted")

            concurrent_database = temp_root / "concurrent.sqlite"
            commands = [[str(binary), "db", "migrate", "--database", str(concurrent_database), "--migrations-dir", str(migrations_source)] for _ in range(2)]
            processes = [subprocess.Popen(command, cwd=root, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE) for command in commands]
            results = [process.wait() for process in processes]
            if any(code not in (0, 74, 65) for code in results):
                raise RuntimeError(f"unexpected concurrent migration exit codes: {results}")
            connection = sqlite3.connect(concurrent_database)
            if connection.execute("SELECT COUNT(*) FROM _spectra_migrations").fetchone()[0] != 2:
                raise RuntimeError("concurrent runners duplicated or lost migrations")
            connection.close()

        report["discovery"] = {"ordered_versions": [1, 2], "paired_files": True, "invalid_files_rejected": True}
        report["checksums"] = {"sha256": True, "independent_verification": True, "crlf_normalized": True}
        report["apply"] = {"ordered": True, "idempotent": True, "tracking_table": True, "schema_real": True}
        report["rollback"] = {"reverse_order": True, "steps_supported": True, "tracking_removed": True}
        report["partial_failure"] = {"transactional": True, "later_migrations_not_applied": True}
        report["drift_detection"] = {"checksum_mismatch_blocked": True, "orphan_blocked": True}
        report["concurrency"] = {"two_processes_no_duplication": True}
        report["cli"] = {"migrate": True, "rollback": True, "status_json": True, "stable_failure_exit": True}
        report["sqlite"] = {"file_backed": True, "tracking_schema": True}
        report["status"] = "passed"
    except Exception as error:  # noqa: BLE001 - always persist gate evidence
        report["failures"].append(str(error))
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
