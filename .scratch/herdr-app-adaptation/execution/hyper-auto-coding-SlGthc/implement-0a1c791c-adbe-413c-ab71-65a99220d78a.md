# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "14-paired-chief-run-acceptancemd-b3424129-f790-4",
  "heads": {
    "herdr-bot": "d5f9e5a725a72893562eb6f9d3dab59cb10eca8d",
    "toolkit": "895106c062261f75a273b88ee5c79a6dbcb39a1b"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票 14 已提交：`06e45b6dd4`、`d5f9e5a725`（herdr-bot）；toolkit 无源码变更。可交给独立集成。

- **P23、P37–P42、P52、P54–P55：** OMP／Pi × TUI／App 四格分批通过。通过实际终端和生产后端核对了首条输入前的 Chat／Run／委派关联、chief 授权及当前接收者、两类通知失败后的同一报告读回、精确聚焦，以及恢复后的原角色与原执行。四格的明确未提交均只重试一次，间隔 5.07–5.12 秒；均有独立进程退出证据。
- **仓库回归：** 后端通过；前端 274 通过；浏览器 382 通过、1 跳过。Toolkit `typecheck`、`test:framework`、`check:integrations` 均通过；全仓 1961 通过，专项 docks 64 通过。CLI 门闩和共享夹具分别 2／8 通过。
- **未验证：** 未使用真实模型网络或用户认证。App Pi 在原生 `/new` 撤回旧 owner 后恢复通过；直接关闭 Tab 曾返回 502，单独 Ctrl+D 未观测到 `launch_ended`。这两条路径未计为通过。

原始日志：`%TEMP%\hb14-checks-zebPwo`。原生证据：`hb14-native-tracer-7`、`hb14-native-app-omp-final`、`hb14-native-app-pi-final`。复跑命令见 `docs/agents/testing.md`。
