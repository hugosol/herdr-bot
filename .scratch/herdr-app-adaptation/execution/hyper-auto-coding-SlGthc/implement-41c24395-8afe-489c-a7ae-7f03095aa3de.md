# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "08-paired-history-recovery-acceptancemd-aa3094f9",
  "heads": {
    "herdr-bot": "0706830f19b58088dbf12e5bfb33c9b80fd2e3af",
    "toolkit": "4eff99ebb38b395965c99bbbfb1401357fa6b790"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

已提交 08 到票据分支：`41a7826ede`、`0706830f19`。未合入集成分支，toolkit 未改源码。**专项入口已交付，整票验收未通过。**

- **真实恢复：** OMP 19.0.0、Pi 1.5.2 的 TUI→App、App→TUI 四格均通过。核对了旧进程退出、新进程的原生 ID/file、实际 cwd、宿主内部焦点及原容器 ID。同端命中、人为同值、最新 Suite 加载和不重放初始指示也有独立证据。修复了真实 Herdr 拒绝的 `pane process-info` 调用，并保留了公开 HTTP 回归的红／绿证据。
- **降级与冲突：** 四种来源环境降级均实跑。App 如实返回「已请求并定位、原生身份未确认」，未强加 connect-bot；文件头和来源 agent 冲突仍是硬失败。另有 43 个受控用例通过，不计作真实宿主证据。
- **仍未通过：** OMP 目录回退后实际改写了历史 cwd；Pi 在目录回退时出现启动失败或身份未确认。真实 Herdr 复用同路径 checkout，未能构造双重 Git 归属，歧义格仍未验证。

后端全量 **76 个目标及 doctest 通过**；前端 **274 通过**；浏览器 **378 通过、1 跳过、1 个旧文案断言失败**。Toolkit typecheck、integration check、定向 dock **17 通过**；framework **767 通过、1 失败**，全量也未通过。

复跑命令见 `docs/agents/testing.md`。原始日志及逐格证据保留在 `C:\Users\hugos\AppData\Local\Temp\hb08-evidence-8MY9K0`，交由独立集成代理接手。
