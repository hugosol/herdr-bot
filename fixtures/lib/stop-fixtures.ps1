<#
.SYNOPSIS
  结束所有“长时间不结束”的验证夹具，并确认没有遗留进程。

.DESCRIPTION
  对每个长期夹具（long-running、detached-child）：
  1. 若 <记录目录>/pids/<夹具>.pid 记录的进程仍在运行、且确实运行着该夹具批处理，
     则写入停止标记 <记录目录>/stop/<夹具>.stop，夹具见到标记后自行结束；
  2. 在宽限期内等待标记被消费或进程结束；
  3. 宽限期后仍存活时，按 PID 执行 taskkill /T /F 强制结束进程树；
  4. 没有运行时只清理残留标记，不做多余等待。

  最后做一次兜底清理：按命令行里出现的夹具批处理名查找仍然存活、但可能还没登记 PID 的夹具进程
  （例如刚被分离启动、尚未写入 PID 文件的子进程），并强制结束它们。

  PID 文件在使用前会先核对进程身份，避免 PID 被系统回收后误杀无关进程。
  结束方式（停止标记、强制结束、兜底清理）会追加到调用记录（fixture=stop-fixtures，stage=stop）。

  即使没有夹具在运行，也可以安全执行。退出码 0 表示无遗留进程，1 表示仍有进程存活。
#>
[CmdletBinding()]
param(
    # 记录目录；省略时用环境变量 OMP_BOT_FIXTURE_RECORD_DIR 或 <fixtures>/runtime。
    [string]$RecordDir = '',
    # 等待夹具自行结束的宽限秒数，超时后强制结束。
    [int]$GraceSeconds = 15
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'fixture-lib.ps1')

# 需要人工结束的长期夹具：名称用于 PID 文件与停止标记，Pattern 用于核对/兜底查找进程。
$targets = @(
    @{ Name = 'long-running'; Pattern = 'long-running.bat' },
    @{ Name = 'detached-child'; Pattern = 'detached-child.bat' }
)

$resolvedRecordDir = Resolve-FixtureRecordDir -Explicit $RecordDir
$recordFile = Initialize-FixtureRecordDir -RecordDir $resolvedRecordDir
$stopDir = Join-Path $resolvedRecordDir 'stop'
$pidDir = Join-Path $resolvedRecordDir 'pids'
# 本脚本所属的夹具目录：查找进程时以它为界，避免影响另一个夹具副本（例如并行测试各有一份拷贝）。
$fixturesRoot = Split-Path -Parent $PSScriptRoot

function Test-ContainsText {
    <#
    .SYNOPSIS 大小写无关地判断文本是否包含片段。
    #>
    param([string]$Text, [string]$Fragment)

    return $null -ne $Text -and $Text.IndexOf($Fragment, [System.StringComparison]::OrdinalIgnoreCase) -ge 0
}

function Get-FixtureProcesses {
    <#
    .SYNOPSIS 查找本夹具目录下、命令行里带着指定夹具批处理的 cmd.exe 进程。
    #>
    param([Parameter(Mandatory = $true)][string]$Pattern)

    return @(Get-CimInstance -ClassName Win32_Process -Filter "Name='cmd.exe'" -ErrorAction Stop |
        Where-Object {
            (Test-ContainsText -Text $_.CommandLine -Fragment $fixturesRoot) -and
            (Test-ContainsText -Text $_.CommandLine -Fragment $Pattern)
        })
}

function Test-FixtureIdentity {
    <#
    .SYNOPSIS 核对某个 PID 是否确实在运行本夹具目录下的指定夹具批处理。
    #>
    param([Parameter(Mandatory = $true)][int]$ProcessId, [Parameter(Mandatory = $true)][string]$Pattern)

    $process = Get-CimInstance -ClassName Win32_Process -Filter "ProcessId=$ProcessId" -ErrorAction SilentlyContinue
    return $null -ne $process -and
        (Test-ContainsText -Text $process.CommandLine -Fragment $fixturesRoot) -and
        (Test-ContainsText -Text $process.CommandLine -Fragment $Pattern)
}

function Stop-FixtureProcessTree {
    <#
    .SYNOPSIS 按 PID 强制结束进程树；进程已经结束不算失败。

    .DESCRIPTION
      taskkill 找不到进程时会写标准错误；在 $ErrorActionPreference = 'Stop' 下，
      重定向的原生标准错误会被升级为终止错误，因此这里局部改用 Continue 捕获输出。
    #>
    param([Parameter(Mandatory = $true)][int]$ProcessId)

    $ErrorActionPreference = 'Continue'
    $null = & taskkill.exe /PID $ProcessId /T /F 2>&1
    return $LASTEXITCODE
}

$actions = @()
$remaining = 0
$stoppedPids = @{}

