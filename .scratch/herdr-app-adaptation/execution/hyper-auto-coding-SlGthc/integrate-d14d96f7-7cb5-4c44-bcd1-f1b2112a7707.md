# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-08-paired-history-recovery-acceptancem",
  "heads": {
    "herdr-bot": "06d703b208835006c2837aa3a56c66eafe52706c",
    "toolkit": "fdccaa4c14c1ceb9774b32e77146e0dfb8c38e08"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票据 08 的两仓全量回归通过，**真实宿主专项 38/44 通过，整票尚未验收通过**。

两仓原开发分支已同步，遗留合并冲突已解决，工作区均干净；未操作集成工作区。`herdr-bot` 合并提交 `895ceda`，最终提交 `06d703b`，拒绝同一 Run 重复关联原生会话；`toolkit` 最终提交 `fdccaa4`，修复配对测试夹具的启动取证与工作区配置。

- **herdr-bot：** 完整后端 76 个目标及 doctest 通过；前端 274 项通过；浏览器 379 项通过、1 项按配置跳过；受控 Resume 回归 44 项、专项 CLI 自测 3 项通过。
- **toolkit：** `typecheck`、`check:integrations` 通过；framework 768 项、全量 1960 项、原生 session/connect-bot dock 17 项通过。
- **真实专项：** OMP/Pi 双向跨端恢复四格全部通过，独立核对了旧进程退出、新进程原生 ID 和文件、实际 cwd、宿主焦点，以及原容器 ID 未改变。

剩余 6 格未通过：Pi 1.5.2 的 3 个缺目录场景显示原生 Continue/Fork/Cancel 选择框，公开 Resume 返回 `502 restore-unconfirmed`，未代选；Herdr 0.9.5 的 2 个 Git 歧义场景因同路径 checkout 返回 `already_open: true`，无法构造两份真实归属证据；TUI→App 的 OMP 缺目录场景在确认旧进程退出后仍遇到 Windows `WinError 32`，未能移除原目录。未用受控结果补算原生格。

本轮原始日志和证据保留在 `%TEMP%\hb08-integration`。
