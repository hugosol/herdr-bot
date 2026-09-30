<#
.SYNOPSIS
  记录一次验证夹具调用，可选等待人工结束标记。

.DESCRIPTION
  每次夹具被调用时向同一个记录文件（默认 <fixtures>/runtime/calls.tsv）追加一行记录，
  包含调用序号、记录行序号、时间戳、夹具标识、阶段、调用标识、夹具进程 PID、实际工作目录与说明。
  互斥与重试由 fixture-lib.ps1 提供，重叠调用不会丢行。

  -HoldUntilStop 供“启动后长时间不结束、必须由人工结束”的夹具使用：记录完成后等待
  <记录目录>/stop/<Fixture>.stop 出现，再写入结束记录后退出。

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File record-call.ps1 -Fixture success -Stage invoke -Detail "exit-code=0"
#>
[CmdletBinding()]
param(
    # 夹具标识（记录中的 fixture 字段）。
    [Parameter(Mandatory = $true)][string]$Fixture,
    # 记录阶段；一次调用开始时用 invoke。
    [Parameter(Mandatory = $true)][string]$Stage,
    # 本次调用的稳定标识；省略时由本脚本生成（同一进程的各阶段共用同一个）。
    [string]$CallId = '',
    # 附加说明，例如夹具自报的退出码。
    [string]$Detail = '',
    # 记录目录；省略时用环境变量 OMP_BOT_FIXTURE_RECORD_DIR 或 <fixtures>/runtime。
    [string]$RecordDir = '',
    # 把夹具进程 PID 写入 <记录目录>/pids/<Fixture>.pid，供人工或脚本强制结束。
    [switch]$TrackPid,
    # 记录后等待停止标记文件出现再退出。
    [switch]$HoldUntilStop,
    # 等待结束后写入的阶段名。
    [string]$StopStage = 'stop-observed'
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'fixture-lib.ps1')

$resolvedRecordDir = Resolve-FixtureRecordDir -Explicit $RecordDir
$recordFile = Initialize-FixtureRecordDir -RecordDir $resolvedRecordDir
$stopFile = Join-Path (Join-Path $resolvedRecordDir 'stop') "$Fixture.stop"
$fixturePid = Get-FixtureProcessId
$workingDirectory = (Get-Location).Path

if ([string]::IsNullOrEmpty($CallId)) {
    # 并发启动的 cmd.exe 里 %RANDOM% 可能得到同一个值，因此由夹具进程自己生成唯一调用标识。
    $CallId = "p$fixturePid-$(([guid]::NewGuid().ToString('N')).Substring(0, 12))"
}

if ($HoldUntilStop) {
    # 上一次运行留下的停止标记不属于本次调用：先清掉，避免本次调用刚开始就被结束。
    Remove-Item -LiteralPath $stopFile -Force -ErrorAction SilentlyContinue
}

if ($TrackPid) {
    Set-Content -LiteralPath (Join-Path (Join-Path $resolvedRecordDir 'pids') "$Fixture.pid") `
        -Value $fixturePid -Encoding ASCII
}

$null = Add-FixtureRecord -RecordFile $recordFile -FixtureName $Fixture -RecordStage $Stage `
    -FixturePid $fixturePid -WorkingDirectory $workingDirectory -CallIdentity $CallId -RecordDetail $Detail

if ($HoldUntilStop) {
    while (-not (Test-Path -LiteralPath $stopFile)) {
        Start-Sleep -Milliseconds 200
    }
    # 先记录“已见到停止标记”，再删除标记；记录失败时标记保留，仍可由 stop-fixtures 强制结束。
    $null = Add-FixtureRecord -RecordFile $recordFile -FixtureName $Fixture -RecordStage $StopStage `
        -FixturePid $fixturePid -WorkingDirectory $workingDirectory -CallIdentity $CallId `
        -RecordDetail 'ended-by-stop-marker'
    Remove-Item -LiteralPath $stopFile -Force -ErrorAction SilentlyContinue
}
