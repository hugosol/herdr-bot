# 关机使用 ExitWindowsEx(EWX_SHUTDOWN | EWX_FORCEIFHUNG)

- 状态：accepted
- 日期：2026-09-30

> [ADR 0005](0005-backend-authoritative-shutdown-countdown.md) 仅替代本文的同步 HTTP 回执。当前关机请求返回 200 表示后端已接受倒计时；到期才执行系统调用，结果另由观察流报告。发出指令不证明机器已经断电。

关机由后端调用 Win32 `ExitWindowsEx(EWX_SHUTDOWN | EWX_FORCEIFHUNG)`：Windows 先向各应用发 `WM_QUERYENDSESSION`/`WM_ENDSESSION` 让其保存并退出，对超过系统等待仍不响应的进程才强制结束，因此“等待应用关闭再断电”是有界的，不会永远停在“应用阻止关机”。需要进程启用 `SeShutdownPrivilege`。按原来的同步 HTTP 设计，调用返回 TRUE 就回 200，返回 FALSE 就回 500 `shutdown-failed`。失败不静默改走强制关机。测试可替换 `PowerOff`，真实断电须由人工观察。

## 备选与代价

- 备选：两段式 `shutdown /s /t 0` + 看门狗 `shutdown /s /f /t 0`（零原生代码，但多一个阈值与竞态）；直接 `shutdown /s /f /t 0` 或 `EWX_FORCE`（不等应用，可能丢数据）。`shutdown /s /t N`（N > 0）隐含强制关闭，不符合先通知应用保存的选择。
- 代价：引入 `windows` crate 与提权代码；`EWX_FORCEIFHUNG` 仍会强杀不响应的进程；测试接缝意味着 `ExitWindowsEx` 本身只在人工验收里被真实执行。
