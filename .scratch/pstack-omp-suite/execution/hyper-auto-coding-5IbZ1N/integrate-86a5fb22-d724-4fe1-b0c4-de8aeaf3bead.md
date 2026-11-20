# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-08-windows-statechartmd-52c170e3-0348-",
  "heads": {
    "toolkit": "cffceeaa5df1da2d7b16c633a9e6f4b5a1dc086f"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

08 已完成集成验收。原 `toolkit` 开发分支已同步 `ab0b5a2`，无冲突，工作区干净。补充提交 `cffceeaa5d` 说明 Windows 验收须直接使用 JDK 的 `java.exe`：首次用 Oracle `javapath` 转发器时，启动 PID 与 TLC 自报 PID 不同；切换至实际 JVM 后，原断言通过。未操作集成工作区或直接合入 `dev`。

- `bun run test`：1980 通过、0 失败；完整 JUnit 覆盖已确认。跨仓用例使用本票工作区内独立构建的 Herdr 安全后端和测试夹具。
- `bun run test:pstack`：19 通过、0 失败。实际 Windows OMP、Java 和 TLC 核验了两状态、两条有向边、最短反例及临时资源清理。管线各场景实际退出为 `0 / 1 / 2 / 2`；解析错误和缺少 TLC 均未发布产物。
- `bun run typecheck` 及 `pstack:build`、`pstack:check`、`pstack:preview`、本地上游对照均退出 `0`。

[原始验收证据](local://08-windows-statechart-integration.tar.gz)。可选 SVG 渲染不在本票验收范围内。
