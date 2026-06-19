# Script de teste automatizado para SpectraLang
# Cobre todos os diretorios de testes:
#   tests/validation/   - devem COMPILAR com sucesso
#   tests/control_flow/ - devem COMPILAR com sucesso
#   tests/projects/     - projetos multi-arquivo devem COMPILAR com sucesso
#   tests/errors/       - devem FALHAR na compilacao (erros esperados)
#   tests/semantic/     - compilados e reportados sem expectativa forcada
#   tests/cli/          - fixtures para validar comandos do CLI
#   scripts/validate_r2003_base_regression_audit.py - separa compile-only de runtime-zero
#   tools/spectra-interop/ - interop Rust/Python/C ABI
#
# Requer que o binario ja esteja compilado:
#   cargo build -p spectra-cli

$binary = (Resolve-Path ".\target\debug\spectralang.exe").Path
$timeoutSeconds = 10
$hostCommandTimeoutSeconds = 120
$env:PATH = "C:\Users\estev\.cargo\bin;" + $env:PATH
$experimentalFlags = @(
    "--enable-experimental", "switch",
    "--enable-experimental", "unless",
    "--enable-experimental", "do-while",
    "--enable-experimental", "loop"
)

if (-not (Test-Path $binary)) {
    Write-Host "Binario nao encontrado. Compilando..." -ForegroundColor Yellow
    & "C:\Users\estev\.cargo\bin\cargo.exe" build -p spectra-cli 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERRO: Falha ao compilar o compilador." -ForegroundColor Red
        exit 1
    }
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "   SPECTRALANG - SUITE DE TESTES" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$totalPassed  = 0
$totalFailed  = 0
$totalInfo    = 0
$totalSkipped = 0
$results      = @()

# ---------------------------------------------------------------------------
# Funcao auxiliar: compila um arquivo .spectra com timeout e retorna o resultado
# ---------------------------------------------------------------------------
function Invoke-SpectraFile([string]$filePath) {
    return Invoke-SpectraCommand -commandArgs @("compile", $filePath) -workingDir (Get-Location).Path -includeExperimental $true
}

function Invoke-SpectraCommand([string[]]$commandArgs, [string]$workingDir, [bool]$includeExperimental = $false, [string]$stdinText = $null) {
    $fullArgs = if ($includeExperimental) { $commandArgs + $experimentalFlags } else { $commandArgs }

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $binary
    $psi.WorkingDirectory = $workingDir
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.RedirectStandardInput = ($null -ne $stdinText)

    $quotedArgs = $fullArgs | ForEach-Object {
        if ($_ -match '[\s"]') {
            '"' + ($_ -replace '"', '\"') + '"'
        } else {
            $_
        }
    }
    $psi.Arguments = ($quotedArgs -join ' ')

    $proc = New-Object System.Diagnostics.Process
    $proc.StartInfo = $psi

    $timedOut = $false
    try {
        [void]$proc.Start()
        if ($null -ne $stdinText) {
            $proc.StandardInput.Write($stdinText)
            $proc.StandardInput.Close()
        }
        if (-not $proc.WaitForExit($timeoutSeconds * 1000)) {
            $timedOut = $true
            $proc.Kill()
            $proc.WaitForExit()
        }
    } catch {
        $timedOut = $true
    }

    $stdout = $proc.StandardOutput.ReadToEnd()
    $stderr = $proc.StandardError.ReadToEnd()
    $combined = "$stdout`n$stderr"

    return [PSCustomObject]@{
        ExitCode = if ($timedOut) { 124 } else { $proc.ExitCode }
        TimedOut = $timedOut
        Output   = $combined
    }
}

function Get-FirstError([string]$output) {
    $line = ($output -split "`n" | Where-Object { $_ -match "error\[|error:|Error:" } | Select-Object -First 1)
    if (-not $line) {
        $line = ($output -split "`n" | Where-Object { $_ -match "Expected|Undefined|not defined" } | Select-Object -First 1)
    }
    if ($line) {
        return $line.Trim().Substring(0, [Math]::Min(80, $line.Trim().Length))
    }
    return ""
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

        if ($r.TimedOut) {
            Write-Host " TIMEOUT" -ForegroundColor Red
            $totalFailed++
            $results += [PSCustomObject]@{ Diretorio = $dir; Teste = $file.Name; Status = "TIMEOUT"; Detalhe = "compilacao excedeu ${timeoutSeconds}s" }
        } elseif ($r.ExitCode -eq 0) {
            Write-Host " PASSOU" -ForegroundColor Green
            $totalPassed++
            $results += [PSCustomObject]@{ Diretorio = $dir; Teste = $file.Name; Status = "PASSOU"; Detalhe = "" }
        } else {
            $err = Get-FirstError $r.Output
            Write-Host " FALHOU" -ForegroundColor Red
            Write-Host "     $err" -ForegroundColor DarkRed
            $totalFailed++
            $results += [PSCustomObject]@{ Diretorio = $dir; Teste = $file.Name; Status = "FALHOU"; Detalhe = $err }
        }
    }
}

# ---------------------------------------------------------------------------
# Grupo 2: testes de erro - devem FALHAR na compilacao
# ---------------------------------------------------------------------------
$projectDir = "tests\projects\valid"
if (Test-Path $projectDir) {
    $projects = Get-ChildItem -Path $projectDir -Directory | Sort-Object Name
    Write-Host ""
    Write-Host "--- $projectDir ($($projects.Count) projetos: devem passar) ---" -ForegroundColor Yellow

    foreach ($project in $projects) {
        Write-Host "  $($project.Name)" -NoNewline
        $r = Invoke-SpectraCommand -commandArgs @("compile", $project.FullName) -workingDir (Get-Location).Path -includeExperimental $true

        if ($r.TimedOut) {
            Write-Host " TIMEOUT" -ForegroundColor Red
            $totalFailed++
            $results += [PSCustomObject]@{ Diretorio = $projectDir; Teste = $project.Name; Status = "TIMEOUT"; Detalhe = "compilacao do projeto excedeu ${timeoutSeconds}s" }
        } elseif ($r.ExitCode -eq 0) {
            Write-Host " PASSOU" -ForegroundColor Green
            $totalPassed++
            $results += [PSCustomObject]@{ Diretorio = $projectDir; Teste = $project.Name; Status = "PASSOU"; Detalhe = "" }
        } else {
            $err = Get-FirstError $r.Output
            Write-Host " FALHOU" -ForegroundColor Red
            Write-Host "     $err" -ForegroundColor DarkRed
            $totalFailed++
            $results += [PSCustomObject]@{ Diretorio = $projectDir; Teste = $project.Name; Status = "FALHOU"; Detalhe = $err }
        }
    }
}

# ---------------------------------------------------------------------------
# Grupo 3: testes de erro - devem FALHAR na compilacao
# ---------------------------------------------------------------------------
$errorDir = "tests\errors"
if (Test-Path $errorDir) {
    $files = Get-ChildItem -Path $errorDir -Filter "*.spectra" | Sort-Object Name
    Write-Host ""
    Write-Host "--- $errorDir ($($files.Count) testes: devem falhar) ---" -ForegroundColor Yellow

    foreach ($file in $files) {
        Write-Host "  $($file.Name)" -NoNewline
        $r = Invoke-SpectraFile $file.FullName

        if ($r.TimedOut) {
            Write-Host " FALHOU (timeout)" -ForegroundColor Red
            $totalFailed++
            $results += [PSCustomObject]@{ Diretorio = $errorDir; Teste = $file.Name; Status = "FALHOU"; Detalhe = "timeout - deveria falhar rapidamente" }
        } elseif ($r.ExitCode -ne 0) {
            Write-Host " PASSOU (erro esperado)" -ForegroundColor Green
            $totalPassed++
            $results += [PSCustomObject]@{ Diretorio = $errorDir; Teste = $file.Name; Status = "PASSOU"; Detalhe = "erro esperado detectado" }
        } else {
            Write-Host " FALHOU (deveria produzir erro, mas compilou)" -ForegroundColor Red
            $totalFailed++
            $results += [PSCustomObject]@{ Diretorio = $errorDir; Teste = $file.Name; Status = "FALHOU"; Detalhe = "compilou sem erro - erro esperado nao detectado" }
        }
    }
}

