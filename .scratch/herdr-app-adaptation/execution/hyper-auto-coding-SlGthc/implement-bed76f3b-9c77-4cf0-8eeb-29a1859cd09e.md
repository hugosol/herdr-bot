# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "13-run-role-recoverymd-1d733d9a-a863-4bc2-8797-2",
  "heads": {
    "herdr-bot": "98300e594f566ac0a778e207d2d12ec4d912843c"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票据 13 已实现并提交：`98300e594f`，待独立集成。

- Resume 沿用原 Run、节点执行和 chief 委派关联，不新建执行、不重放输入，也不接管或迁移 Run。
- 子 Run 管理者保留双角色；结束节点保留执行关联，但与独立普通 Chat 一样不获得节点职责。
- 环境降级时保留原归属，明确列出未显式装配的角色扩展，不声称已经加载。

**验证：** P42 的 S1 专项和相关回归共 10 个测试通过，覆盖 OMP／Pi × TUI／App。全后端检查在 `history_workspace` 中止：既有 Pi 定位夹具的原生会话头缺少 `cwd`，在进入角色解析前被拒绝。其余全量检查未完成；真实宿主的原生角色行为由票据 14 验证。
