# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "01-resource-maintenancemd-0990da4c-8227-497a-85d",
  "heads": {
    "toolkit": "0bb0dd060c8c0314aee55256de1063f1a221721e"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票据 01 已实现并提交：`0bb0dd060c`。未合入 `dev`，交由独立集成代理接管。

- **P5、P8、P9、P12：** 验证构建不改上游快照，重复生成字节一致，文本和二进制产物符合独立预期；只读检查能发现手改、缺失和过期产物，且不改变文件或目录清单。
- **P6、P7、P10：** 验证具名适配与上游差异分开报告；缺失、歧义锚点和冲突编辑均明确失败。实际 OMP 19.0.1 确认生成的隐藏技能仅在显式调用时进入请求。
- **P13：** 验证部分构建失败会报告已发生的文件操作；清理只删除内容未变的过期生成文件，保留用户文件和证据，不跟随目录链接。

`bun run test:pstack` **14 通过、0 失败**；`bun run typecheck` 通过。四个维护命令均在真实快照上验证成功。原始日志和 OMP 请求证据保存在 `.scratch/pstack-omp-suite/evidence/01-resource-maintenance/`。本票无未验证项；未将受控请求证据当作模型行为验证。
