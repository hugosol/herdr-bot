# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-06-history-placementmd-80a9a1f6-d5af-4",
  "heads": {
    "herdr-bot": "73e9fc29dd979ee0e0237fbc7cd1cc3a415ebc35"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票据 06 集成验收通过。原开发分支停在 `73e9fc29`，工作区干净；未操作集成工作区，也未直接合入 `dev`。

已在原 worktree 同步票据 03 的启动接口和回归修复，保留两票的夹具路由，并修复真实后端重启及并发历史测试。验收覆盖原容器 ID 原样查询与持久记录不变、cwd 降级、App 目标聚焦及失败分类。

- `node scripts/test-backend.mjs --binary-jobs 2`：全部通过，含 doctest；`chat_resume` 13/13，`history_workspace` 6/6。
- `npm --prefix frontend test`：274/274 通过。
- `npm --prefix frontend run test:browser:build -- --workers=2`：377 通过、1 跳过、0 失败。

票据 06 只涉及 `herdr-bot`；真实 TUI ↔ App 跨端恢复验收归票据 08，本轮未将受控夹具结果当作真实宿主证据。
