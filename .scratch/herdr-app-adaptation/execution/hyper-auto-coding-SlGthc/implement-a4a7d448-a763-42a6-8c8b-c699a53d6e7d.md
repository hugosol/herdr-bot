# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "11-precise-auto-closemd-5297d332-7b72-4d25-843d-",
  "heads": {
    "herdr-bot": "512c4bf8641ab55beaa4cec8095b01bcf0306444"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票据 11 已提交：`512c4bf864`，未合入 `dev`。

- **P43–P45：** TUI 和 App 只尝试关闭已完成节点执行所关联的 Tab。App 核对 Space、Pane 和 core 终端后显式确认；两端允许连带关闭因此变空的容器，不扩大到其他工作。测试分别观察关闭回执、布局移除和终端退出。
- **P46：** 关闭被拒绝或失败时，Run 仍可读回原完成事实，页面独立显示失败原因；不会重试关闭或重放完成操作。

S1 定向验证通过：`workflow_dispatch` 18 例、`workflow_start` 1 例（含 4 个 App 场景）、Playwright 14 例。

真实宿主布局和进程退出由票据 15 验收。TUI 已知会级联的组会在发送前被拒绝；查询与关闭之间新出现的组仍依赖原生默认 `ui.confirm_close = true` 保护。
