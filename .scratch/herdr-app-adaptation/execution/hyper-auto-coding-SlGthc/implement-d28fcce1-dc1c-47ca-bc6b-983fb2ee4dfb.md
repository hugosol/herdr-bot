# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "09-chief-dispatch-reportingmd-ea50eb51-eb48-44bf",
  "heads": {
    "herdr-bot": "6ae90e6c14072eb483ae0055f9d9db692a488192"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票据 09 已实现并提交：`6ae90e6c14`。未合入 `dev`，交由独立集成代理接管。

- **P23／P37**：公开分派在 OMP／Pi × TUI／App 四种组合验证了当前 chief 授权、实际委派目标，以及历史和委派关联先于首次业务输入；未通过启动参数预填。
- **P38**：更换 chief 后，报告按当前原生会话身份送往正确的 Pane 或终端；原分派者仍保留，App 两个 chief 同名也不影响路由。
- **P40／P52**：通知明确失败时，公开接口仍可读回同一份报告。App 两种 agent 各持续观察至少 46 秒，均只有一次通知尝试，没有重投或复发派。

前端构建成功；`chief_delegation` 测试目标 27 项通过，最后扩充的 TUI 接收者用例单独复验通过。验证使用受控宿主，不替代票据 14 的真实安装宿主验收。
