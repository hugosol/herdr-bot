@echo off
rem ============================================================
rem  Fixture child: started detached by spawn-detached.bat and
rem  long running. Same stop mechanism as long-running.bat.
rem
rem  Runs in its own window titled "omp-bot detached child", so it is
rem  identifiable. Writes a call record (fixture=detached-child, stage=child-start)
rem  and its PID to <record-dir>\pids\detached-child.pid, then waits for
rem  <record-dir>\stop\detached-child.stop and records stage=stop-observed.
rem
rem  Exit code 97 means the fixture's own recording mechanism failed.
rem
rem  Batch files stay ASCII-only on purpose: cmd.exe parses them with the
rem  system ANSI code page, so non-ASCII comments can shift byte pairs and
rem  make comment fragments run as commands. See fixtures/README.md.
rem ============================================================
setlocal
powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0..\lib\record-call.ps1" -Fixture detached-child -Stage child-start -Detail "spawned-detached-long-child" -TrackPid -HoldUntilStop -StopStage stop-observed
if errorlevel 1 (
    echo [fixtures] detached-child.bat: recording the call failed 1>&2
    exit /b 97
)
exit /b 0
