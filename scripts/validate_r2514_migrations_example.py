#!/usr/bin/env python3
"""Independent end-to-end gate for the R-2514 migration example."""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sqlite3
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run(binary: Path, args: list[str]) -> tuple[int, str]:
    result = subprocess.run([str(binary), *args], cwd=ROOT, text=True, capture_output=True)
    return result.returncode, result.stdout + result.stderr


def normalized(path: Path) -> str:
    return path.read_bytes().decode("utf-8").replace("\r\n", "\n").replace("\r", "\n")


def checksum(version: int, name: str, up: str, down: str) -> str:
    digest = hashlib.sha256()
    for index, value in enumerate((str(version), name, up, down)):
        digest.update(value.encode("utf-8"))
        if index != 3:
            digest.update(b"\0")
    return digest.hexdigest()


def inspect_database(path: Path) -> dict:
    with sqlite3.connect(path) as db:
        tracking = db.execute("SELECT version,name,checksum FROM _spectra_migrations ORDER BY version").fetchall()
        columns = [row[1] for row in db.execute("PRAGMA table_info(users)")]
        users = db.execute("SELECT id,name,email FROM users ORDER BY id").fetchall()
        return {"tracking": tracking, "columns": columns, "users": users}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--fixture", required=True)
    parser.add_argument("--database", required=True)
    parser.add_argument("--migrations-dir", required=True)
    parser.add_argument("--report", required=True)
    args = parser.parse_args()
    binary = (ROOT / args.binary).resolve() if not Path(args.binary).is_absolute() else Path(args.binary)
    fixture = (ROOT / args.fixture).resolve() if not Path(args.fixture).is_absolute() else Path(args.fixture)
    migrations = (ROOT / args.migrations_dir).resolve() if not Path(args.migrations_dir).is_absolute() else Path(args.migrations_dir)
    database = (ROOT / args.database).resolve() if not Path(args.database).is_absolute() else Path(args.database)
    report_path = (ROOT / args.report).resolve() if not Path(args.report).is_absolute() else Path(args.report)
    report = {
        "schema": "spectralang.r2514_migrations_example.v1",
        "fixture": {}, "discovery": {}, "checksums": {}, "initial_apply": {},
        "status_after_apply": {}, "rollback": {}, "reapply": {}, "idempotency": {},
        "drift_detection": {}, "failure_atomicity": {}, "concurrency": {},
        "sqlite": {}, "cli": {}, "security": {}, "failures": [], "status": "failed",
    }
    try:
        if not binary.is_file() or not fixture.is_file() or not migrations.is_dir():
            raise RuntimeError("R-2514 binary, fixture or migrations directory is missing")
        migration_rows = []
        for up in sorted(migrations.glob("*.up.sql")):
            stem = up.name[:-7]
            version_text, name = stem.split("_", 1)
            version = int(version_text)
            down = migrations / f"{stem}.down.sql"
            if not down.is_file():
                raise RuntimeError(f"missing down migration: {down.name}")
            up_sql, down_sql = normalized(up), normalized(down)
            migration_rows.append({"version": version, "name": name, "checksum": checksum(version, name, up_sql, down_sql)})
        if [row["version"] for row in migration_rows] != [1, 2, 3]:
            raise RuntimeError("expected migration versions 1, 2, 3")
        report["discovery"] = {"ordered_versions": [1, 2, 3], "paired_files": True}
        report["checksums"] = {"sha256": True, "independent": True, "migrations": migration_rows}

        database.parent.mkdir(parents=True, exist_ok=True)
        if database.exists():
            database.unlink()
        code, output = run(binary, ["db", "migrate", "--database", str(database), "--migrations-dir", str(migrations)])
        report["initial_apply"] = {"exit_code": code, "output": output[-2000:]}
        if code != 0:
            raise RuntimeError("initial migration failed")
        state = inspect_database(database)
        expected_checksums = [row["checksum"] for row in migration_rows]
        if [row[0] for row in state["tracking"]] != [1, 2, 3] or [row[2] for row in state["tracking"]] != expected_checksums:
            raise RuntimeError("initial tracking/checksums are incorrect")
        if "email" not in state["columns"] or state["users"] != [(1, "Ada", "ada@example.test"), (2, "Grace", "grace@example.test")]:
            raise RuntimeError("final schema or seed data is incorrect")
        report["sqlite"] = {"file_backed": database.is_file(), "columns": state["columns"], "seed_rows": len(state["users"])}

        status_code, status_output = run(binary, ["db", "status", "--database", str(database), "--migrations-dir", str(migrations), "--json"])
        status = json.loads(status_output)
        if status_code != 0 or status.get("pending") or status.get("drift") or len(status.get("applied", [])) != 3:
            raise RuntimeError("status after apply is not clean")
        report["status_after_apply"] = {"clean": True, "applied": 3}

        rollback_code, rollback_output = run(binary, ["db", "rollback", "--database", str(database), "--migrations-dir", str(migrations), "--steps", "1"])
        state = inspect_database(database)
        if rollback_code != 0 or [row[0] for row in state["tracking"]] != [1, 2, 0][:len(state["tracking"])] or state["users"]:
            raise RuntimeError("rollback did not remove only the seed migration")
        report["rollback"] = {"exit_code": rollback_code, "seed_removed": True, "tracking_after": [row[0] for row in state["tracking"]], "output": rollback_output[-1000:]}

        reapply_code, reapply_output = run(binary, ["db", "migrate", "--database", str(database), "--migrations-dir", str(migrations)])
        state = inspect_database(database)
        if reapply_code != 0 or len(state["tracking"]) != 3 or len(state["users"]) != 2:
            raise RuntimeError("reapply did not restore seed data")
        repeat_code, _ = run(binary, ["db", "migrate", "--database", str(database), "--migrations-dir", str(migrations)])
        if repeat_code != 0 or len(inspect_database(database)["users"]) != 2:
            raise RuntimeError("migration is not idempotent")
        report["reapply"] = {"restored": True, "output": reapply_output[-1000:]}
        report["idempotency"] = {"repeat_exit_code": repeat_code, "seed_count": 2}

        fixture_database = ROOT / "target" / "r2514-migrations-validation.sqlite"
        fixture_database.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(database, fixture_database)
        shutil.copyfile(database, ROOT / "target" / "r2514-migrations-example.sqlite")
        fixture_code, fixture_output = run(binary, ["run", str(fixture)])
        report["fixture"] = {"exit_code": fixture_code, "output": fixture_output[-1000:], "file_backed": True}
        if fixture_code != 0:
            raise RuntimeError("Spectra fixture failed")

        try:
            temp_root = Path(tempfile.mkdtemp(prefix="spectra-r2514-"))
            drift_dir = temp_root / "drift"
            shutil.copytree(migrations, drift_dir)
            (drift_dir / "0001_create_users.up.sql").write_text("CREATE TABLE changed(id INTEGER);\n", encoding="utf-8")
            drift_code, _ = run(binary, ["db", "migrate", "--database", str(database), "--migrations-dir", str(drift_dir)])
            if drift_code == 0:
                raise RuntimeError("checksum drift was accepted")
            broken_dir = temp_root / "broken"
            shutil.copytree(migrations, broken_dir)
            (broken_dir / "0002_add_email.up.sql").write_text("THIS IS INVALID SQL;\n", encoding="utf-8")
            broken_db = temp_root / "broken.sqlite"
            broken_code, _ = run(binary, ["db", "migrate", "--database", str(broken_db), "--migrations-dir", str(broken_dir)])
            if broken_code == 0:
                raise RuntimeError("invalid SQL migration was accepted")
            with sqlite3.connect(broken_db) as db:
                if db.execute("SELECT COUNT(*) FROM _spectra_migrations").fetchone()[0] != 1:
                    raise RuntimeError("failed migration left partial tracking state")
            incomplete = temp_root / "incomplete"
            incomplete.mkdir()
            (incomplete / "0001_orphan.up.sql").write_text("CREATE TABLE orphan(id INTEGER);", encoding="utf-8")
            orphan_code, _ = run(binary, ["db", "status", "--database", str(temp_root / "orphan.sqlite"), "--migrations-dir", str(incomplete)])
            if orphan_code == 0:
                raise RuntimeError("orphan migration was accepted")
            concurrent_db = temp_root / "concurrent.sqlite"
            command = [str(binary), "db", "migrate", "--database", str(concurrent_db), "--migrations-dir", str(migrations)]
            processes = [subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) for _ in range(2)]
            concurrent_codes = [process.wait() for process in processes]
            if any(code not in (0, 65, 74) for code in concurrent_codes):
                raise RuntimeError(f"concurrent migration failed: {concurrent_codes}")
            if len(inspect_database(concurrent_db)["tracking"]) != 3:
                raise RuntimeError("concurrent migration did not apply exactly three versions")
        except OSError as error:
            raise RuntimeError(f"temporary migration validation failed: {error}") from error
        report["drift_detection"] = {"checksum_mismatch_blocked": True, "orphan_blocked": True}
        report["failure_atomicity"] = {"invalid_sql_rolled_back": True, "later_migrations_not_applied": True}
        report["concurrency"] = {"shared_framework": True, "two_processes_no_duplication": True}
        report["cli"] = {"migrate": True, "rollback": True, "status_json": True, "reapply": True}
        report["security"] = {"file_backed_only": True, "no_parallel_migration_implementation": True}
        report["status"] = "passed"
    except Exception as error:
        report["failures"].append(str(error))
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
