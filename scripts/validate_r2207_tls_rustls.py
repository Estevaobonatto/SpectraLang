from __future__ import annotations

import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def fail(message: str) -> None:
    print(f"R-2207 validation failed: {message}", file=sys.stderr)
    sys.exit(1)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def run_command(args: list[str]) -> str:
    completed = subprocess.run(
        args,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if completed.returncode != 0:
        fail(f"command {' '.join(args)} failed:\n{completed.stdout}")
    return completed.stdout


def parse_toml(path: str):
    with (ROOT / path).open("rb") as fh:
        return tomllib.load(fh)


def validate_tls_surface() -> None:
    cargo = read("packages/spectra-api/Cargo.toml")
    for term in [
        "rustls",
        "webpki-roots",
        "rcgen",
    ]:
        require(term in cargo, f"Cargo.toml missing {term}")

    tls = read("packages/spectra-api/src/tls.rs")
    for term in [
        "pub enum TlsErrorKind",
        "pub struct TlsError",
        "pub struct TlsServerConfig",
        "pub struct TlsClientConfig",
        "pub struct HttpsResponse",
        "pub struct HttpsServerExchange",
        "server_config_from_der",
        "client_config_with_roots",
        "client_config_with_webpki_roots",
        "https_get",
        "https_round_trip",
        "serve_single_https_request",
        "ClientConnection::new",
        "ServerConnection::new",
        "RootCertStore",
        "ServerName::try_from",
        "DEFAULT_TLS_ALPN_HTTP11",
        "TlsErrorKind::CertificateValidation",
        "TlsErrorKind::Handshake",
        "TlsErrorKind::InvalidServerName",
        "webpki_roots::TLS_SERVER_ROOTS",
    ]:
        require(term in tls, f"missing TLS implementation term {term}")

    for test in [
        "self_signed_https_server_and_client_round_trip",
        "known_external_endpoint_validates_chain",
        "alpn_defaults_to_http11_on_server_and_client_configs",
        "handshake_failures_are_typed_and_keep_cause",
        "local_client_rejects_untrusted_self_signed_chain",
    ]:
        require(test in tls, f"missing R-2207 regression test {test}")


def validate_planning() -> None:
    roadmap = parse_toml("roadmap/roadmap.toml")
    items = {item["id"]: item for item in roadmap["items"]}
    r2207 = items.get("R-2207")
    require(r2207 is not None, "R-2207 missing from roadmap")
    require(r2207.get("status") == "complete", "R-2207 must be marked complete")
    require(r2207.get("owner") == "web", "R-2207 owner must remain web")
    require(r2207.get("dependencies") == ["R-2206", "R-2205"], "R-2207 dependencies changed")
    acceptance = "\n".join(r2207.get("acceptance", []))
    for term in [
        "self-signed certificate",
        "known external test endpoint",
        "validates the chain",
        "ALPN advertises `http/1.1`",
        "typed errors with the underlying cause",
        "cargo test -p spectra-api tls",
        "scripts/validate_r2207_tls_rustls.py",
    ]:
        require(term in acceptance, f"R-2207 acceptance must mention {term}")

    backlog = read("docs/roadmap-backlog.md")
    require("## R-2207 TLS via rustls (HTTPS Server and Client)" in backlog, "backlog R-2207 missing")
    r2207_block = backlog.split("## R-2207 TLS via rustls (HTTPS Server and Client)", 1)[1].split("## R-2208", 1)[0]
    for term in [
        "Status: `complete`",
        "packages/spectra-api/src/tls.rs",
        "TlsServerConfig",
        "TlsClientConfig",
        "HttpsResponse",
        "TlsErrorKind",
        "validate_r2207_tls_rustls.py",
    ]:
        require(term in r2207_block, f"backlog R-2207 block must mention {term}")

    plan = read("docs/production-ai-implementation-plan.md")
    require(
        "R-2207` TLS via `rustls` (complete;" in plan,
        "implementation plan must mark R-2207 complete",
    )


def validate_runner() -> None:
    runner = read("run_tests.ps1")
    require(
        "validate_r2207_tls_rustls.py" in runner,
        "run_tests.ps1 must run R-2207 validator",
    )
    require(
        'Teste = "validate_r2207_tls_rustls"' in runner,
        "run_tests.ps1 must record R-2207 result",
    )


def main() -> None:
    validate_tls_surface()
    run_command(["cargo", "test", "-q", "-p", "spectra-api", "tls", "--offline"])
    run_command(
        [
            "cargo",
            "test",
            "-q",
            "-p",
            "spectra-api",
            "tls::tests::known_external_endpoint_validates_chain",
            "--offline",
            "--",
            "--ignored",
        ]
    )
    validate_planning()
    validate_runner()
    print("validated R-2207 TLS via rustls")


if __name__ == "__main__":
    main()
