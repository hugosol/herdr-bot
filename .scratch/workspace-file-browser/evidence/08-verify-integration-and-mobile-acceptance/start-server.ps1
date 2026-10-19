#requires -Version 5.1
<#
Starts the delivery process with the isolated configuration produced by
prepare-scenario.ps1. Run it in a console window.

    powershell -NoProfile -ExecutionPolicy Bypass -File start-server.ps1

If the build artifacts are missing, run first:

    npm --prefix frontend run build
    cargo build --manifest-path backend\Cargo.toml
#>
$ErrorActionPreference = 'Stop'

$Root = Join-Path $env:TEMP 'omp-bot-08-h1'
$Config = Join-Path $Root 'omp-bot.toml'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$Exe = Join-Path $RepoRoot 'backend\target\debug\omp-bot.exe'

if (-not (Test-Path $Config)) {
    throw "missing isolated config: $Config (run prepare-scenario.ps1 first)"
}
if (-not (Test-Path $Exe)) {
    throw "missing delivery binary: $Exe (build backend first)"
}

Write-Host "starting: $Exe --config $Config"
& $Exe --config $Config
