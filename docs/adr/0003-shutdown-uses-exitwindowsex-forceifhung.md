# 关机使用 ExitWindowsEx(EWX_SHUTDOWN | EWX_FORCEIFHUNG)

- 状态：accepted
- 日期：2026-09-30

关机由后端调用 Win32 `ExitWindowsEx(EWX_SHUTDOWN | EWX_FORCEIFHUNG)`：Windows 先向各应用发 `WM_QUERYENDSESSION`/`WM_ENDSESSION` 让其保存并退出，对超过系统等待仍不响应的进程才强制结束，因此“等待应用关闭再断电”是有界的，不会永远停在“应用阻止关机”。需要进程启用 `SeShutdownPrivilege`。调用是同步的：返回 TRUE 就回 200，返回 FALSE 就回 500 `shutdown-failed`，不静默改走强制关机。内部通过一个可替换的 `PowerOff` 接缝调用，测试注入记录型实现，真实断电只做一次人工验收。

## 备选与代价

- 备选：两段式 `shutdown /s /t 0` + 看门狗 `shutdown /s /f /t 0`（零原生代码，但多一个阈值与竞态）；直接 `shutdown /s /f /t 0` 或 `EWX_FORCE`（不等应用，可能丢数据）。分析见 [`docs/omp-auto-shutdown-watcher.md`](../omp-auto-shutdown-watcher.md) 的 Q18。
- 代价：引入 `windows` crate 与提权代码；`EWX_FORCEIFHUNG` 仍会强杀不响应的进程；测试接缝意味着 `ExitWindowsEx` 本身只在人工验收里被真实执行。