# ---------------------------------------------------------------------------
# Grupo 4: testes semanticos - informativo apenas
# ---------------------------------------------------------------------------
$semanticDir = "tests\semantic"
if (Test-Path $semanticDir) {
    $files = Get-ChildItem -Path $semanticDir -Filter "*.spectra" | Sort-Object Name
    Write-Host ""
    Write-Host "--- $semanticDir ($($files.Count) testes: informativo) ---" -ForegroundColor Yellow

    foreach ($file in $files) {
        Write-Host "  $($file.Name)" -NoNewline
        $r = Invoke-SpectraFile $file.FullName

        if ($r.TimedOut) {
            Write-Host " TIMEOUT" -ForegroundColor Red
            $totalInfo++
            $results += [PSCustomObject]@{ Diretorio = $semanticDir; Teste = $file.Name; Status = "INFO:TIMEOUT"; Detalhe = "compilacao excedeu ${timeoutSeconds}s" }
        } elseif ($r.ExitCode -eq 0) {
            Write-Host " COMPILOU" -ForegroundColor Cyan
            $totalInfo++
            $results += [PSCustomObject]@{ Diretorio = $semanticDir; Teste = $file.Name; Status = "INFO:COMPILOU"; Detalhe = "" }
        } else {
            $err = Get-FirstError $r.Output
            Write-Host " ERRO" -ForegroundColor DarkYellow
            $totalInfo++
            $results += [PSCustomObject]@{ Diretorio = $semanticDir; Teste = $file.Name; Status = "INFO:ERRO"; Detalhe = $err }
        }
    }
}

# ---------------------------------------------------------------------------
# Grupo 5: testes diretos do CLI
# ---------------------------------------------------------------------------
$cliTests = @(
    [PSCustomObject]@{
        Nome = "help"
        Args = @("--help")
        ExpectExit = 0
        Contains = "USAGE:"
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "list_experimental"
        Args = @("--list-experimental")
        ExpectExit = 0
        Contains = "none"
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "check_valid_file"
        Args = @("check", "tests\validation\60_pattern_control_surface.spectra")
        ExpectExit = 0
        Contains = ""
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "check_invalid_file"
        Args = @("check", "tests\errors\type_mismatch.spectra")
        ExpectExit = 65
        Contains = "error"
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "check_json_invalid_file"
        Args = @("check", "--json", "tests\errors\type_mismatch.spectra")
        ExpectExit = 65
        Contains = '"phase":"semantic"'
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "compile_json_invalid_file"
        Args = @("compile", "--json", "tests\errors\type_mismatch.spectra")
        ExpectExit = 65
        Contains = '"code":"E004"'
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "check_sarif_invalid_file"
        Args = @("check", "--sarif", "tests\errors\type_mismatch.spectra")
        ExpectExit = 65
        Contains = '"version": "2.1.0"'
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "lint_clean"
        Args = @("lint", "tests\cli\lint_clean.spectra")
        ExpectExit = 0
        Contains = ""
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "lint_warn_unused"
        Args = @("lint", "tests\cli\lint_warning_unused.spectra")
        ExpectExit = 0
        Contains = "unused-binding"
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "lint_deny_shadowing"
        Args = @("lint", "--deny", "shadowing", "tests\cli\lint_warning_shadowing.spectra")
        ExpectExit = 65
        Contains = "shadowing"
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "fmt_check_formatted"
        Args = @("fmt", "--check", "tests\cli\fmt_formatted.spectra")
        ExpectExit = 0
        Contains = ""
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "fmt_check_unformatted"
        Args = @("fmt", "--check", "tests\cli\fmt_unformatted.spectra")
        ExpectExit = 65
        Contains = "fmt_unformatted"
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "fmt_stdout"
        Args = @("fmt", "--stdout", "tests\cli\fmt_unformatted.spectra")
        ExpectExit = 0
        Contains = "pub fn main() -> int {"
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "fmt_stdin"
        Args = @()
        ExpectExit = 0
        Contains = "pub fn main() -> int {"
        UseStdin = $true
        StdinFile = "tests\cli\fmt_unformatted.spectra"
    }
    [PSCustomObject]@{
        Nome = "compile_project_dir"
        Args = @("compile", "tests\projects\valid\basic_project")
        ExpectExit = 0
        Contains = ""
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "package_lock_workspace"
        Args = @("package", "lock", "--root", "tests\projects\valid\package_workspace")
        ExpectExit = 0
        Contains = "Locked"
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "package_build_workspace"
        Args = @("package", "build", "--root", "tests\projects\valid\package_workspace")
        ExpectExit = 0
        Contains = "Finished"
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "package_check_workspace"
        Args = @("package", "check", "--root", "tests\projects\valid\package_workspace")
        ExpectExit = 0
        Contains = "Finished"
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "package_doc_workspace"
        Args = @("package", "doc", "--root", "tests\projects\valid\package_workspace")
        ExpectExit = 0
        Contains = "Written docs"
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "bench_json_valid_file"
        Args = @("bench", "--bench-json", "target\bench_cli_test.json", "tests\validation\01_basic_syntax.spectra")
        ExpectExit = 0
        Contains = "Written bench report"
        UseStdin = $false
    }
    [PSCustomObject]@{
        Nome = "runtime_nonzero_trace"
        Args = @("run", "tests\cli\runtime_nonzero.spectra")
        ExpectExit = 7
        Contains = "0: main()"
        UseStdin = $false
    }
)

Write-Host ""
Write-Host "--- CLI ($($cliTests.Count) testes: devem seguir a expectativa do comando) ---" -ForegroundColor Yellow

foreach ($cliTest in $cliTests) {
    Write-Host "  $($cliTest.Nome)" -NoNewline

    if ($cliTest.UseStdin) {
        $stdinPath = Join-Path (Get-Location).Path $cliTest.StdinFile
        $stdinText = Get-Content -LiteralPath $stdinPath -Raw
        $r = Invoke-SpectraCommand -commandArgs @("fmt", "--stdin") -workingDir (Get-Location).Path -stdinText $stdinText
    } else {
        $needsExperimental = ($cliTest.Args.Count -gt 0 -and @("compile", "check", "lint", "run") -contains $cliTest.Args[0])
        $r = Invoke-SpectraCommand -commandArgs $cliTest.Args -workingDir (Get-Location).Path -includeExperimental $needsExperimental
    }

    $exitMatches = $false
    if ($cliTest.ExpectExit -eq 65) {
        $exitMatches = ($r.ExitCode -ne 0 -and -not $r.TimedOut)
    } else {
        $exitMatches = ($r.ExitCode -eq $cliTest.ExpectExit -and -not $r.TimedOut)
    }

    $containsMatches = $true
    if ($cliTest.Contains) {
        $containsMatches = $r.Output -match [Regex]::Escape($cliTest.Contains)
    }

    if ($r.TimedOut) {
        Write-Host " TIMEOUT" -ForegroundColor Red
        $totalFailed++
        $results += [PSCustomObject]@{ Diretorio = "tests\cli"; Teste = $cliTest.Nome; Status = "FALHOU"; Detalhe = "timeout" }
    } elseif ($exitMatches -and $containsMatches) {
        Write-Host " PASSOU" -ForegroundColor Green
        $totalPassed++
        $results += [PSCustomObject]@{ Diretorio = "tests\cli"; Teste = $cliTest.Nome; Status = "PASSOU"; Detalhe = "" }
    } else {
        $detail = if (-not $exitMatches) { "exit code inesperado: $($r.ExitCode)" } else { "saida nao contem: $($cliTest.Contains)" }
        Write-Host " FALHOU" -ForegroundColor Red
        Write-Host "     $detail" -ForegroundColor DarkRed
        $totalFailed++
        $results += [PSCustomObject]@{ Diretorio = "tests\cli"; Teste = $cliTest.Nome; Status = "FALHOU"; Detalhe = $detail }
    }
}

# ---------------------------------------------------------------------------
# Grupo 6: teste do comando `new`
# ---------------------------------------------------------------------------
$newProjectRoot = Join-Path $env:TEMP "spectra_cli_new_test_$PID"
if (Test-Path $newProjectRoot) {
    Remove-Item -LiteralPath $newProjectRoot -Recurse -Force
}

Write-Host ""
Write-Host "--- CLI scaffold (1 teste: deve passar) ---" -ForegroundColor Yellow
Write-Host "  new_project" -NoNewline
$newResult = Invoke-SpectraCommand -commandArgs @("new", $newProjectRoot) -workingDir (Get-Location).Path
$newManifest = Join-Path $newProjectRoot "spectra.toml"
$newMain = Join-Path $newProjectRoot "src\main.spectra"

if ($newResult.TimedOut) {
    Write-Host " TIMEOUT" -ForegroundColor Red
    $totalFailed++
    $results += [PSCustomObject]@{ Diretorio = "tests\cli"; Teste = "new_project"; Status = "FALHOU"; Detalhe = "timeout" }
} elseif ($newResult.ExitCode -eq 0 -and (Test-Path $newManifest) -and (Test-Path $newMain)) {
    Write-Host " PASSOU" -ForegroundColor Green
    $totalPassed++
    $results += [PSCustomObject]@{ Diretorio = "tests\cli"; Teste = "new_project"; Status = "PASSOU"; Detalhe = "" }
} else {
    $detail = "scaffold incompleto ou exit code inesperado: $($newResult.ExitCode)"
    Write-Host " FALHOU" -ForegroundColor Red
    Write-Host "     $detail" -ForegroundColor DarkRed
    $totalFailed++
    $results += [PSCustomObject]@{ Diretorio = "tests\cli"; Teste = "new_project"; Status = "FALHOU"; Detalhe = $detail }
}

