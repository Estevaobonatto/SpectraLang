param(
    [Parameter(Mandatory = $true)]
    [string]$Binary
)

$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot
$failed = $false

function Invoke-Check([string]$Name, [string]$FileName, [string[]]$Arguments) {
    Write-Host "  $Name" -NoNewline
    & $FileName @Arguments 2>&1 | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -eq 0) {
        Write-Host " PASSOU" -ForegroundColor Green
        return $true
    }
    Write-Host " FALHOU (exit code $LASTEXITCODE)" -ForegroundColor Red
    return $false
}

Push-Location $root
try {
    Write-Host "--- R-2701 focused phase gate ---" -ForegroundColor Yellow
    if (-not (Invoke-Check "r2701_runtime" "cargo" @("test", "-p", "spectra-runtime"))) { $failed = $true }
    if (-not (Invoke-Check "r2701_http_client" "cargo" @("test", "-p", "spectra-api", "--lib", "client::tests::client_injects_w3c_trace_context_and_emits_client_span", "--", "--exact"))) { $failed = $true }
    if (-not (Invoke-Check "r2701_http_concurrency" "cargo" @("test", "-p", "spectra-api", "--lib", "client::tests::concurrent_requests_isolate_trace_context", "--", "--exact"))) { $failed = $true }
    if (-not (Invoke-Check "r2701_api" "cargo" @("test", "-p", "spectra-api", "--lib", "--", "--test-threads=1"))) { $failed = $true }
    if (-not (Invoke-Check "r2701_db" "cargo" @("test", "-p", "spectra-db"))) { $failed = $true }
    if (-not (Invoke-Check "r2701_cli" "cargo" @("check", "-p", "spectra-cli"))) { $failed = $true }

    foreach ($mode in @("success", "http_500", "invalid_content_type", "connection_drop", "delayed_response")) {
        $report = Join-Path $root "target\r2701-tracing\$mode.json"
        if (-not (Invoke-Check "r2701_validator_$mode" "python" @("scripts\validate_r2701_tracing.py", "--binary", $Binary, "--fixture", "tests\validation\193_opentelemetry_tracing.spectra", "--report", $report, "--mode", $mode))) { $failed = $true }
        if (-not (Test-Path $report)) {
            Write-Host "  r2701_report_$mode FALHOU (missing)" -ForegroundColor Red
            $failed = $true
        } else {
            try {
                $json = Get-Content -LiteralPath $report -Raw | ConvertFrom-Json
                if ($json.status -ne "passed") {
                    Write-Host "  r2701_report_$mode FALHOU (status=$($json.status))" -ForegroundColor Red
                    $failed = $true
                }
            } catch {
                Write-Host "  r2701_report_$mode FALHOU (invalid JSON)" -ForegroundColor Red
                $failed = $true
            }
        }
    }

    if (-not (Invoke-Check "r2701_diff_check" "git" @("diff", "--check"))) { $failed = $true }
} finally {
    Pop-Location
}

if ($failed) {
    Write-Host "R-2701 focused gate: FALHOU" -ForegroundColor Red
    exit 1
}
Write-Host "R-2701 focused gate: PASSOU" -ForegroundColor Green
exit 0
