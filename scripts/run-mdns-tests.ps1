<#
.SYNOPSIS
Builds the mDNS integration tests and runs stable copies of their executables.

.DESCRIPTION
Cargo names integration test executables with a content hash. Windows Firewall
keys its "allow this app" rules by executable path, so every rebuild can create a
new rule and show a new prompt. This script copies the two mDNS test executables
to stable paths under backend\target\debug and runs those copies instead.

Run -ConfigureFirewall once (the script asks for UAC elevation):
    .\scripts\run-mdns-tests.ps1 -ConfigureFirewall

Then run the tests:
    .\scripts\run-mdns-tests.ps1

The mdns_conflict tests require exclusive ownership of herdr-bot.local. Stop the
running herdr-bot panel before running this script. The script refuses to run
mdns_conflict while port 8787 is listening.

.PARAMETER ConfigureFirewall
Adds inbound allow rules for the two stable test runners and the debug
herdr-bot.exe. Requires administrator privileges; the script self-elevates.

.PARAMETER NoBuild
Reuse the existing stable runner copies instead of rebuilding and copying.

.PARAMETER ListOnly
Build/copy (unless -NoBuild is set) and print the stable runner paths without
running any tests.
#>
[CmdletBinding()]
param(
    [switch]$ConfigureFirewall,
    [switch]$NoBuild,
    [switch]$ListOnly
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$backendDir = Join-Path $repoRoot 'backend'
$manifest = Join-Path $backendDir 'Cargo.toml'
$debugDir = Join-Path $backendDir 'target\debug'
$testNames = @('mdns_failure', 'mdns_conflict')
$runners = @{}

function Get-StableRunnerPath([string]$name) {
    return Join-Path $debugDir ("{0}_runner.exe" -f $name)
}

function Build-And-CopyRunners {
    Write-Host 'Building test executables...'
    $jsonLines = & cargo test --manifest-path $manifest --no-run --message-format=json
    if ($LASTEXITCODE -ne 0) {
        throw "cargo test --no-run failed with exit code $LASTEXITCODE"
    }

    foreach ($line in $jsonLines) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        try { $message = $line | ConvertFrom-Json } catch { continue }
        if ($message.reason -ne 'compiler-artifact') { continue }
        if (-not $message.executable) { continue }
        if ($testNames -notcontains $message.target.name) { continue }
        $runners[$message.target.name] = $message.executable
    }

    foreach ($name in $testNames) {
        if (-not $runners.ContainsKey($name)) {
            throw "Could not find the cargo test executable for '$name'."
        }
        $stable = Get-StableRunnerPath $name
        Copy-Item -LiteralPath $runners[$name] -Destination $stable -Force
        Write-Host ("  {0} -> {1}" -f $runners[$name], $stable)
    }
}

function Test-IsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Configure-TestFirewallRules {
    if (-not (Test-IsAdministrator)) {
        Write-Host 'Requesting administrator privileges for firewall configuration...'
        $arguments = @(
            '-NoProfile',
            '-ExecutionPolicy', 'Bypass',
            '-File', $PSCommandPath,
            '-ConfigureFirewall',
            '-NoBuild'
        )
        $process = Start-Process -FilePath powershell.exe -ArgumentList $arguments -Verb RunAs -Wait -PassThru
        exit $process.ExitCode
    }

    $programs = @(
        @{ Name = 'herdr-bot-mdns-test-mdns-failure-runner'; Path = (Get-StableRunnerPath 'mdns_failure') },
        @{ Name = 'herdr-bot-mdns-test-mdns-conflict-runner'; Path = (Get-StableRunnerPath 'mdns_conflict') },
        @{ Name = 'herdr-bot-mdns-test-debug-panel'; Path = (Join-Path $debugDir 'herdr-bot.exe') }
    )

    foreach ($program in $programs) {
        if (-not (Test-Path -LiteralPath $program.Path)) {
            throw "Missing program for firewall rule: $($program.Path)"
        }
        $existing = Get-NetFirewallRule -DisplayName $program.Name -ErrorAction SilentlyContinue
        if ($existing) {
            Write-Host ("Firewall rule already exists: {0}" -f $program.Name)
            continue
        }
        New-NetFirewallRule -DisplayName $program.Name -Direction Inbound -Program $program.Path -Action Allow -Profile Any | Out-Null
        Write-Host ("Added firewall rule: {0} -> {1}" -f $program.Name, $program.Path)
    }
    Write-Host 'Firewall configuration complete.'
}

function Test-PanelIsListening {
    $connection = Get-NetTCPConnection -LocalPort 8787 -State Listen -ErrorAction SilentlyContinue
    return [bool]$connection
}

if (-not $NoBuild) {
    Build-And-CopyRunners
} else {
    foreach ($name in $testNames) {
        $stable = Get-StableRunnerPath $name
        if (-not (Test-Path -LiteralPath $stable)) {
            throw "Missing stable runner '$stable'. Run without -NoBuild first."
        }
        Write-Host ("Using existing runner: {0}" -f $stable)
    }
}

if ($ConfigureFirewall) {
    Configure-TestFirewallRules
    exit 0
}

if ($ListOnly) {
    foreach ($name in $testNames) {
        Write-Host (Get-StableRunnerPath $name)
    }
    exit 0
}

$failureRunner = Get-StableRunnerPath 'mdns_failure'
$conflictRunner = Get-StableRunnerPath 'mdns_conflict'
$exitCode = 0

Push-Location $backendDir
try {
    Write-Host 'Running mdns_failure tests...'
    & $failureRunner --ignored --nocapture
    if ($LASTEXITCODE -ne 0) { $exitCode = 1 }

    if (Test-PanelIsListening) {
        Write-Warning 'Port 8787 is listening. mdns_conflict requires exclusive herdr-bot.local.; stop the panel and rerun.'
        $exitCode = 1
    } else {
        Write-Host 'Running mdns_conflict tests...'
        & $conflictRunner --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { $exitCode = 1 }
    }
} finally {
    Pop-Location
}

exit $exitCode
