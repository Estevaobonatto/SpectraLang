param(
    [Parameter(Mandatory = $true)]
    [string]$Binary,
    [string]$VersionProbeDockerContainer
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$report = Join-Path $root "target\r2505-postgres\report.json"

if (-not $env:SPECTRA_POSTGRES_URL) {
    Write-Error "R-2505 requires SPECTRA_POSTGRES_URL pointing to PostgreSQL 16."
    exit 1
}

Push-Location $root
try {
    & cargo test -p spectra-runtime background_task --no-fail-fast
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    & cargo test -p spectra-api tests::register_adds_all_api_host_calls_to_runtime_registry -- --exact
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $validatorArgs = @(
        "scripts\validate_r2505_postgres.py",
        "--binary", $Binary,
        "--database-url", $env:SPECTRA_POSTGRES_URL,
        "--fixture", "tests\validation\195_postgres_driver.spectra",
        "--require-database",
        "--report", $report
    )
    if ($VersionProbeDockerContainer) {
        $validatorArgs += @(
            "--version-probe-docker-container",
            $VersionProbeDockerContainer
        )
    }
    & python @validatorArgs
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $evidence = Get-Content -Raw -LiteralPath $report | ConvertFrom-Json
    if ($evidence.schema -ne "spectralang.r2505_postgres.v2" -or $evidence.status -ne "passed") {
        Write-Error "R-2505 report is not a certifying passed v2 report."
        exit 1
    }

    Write-Host "R-2505 PostgreSQL 16 production gate PASSOU" -ForegroundColor Green
    exit 0
}
finally {
    Pop-Location
}
