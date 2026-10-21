@echo off
rem ============================================================
rem  Fixture: starts a detached long-running child process and
rem  exits immediately (the existing start script opens windows
rem  for long-running Python programs the same way).
rem
rem  Writes a call record (fixture=spawn-detached, stage=invoke) and exits 0
rem  without waiting: "start" without /B gives the child its own console, so it
rem  outlives this batch. The child registers itself via detached-child.bat.
rem
rem  Exit code 97 means the fixture's own recording mechanism failed.
rem
rem  Batch files stay ASCII-only on purpose: cmd.exe parses them with the
rem  system ANSI code page, so non-ASCII comments can shift byte pairs and
rem  make comment fragments run as commands. See fixtures/README.md.
rem ============================================================
setlocal
powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0..\lib\record-call.ps1" -Fixture spawn-detached -Stage invoke -Detail "spawns-detached-child"
if errorlevel 1 (
    echo [fixtures] spawn-detached.bat: recording the call failed 1>&2
    exit /b 97
)
start "herdr-bot detached child" /MIN cmd.exe /C "%~dp0detached-child.bat"
exit /b 0
