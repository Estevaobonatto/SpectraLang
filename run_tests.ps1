# Script de teste automatizado para SpectraLang
# Cobre todos os diretórios de testes:
#   tests/validation/   — devem COMPILAR com sucesso
#   tests/control_flow/ — devem COMPILAR com sucesso
#   tests/errors/       — devem FALHAR na compilação (erros esperados)
#   tests/semantic/     — executados e reportados sem expectativa forçada

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "   SPECTRALANG - SUITE DE TESTES" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$totalPassed  = 0
$totalFailed  = 0
$totalInfo    = 0
$results      = @()

# ---------------------------------------------------------------------------
# Função auxiliar: executa um arquivo .spectra e retorna o resultado
# ---------------------------------------------------------------------------
function Invoke-SpectraFile([string]$filePath) {
    $output = cargo run -- $filePath 2>&1 | Out-String
    return [PSCustomObject]@{
        ExitCode = $LASTEXITCODE
        Output   = $output
    }
}

function Get-FirstError([string]$output) {
    $line = ($output -split "`n" | Where-Object { $_ -match "error:|Error:" } | Select-Object -First 1)
    if (-not $line) {
        $line = ($output -split "`n" | Where-Object { $_ -match "Expected|Undefined" } | Select-Object -First 1)
    }
    return $line.Trim()
}

# ---------------------------------------------------------------------------
# Grupo 1: testes que devem compilar com SUCESSO
# ---------------------------------------------------------------------------
$successDirs = @("tests\validation", "tests\control_flow")

foreach ($dir in $successDirs) {
    if (-not (Test-Path $dir)) { continue }
    $files = Get-ChildItem -Path $dir -Filter "*.spectra" | Sort-Object Name
    Write-Host ""
    Write-Host "--- $dir ($($files.Count) testes: devem passar) ---" -ForegroundColor Yellow

    foreach ($file in $files) {
        Write-Host "  $($file.Name)" -NoNewline
        $r = Invoke-SpectraFile $file.FullName

        if ($r.ExitCode -eq 0) {
            Write-Host " ✅ PASSOU" -ForegroundColor Green
            $totalPassed++
            $results += [PSCustomObject]@{ Diretório = $dir; Teste = $file.Name; Status = "PASSOU"; Detalhe = "" }
        } else {
            $err = Get-FirstError $r.Output
            Write-Host " ❌ FALHOU" -ForegroundColor Red
            Write-Host "     $err" -ForegroundColor DarkRed
            $totalFailed++
            $results += [PSCustomObject]@{ Diretório = $dir; Teste = $file.Name; Status = "FALHOU"; Detalhe = $err }
        }
    }
}

# ---------------------------------------------------------------------------
# Grupo 2: testes de erro — devem FALHAR na compilação
# ---------------------------------------------------------------------------
$errorDir = "tests\errors"
if (Test-Path $errorDir) {
    $files = Get-ChildItem -Path $errorDir -Filter "*.spectra" | Sort-Object Name
    Write-Host ""
    Write-Host "--- $errorDir ($($files.Count) testes: devem falhar) ---" -ForegroundColor Yellow

    foreach ($file in $files) {
        Write-Host "  $($file.Name)" -NoNewline
        $r = Invoke-SpectraFile $file.FullName

        if ($r.ExitCode -ne 0) {
            Write-Host " ✅ PASSOU (erro esperado)" -ForegroundColor Green
            $totalPassed++
            $results += [PSCustomObject]@{ Diretório = $errorDir; Teste = $file.Name; Status = "PASSOU"; Detalhe = "erro esperado detectado" }
        } else {
            Write-Host " ❌ FALHOU (deveria produzir erro, mas compilou)" -ForegroundColor Red
            $totalFailed++
            $results += [PSCustomObject]@{ Diretório = $errorDir; Teste = $file.Name; Status = "FALHOU"; Detalhe = "compilou sem erro — erro esperado não detectado" }
        }
    }
}

# ---------------------------------------------------------------------------
# Grupo 3: testes semânticos — informativo apenas
# ---------------------------------------------------------------------------
$semanticDir = "tests\semantic"
if (Test-Path $semanticDir) {
    $files = Get-ChildItem -Path $semanticDir -Filter "*.spectra" | Sort-Object Name
    Write-Host ""
    Write-Host "--- $semanticDir ($($files.Count) testes: informativo) ---" -ForegroundColor Yellow

    foreach ($file in $files) {
        Write-Host "  $($file.Name)" -NoNewline
        $r = Invoke-SpectraFile $file.FullName

        if ($r.ExitCode -eq 0) {
            Write-Host " ℹ COMPILOU" -ForegroundColor Cyan
            $totalInfo++
            $results += [PSCustomObject]@{ Diretório = $semanticDir; Teste = $file.Name; Status = "INFO:COMPILOU"; Detalhe = "" }
        } else {
            $err = Get-FirstError $r.Output
            Write-Host " ℹ ERRO" -ForegroundColor DarkYellow
            $totalInfo++
            $results += [PSCustomObject]@{ Diretório = $semanticDir; Teste = $file.Name; Status = "INFO:ERRO"; Detalhe = $err }
        }
    }
}

# ---------------------------------------------------------------------------
# Resumo
# ---------------------------------------------------------------------------
$totalDecisive = $totalPassed + $totalFailed
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "            RESUMO DOS TESTES" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Testes com resultado esperado: $totalDecisive" -ForegroundColor White
Write-Host "  Passou : $totalPassed" -ForegroundColor Green
Write-Host "  Falhou : $totalFailed" -ForegroundColor $(if ($totalFailed -eq 0) { "Green" } else { "Red" })
Write-Host "Testes informativos (semantic): $totalInfo" -ForegroundColor Cyan

if ($totalDecisive -gt 0) {
    $pct = [math]::Round(($totalPassed / $totalDecisive) * 100, 1)
    Write-Host "Taxa de sucesso: $pct%" -ForegroundColor $(if ($pct -eq 100) { "Green" } else { "Yellow" })
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan

# Tabela completa
Write-Host ""
$results | Format-Table -AutoSize

# Salva relatório
$reportPath = "TEST_RESULTS.txt"
$results | Out-File -FilePath $reportPath -Encoding UTF8
Write-Host "Relatório salvo em: $reportPath" -ForegroundColor Cyan
