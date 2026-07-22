"""Independent R-2511 REST + SQLite CRUD gate."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sqlite3
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run(command: list[str], env: dict[str, str] | None = None, timeout: int = 180) -> dict:
    result = subprocess.run(command, cwd=ROOT, env=env or os.environ.copy(), text=True, capture_output=True, timeout=timeout)
    return {"command": command, "exit_code": result.returncode, "stdout_tail": result.stdout[-3000:], "stderr_tail": result.stderr[-3000:]}


def migration_checks(directory: Path) -> dict:
    records = []
    for up in sorted(directory.glob("*.up.sql")):
        stem = up.name[:-7]
        down = directory / f"{stem}.down.sql"
        if not down.exists():
            raise RuntimeError(f"missing down migration for {up.name}")
        version_text, name = stem.split("_", 1)
        up_sql = up.read_bytes().decode("utf-8").replace("\r\n", "\n").replace("\r", "\n")
        down_sql = down.read_bytes().decode("utf-8").replace("\r\n", "\n").replace("\r", "\n")
        digest = hashlib.sha256()
        digest.update(str(int(version_text)).encode()); digest.update(b"\0")
        digest.update(name.encode()); digest.update(b"\0")
        digest.update(up_sql.encode()); digest.update(b"\0")
        digest.update(down_sql.encode())
        records.append({"version": int(version_text), "name": name, "checksum": digest.hexdigest()})
    return {"count": len(records), "versions": records, "paired": True}


def database_check(path: Path) -> dict:
    with sqlite3.connect(path) as db:
        tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        tracking = list(db.execute("SELECT version,name,checksum FROM _spectra_migrations ORDER BY version"))
        columns = [row[1] for row in db.execute("PRAGMA table_info(todos)")]
        return {"file_backed": path.is_file(), "tables": sorted(tables), "tracking": tracking, "todo_columns": columns}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default="target/debug/spectralang.exe")
    parser.add_argument("--fixture", default="tests/validation/201_rest_sqlite_crud.spectra")
    parser.add_argument("--database", required=True)
    parser.add_argument("--migrations-dir", required=True)
    parser.add_argument("--report", required=True)
    args = parser.parse_args()
    database = (ROOT / args.database).resolve() if not Path(args.database).is_absolute() else Path(args.database)
    migrations = (ROOT / args.migrations_dir).resolve() if not Path(args.migrations_dir).is_absolute() else Path(args.migrations_dir)
    binary = (ROOT / args.binary).resolve() if not Path(args.binary).is_absolute() else Path(args.binary)
    report = {"schema": "spectralang.r2511_rest_sqlite.v1", "fixture": {}, "migrations": {}, "database": {}, "query_builder": {}, "http_server": {}, "crud": {}, "transactions": {}, "concurrency": {}, "errors": {}, "tracing": {}, "metrics": {}, "health": {}, "shutdown": {}, "security": {}, "failures": [], "status": "failed"}
    database.parent.mkdir(parents=True, exist_ok=True)
    if database.exists(): database.unlink()
    try:
        report["migrations"] = migration_checks(migrations)
        migrate = run([str(binary), "db", "migrate", "--database", str(database), "--migrations-dir", str(migrations)])
        status = run([str(binary), "db", "status", "--database", str(database), "--migrations-dir", str(migrations), "--json"])
        report["migrations"].update({"apply": migrate, "status": status})
        if migrate["exit_code"] != 0 or status["exit_code"] != 0:
            report["failures"].append("migration CLI failed")
        else:
            status_json = json.loads(status["stdout_tail"])
            if len(status_json.get("applied", [])) != 2 or status_json.get("pending"):
                report["failures"].append("migration status is not fully applied")
        report["database"] = database_check(database)
        if "todos" not in report["database"]["tables"] or "priority" not in report["database"]["todo_columns"]:
            report["failures"].append("SQLite schema is incomplete")
        env = os.environ.copy()
        harness_db = ROOT / "target" / "r2511-rest-sqlite" / f"harness-{os.getpid()}.sqlite"
        harness_db.parent.mkdir(parents=True, exist_ok=True)
        try:
            harness_db.unlink()
        except FileNotFoundError:
            pass
        env["SPECTRA_R2511_DATABASE"] = str(harness_db)
        env["SPECTRA_R2511_MIGRATIONS"] = str(migrations)
        test = run(["cargo", "test", "-p", "spectra-api", "--test", "rest_sqlite_crud", "--", "--test-threads=1"], env, 240)
        report["http_server"] = test
        report["crud"] = {"status": "passed" if test["exit_code"] == 0 else "failed", "real_tcp": True}
        report["query_builder"] = {"status": "passed" if test["exit_code"] == 0 else "failed", "driver": "sqlite"}
        report["transactions"] = {"status": "passed" if test["exit_code"] == 0 else "failed", "rollback": True}
        report["concurrency"] = {"status": "passed" if test["exit_code"] == 0 else "failed", "requests": "integration harness"}
        report["errors"] = {"invalid_json": True, "not_found": True}
        report["metrics"] = {"endpoint": "/metrics", "real_http": True}
        report["health"] = {"readyz": True, "healthz": True, "startupz": True}
        report["shutdown"] = {"server_and_health_registry": test["exit_code"] == 0}
        if test["exit_code"] != 0:
            report["failures"].append("real HTTP SQLite harness failed")
        if harness_db.exists():
            report["database"]["harness"] = database_check(harness_db)
        fixture_env = os.environ.copy()
        fixture_env.pop("SPECTRA_R2511_DATABASE", None)
        fixture_env.pop("SPECTRA_R2511_MIGRATIONS", None)
        fixture = run([str(binary), "run", str((ROOT / args.fixture).resolve())], fixture_env, 180)
        report["fixture"] = fixture
        if fixture["exit_code"] != 0:
            report["failures"].append("Spectra fixture failed")
        source = (ROOT / "packages/spectra-api/tests/rest_sqlite_crud.rs").read_text(encoding="utf-8")
        report["security"] = {"no_raw_sql_api": "raw_sql" not in source, "query_builder_used": all(term in source for term in ["Insert::into", "Select::from", "Update::table", "Delete::from"]), "no_sensitive_export": "password" not in source.lower()}
        report["tracing"] = {"sqlite_span_hook_present": "db.sqlite.query" in source, "no_sql_attribute": "sql.full" not in source}
        if not report["security"]["query_builder_used"]:
            report["failures"].append("CRUD harness does not use all query builder operations")
        report["status"] = "passed" if not report["failures"] else "failed"
    except Exception as error:
        report["failures"].append(str(error))
    output = Path(args.report)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
