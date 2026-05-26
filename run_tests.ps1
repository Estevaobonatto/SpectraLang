# Script de teste automatizado para SpectraLang
# Cobre todos os diretorios de testes:
#   tests/validation/   - devem COMPILAR com sucesso
#   tests/control_flow/ - devem COMPILAR com sucesso
#   tests/projects/     - projetos multi-arquivo devem COMPILAR com sucesso
#   tests/errors/       - devem FALHAR na compilacao (erros esperados)
#   tests/semantic/     - compilados e reportados sem expectativa forcada
#   tests/cli/          - fixtures para validar comandos do CLI
#   tools/spectra-interop/ - interop Rust/Python/C ABI
#
# Requer que o binario ja esteja compilado:
#   cargo build -p spectra-cli

$binary = (Resolve-Path ".\target\debug\spectralang.exe").Path
$timeoutSeconds = 10
$hostCommandTimeoutSeconds = 120
$experimentalFlags = @(
    "--enable-experimental", "switch",
    "--enable-experimental", "unless",
    "--enable-experimental", "do-while",
    "--enable-experimental", "loop"
)

if (-not (Test-Path $binary)) {
    Write-Host "Binario nao encontrado. Compilando..." -ForegroundColor Yellow
    $env:PATH = "C:\Users\estev\.cargo\bin;" + $env:PATH
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
        Contains = "switch"
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
# Grupo 7: interop Python / Rust / C ABI
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
