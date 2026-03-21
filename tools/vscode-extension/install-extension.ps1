Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Step {
    param([string]$Message)
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Resolve-CodeCommand {
    foreach ($candidate in @('code.cmd', 'code')) {
        $command = Get-Command $candidate -ErrorAction SilentlyContinue
        if ($null -ne $command) {
            return $command.Source
        }
    }

    throw "VS Code CLI não encontrado no PATH. Instale o comando 'code' pelo VS Code e tente novamente."
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = (Resolve-Path (Join-Path $scriptDir '..\..')).Path
$packageJsonPath = Join-Path $scriptDir 'package.json'
$packageJson = Get-Content $packageJsonPath -Raw | ConvertFrom-Json
$vsixName = "$($packageJson.name)-$($packageJson.version).vsix"
$vsixPath = Join-Path $scriptDir $vsixName
$codeCommand = Resolve-CodeCommand
$bundledServerDir = Join-Path $scriptDir 'server'
$bundledServerPath = Join-Path $bundledServerDir 'spectra-lsp.exe'

Push-Location $repoRoot
try {
    Write-Step 'Compilando o servidor spectra-lsp'
    cargo build -p spectra-lsp

    $builtServerPath = Join-Path $repoRoot 'target\debug\spectra-lsp.exe'
    if (-not (Test-Path $builtServerPath)) {
        throw "Binário spectra-lsp não encontrado em $builtServerPath"
    }

    Push-Location $scriptDir
    try {
        if (-not (Test-Path $bundledServerDir)) {
            New-Item -ItemType Directory -Path $bundledServerDir | Out-Null
        }

        Write-Step 'Copiando o servidor para dentro da extensão'
        Copy-Item $builtServerPath $bundledServerPath -Force

        if (-not (Test-Path (Join-Path $scriptDir 'node_modules'))) {
            Write-Step 'Instalando dependências npm da extensão'
            npm install
        }

        Write-Step 'Compilando a extensão VS Code'
        npm run compile

        if (Test-Path $vsixPath) {
            Remove-Item $vsixPath -Force
        }

        Write-Step 'Empacotando a extensão em VSIX'
        npx @vscode/vsce package --out $vsixPath

        Write-Step 'Instalando a extensão no VS Code'
        & $codeCommand --install-extension $vsixPath --force

        Write-Host "Extensão instalada com sucesso: $vsixPath" -ForegroundColor Green
    }
    finally {
        Pop-Location
    }
}
finally {
    Pop-Location
}