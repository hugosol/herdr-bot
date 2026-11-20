# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "01-framework-readymd-4fbdec8f-17ce-41ee-81ca-9e7",
  "heads": {
    "toolkit": "2a2d926bdc0ef0c8dd286217952eec52e1dcbf35"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

已提交 `2a2d926bdc`，未合入 `dev`。

- **P16–P18**：新增会话级 `session_ready`，不改变既有启动、扩展初始化、登记或活动状态语义。真实 OMP 在启动 hook 挂起时不发布，原生提交闸门打开后才发布；真实 Pi 首启与恢复均验证了终端输入及原生历史，后续原生模态证明 ready 不保证持续可输入。
- **P19**：生产 connect-bot 通过现有事件接口上报启动关联、真实会话 ID、原生文件路径（可用时）和 PID。owner 切换或结束后撤回证据并取消待完成的请求，替换 owner 不能复用关联，普通登记仍可用。已与 03 对齐协议；硬退出时仍由接收端验证目标和进程是否存活。

最终验证通过：typecheck、`check:integrations`、framework **768/768**、integration **72/72**、connect-bot **174/174**、受影响原生 dock **21/21**。环境为 Bun 1.4.4、OMP 19.0.0、Pi 1.5.2 / Node 24.18.0；新增原生用例使用受控 loopback provider 和 HTTP peer，不构成 Herdr TUI/App 配对验收。
