# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "02-core-routingmd-4165cb94-d16d-4e73-bac5-dd757d",
  "heads": {
    "toolkit": "7b77008aff5e07d7d9b133a5f4e74ad682537733"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票据 02 已提交：`7b77008aff`。未合入 `dev`，可交由独立集成代理接管。

- **P19**：基于固定快照和具名映射迁移核心路由、24 条工程原则与 14 个 playbook。逐项审读了生成正文；worker 与持续模式共用同一份生成规则。
- **P43**：记录了资源分类、来源和许可证，明确区分暂缓专题与保留的决策日志、安全暂停、WIP 和本地交接。
- **P44**：自动核对默认技能集合和引用。已安装的 OMP 19.0.1 成功按需读取调查 playbook；显式读取 6 个暂缓技能均返回原生 `Unknown skill` 错误。

`pstack:preview`、`pstack:build`、`pstack:check`、`pstack:upstream`、`bun run typecheck` 均退出 `0`；`bun run test:pstack` **16 通过、0 失败**。只读检查前后的文件清单和摘要一致。日志、原生结果和内容审查记录保留在 `.scratch/pstack-omp-suite/evidence/02-core-routing/`。

未验证持续模式的运行时接入、H1/H2 人工验收或模型自主路由；本票不认领 P4/P11 最终包完整性。
