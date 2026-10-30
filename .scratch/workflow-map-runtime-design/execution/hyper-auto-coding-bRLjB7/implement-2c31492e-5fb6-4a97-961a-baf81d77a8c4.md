# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "01-definition-cutovermd-282f8123-7369-4d16-9627-",
  "head": "36dc3b0b6b52931f08209d8ae4c5eba5475c3f4c",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "01-definition-cutover",
  "status": "实现及本票验收完成，已提交；worktree 可交给独立集成代理。尚未合入，因此票据按仓库规则保留 in-progress。",
  "commit": "36dc3b0",
  "branch": "hyper/workflow-map-runtime-design/01-definition-cutover-905cd6-a67b868fd8ba",
  "delivered": [
    "Map/node/Bot 唯一新格式切换；拒绝版本及旧字段，无运行时兼容路径。",
    "node 使用字符串 input 和显式 zone；Bot 使用独立 when、plan_guide、node_guide 文档及可缺省 task_id 引用。",
    "同步更新真实 scheduler 编辑、预览、错误反馈、调用者、既有测试、独立样例和长期文档。"
  ],
  "acceptance": [
    {
      "id": "P1.01",
      "result": "通过",
      "evidence": "schedulerDefinitionCutover.spec.ts：真实 Bot/node 保存、HTTP 回读及持久文件核对；schedulerRecovery.spec.ts：Map 旧字段及版本拒绝。"
    },
    {
      "id": "P2.01",
      "result": "通过",
      "evidence": "三个互异文档标记按逆序读取；逐份清空保存并核对其他文档不受影响；schedulerNodeEditing.spec.ts 验证 Markdown 指引不校验空 input。"
    },
    {
      "id": "P3.01",
      "result": "通过",
      "evidence": "真实保存缺 task_id、无效 Task 引用、缺失 Bot 引用及未登记/失效 zone，精确回读确认没有默认绑定。"
    },
    {
      "id": "P4.01",
      "result": "通过",
      "evidence": "未知字段、错误类型、缺必需字段、未知同图依赖、自依赖及双节点环均拒绝；联合检查原文件字节、回读结果和页面草稿保留。"
    }
  ],
  "verification": [
    {
      "command": "npm --prefix frontend run test:browser -- schedulerDefinitionCutover.spec.ts --workers=1",
      "result": "最后一次 3 passed；保留截图、Bot/Map JSON 和实际空外部启动记录。"
    },
    {
      "command": "npm --prefix frontend test",
      "result": "27 files、205 tests passed。"
    },
    {
      "command": "node scripts/test-backend.mjs --binary-jobs 2",
      "result": "passed，包含构建、全部派发测试二进制及 doctest。",
      "evidence": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-8MaBvZ/summary.json"
    },
    {
      "command": "npm --prefix frontend run test:browser:build -- --workers=4",
      "result": "289 passed、1 skipped、1 failed。本票 scheduler 用例全部通过；完整浏览器回归不能宣称全绿。",
      "failure": "未修改的 tasks.spec.ts:192：实际 Skill 选项包含 wayfinder，独立预期缺少该项。已通知 Main 与 Task 票所有者；未修改其拥有的文件。",
      "evidence": "artifact://88"
    }
  ],
  "evidence_directories": [
    "frontend/test-results/schedulerDefinitionCutover-f12c3-ntly-without-a-Task-binding/",
    "frontend/test-results/schedulerDefinitionCutover-ddfd2-alid-edits-preserve-the-Map/"
  ],
  "limitations": [
    "真实文件符号链接用例因没有显式授权保持跳过，不能算通过。",
    "P2.02 的空文档启动属于票据 04，本票未认领或验证。",
    "外部调用缺席仅证明隔离测试启动至最后定义响应的有界窗口；不宣称原生 Herdr/omp 或模型业务质量。",
    "没有操作集成分支工作区、合入、reset、retro 或派发其他票据。"
  ]
}
