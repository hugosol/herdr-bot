@echo off
rem ============================================================
rem  herdr-bot fixtures: end the long-running fixtures (human stop entry point).
rem
rem  Usage:  fixtures\stop-fixtures.bat
rem
rem  Writes stop markers so the fixtures end by themselves, force-kills the
rem  process tree by PID file after a grace period, and appends the ending
rem  method to the call record. Safe to run when nothing is running.
rem
rem  Batch files stay ASCII-only on purpose: cmd.exe parses them with the
rem  system ANSI code page, so non-ASCII comments can shift byte pairs and
rem  make comment fragments run as commands. See fixtures/README.md.
rem ============================================================
setlocal
powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0lib\stop-fixtures.ps1" %*
exit /b %ERRORLEVEL%
