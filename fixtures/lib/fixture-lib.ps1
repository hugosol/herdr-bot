<#
.SYNOPSIS
  herdr-bot 验证夹具共用的调用记录与进程工具。

.DESCRIPTION
  被 record-call.ps1 与 stop-fixtures.ps1 点源引用，提供三件事：
  * 解析记录目录（显式参数 > 环境变量 HERDR_BOT_FIXTURE_RECORD_DIR > <fixtures>/runtime）；
  * 以命名互斥保护的追加写，向记录文件写一行可核对的调用记录；
  * 查询夹具进程 PID、判断进程是否仍在运行。

  记录文件为制表符分隔的一行一记录，字段顺序见 fixtures/README.md：
  call, seq, timestamp, fixture, stage, callid, pid, cwd, detail。
#>

Set-StrictMode -Version Latest

function Resolve-FixtureRecordDir {
    <#
    .SYNOPSIS 解析记录目录；省略时使用环境变量或夹具根目录下的 runtime。
    #>
    param([string]$Explicit = '')

    if ($Explicit -ne '') { return $Explicit }
    if ($env:HERDR_BOT_FIXTURE_RECORD_DIR) { return $env:HERDR_BOT_FIXTURE_RECORD_DIR }
    # 本文件位于 <fixtures>/lib，因此向上一级就是夹具根目录。
    return (Join-Path (Split-Path -Parent $PSScriptRoot) 'runtime')
}

function Initialize-FixtureRecordDir {
    <#
    .SYNOPSIS 创建记录目录及其子目录，返回记录文件路径。
    #>
    param([Parameter(Mandatory = $true)][string]$RecordDir)

    foreach ($dir in @($RecordDir, (Join-Path $RecordDir 'pids'), (Join-Path $RecordDir 'stop'))) {
        if (-not (Test-Path -LiteralPath $dir)) {
            New-Item -ItemType Directory -Path $dir -Force | Out-Null
        }
    }
    return (Join-Path $RecordDir 'calls.tsv')
}

function Get-FixtureProcessId {
    <#
    .SYNOPSIS 返回夹具进程 PID：运行批处理的 cmd.exe，也就是本 powershell 的父进程。
    #>
    try {
        $parent = (Get-CimInstance -ClassName Win32_Process -Filter "ProcessId=$PID" -ErrorAction Stop).ParentProcessId
        if ($parent -and $parent -gt 0) { return [int]$parent }
    } catch {
        Write-Warning "无法查询父进程 PID，改用当前 powershell 进程 PID：$($_.Exception.Message)"
    }
    return [int]$PID
}

function Test-FixtureProcessAlive {
    <#
    .SYNOPSIS 判断进程是否仍在运行。
    #>
    param([Parameter(Mandatory = $true)][int]$ProcessId)

    return $null -ne (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)
}

function Add-FixtureRecord {
    <#
    .SYNOPSIS 追加一行调用记录，返回 (调用序号, 记录行序号)。

    .DESCRIPTION
      整个“读记录、算序号、追加一行”的过程由命名互斥保护，并用重试吸收与读取端
      （例如核对记录文件的测试进程）之间的文件共享冲突，因此重叠调用不会丢行。
      同一次调用的各阶段（同一 CallIdentity）得到同一个调用序号。
    #>
    param(
        [Parameter(Mandatory = $true)][string]$RecordFile,
        [Parameter(Mandatory = $true)][string]$FixtureName,
        [Parameter(Mandatory = $true)][string]$RecordStage,
        [Parameter(Mandatory = $true)][int]$FixturePid,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [string]$CallIdentity = '',
        [string]$RecordDetail = '',
        # 记录互斥名：跨进程串行化追加写，保证重叠调用不丢行、序号不乱。
        [string]$MutexName = 'Local\herdr-bot-fixture-record'
    )

    $mutex = New-Object System.Threading.Mutex($false, $MutexName)
    $held = $false
    try {
        $held = $mutex.WaitOne([TimeSpan]::FromSeconds(120))
        if (-not $held) { throw '等待调用记录互斥锁超时' }

        $attempt = 0
        while ($true) {
            try {
                $lines = @()
                if (Test-Path -LiteralPath $RecordFile) {
                    # 外层 @() 必需：只有一行时管道会退化成字符串，严格模式下 .Count 会失败。
                    $lines = @(@(Get-Content -LiteralPath $RecordFile -Encoding UTF8) |
                        Where-Object { $_.Trim() -ne '' })
                }

                $callOrdinal = 0
                if ($CallIdentity -ne '') {
                    foreach ($line in $lines) {
                        $fields = $line -split "`t"
                        if ($fields.Count -ge 6 -and $fields[5] -eq $CallIdentity) {
                            $callOrdinal = [int]$fields[0]
                        }
                    }
                }
                if ($callOrdinal -eq 0) {
                    $max = 0
                    foreach ($line in $lines) {
                        $fields = $line -split "`t"
                        $value = 0
                        if ($fields.Count -ge 1 -and [int]::TryParse($fields[0], [ref]$value) -and $value -gt $max) {
                            $max = $value
                        }
                    }
                    $callOrdinal = $max + 1
                }

                $timestamp = (Get-Date).ToString('yyyy-MM-ddTHH:mm:ss.fffK')
                $recordLine = @(
                    $callOrdinal,
                    ($lines.Count + 1),
                    $timestamp,
                    $FixtureName,
                    $RecordStage,
                    $CallIdentity,
                    $FixturePid,
                    $WorkingDirectory,
                    $RecordDetail
                ) -join "`t"
                [System.IO.File]::AppendAllText(
                    $RecordFile,
                    $recordLine + "`r`n",
                    (New-Object System.Text.UTF8Encoding($false))
                )
                return @($callOrdinal, ($lines.Count + 1))
            } catch [System.IO.IOException] {
                $attempt++
                if ($attempt -ge 60) { throw }
                Start-Sleep -Milliseconds 100
            }
        }
    } finally {
        if ($held) { $mutex.ReleaseMutex() }
        $mutex.Dispose()
    }
}
