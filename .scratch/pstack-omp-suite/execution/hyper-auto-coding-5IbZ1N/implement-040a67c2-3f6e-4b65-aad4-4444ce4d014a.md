# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "12-omp-suite-selectionmd-cd2b17ce-4cc0-424e-a4ce",
  "heads": {
    "herdr-bot": "5022cf9b6b899642701af4952142ce0d59a78702"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票据 12 的 P1 已实现并验证，提交为 `b7c21ac2a8`、`5022cf9b6b`，未合入 `dev`。

- `pstack` 作为独立 OMP-only Suite 交付，只引用 toolkit 的 pstack 资源。未修改 Suite schema、宿主加载器、生产 Task、现有 Chat 或用户配置。
- 本 worktree 构建的页面确认：OMP 可选择、保存并从 zone 发起 Chat；切换到 Pi 会保留原引用并显示不兼容提示。直接通过 HTTP 新增、更新、预览或发起均拒绝不兼容配置；受控 Herdr 实际调用记录确认没有新增启动或 Tab。
- P1 浏览器用例先因缺少 `pstack` 选项失败，接入后通过。前端测试 274 通过；完整浏览器回归 383 通过、1 项符号链接用例跳过。后端 Suite／Task／Chat 定向目标通过；完整后端首轮的两项历史快照断言已修正，迁移组 4 项、`zone_watch` 7 项及 doctest 补跑通过，未再次执行全量。

截图、HTTP 回执及调用记录已保留于 `%TEMP%/pstack-12-evidence-RNyhmw`。实际 OMP 原生资源装配不由本票认证，归票据 13。