if (Test-Path $newProjectRoot) {
    Remove-Item -LiteralPath $newProjectRoot -Recurse -Force
}

# ---------------------------------------------------------------------------
# Grupo 7: package registry local - publish/add/build
# ---------------------------------------------------------------------------
$packageRegistryRoot = Join-Path $env:TEMP "spectra_pkg_registry_test_$PID"
$packageConsumerRoot = Join-Path $env:TEMP "spectra_pkg_consumer_test_$PID"
if (Test-Path $packageRegistryRoot) {
    Remove-Item -LiteralPath $packageRegistryRoot -Recurse -Force
}
if (Test-Path $packageConsumerRoot) {
    Remove-Item -LiteralPath $packageConsumerRoot -Recurse -Force
}

Write-Host ""
Write-Host "--- Package registry (4 testes: devem passar) ---" -ForegroundColor Yellow

$lockPath = "tests\projects\valid\package_workspace\spectra.lock"
$lockBefore = if (Test-Path $lockPath) { Get-Content -LiteralPath $lockPath -Raw } else { "" }
$lockAgain = Invoke-SpectraCommand -commandArgs @("package", "lock", "--root", "tests\projects\valid\package_workspace") -workingDir (Get-Location).Path
$lockAfter = if (Test-Path $lockPath) { Get-Content -LiteralPath $lockPath -Raw } else { "" }
Write-Host "  package_lock_deterministic" -NoNewline
if (-not $lockAgain.TimedOut -and $lockAgain.ExitCode -eq 0 -and $lockBefore -eq $lockAfter) {
    Write-Host " PASSOU" -ForegroundColor Green
    $totalPassed++
    $results += [PSCustomObject]@{ Diretorio = "package"; Teste = "package_lock_deterministic"; Status = "PASSOU"; Detalhe = "" }
} else {
    Write-Host " FALHOU" -ForegroundColor Red
    $totalFailed++
    $results += [PSCustomObject]@{ Diretorio = "package"; Teste = "package_lock_deterministic"; Status = "FALHOU"; Detalhe = "spectra.lock mudou entre resolucoes equivalentes" }
}

$publishSource = (Resolve-Path "tests\projects\valid\package_workspace\packages\core").Path
$publishResult = Invoke-SpectraCommand -commandArgs @("package", "publish", "--root", $publishSource, "--registry", $packageRegistryRoot) -workingDir (Get-Location).Path
Write-Host "  package_publish_core" -NoNewline
if (-not $publishResult.TimedOut -and $publishResult.ExitCode -eq 0) {
    Write-Host " PASSOU" -ForegroundColor Green
    $totalPassed++
    $results += [PSCustomObject]@{ Diretorio = "package"; Teste = "package_publish_core"; Status = "PASSOU"; Detalhe = "" }
} else {
    Write-Host " FALHOU" -ForegroundColor Red
    $totalFailed++
    $results += [PSCustomObject]@{ Diretorio = "package"; Teste = "package_publish_core"; Status = "FALHOU"; Detalhe = Get-FirstError $publishResult.Output }
}

$newConsumerResult = Invoke-SpectraCommand -commandArgs @("new", $packageConsumerRoot) -workingDir (Get-Location).Path
$addResult = Invoke-SpectraCommand -commandArgs @("package", "add", "core", "--root", $packageConsumerRoot, "--version", "0.1.0", "--registry", $packageRegistryRoot) -workingDir (Get-Location).Path
Write-Host "  package_add_registry_core" -NoNewline
if (-not $newConsumerResult.TimedOut -and $newConsumerResult.ExitCode -eq 0 -and -not $addResult.TimedOut -and $addResult.ExitCode -eq 0) {
    Write-Host " PASSOU" -ForegroundColor Green
    $totalPassed++
    $results += [PSCustomObject]@{ Diretorio = "package"; Teste = "package_add_registry_core"; Status = "PASSOU"; Detalhe = "" }
} else {
    Write-Host " FALHOU" -ForegroundColor Red
    $totalFailed++
    $detail = if ($newConsumerResult.ExitCode -ne 0) { Get-FirstError $newConsumerResult.Output } else { Get-FirstError $addResult.Output }
    $results += [PSCustomObject]@{ Diretorio = "package"; Teste = "package_add_registry_core"; Status = "FALHOU"; Detalhe = $detail }
}

$buildConsumerResult = Invoke-SpectraCommand -commandArgs @("package", "build", "--root", $packageConsumerRoot) -workingDir (Get-Location).Path
Write-Host "  package_build_registry_consumer" -NoNewline
if (-not $buildConsumerResult.TimedOut -and $buildConsumerResult.ExitCode -eq 0) {
    Write-Host " PASSOU" -ForegroundColor Green
    $totalPassed++
    $results += [PSCustomObject]@{ Diretorio = "package"; Teste = "package_build_registry_consumer"; Status = "PASSOU"; Detalhe = "" }
} else {
    Write-Host " FALHOU" -ForegroundColor Red
    $totalFailed++
    $results += [PSCustomObject]@{ Diretorio = "package"; Teste = "package_build_registry_consumer"; Status = "FALHOU"; Detalhe = Get-FirstError $buildConsumerResult.Output }
}

if (Test-Path $packageRegistryRoot) {
    Remove-Item -LiteralPath $packageRegistryRoot -Recurse -Force
}
if (Test-Path $packageConsumerRoot) {
    Remove-Item -LiteralPath $packageConsumerRoot -Recurse -Force
}

# ---------------------------------------------------------------------------
# Grupo 8: interop Python / Rust / C ABI
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- Interop (Rust/Python/C ABI) ---" -ForegroundColor Yellow

function Invoke-HostCommand([string]$name, [string]$fileName, [string[]]$arguments, [string]$workingDir) {
    Write-Host "  $name" -NoNewline

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $fileName
    $psi.WorkingDirectory = $workingDir
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $quotedArgs = $arguments | ForEach-Object {
        if ($_ -match '[\s"]') {
            '"' + ($_ -replace '"', '\"') + '"'
        } else {
            $_
        }
    }
    $psi.Arguments = ($quotedArgs -join ' ')

    $proc = New-Object System.Diagnostics.Process
    $proc.StartInfo = $psi
    $timedOut = $false

    try {
        [void]$proc.Start()
        if (-not $proc.WaitForExit($hostCommandTimeoutSeconds * 1000)) {
            $timedOut = $true
            $proc.Kill()
            $proc.WaitForExit()
        }
    } catch {
        Write-Host " FALHOU" -ForegroundColor Red
        return [PSCustomObject]@{ Status = "FALHOU"; Detail = $_.Exception.Message }
    }

    $stdout = $proc.StandardOutput.ReadToEnd()
    $stderr = $proc.StandardError.ReadToEnd()
    $combined = "$stdout`n$stderr"

    if ($timedOut) {
        Write-Host " TIMEOUT" -ForegroundColor Red
        return [PSCustomObject]@{ Status = "TIMEOUT"; Detail = "comando excedeu ${hostCommandTimeoutSeconds}s" }
    }
    if ($proc.ExitCode -eq 0) {
        Write-Host " PASSOU" -ForegroundColor Green
        return [PSCustomObject]@{ Status = "PASSOU"; Detail = "" }
    }
    $err = Get-FirstError $combined
    if (-not $err) {
        $err = "exit code inesperado: $($proc.ExitCode)"
    }
    Write-Host " FALHOU" -ForegroundColor Red
    Write-Host "     $err" -ForegroundColor DarkRed
    return [PSCustomObject]@{ Status = "FALHOU"; Detail = $err }
}

$cargoPath = (Get-Command cargo -ErrorAction SilentlyContinue).Source
if (-not $cargoPath) {
    $cargoPath = "C:\Users\estev\.cargo\bin\cargo.exe"
}

$interopChecks = @(
    [PSCustomObject]@{ Nome = "cargo_test_spectra_interop"; File = $cargoPath; Args = @("test", "-p", "spectra-interop") }
    [PSCustomObject]@{ Nome = "cargo_test_spectra_lsp"; File = $cargoPath; Args = @("test", "-p", "spectra-lsp") }
    [PSCustomObject]@{ Nome = "cargo_build_spectra_interop_release"; File = $cargoPath; Args = @("build", "-p", "spectra-interop", "--release") }
    [PSCustomObject]@{ Nome = "rust_ffi_sample"; File = $cargoPath; Args = @("run", "-p", "spectra-interop", "--example", "rust_ffi_sample") }
    [PSCustomObject]@{ Nome = "python_phase8_demo"; File = "python"; Args = @("python\demo_phase8.py") }
)

