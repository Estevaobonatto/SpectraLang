#requires -Version 5.1
<#
.SYNOPSIS
    Faz bump de versão local do SpectraLang em Cargo.toml e package.json.

.DESCRIPTION
    Lê a versão atual de tools/spectra-cli/Cargo.toml (bloco [package]) e
    atualiza ela em todos os arquivos do projeto. Suporta:

      - bump automático:   -Bump Major | Minor | Patch
      - versão exata:      -Version 1.2.3  (ou 1.2.3-rc.1)

    Este script SÓ ALTERA VERSÕES. Ele não faz commit, push, tag, ou
    qualquer operação git. Para publicar, faça commit/push manualmente
    ou deixe o workflow .github/workflows/release.yml fazer isso.

    Arquivos atualizados (devem bater com release.yml):
      - tools/spectra-cli/Cargo.toml       (bloco [package])
      - tools/vscode-extension/package.json

.PARAMETER Bump
    Tipo de bump automático. Padrão: Patch. Ignorado se -Version for
    fornecido.

.PARAMETER Version
    Versão alvo no formato X.Y.Z (sem prefixo 'v'). Suporta prerelease
    (ex: 1.0.0-rc.1). Se fornecido, sobrescreve -Bump.

.PARAMETER DryRun
    Mostra as mudanças que seriam feitas sem gravar nada no disco.

.EXAMPLE
    .\scripts\bump-version.ps1
    # 0.2.1 -> 0.2.2 (bump patch, padrão)

.EXAMPLE
    .\scripts\bump-version.ps1 -Bump Minor
    # 0.2.1 -> 0.3.0

.EXAMPLE
    .\scripts\bump-version.ps1 -Version 1.0.0-rc.1
    # Define versão exata com prerelease

.EXAMPLE
    .\scripts\bump-version.ps1 -Bump Patch -DryRun
    # Mostra o que mudaria sem gravar
#>

