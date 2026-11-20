# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "02-host-selectionmd-e4c037b3-c249-4693-9556-4723",
  "heads": {
    "herdr-bot": "e1bc76b3c5deb8e4636d7e711f2aaf0cc5f136df"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

已提交 `e1bc76b3c5`，未合入 `dev`。

- **P1–P4、P8–P9**：真实 TUI／App 验证了显式选择、默认 TUI、非法值拒绝，以及两种默认容器名称的精确匹配。零匹配或多匹配均拒绝，未创建容器或 Tab。
- **P5**：通过 App 布局和 core 终端归属独立核对目标；另一 core 存在相同短终端地址时，操作仍明确拒绝。
- **P51**：两端固定脚本绑定通过验证。实际宿主状态与脚本回执分开观测，页面仅显示脚本结果。

真实宿主专项通过，进程清理无错误；配置及固定操作测试 8 项通过，浏览器 2 项通过。全量后端回归未通过：`workflow_dispatch` 有 3 项发布定义断言失败，涉及 Map 数量和 `meta-data/task-options.toml` 快照；11 个后续目标及 doctest 未运行。

本票验证到真实 Tab／终端放置；真实 Task 初始化和输入仍由 05 验收。