foreach ($target in $targets) {
    $name = $target.Name
    $marker = Join-Path $stopDir "$name.stop"
    $pidFile = Join-Path $pidDir "$name.pid"

    $fixturePid = 0
    if (Test-Path -LiteralPath $pidFile) {
        $parsed = 0
        if ([int]::TryParse((Get-Content -LiteralPath $pidFile -Raw).Trim(), [ref]$parsed)) {
            $fixturePid = $parsed
        }
    }

    $alive = $fixturePid -gt 0 -and (Test-FixtureProcessAlive -ProcessId $fixturePid)
    if ($alive -and -not (Test-FixtureIdentity -ProcessId $fixturePid -Pattern $target.Pattern)) {
        # PID 已被系统回收给别的进程：不动它，只清掉过期 PID 文件。
        Remove-Item -LiteralPath $pidFile -Force -ErrorAction SilentlyContinue
        $actions += "$name=stale-pid-file(pid=$fixturePid)"
        continue
    }

    if (-not $alive) {
        Remove-Item -LiteralPath $marker -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $pidFile -Force -ErrorAction SilentlyContinue
        $actions += "$name=not-running"
        continue
    }

    Set-Content -LiteralPath $marker -Value (Get-Date).ToString('yyyy-MM-ddTHH:mm:ss.fffK') -Encoding ASCII

    $deadline = (Get-Date).AddSeconds($GraceSeconds)
    while ((Get-Date) -lt $deadline) {
        if (-not (Test-Path -LiteralPath $marker)) { break }
        if (-not (Test-FixtureProcessAlive -ProcessId $fixturePid)) { break }
        Start-Sleep -Milliseconds 200
    }
    $endedByMarker = -not (Test-Path -LiteralPath $marker)
    Remove-Item -LiteralPath $marker -Force -ErrorAction SilentlyContinue

    if ($endedByMarker) {
        # 夹具看到标记后先写结束记录、再退出；给它一个短暂的退出窗口，
        # 避免在 cmd.exe 尚未退出的瞬间把它误记为 force-killed。
        $exitDeadline = (Get-Date).AddSeconds(5)
        while ((Get-Date) -lt $exitDeadline -and (Test-FixtureProcessAlive -ProcessId $fixturePid)) {
            Start-Sleep -Milliseconds 100
        }
    }

    if (-not (Test-FixtureProcessAlive -ProcessId $fixturePid)) {
        Remove-Item -LiteralPath $pidFile -Force -ErrorAction SilentlyContinue
        if ($endedByMarker) { $actions += "$name=ended-by-stop-marker" }
        else { $actions += "$name=ended-during-stop(pid=$fixturePid)" }
        $stoppedPids[$fixturePid] = $true
        continue
    }

    $null = Stop-FixtureProcessTree -ProcessId $fixturePid
    Start-Sleep -Milliseconds 300
    if (Test-FixtureProcessAlive -ProcessId $fixturePid) {
        $actions += "$name=still-running(pid=$fixturePid)"
        $remaining++
    } else {
        Remove-Item -LiteralPath $pidFile -Force -ErrorAction SilentlyContinue
        $actions += "$name=force-killed(pid=$fixturePid)"
        $stoppedPids[$fixturePid] = $true
    }
}

# 兜底清理：可能尚未登记 PID 就已在运行的夹具进程（例如刚被分离启动的子进程）。
foreach ($target in $targets) {
    foreach ($process in (Get-FixtureProcesses -Pattern $target.Pattern)) {
        if ($process.ProcessId -eq $PID) { continue }
        if ($stoppedPids.ContainsKey([int]$process.ProcessId)) { continue }
        if (-not (Test-FixtureProcessAlive -ProcessId $process.ProcessId)) { continue }
        $null = Stop-FixtureProcessTree -ProcessId $process.ProcessId
        Start-Sleep -Milliseconds 300
        if (Test-FixtureProcessAlive -ProcessId $process.ProcessId) {
            $actions += "$($target.Name)=sweep-still-running(pid=$($process.ProcessId))"
            $remaining++
        } else {
            $actions += "$($target.Name)=sweep-killed(pid=$($process.ProcessId))"
        }
    }
}

$detail = $actions -join ';'
$null = Add-FixtureRecord -RecordFile $recordFile -FixtureName 'stop-fixtures' -RecordStage 'stop' `
    -FixturePid (Get-FixtureProcessId) -WorkingDirectory (Get-Location).Path `
    -CallIdentity ([guid]::NewGuid().ToString('N')) -RecordDetail $detail

Write-Host "[fixtures] 结束夹具：$detail"
Write-Host "[fixtures] 记录目录：$resolvedRecordDir"

if ($remaining -gt 0) { exit 1 }
exit 0