foreach ($check in $interopChecks) {
    $r = Invoke-HostCommand -name $check.Nome -fileName $check.File -arguments $check.Args -workingDir (Get-Location).Path
    if ($r.Status -eq "PASSOU") {
        $totalPassed++
    } else {
        $totalFailed++
    }
    $results += [PSCustomObject]@{ Diretorio = "interop"; Teste = $check.Nome; Status = $r.Status; Detalhe = $r.Detail }
}

$cCompiler = $null
$cCompilerKind = $null
foreach ($candidate in @("clang", "gcc", "cl")) {
    $resolved = Get-Command $candidate -ErrorAction SilentlyContinue
    if ($resolved) {
        $cCompiler = $resolved.Source
        $cCompilerKind = $candidate
        break
    }
}
if (-not $cCompiler -and (Test-Path "C:\Program Files\LLVM\bin\clang.exe")) {
    $cCompiler = "C:\Program Files\LLVM\bin\clang.exe"
    $cCompilerKind = "clang"
}

if ($cCompiler) {
    $sampleExe = "target\release\c_ffi_sample.exe"
    $interopLib = "target\release\spectra_interop.dll.lib"
    if ($cCompilerKind -eq "cl") {
        $compileArgs = @(
            "/nologo",
            "/I", "tools\spectra-interop\include",
            "tools\spectra-interop\examples\c_ffi_sample.c",
            $interopLib,
            "/Fe:$sampleExe"
        )
    } else {
        $compileArgs = @(
            "-I", "tools\spectra-interop\include",
            "tools\spectra-interop\examples\c_ffi_sample.c",
            $interopLib,
            "-o", $sampleExe
        )
    }

    $compileResult = Invoke-HostCommand -name "c_ffi_sample_compile" -fileName $cCompiler -arguments $compileArgs -workingDir (Get-Location).Path
    if ($compileResult.Status -eq "PASSOU") {
        $totalPassed++
    } else {
        $totalFailed++
    }
    $results += [PSCustomObject]@{ Diretorio = "interop"; Teste = "c_ffi_sample_compile"; Status = $compileResult.Status; Detalhe = $compileResult.Detail }

    $sampleExeFullPath = Join-Path (Get-Location).Path $sampleExe
    $runResult = Invoke-HostCommand -name "c_ffi_sample_run" -fileName $sampleExeFullPath -arguments @() -workingDir (Join-Path (Get-Location).Path "target\release")
    if ($runResult.Status -eq "PASSOU") {
        $totalPassed++
    } else {
        $totalFailed++
    }
    $results += [PSCustomObject]@{ Diretorio = "interop"; Teste = "c_ffi_sample_run"; Status = $runResult.Status; Detalhe = $runResult.Detail }
} else {
    Write-Host "  c_ffi_sample" -NoNewline
    Write-Host " SKIP (compilador C ausente)" -ForegroundColor DarkYellow
    $totalSkipped++
    $results += [PSCustomObject]@{ Diretorio = "interop"; Teste = "c_ffi_sample"; Status = "SKIP"; Detalhe = "cl/clang/gcc nao encontrados; sample C nao foi compilado localmente" }
}

