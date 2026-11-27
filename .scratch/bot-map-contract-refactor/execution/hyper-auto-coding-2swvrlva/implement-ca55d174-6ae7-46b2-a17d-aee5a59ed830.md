# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "01-centralize-role-contextmd-72cdb4eb-7741-4a2e-",
  "heads": {
    "bot": "068614b77d12bb9bdb599ce3aeb37fcadd76adc7"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票据 01 已实现并提交：`068614b77d`，未合入集成分支。

将独立 Task、Run 和 chief 委派的角色说明、运行事实格式、词典与消息装配集中到 `prompts`。身份判定、状态准入、材料解析、启动和恢复仍由原调用方负责。保留初始 Skill、字面用户输入、根／子管理者差异和结束节点例外；未引入新业务政策。

`cargo check --manifest-path backend/Cargo.toml --all-targets` 通过；5 项单元回归和 11 项定向公开接口回归全部通过。证据已写入本票。未执行真实宿主或模型验收；票据待独立集成后关闭。
