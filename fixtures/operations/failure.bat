@echo off
rem ============================================================
rem  Fixture: ends with the explicit failure exit code 3.
rem
rem  Writes one call record (fixture=failure, stage=invoke, detail=exit-code=3)
rem  and exits 3. Exit code 97 means the fixture's own recording failed.
rem
rem  Batch files stay ASCII-only on purpose: cmd.exe parses them with the
rem  system ANSI code page, so non-ASCII comments can shift byte pairs and
rem  make comment fragments run as commands. See fixtures/README.md.
rem ============================================================
setlocal
powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0..\lib\record-call.ps1" -Fixture failure -Stage invoke -Detail "exit-code=3"
if errorlevel 1 (
    echo [fixtures] failure.bat: recording the call failed 1>&2
    exit /b 97
)
exit /b 3
