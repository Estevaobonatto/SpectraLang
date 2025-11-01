# Script de teste automatizado para SpectraLang
# Executa todos os arquivos de validação e reporta resultados

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "   SPECTRALANG - TESTE DE VALIDAÇÃO" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$testDir = "tests\validation"
$testFiles = Get-ChildItem -Path $testDir -Filter "*.spectra" | Sort-Object Name

$passed = 0
$failed = 0
$results = @()

Write-Host "Executando $($testFiles.Count) testes..." -ForegroundColor Yellow
Write-Host ""

foreach ($file in $testFiles) {
    $testName = $file.Name
    Write-Host "Testando: $testName" -NoNewline
    
    $output = cargo run $file.FullName 2>&1 | Out-String
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host " ✅ PASSOU" -ForegroundColor Green
        $passed++
        $results += [PSCustomObject]@{
            Teste = $testName
            Status = "PASSOU"
            Erro = ""
        }
    } else {
        Write-Host " ❌ FALHOU" -ForegroundColor Red
        $failed++
        
        # Extrai primeira linha de erro
        $errorLine = ($output -split "`n" | Where-Object { $_ -match "error:|Error:" } | Select-Object -First 1)
        if (-not $errorLine) {
            $errorLine = ($output -split "`n" | Where-Object { $_ -match "Expected|Undefined" } | Select-Object -First 1)
        }
        
        $results += [PSCustomObject]@{
            Teste = $testName
            Status = "FALHOU"
            Erro = $errorLine
        }
        
        Write-Host "   Erro: $errorLine" -ForegroundColor DarkRed
    }
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "            RESUMO DOS TESTES" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Total de testes: $($testFiles.Count)" -ForegroundColor White
Write-Host "Passou: $passed" -ForegroundColor Green
Write-Host "Falhou: $failed" -ForegroundColor Red

$percentSuccess = [math]::Round(($passed / $testFiles.Count) * 100, 2)
Write-Host "Taxa de sucesso: $percentSuccess%" -ForegroundColor $(if ($percentSuccess -eq 100) { "Green" } else { "Yellow" })

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan

# Mostra tabela de resultados
Write-Host ""
$results | Format-Table -AutoSize

# Salva relatório
$reportPath = "TEST_RESULTS.txt"
$results | Out-File -FilePath $reportPath -Encoding UTF8
Write-Host "Relatório salvo em: $reportPath" -ForegroundColor Cyan
