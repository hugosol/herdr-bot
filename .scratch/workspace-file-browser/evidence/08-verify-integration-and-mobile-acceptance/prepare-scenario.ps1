#requires -Version 5.1
<#
Reproducible isolated scenario for the mobile H1 round.

Creates two real Git worktrees with their own bare remotes under
%TEMP%\omp-bot-08-h1, plus an isolated workspace registry and an omp-bot
configuration whose fixed operations point at the repository's safe fixtures.
Rerunning the script removes and recreates that directory; it never touches a
user worktree or a business remote.

    powershell -NoProfile -ExecutionPolicy Bypass -File prepare-scenario.ps1

Then start the delivery process with start-server.ps1 (port 0.0.0.0:8787).
#>
$ErrorActionPreference = 'Stop'

$Root = Join-Path $env:TEMP 'omp-bot-08-h1'
$Repo = Join-Path $Root 'repo'
$Clean = Join-Path $Root 'clean-repo'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$Fixtures = Join-Path $RepoRoot 'fixtures\operations'

if (Test-Path $Root) {
    Remove-Item -LiteralPath $Root -Recurse -Force
}
New-Item -ItemType Directory -Path $Root | Out-Null

function Invoke-Git([string]$Directory, [string[]]$Arguments) {
    & git -C $Directory @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "git -C $Directory $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Initialize-Repository([string]$Directory, [string]$Branch) {
    New-Item -ItemType Directory -Path $Directory | Out-Null
    Invoke-Git $Directory @('init', '--quiet', "--initial-branch=$Branch")
    Invoke-Git $Directory @('config', 'user.name', 'omp-bot-h1')
    Invoke-Git $Directory @('config', 'user.email', 'h1@example.test')
}

function Write-RepoFile([string]$Directory, [string]$Relative, [string]$Content) {
    $Path = Join-Path $Directory $Relative
    $Parent = Split-Path -Parent $Path
    if (-not (Test-Path $Parent)) {
        New-Item -ItemType Directory -Path $Parent -Force | Out-Null
    }
    Set-Content -LiteralPath $Path -Value $Content -NoNewline
}

function New-BareRemote([string]$Repository, [string]$RemotePath, [string]$RemoteName, [string]$Branch) {
    & git init --bare --quiet "--initial-branch=$Branch" $RemotePath
    if ($LASTEXITCODE -ne 0) { throw "failed to create bare remote: $RemotePath" }
    Invoke-Git $Repository @('remote', 'add', $RemoteName, $RemotePath)
    Invoke-Git $Repository @('push', '--quiet', '-u', $RemoteName, $Branch)
}

function Step-RemoteAhead([string]$RemotePath, [string]$ClonePath, [string]$Branch, [string]$File, [string]$Content) {
    & git clone --quiet $RemotePath $ClonePath
    if ($LASTEXITCODE -ne 0) { throw "failed to clone remote: $RemotePath" }
    Invoke-Git $ClonePath @('config', 'user.name', 'remote-advance')
    Invoke-Git $ClonePath @('config', 'user.email', 'advance@example.test')
    Write-RepoFile $ClonePath $File $Content
    Invoke-Git $ClonePath @('add', '--all')
    Invoke-Git $ClonePath @('commit', '--quiet', '-m', 'remote advance')
    Invoke-Git $ClonePath @('push', '--quiet', 'origin', $Branch)
}

# ---- repo: a Git worktree with pending changes (diffs, Commit, touch checks) ----------
Initialize-Repository $Repo 'feature'
Write-RepoFile $Repo '.gitignore' "ignored.log`n"
Write-RepoFile $Repo 'tracked.txt' "tracked base`n"
Write-RepoFile $Repo 'deleted.txt' "delete me`n"
Write-RepoFile $Repo 'src/old-name.txt' "rename me`n"
$LongLines = (1..90 | ForEach-Object { "line $_" }) -join "`n"
Write-RepoFile $Repo 'src/long.txt' "$LongLines`n"
Invoke-Git $Repo @('add', '--all')
Invoke-Git $Repo @('commit', '--quiet', '-m', 'initial state')
$Remote = Join-Path $Root 'remote.git'
New-BareRemote $Repo $Remote 'origin' 'feature'
Step-RemoteAhead $Remote (Join-Path $Root 'remote-advance') 'feature' 'remote.txt' "remote advance`n"

# Local changes: modified, deleted, untracked, ignored, a staged rename, and a long diff.
Write-RepoFile $Repo 'tracked.txt' "tracked changed`n"
Write-RepoFile $Repo 'added.txt' "added file`n"
Write-RepoFile $Repo 'ignored.log' "ignored but visible`n"
Remove-Item -LiteralPath (Join-Path $Repo 'deleted.txt')
Invoke-Git $Repo @('mv', 'src/old-name.txt', 'src/renamed.txt')
$ChangedLong = (1..90 | ForEach-Object {
    if ($_ -eq 5) { 'line 5 changed' }
    elseif ($_ -eq 60) { 'line 60 changed' }
    else { "line $_" }
}) -join "`n"
Write-RepoFile $Repo 'src/long.txt' "$ChangedLong`n"
Write-RepoFile $Repo 'notes/untracked.txt' "an untracked note`n"

# ---- clean-repo: clean, with an upstream that advanced independently (Sync flow) ------
Initialize-Repository $Clean 'main'
Write-RepoFile $Clean 'clean.txt' "clean base`n"
Invoke-Git $Clean @('add', '--all')
Invoke-Git $Clean @('commit', '--quiet', '-m', 'clean initial')
$CleanRemote = Join-Path $Root 'clean-remote.git'
New-BareRemote $Clean $CleanRemote 'origin' 'main'
Step-RemoteAhead $CleanRemote (Join-Path $Root 'clean-remote-advance') 'main' 'remote-update.txt' "remote update`n"

# ---- registry and configuration -------------------------------------------------------
$Registry = Join-Path $Root 'workspaces.json'
@{ workspaces = @(@{ path = $Repo }, @{ path = $Clean }) } |
    ConvertTo-Json -Depth 4 |
    Set-Content -LiteralPath $Registry -Encoding ascii

$Config = Join-Path $Root 'omp-bot.toml'
$Toml = @"
# 08 H1 isolated scenario: fixed operations use the repository's safe fixtures.
listen_addr = "0.0.0.0:8787"

[operations.start]
script = '$Fixtures\success.bat'
working_directory = '$Fixtures'

[operations.stop]
script = '$Fixtures\success.bat'
working_directory = '$Fixtures'

[workspaces]
registry_file = '$Registry'
preview_limit_bytes = 1048576
refresh_interval_seconds = 10

[git]
command_timeout_seconds = 300
"@
Set-Content -LiteralPath $Config -Value $Toml -Encoding ascii

Write-Host "scenario ready: $Root"
Write-Host "  pending repo: $Repo"
Write-Host "  clean repo:   $Clean"
Write-Host "  registry:     $Registry"
Write-Host "  config:       $Config"
Write-Host "next: powershell -NoProfile -ExecutionPolicy Bypass -File start-server.ps1"