# ---------------------------------------------------------------------------
# Grupo 8.5: R-105 diagnostics standardization
# ---------------------------------------------------------------------------
$diagnosticsTemp = Join-Path (Get-Location).Path "target\r105-diagnostics"
if (Test-Path $diagnosticsTemp) {
    Remove-Item -LiteralPath $diagnosticsTemp -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $diagnosticsTemp | Out-Null

$jsonReport = Join-Path $diagnosticsTemp "diagnostics.json"
$sarifReport = Join-Path $diagnosticsTemp "diagnostics.sarif"

$jsonDiag = Invoke-SpectraCommand -commandArgs @("check", "--json", "tests\errors\type_mismatch.spectra") -workingDir (Get-Location).Path -includeExperimental $true
$sarifDiag = Invoke-SpectraCommand -commandArgs @("check", "--sarif", "tests\errors\type_mismatch.spectra") -workingDir (Get-Location).Path -includeExperimental $true
Set-Content -LiteralPath $jsonReport -Value $jsonDiag.Output -Encoding UTF8
Set-Content -LiteralPath $sarifReport -Value $sarifDiag.Output -Encoding UTF8

Write-Host ""
Write-Host "--- R-105 diagnostics standardization ---" -ForegroundColor Yellow
$diagValidate = Invoke-HostCommand -name "validate_diagnostics_standardization" -fileName "python" -arguments @("scripts\validate_diagnostics_standardization.py", "--json-report", $jsonReport, "--sarif-report", $sarifReport) -workingDir (Get-Location).Path
if (-not $jsonDiag.TimedOut -and -not $sarifDiag.TimedOut -and $diagValidate.Status -eq "PASSOU") {
    $totalPassed++
    $results += [PSCustomObject]@{ Diretorio = "phase1-diagnostics"; Teste = "validate_diagnostics_standardization"; Status = "PASSOU"; Detalhe = "" }
} else {
    $totalFailed++
    $detail = if ($diagValidate.Status -ne "PASSOU") { $diagValidate.Detail } else { "JSON/SARIF diagnostics timed out" }
    $results += [PSCustomObject]@{ Diretorio = "phase1-diagnostics"; Teste = "validate_diagnostics_standardization"; Status = "FALHOU"; Detalhe = $detail }
}

# ---------------------------------------------------------------------------
# Grupo 8.5b: R-108 diagnostic classification hardening
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-108 diagnostic classification hardening ---" -ForegroundColor Yellow
$diagClassification = Invoke-HostCommand -name "validate_r108_diagnostic_classification" -fileName "python" -arguments @("scripts\validate_r108_diagnostic_classification.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($diagClassification.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase1-diagnostics"; Teste = "validate_r108_diagnostic_classification"; Status = $diagClassification.Status; Detalhe = $diagClassification.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.5c: R-109 cross-module string value handling
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-109 cross-module string value handling ---" -ForegroundColor Yellow
$crossModuleStrings = Invoke-HostCommand -name "validate_r109_cross_module_strings" -fileName "python" -arguments @("scripts\validate_r109_cross_module_strings.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($crossModuleStrings.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase1-backend"; Teste = "validate_r109_cross_module_strings"; Status = $crossModuleStrings.Status; Detalhe = $crossModuleStrings.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.5d: R-110 cross-module type and method resolution
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-110 cross-module type and method resolution ---" -ForegroundColor Yellow
$crossModuleTypesMethods = Invoke-HostCommand -name "validate_r110_cross_module_types_methods" -fileName "python" -arguments @("scripts\validate_r110_cross_module_types_methods.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($crossModuleTypesMethods.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase1-semantics"; Teste = "validate_r110_cross_module_types_methods"; Status = $crossModuleTypesMethods.Status; Detalhe = $crossModuleTypesMethods.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.5e: R-111 cross-module aggregate codegen
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-111 cross-module aggregate codegen ---" -ForegroundColor Yellow
$crossModuleAggregates = Invoke-HostCommand -name "validate_r111_cross_module_aggregate_codegen" -fileName "python" -arguments @("scripts\validate_r111_cross_module_aggregate_codegen.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($crossModuleAggregates.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase1-backend"; Teste = "validate_r111_cross_module_aggregate_codegen"; Status = $crossModuleAggregates.Status; Detalhe = $crossModuleAggregates.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.5f: R-112 runtime float-to-int cast codegen
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-112 runtime float-to-int cast codegen ---" -ForegroundColor Yellow
$runtimeFloatCasts = Invoke-HostCommand -name "validate_r112_runtime_float_cast_codegen" -fileName "python" -arguments @("scripts\validate_r112_runtime_float_cast_codegen.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($runtimeFloatCasts.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase1-backend"; Teste = "validate_r112_runtime_float_cast_codegen"; Status = $runtimeFloatCasts.Status; Detalhe = $runtimeFloatCasts.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.6: R-106 feature maturity policy
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-106 feature maturity policy ---" -ForegroundColor Yellow
$featureMaturity = Invoke-HostCommand -name "validate_feature_maturity" -fileName "python" -arguments @("scripts\validate_feature_maturity.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($featureMaturity.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase1-features"; Teste = "validate_feature_maturity"; Status = $featureMaturity.Status; Detalhe = $featureMaturity.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.7: R-203 pattern ergonomics
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-203 pattern ergonomics ---" -ForegroundColor Yellow
$patternErgonomics = Invoke-HostCommand -name "validate_pattern_ergonomics" -fileName "python" -arguments @("scripts\validate_pattern_ergonomics.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($patternErgonomics.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase2-patterns"; Teste = "validate_pattern_ergonomics"; Status = $patternErgonomics.Status; Detalhe = $patternErgonomics.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.8: R-206 generic return type enforcement
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-206 generic return type enforcement ---" -ForegroundColor Yellow
$genericReturn = Invoke-HostCommand -name "validate_r206_generic_return_enforcement" -fileName "python" -arguments @("scripts\validate_r206_generic_return_enforcement.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($genericReturn.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase2-generics"; Teste = "validate_r206_generic_return_enforcement"; Status = $genericReturn.Status; Detalhe = $genericReturn.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.9: R-205 float const cast codegen
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-205 float const cast codegen ---" -ForegroundColor Yellow
$floatConstCast = Invoke-HostCommand -name "validate_r205_float_const_cast_codegen" -fileName "python" -arguments @("scripts\validate_r205_float_const_cast_codegen.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($floatConstCast.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase2-codegen"; Teste = "validate_r205_float_const_cast_codegen"; Status = $floatConstCast.Status; Detalhe = $floatConstCast.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.10: R-1002 debugger and stack traces
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1002 debugger and stack traces ---" -ForegroundColor Yellow
$debuggerStackTraces = Invoke-HostCommand -name "validate_debugger_stack_traces" -fileName "python" -arguments @("scripts\validate_debugger_stack_traces.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($debuggerStackTraces.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase10-debugger"; Teste = "validate_debugger_stack_traces"; Status = $debuggerStackTraces.Status; Detalhe = $debuggerStackTraces.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.10: R-1501 numerical performance benchmark gate
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1501 numerical performance benchmarks ---" -ForegroundColor Yellow
$r1501Bench = Invoke-HostCommand -name "validate_r1501_bench" -fileName "python" -arguments @("scripts\validate_r1501_bench.py") -workingDir (Get-Location).Path
if ($r1501Bench.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase15-performance"; Teste = "validate_r1501_bench"; Status = $r1501Bench.Status; Detalhe = $r1501Bench.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.11: R-1503 numerical correctness certification
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1503 numerical correctness certification ---" -ForegroundColor Yellow
$r1503Correctness = Invoke-HostCommand -name "validate_r1503_correctness" -fileName "python" -arguments @("scripts\validate_r1503_correctness.py") -workingDir (Get-Location).Path
if ($r1503Correctness.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase15-correctness"; Teste = "validate_r1503_correctness"; Status = $r1503Correctness.Status; Detalhe = $r1503Correctness.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.12: R-1601 tensor graph IR
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1601 tensor graph IR ---" -ForegroundColor Yellow
$r1601TensorGraph = Invoke-HostCommand -name "tensor_graph_tests" -fileName "cargo" -arguments @("test", "-p", "spectra-midend", "--test", "tensor_graph_tests") -workingDir (Get-Location).Path
if ($r1601TensorGraph.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase16-graph"; Teste = "tensor_graph_tests"; Status = $r1601TensorGraph.Status; Detalhe = $r1601TensorGraph.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.13: R-1602 graph optimization and fusion
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1602 graph optimization and fusion ---" -ForegroundColor Yellow
$r1602GraphOptimization = Invoke-HostCommand -name "tensor_graph_optimization_tests" -fileName "cargo" -arguments @("test", "-p", "spectra-midend", "--test", "tensor_graph_tests", "optimizer") -workingDir (Get-Location).Path
if ($r1602GraphOptimization.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase16-optimization"; Teste = "tensor_graph_optimization_tests"; Status = $r1602GraphOptimization.Status; Detalhe = $r1602GraphOptimization.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.14: R-1603 production GPU backend
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1603 production GPU backend ---" -ForegroundColor Yellow
$r1603GpuBackend = Invoke-HostCommand -name "validate_r1603_gpu_backend" -fileName "python" -arguments @("scripts\validate_r1603_gpu_backend.py") -workingDir (Get-Location).Path
if ($r1603GpuBackend.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase16-gpu"; Teste = "validate_r1603_gpu_backend"; Status = $r1603GpuBackend.Status; Detalhe = $r1603GpuBackend.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.15: R-1701 dataset and dataframe runtime
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1701 dataset and dataframe runtime ---" -ForegroundColor Yellow
$r1701DataRuntime = Invoke-HostCommand -name "validate_r1701_data_runtime" -fileName "python" -arguments @("scripts\validate_r1701_data_runtime.py") -workingDir (Get-Location).Path
if ($r1701DataRuntime.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase17-data"; Teste = "validate_r1701_data_runtime"; Status = $r1701DataRuntime.Status; Detalhe = $r1701DataRuntime.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.16: R-1702 experiment tracking and reproducibility
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1702 experiment tracking and reproducibility ---" -ForegroundColor Yellow
$r1702ExperimentTracking = Invoke-HostCommand -name "validate_r1702_experiment_tracking" -fileName "python" -arguments @("scripts\validate_r1702_experiment_tracking.py") -workingDir (Get-Location).Path
if ($r1702ExperimentTracking.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase17-experiments"; Teste = "validate_r1702_experiment_tracking"; Status = $r1702ExperimentTracking.Status; Detalhe = $r1702ExperimentTracking.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.17: R-1703 distributed training foundations
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1703 distributed training foundations ---" -ForegroundColor Yellow
$r1703DistributedTraining = Invoke-HostCommand -name "validate_r1703_distributed_training" -fileName "python" -arguments @("scripts\validate_r1703_distributed_training.py") -workingDir (Get-Location).Path
if ($r1703DistributedTraining.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase17-distributed"; Teste = "validate_r1703_distributed_training"; Status = $r1703DistributedTraining.Status; Detalhe = $r1703DistributedTraining.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.18: R-1801 ONNX import and export
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1801 ONNX import and export ---" -ForegroundColor Yellow
$r1801OnnxImportExport = Invoke-HostCommand -name "validate_r1801_onnx_import_export" -fileName "python" -arguments @("scripts\validate_r1801_onnx_import_export.py") -workingDir (Get-Location).Path
if ($r1801OnnxImportExport.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase18-onnx"; Teste = "validate_r1801_onnx_import_export"; Status = $r1801OnnxImportExport.Status; Detalhe = $r1801OnnxImportExport.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.19: R-1802 transformer and LLM runtime primitives
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1802 transformer and LLM runtime primitives ---" -ForegroundColor Yellow
$r1802TransformerPrimitives = Invoke-HostCommand -name "validate_r1802_transformer_primitives" -fileName "python" -arguments @("scripts\validate_r1802_transformer_primitives.py") -workingDir (Get-Location).Path
if ($r1802TransformerPrimitives.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase18-transformers"; Teste = "validate_r1802_transformer_primitives"; Status = $r1802TransformerPrimitives.Status; Detalhe = $r1802TransformerPrimitives.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.20: R-1803 tokenization embeddings and RAG toolkit
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1803 tokenization embeddings and RAG toolkit ---" -ForegroundColor Yellow
$r1803RagToolkit = Invoke-HostCommand -name "validate_r1803_rag_toolkit" -fileName "python" -arguments @("scripts\validate_r1803_rag_toolkit.py") -workingDir (Get-Location).Path
if ($r1803RagToolkit.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase18-rag"; Teste = "validate_r1803_rag_toolkit"; Status = $r1803RagToolkit.Status; Detalhe = $r1803RagToolkit.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.21: R-1901 model evaluation and metrics suite
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1901 model evaluation and metrics suite ---" -ForegroundColor Yellow
$r1901EvaluationMetrics = Invoke-HostCommand -name "validate_r1901_evaluation_metrics" -fileName "python" -arguments @("scripts\validate_r1901_evaluation_metrics.py") -workingDir (Get-Location).Path
if ($r1901EvaluationMetrics.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase19-evaluation"; Teste = "validate_r1901_evaluation_metrics"; Status = $r1901EvaluationMetrics.Status; Detalhe = $r1901EvaluationMetrics.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.22: R-1902 AI safety and guardrail runtime
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1902 AI safety and guardrail runtime ---" -ForegroundColor Yellow
$r1902SafetyGuardrails = Invoke-HostCommand -name "validate_r1902_ai_safety_guardrails" -fileName "python" -arguments @("scripts\validate_r1902_ai_safety_guardrails.py") -workingDir (Get-Location).Path
if ($r1902SafetyGuardrails.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase19-safety"; Teste = "validate_r1902_ai_safety_guardrails"; Status = $r1902SafetyGuardrails.Status; Detalhe = $r1902SafetyGuardrails.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.23: R-1903 model monitoring and drift detection
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-1903 model monitoring and drift detection ---" -ForegroundColor Yellow
$r1903ModelMonitoring = Invoke-HostCommand -name "validate_r1903_model_monitoring" -fileName "python" -arguments @("scripts\validate_r1903_model_monitoring.py") -workingDir (Get-Location).Path
if ($r1903ModelMonitoring.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase19-monitoring"; Teste = "validate_r1903_model_monitoring"; Status = $r1903ModelMonitoring.Status; Detalhe = $r1903ModelMonitoring.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.24: R-2001 AI conformance suite
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2001 AI conformance suite ---" -ForegroundColor Yellow
$r2001AiConformance = Invoke-HostCommand -name "validate_r2001_ai_conformance" -fileName "python" -arguments @("scripts\validate_r2001_ai_conformance.py", "--keep-going") -workingDir (Get-Location).Path
if ($r2001AiConformance.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase20-conformance"; Teste = "validate_r2001_ai_conformance"; Status = $r2001AiConformance.Status; Detalhe = $r2001AiConformance.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.25: R-2002 production release channels
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2002 production release channels ---" -ForegroundColor Yellow
$r2002ReleaseChannels = Invoke-HostCommand -name "validate_r2002_release_channels" -fileName "python" -arguments @("scripts\validate_r2002_release_channels.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($r2002ReleaseChannels.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase20-release"; Teste = "validate_r2002_release_channels"; Status = $r2002ReleaseChannels.Status; Detalhe = $r2002ReleaseChannels.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.26: R-2003 base language and std regression audit
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2003 base language and std regression audit ---" -ForegroundColor Yellow
$r2003BaseRegression = Invoke-HostCommand -name "validate_r2003_base_regression_audit" -fileName "python" -arguments @("scripts\validate_r2003_base_regression_audit.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($r2003BaseRegression.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase20-base-stabilization"; Teste = "validate_r2003_base_regression_audit"; Status = $r2003BaseRegression.Status; Detalhe = $r2003BaseRegression.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.27: R-2005 core std/runtime host-status hardening
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2005 core std/runtime host-status hardening ---" -ForegroundColor Yellow
$r2005RuntimeHardening = Invoke-HostCommand -name "validate_r2005_runtime_hardening" -fileName "python" -arguments @("scripts\validate_r2005_runtime_hardening.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($r2005RuntimeHardening.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase20-runtime-hardening"; Teste = "validate_r2005_runtime_hardening"; Status = $r2005RuntimeHardening.Status; Detalhe = $r2005RuntimeHardening.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.28: R-2006 tensor/std performance refresh
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2006 tensor/std performance refresh ---" -ForegroundColor Yellow
$r2006PerformanceRefresh = Invoke-HostCommand -name "validate_r2006_performance_refresh" -fileName "python" -arguments @("scripts\validate_r2006_performance_refresh.py") -workingDir (Get-Location).Path
if ($r2006PerformanceRefresh.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase20-performance-refresh"; Teste = "validate_r2006_performance_refresh"; Status = $r2006PerformanceRefresh.Status; Detalhe = $r2006PerformanceRefresh.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.29: R-2007 backend/codegen robustness
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2007 backend/codegen robustness ---" -ForegroundColor Yellow
$r2007BackendCodegen = Invoke-HostCommand -name "validate_r2007_backend_codegen" -fileName "python" -arguments @("scripts\validate_r2007_backend_codegen.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($r2007BackendCodegen.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase20-backend-codegen"; Teste = "validate_r2007_backend_codegen"; Status = $r2007BackendCodegen.Status; Detalhe = $r2007BackendCodegen.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.30: R-2101 async/await execution model ADR
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2101 async/await execution model ADR ---" -ForegroundColor Yellow
$r2101AsyncAdr = Invoke-HostCommand -name "validate_r2101_async_adr" -fileName "python" -arguments @("scripts\validate_r2101_async_adr.py") -workingDir (Get-Location).Path
if ($r2101AsyncAdr.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase21-async"; Teste = "validate_r2101_async_adr"; Status = $r2101AsyncAdr.Status; Detalhe = $r2101AsyncAdr.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.28: R-2102 async frontend surface
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2102 async frontend surface ---" -ForegroundColor Yellow
$r2102AsyncFrontend = Invoke-HostCommand -name "validate_r2102_async_frontend" -fileName "python" -arguments @("scripts\validate_r2102_async_frontend.py") -workingDir (Get-Location).Path
if ($r2102AsyncFrontend.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase21-async"; Teste = "validate_r2102_async_frontend"; Status = $r2102AsyncFrontend.Status; Detalhe = $r2102AsyncFrontend.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.29: R-2103 await expression and async lowering
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2103 await expression and async lowering ---" -ForegroundColor Yellow
$r2103AsyncLowering = Invoke-HostCommand -name "validate_r2103_async_lowering" -fileName "python" -arguments @("scripts\validate_r2103_async_lowering.py") -workingDir (Get-Location).Path
if ($r2103AsyncLowering.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase21-async"; Teste = "validate_r2103_async_lowering"; Status = $r2103AsyncLowering.Status; Detalhe = $r2103AsyncLowering.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.30: R-2104 event loop multiplexer
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2104 event loop multiplexer ---" -ForegroundColor Yellow
$r2104Reactor = Invoke-HostCommand -name "validate_r2104_reactor" -fileName "python" -arguments @("scripts\validate_r2104_reactor.py") -workingDir (Get-Location).Path
if ($r2104Reactor.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase21-async"; Teste = "validate_r2104_reactor"; Status = $r2104Reactor.Status; Detalhe = $r2104Reactor.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.31: R-2105 cancellation, timeouts, and structured concurrency
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2105 cancellation, timeouts, and structured concurrency ---" -ForegroundColor Yellow
$r2105StructuredConcurrency = Invoke-HostCommand -name "validate_r2105_structured_concurrency" -fileName "python" -arguments @("scripts\validate_r2105_structured_concurrency.py") -workingDir (Get-Location).Path
if ($r2105StructuredConcurrency.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase21-async"; Teste = "validate_r2105_structured_concurrency"; Status = $r2105StructuredConcurrency.Status; Detalhe = $r2105StructuredConcurrency.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.31: R-2106 Stream type and stream adaptors
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2106 Stream type and stream adaptors ---" -ForegroundColor Yellow
$r2106Streams = Invoke-HostCommand -name "validate_r2106_streams" -fileName "python" -arguments @("scripts\validate_r2106_streams.py") -workingDir (Get-Location).Path
if ($r2106Streams.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase21-async"; Teste = "validate_r2106_streams"; Status = $r2106Streams.Status; Detalhe = $r2106Streams.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.32: R-2107 async standard library surface
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2107 async standard library surface ---" -ForegroundColor Yellow
$r2107AsyncStdlib = Invoke-HostCommand -name "validate_r2107_async_stdlib" -fileName "python" -arguments @("scripts\validate_r2107_async_stdlib.py") -workingDir (Get-Location).Path
if ($r2107AsyncStdlib.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase21-async"; Teste = "validate_r2107_async_stdlib"; Status = $r2107AsyncStdlib.Status; Detalhe = $r2107AsyncStdlib.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.33: R-2108 async trait objects and dyn Future/Stream
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2108 async trait objects and dyn Future/Stream ---" -ForegroundColor Yellow
$r2108AsyncTraitObjects = Invoke-HostCommand -name "validate_r2108_async_trait_objects" -fileName "python" -arguments @("scripts\validate_r2108_async_trait_objects.py") -workingDir (Get-Location).Path
if ($r2108AsyncTraitObjects.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase21-async"; Teste = "validate_r2108_async_trait_objects"; Status = $r2108AsyncTraitObjects.Status; Detalhe = $r2108AsyncTraitObjects.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.34: R-2109 async test runtime and macros
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2109 async test runtime and macros ---" -ForegroundColor Yellow
$r2109AsyncTestRuntime = Invoke-HostCommand -name "validate_r2109_async_test_runtime" -fileName "python" -arguments @("scripts\validate_r2109_async_test_runtime.py") -workingDir (Get-Location).Path
if ($r2109AsyncTestRuntime.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase21-async"; Teste = "validate_r2109_async_test_runtime"; Status = $r2109AsyncTestRuntime.Status; Detalhe = $r2109AsyncTestRuntime.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.35: R-2110 async diagnostics and Send/Sync validation
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2110 async diagnostics and Send/Sync validation ---" -ForegroundColor Yellow
$r2110AsyncSendSync = Invoke-HostCommand -name "validate_r2110_async_send_sync" -fileName "python" -arguments @("scripts\validate_r2110_async_send_sync.py") -workingDir (Get-Location).Path
if ($r2110AsyncSendSync.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase21-async"; Teste = "validate_r2110_async_send_sync"; Status = $r2110AsyncSendSync.Status; Detalhe = $r2110AsyncSendSync.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.36: R-2111 async benchmarks and profiling
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2111 async benchmarks and profiling ---" -ForegroundColor Yellow
$r2111AsyncBench = Invoke-HostCommand -name "validate_r2111_async_bench" -fileName "python" -arguments @("scripts\validate_r2111_async_bench.py") -workingDir (Get-Location).Path
if ($r2111AsyncBench.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase21-async"; Teste = "validate_r2111_async_bench"; Status = $r2111AsyncBench.Status; Detalhe = $r2111AsyncBench.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.37: R-2112 formal Send/Sync trait bounds
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2112 formal Send/Sync trait bounds ---" -ForegroundColor Yellow
$r2112FormalSendSync = Invoke-HostCommand -name "validate_r2112_formal_send_sync_bounds" -fileName "python" -arguments @("scripts\validate_r2112_formal_send_sync_bounds.py") -workingDir (Get-Location).Path
if ($r2112FormalSendSync.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase21-async"; Teste = "validate_r2112_formal_send_sync_bounds"; Status = $r2112FormalSendSync.Status; Detalhe = $r2112FormalSendSync.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.38: R-2201 API library architecture ADR
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2201 API library architecture ADR ---" -ForegroundColor Yellow
$r2201ApiAdr = Invoke-HostCommand -name "validate_r2201_api_adr" -fileName "python" -arguments @("scripts\validate_r2201_api_adr.py") -workingDir (Get-Location).Path
if ($r2201ApiAdr.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2201_api_adr"; Status = $r2201ApiAdr.Status; Detalhe = $r2201ApiAdr.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.39: R-2202 spectra-api host-call registration
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2202 spectra-api host-call registration ---" -ForegroundColor Yellow
$r2202ApiHostcalls = Invoke-HostCommand -name "validate_r2202_spectra_api_hostcalls" -fileName "python" -arguments @("scripts\validate_r2202_spectra_api_hostcalls.py") -workingDir (Get-Location).Path
if ($r2202ApiHostcalls.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2202_spectra_api_hostcalls"; Status = $r2202ApiHostcalls.Status; Detalhe = $r2202ApiHostcalls.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.40: R-2203 std.api semantic and tooling surface
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2203 std.api semantic and tooling surface ---" -ForegroundColor Yellow
$r2203StdApiSurface = Invoke-HostCommand -name "validate_r2203_std_api_surface" -fileName "python" -arguments @("scripts\validate_r2203_std_api_surface.py") -workingDir (Get-Location).Path
if ($r2203StdApiSurface.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2203_std_api_surface"; Status = $r2203StdApiSurface.Status; Detalhe = $r2203StdApiSurface.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.41: R-2204 HTTP/1.1 parser
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2204 HTTP/1.1 parser ---" -ForegroundColor Yellow
$r2204Http1Parser = Invoke-HostCommand -name "validate_r2204_http1_parser" -fileName "python" -arguments @("scripts\validate_r2204_http1_parser.py") -workingDir (Get-Location).Path
if ($r2204Http1Parser.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2204_http1_parser"; Status = $r2204Http1Parser.Status; Detalhe = $r2204Http1Parser.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.42: R-2205 HTTP/1.1 server
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2205 HTTP/1.1 server ---" -ForegroundColor Yellow
$r2205Http1Server = Invoke-HostCommand -name "validate_r2205_http1_server" -fileName "python" -arguments @("scripts\validate_r2205_http1_server.py") -workingDir (Get-Location).Path
if ($r2205Http1Server.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2205_http1_server"; Status = $r2205Http1Server.Status; Detalhe = $r2205Http1Server.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.43: R-2206 HTTP/1.1 client
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2206 HTTP/1.1 client ---" -ForegroundColor Yellow
$r2206Http1Client = Invoke-HostCommand -name "validate_r2206_http1_client" -fileName "python" -arguments @("scripts\validate_r2206_http1_client.py") -workingDir (Get-Location).Path
if ($r2206Http1Client.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2206_http1_client"; Status = $r2206Http1Client.Status; Detalhe = $r2206Http1Client.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.44: R-2207 TLS via rustls
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2207 TLS via rustls ---" -ForegroundColor Yellow
$r2207TlsRustls = Invoke-HostCommand -name "validate_r2207_tls_rustls" -fileName "python" -arguments @("scripts\validate_r2207_tls_rustls.py") -workingDir (Get-Location).Path
if ($r2207TlsRustls.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2207_tls_rustls"; Status = $r2207TlsRustls.Status; Detalhe = $r2207TlsRustls.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.45: R-2208 std.api.json encoder and decoder
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2208 std.api.json encoder and decoder ---" -ForegroundColor Yellow
$r2208JsonCodec = Invoke-HostCommand -name "validate_r2208_json_codec" -fileName "python" -arguments @("scripts\validate_r2208_json_codec.py") -workingDir (Get-Location).Path
if ($r2208JsonCodec.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2208_json_codec"; Status = $r2208JsonCodec.Status; Detalhe = $r2208JsonCodec.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.46: R-2209 JSON derive Serialize/Deserialize
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2209 JSON derive Serialize/Deserialize ---" -ForegroundColor Yellow
$r2209JsonDerive = Invoke-HostCommand -name "validate_r2209_json_derive" -fileName "python" -arguments @("scripts\validate_r2209_json_derive.py") -workingDir (Get-Location).Path
if ($r2209JsonDerive.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2209_json_derive"; Status = $r2209JsonDerive.Status; Detalhe = $r2209JsonDerive.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.47: R-2210 HTTP core types
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2210 HTTP core types ---" -ForegroundColor Yellow
$r2210HttpCoreTypes = Invoke-HostCommand -name "validate_r2210_http_core_types" -fileName "python" -arguments @("scripts\validate_r2210_http_core_types.py") -workingDir (Get-Location).Path
if ($r2210HttpCoreTypes.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2210_http_core_types"; Status = $r2210HttpCoreTypes.Status; Detalhe = $r2210HttpCoreTypes.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.48: R-2211 router path matching and wildcards
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2211 router path matching and wildcards ---" -ForegroundColor Yellow
$r2211RouterMatching = Invoke-HostCommand -name "validate_r2211_router_matching" -fileName "python" -arguments @("scripts\validate_r2211_router_matching.py") -workingDir (Get-Location).Path
if ($r2211RouterMatching.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2211_router_matching"; Status = $r2211RouterMatching.Status; Detalhe = $r2211RouterMatching.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.49: R-2212 query string parser and binding
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2212 query string parser and binding ---" -ForegroundColor Yellow
$r2212QueryBinding = Invoke-HostCommand -name "validate_r2212_query_binding" -fileName "python" -arguments @("scripts\validate_r2212_query_binding.py") -workingDir (Get-Location).Path
if ($r2212QueryBinding.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2212_query_binding"; Status = $r2212QueryBinding.Status; Detalhe = $r2212QueryBinding.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.50: R-2213 URL-encoded form binding
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2213 URL-encoded form binding ---" -ForegroundColor Yellow
$r2213FormBinding = Invoke-HostCommand -name "validate_r2213_form_binding" -fileName "python" -arguments @("scripts\validate_r2213_form_binding.py") -workingDir (Get-Location).Path
if ($r2213FormBinding.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2213_form_binding"; Status = $r2213FormBinding.Status; Detalhe = $r2213FormBinding.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.51: R-2214 multipart form and file uploads
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2214 multipart form and file uploads ---" -ForegroundColor Yellow
$r2214MultipartUploads = Invoke-HostCommand -name "validate_r2214_multipart_uploads" -fileName "python" -arguments @("scripts\validate_r2214_multipart_uploads.py") -workingDir (Get-Location).Path
if ($r2214MultipartUploads.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2214_multipart_uploads"; Status = $r2214MultipartUploads.Status; Detalhe = $r2214MultipartUploads.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.52: R-2215 handler trait and response return
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2215 handler trait and response return ---" -ForegroundColor Yellow
$r2215HandlerResponse = Invoke-HostCommand -name "validate_r2215_handler_response" -fileName "python" -arguments @("scripts\validate_r2215_handler_response.py") -workingDir (Get-Location).Path
if ($r2215HandlerResponse.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2215_handler_response"; Status = $r2215HandlerResponse.Status; Detalhe = $r2215HandlerResponse.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.53: R-2216 server lifecycle, listen, serve, and graceful shutdown
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2216 server lifecycle, listen, serve, and graceful shutdown ---" -ForegroundColor Yellow
$r2216ServerLifecycle = Invoke-HostCommand -name "validate_r2216_server_lifecycle" -fileName "python" -arguments @("scripts\validate_r2216_server_lifecycle.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($r2216ServerLifecycle.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2216_server_lifecycle"; Status = $r2216ServerLifecycle.Status; Detalhe = $r2216ServerLifecycle.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.54: R-2217 spectra.api local registry package
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2217 spectra.api local registry package ---" -ForegroundColor Yellow
$r2217SpectraApiRegistry = Invoke-HostCommand -name "validate_r2217_spectra_api_registry" -fileName "python" -arguments @("scripts\validate_r2217_spectra_api_registry.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($r2217SpectraApiRegistry.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2217_spectra_api_registry"; Status = $r2217SpectraApiRegistry.Status; Detalhe = $r2217SpectraApiRegistry.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.55: R-2218 Hello HTTP book chapter
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2218 Hello HTTP book chapter ---" -ForegroundColor Yellow
$r2218HelloHttpBook = Invoke-HostCommand -name "validate_r2218_hello_http_book" -fileName "python" -arguments @("scripts\validate_r2218_hello_http_book.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($r2218HelloHttpBook.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2218_hello_http_book"; Status = $r2218HelloHttpBook.Status; Detalhe = $r2218HelloHttpBook.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.56: R-2219 REST CRUD API example
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2219 REST CRUD API example ---" -ForegroundColor Yellow
$r2219RestCrudExample = Invoke-HostCommand -name "validate_r2219_rest_crud_example" -fileName "python" -arguments @("scripts\validate_r2219_rest_crud_example.py", "--binary", $binary) -workingDir (Get-Location).Path
if ($r2219RestCrudExample.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2219_rest_crud_example"; Status = $r2219RestCrudExample.Status; Detalhe = $r2219RestCrudExample.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.57: R-2220 API conformance suite v0
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2220 API conformance suite v0 ---" -ForegroundColor Yellow
$r2220ApiConformanceV0 = Invoke-HostCommand -name "validate_r2220_api_conformance_v0" -fileName "python" -arguments @("scripts\validate_r2220_api_conformance_v0.py") -workingDir (Get-Location).Path
if ($r2220ApiConformanceV0.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase22-api"; Teste = "validate_r2220_api_conformance_v0"; Status = $r2220ApiConformanceV0.Status; Detalhe = $r2220ApiConformanceV0.Detail }

# ---------------------------------------------------------------------------
# Grupo 8.58: R-2301 Middleware chain and deterministic ordering
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-2301 Middleware chain and deterministic ordering ---" -ForegroundColor Yellow
$r2301MiddlewareChain = Invoke-HostCommand -name "validate_r2301_middleware_chain" -fileName "python" -arguments @("scripts\validate_r2301_middleware_chain.py") -workingDir (Get-Location).Path
if ($r2301MiddlewareChain.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase23-api"; Teste = "validate_r2301_middleware_chain"; Status = $r2301MiddlewareChain.Status; Detalhe = $r2301MiddlewareChain.Detail }

# ---------------------------------------------------------------------------
# Grupo 9: Phase 12 security evidence and stress/soak smoke
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- Phase 12 (security/stress) ---" -ForegroundColor Yellow

$phase12Temp = Join-Path (Get-Location).Path "target\phase12-validation"
$phase12ArtifactDir = Join-Path $phase12Temp "artifacts"
$phase12EvidenceDir = Join-Path $phase12Temp "evidence"
if (Test-Path $phase12Temp) {
    Remove-Item -LiteralPath $phase12Temp -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $phase12ArtifactDir | Out-Null
"spectralang phase12 validation artifact" | Out-File -FilePath (Join-Path $phase12ArtifactDir "spectralang-phase12.txt") -Encoding UTF8

$phase12Checks = @(
    [PSCustomObject]@{
        Nome = "release_security_create"
        File = "python"
        Args = @("scripts\release_security.py", "create", "--artifact", $phase12ArtifactDir, "--out", $phase12EvidenceDir, "--version", "0.0.0-phase12-test", "--allow-dev-key")
    }
    [PSCustomObject]@{
        Nome = "release_security_verify"
        File = "python"
        Args = @("scripts\release_security.py", "verify", "--evidence", $phase12EvidenceDir, "--allow-dev-key")
    }
    [PSCustomObject]@{
        Nome = "stress_soak_smoke"
        File = "python"
        Args = @("scripts\stress_soak.py", "--iterations", "1", "--timeout-seconds", "20", "--memory-limit-mb", "1024", "--json-out", "target\stress-soak-smoke.json")
    }
    [PSCustomObject]@{
        Nome = "validate_r1203_fs_path_safety"
        File = "python"
        Args = @("scripts\validate_r1203_fs_path_safety.py", "--binary", $binary)
    }
    [PSCustomObject]@{
        Nome = "validate_r1204_std_unwrap_safety"
        File = "python"
        Args = @("scripts\validate_r1204_std_unwrap_safety.py")
    }
)

foreach ($check in $phase12Checks) {
    $r = Invoke-HostCommand -name $check.Nome -fileName $check.File -arguments $check.Args -workingDir (Get-Location).Path
    if ($r.Status -eq "PASSOU") {
        $totalPassed++
    } else {
        $totalFailed++
    }
    $results += [PSCustomObject]@{ Diretorio = "phase12"; Teste = $check.Nome; Status = $r.Status; Detalhe = $r.Detail }
}

# ---------------------------------------------------------------------------
# Grupo 9.5: R-104 compiler test pyramid structure
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- R-104 compiler test pyramid ---" -ForegroundColor Yellow
$testPyramid = Invoke-HostCommand -name "validate_test_pyramid" -fileName "python" -arguments @("scripts\validate_test_pyramid.py") -workingDir (Get-Location).Path
if ($testPyramid.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase1-tests"; Teste = "validate_test_pyramid"; Status = $testPyramid.Status; Detalhe = $testPyramid.Detail }

# ---------------------------------------------------------------------------
# Grupo 10: Phase 13 AI reference examples
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "--- Phase 13 book validation ---" -ForegroundColor Yellow
$phase13Book = Invoke-HostCommand -name "validate_ai_book" -fileName "python" -arguments @("scripts\validate_ai_book.py") -workingDir (Get-Location).Path
if ($phase13Book.Status -eq "PASSOU") {
    $totalPassed++
} else {
    $totalFailed++
}
$results += [PSCustomObject]@{ Diretorio = "phase13-docs"; Teste = "validate_ai_book"; Status = $phase13Book.Status; Detalhe = $phase13Book.Detail }

$aiExamplesDir = "examples\ai"
if (Test-Path $aiExamplesDir) {
    Write-Host ""
    $files = Get-ChildItem -Path $aiExamplesDir -Filter "*.spectra" | Sort-Object Name
    Write-Host "--- Phase 13 AI examples ($($files.Count) exemplos: devem executar) ---" -ForegroundColor Yellow
    New-Item -ItemType Directory -Force -Path "target\ai-examples" | Out-Null

    foreach ($file in $files) {
        Write-Host "  $($file.Name)" -NoNewline
        $r = Invoke-SpectraCommand -commandArgs @("run", $file.FullName) -workingDir (Get-Location).Path -includeExperimental $true

        if ($r.TimedOut) {
            Write-Host " TIMEOUT" -ForegroundColor Red
            $totalFailed++
            $results += [PSCustomObject]@{ Diretorio = "phase13-ai"; Teste = $file.Name; Status = "TIMEOUT"; Detalhe = "execucao excedeu ${timeoutSeconds}s" }
        } elseif ($r.ExitCode -eq 0) {
            Write-Host " PASSOU" -ForegroundColor Green
            $totalPassed++
            $results += [PSCustomObject]@{ Diretorio = "phase13-ai"; Teste = $file.Name; Status = "PASSOU"; Detalhe = "" }
        } else {
            $err = Get-FirstError $r.Output
            if (-not $err) {
                $err = "exit code inesperado: $($r.ExitCode)"
            }
            Write-Host " FALHOU" -ForegroundColor Red
            Write-Host "     $err" -ForegroundColor DarkRed
            $totalFailed++
            $results += [PSCustomObject]@{ Diretorio = "phase13-ai"; Teste = $file.Name; Status = "FALHOU"; Detalhe = $err }
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
Write-Host "Testes ignorados por ambiente: $totalSkipped" -ForegroundColor DarkYellow

if ($totalDecisive -gt 0) {
    $pct = [math]::Round(($totalPassed / $totalDecisive) * 100, 1)
    Write-Host "Taxa de sucesso: $pct%" -ForegroundColor $(if ($pct -eq 100) { "Green" } else { "Yellow" })
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan

# Tabela completa
Write-Host ""
$results | Format-Table -AutoSize

# Salva relatorio
$reportPath = "TEST_RESULTS.txt"
$results | Out-File -FilePath $reportPath -Encoding UTF8
Write-Host "Relatorio salvo em: $reportPath" -ForegroundColor Cyan
