@echo off
rem ============================================================
rem  Fixture: ends immediately with the success exit code (0).
rem
rem  Writes one call record (fixture=success, stage=invoke) and exits 0.
rem  Exit code 97 means the fixture's own recording mechanism failed.
rem
rem  Batch files stay ASCII-only on purpose: cmd.exe parses them with the
rem  system ANSI code page, so non-ASCII comments can shift byte pairs and
rem  make comment fragments run as commands. See fixtures/README.md.
rem ============================================================
setlocal
powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0..\lib\record-call.ps1" -Fixture success -Stage invoke -Detail "exit-code=0"
if errorlevel 1 (
    echo [fixtures] success.bat: recording the call failed 1>&2
    exit /b 97
)
exit /b 0