[CmdletBinding()]
param(
    [ValidateSet('Major', 'Minor', 'Patch')]
    [string]$Bump = 'Patch',

    [string]$Version,

    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

# ----------------------------------------------------------------------------
# Configuração
# ----------------------------------------------------------------------------
$RepoRoot    = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$CargoCli    = Join-Path $RepoRoot 'tools/spectra-cli/Cargo.toml'
$PackageJson = Join-Path $RepoRoot 'tools/vscode-extension/package.json'

# ----------------------------------------------------------------------------
# Utilitários
# ----------------------------------------------------------------------------
function Test-Semver {
    param([string]$V)
    return ($V -match '^\d+\.\d+\.\d+(-[0-9A-Za-z\-\.]+)?$')
}

function Get-CurrentVersion {
    $lines = Get-Content -LiteralPath $CargoCli
    $inPkg = $false
    foreach ($line in $lines) {
        if ($line -match '^\[package\]')   { $inPkg = $true;  continue }
        if ($line -match '^\[')             { $inPkg = $false; continue }
        if ($inPkg -and $line -match '^version\s*=\s*"([^"]+)"') {
            return $matches[1]
        }
    }
    throw "Não foi possível ler a versão em $CargoCli (bloco [package])."
}

function Invoke-BumpVersion {
    param([string]$Current, [string]$Mode)
    $parts = $Current.Split('.')
    if ($parts.Count -lt 3) {
        throw "Versão atual '$Current' não está no formato X.Y.Z"
    }
    $major = [int]$parts[0]
    $minor = [int]$parts[1]
    $patch = [int]$parts[2]

    switch ($Mode) {
        'Major' { $major++; $minor = 0; $patch = 0 }
        'Minor' { $minor++; $patch = 0 }
        'Patch' { $patch++ }
    }
    return "$major.$minor.$patch"
}

function Update-CargoVersion {
    param([string]$OldVersion, [string]$NewVersion)
    $lines   = Get-Content -LiteralPath $CargoCli
    $inPkg   = $false
    $changed = $false
    $out     = for ($i = 0; $i -lt $lines.Count; $i++) {
        $line = $lines[$i]
        if ($line -match '^\[package\]')   { $inPkg = $true }
        elseif ($line -match '^\[')         { $inPkg = $false }
        elseif ($inPkg -and $line -match '^version\s*=\s*"([^"]+)"') {
            $old = $matches[1]
            if ($old -eq $OldVersion) {
                $line = $line -replace [regex]::Escape($OldVersion), $NewVersion
                $changed = $true
            }
        }
        $line
    }
    if (-not $changed) {
        throw "Falha ao atualizar versão em $CargoCli (não encontrou version = '$OldVersion' no bloco [package])."
    }
    Set-Content -LiteralPath $CargoCli -Value $out -Encoding UTF8
}

function Update-PackageJsonVersion {
    param([string]$OldVersion, [string]$NewVersion)
    $lines   = Get-Content -LiteralPath $PackageJson
    $changed = $false
    $pattern = '"version"\s*:\s*"' + [regex]::Escape($OldVersion) + '"'
    $out     = foreach ($line in $lines) {
        if ($line -match $pattern) {
            $line = $line -replace [regex]::Escape($OldVersion), $NewVersion
            $changed = $true
        }
        $line
    }
    if (-not $changed) {
        throw "Falha ao atualizar versão em $PackageJson (não encontrou 'version': '$OldVersion')."
    }
    Set-Content -LiteralPath $PackageJson -Value $out -Encoding UTF8
}

# ----------------------------------------------------------------------------
# Execução
# ----------------------------------------------------------------------------
Write-Host ''
Write-Host '=== SpectraLang version bump ===' -ForegroundColor Cyan

if (-not (Test-Path $CargoCli))    { throw "Arquivo não encontrado: $CargoCli" }
if (-not (Test-Path $PackageJson)) { throw "Arquivo não encontrado: $PackageJson" }

$current = Get-CurrentVersion
Write-Host ("  Atual : {0}" -f $current) -ForegroundColor Yellow

if ($Version) {
    if (-not (Test-Semver $Version)) {
        throw "Versão inválida '$Version'. Use o formato X.Y.Z (opcional -prerelease)."
    }
    $target = $Version
    $reason = 'manual'
} else {
    $target = Invoke-BumpVersion -Current $current -Mode $Bump
    $reason = "bump $Bump"
}

if ($target -eq $current) {
    Write-Warning "A versão alvo é igual à atual ($current). Nada a fazer."
    exit 0
}

Write-Host ("  Alvo  : {0}  ({1})" -f $target, $reason) -ForegroundColor Green
Write-Host ("  Modo  : {0}" -f $(if ($DryRun) { 'DRY-RUN' } else { 'GRAVAR' }))
Write-Host ''

if ($DryRun) {
    Write-Host 'Arquivos que seriam alterados:' -ForegroundColor Cyan
    Write-Host ("  - {0}" -f $CargoCli)    -ForegroundColor Gray
    Write-Host ("  - {0}" -f $PackageJson) -ForegroundColor Gray
    Write-Host ''
    Write-Host 'Nenhuma alteração foi feita (DryRun).' -ForegroundColor DarkYellow
    exit 0
}

# --- Aplica mudanças --------------------------------------------------------
Write-Host 'Atualizando arquivos...' -ForegroundColor Cyan

Update-CargoVersion      -OldVersion $current -NewVersion $target
Update-PackageJsonVersion -OldVersion $current -NewVersion $target

Write-Host ("  ok  {0}" -f $CargoCli)    -ForegroundColor Green
Write-Host ("  ok  {0}" -f $PackageJson) -ForegroundColor Green
Write-Host ''
Write-Host ("  {0}  ->  {1}" -f $current, $target) -ForegroundColor Green
Write-Host ''
Write-Host 'Concluído. (nenhum commit/push/tag foi feito)' -ForegroundColor Cyan
exit 0
