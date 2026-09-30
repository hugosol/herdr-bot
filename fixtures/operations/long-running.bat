@echo off
rem ============================================================
rem  Fixture: keeps running for a long time; only a human or
rem  stop-fixtures.bat can end it (running state, dropped
rem  connection, no automatic termination).
rem
rem  Writes a call record (fixture=long-running, stage=invoke), writes its PID to
rem  <record-dir>\pids\long-running.pid, then waits for <record-dir>\stop\long-running.stop.
rem  When that stop marker appears it records stage=stop-observed and exits 0.
rem
rem  Exit code 97 means the fixture's own recording mechanism failed.
rem
rem  Batch files stay ASCII-only on purpose: cmd.exe parses them with the
rem  system ANSI code page, so non-ASCII comments can shift byte pairs and
rem  make comment fragments run as commands. See fixtures/README.md.
rem ============================================================
setlocal
powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0..\lib\record-call.ps1" -Fixture long-running -Stage invoke -Detail "holds-until-stop-marker" -TrackPid -HoldUntilStop -StopStage stop-observed
if errorlevel 1 (
    echo [fixtures] long-running.bat: recording the call failed 1>&2
    exit /b 97
)
exit /b 0
