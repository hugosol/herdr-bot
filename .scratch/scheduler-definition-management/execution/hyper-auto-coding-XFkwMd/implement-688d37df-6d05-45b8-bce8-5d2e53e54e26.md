# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "01-template-browsingmd-6b311f8f-76fe-4ff7-bb2f-5",
  "head": "2ea4f040167e8b5df423465bea091c6e39c42044",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "implemented-and-verified",
  "ticket": ".scratch/scheduler-definition-management/implementation/01-template-browsing.md",
  "commit": "2ea4f04",
  "branch": "hyper/scheduler-definition-management/01-template-browsing-013f48-3a96c9da721e",
  "summary": "已实现同 Tab 共享模板列表与可读预览；node 的“模板明细”统一进入准确模板身份的独立预览，移除旧内联展开。复用生产模板目录读取与诊断，更新全部受影响的浏览器交互及使用说明。实现已提交，可交独立集成代理接管。",
  "acceptance_evidence": [
    {
      "criteria": "P1 本票分区",
      "evidence": "schedulerTemplateBrowsing.spec.ts 核对入口位于刷新旁、同 Tab 列表、不新增浏览器页、真实空目录与返回 Map。"
    },
    {
      "criteria": "P2、P3",
      "evidence": "schedulerTemplates.spec.ts 的 P2 P3 用例核对预先指定的 ID、标题、适用场景、能力说明、命名输入输出及 chat 类型；列表、两个相同引用和不同模板引用使用同一预览；确认无编辑器及旧内联区域。"
    },
    {
      "criteria": "P5",
      "evidence": "schedulerTemplateBrowsing.spec.ts、schedulerTemplates.spec.ts、schedulerTemplateProblems.spec.ts 通过实际页面及生产 HTTP 验证真实空目录、Windows ACL 目录／单文件读取故障、缺失引用、重复 ID、损坏文件、健康对象隔离与恢复。"
    },
    {
      "criteria": "P35",
      "evidence": "恢复用例逐阶段比较 Map／模板完整文件集合与字节；既有安全用例继续验证正常、缺失、损坏、恢复及目录初始化均不改写已有定义、不安装样例、不执行工作。"
    },
    {
      "criteria": "使用说明",
      "evidence": "README.md 与 docs/example/README.md 已更新为共享列表、独立可读预览与显式返回导航；未修改历史 ADR。"
    }
  ],
  "verification": [
    {
      "command": "npm --prefix frontend run test:browser:build -- --workers=4",
      "result": "通过：当前前后端、安全壳构建及网络护栏成功，246 个完整浏览器测试全部通过。"
    },
    {
      "command": "npm --prefix frontend test",
      "result": "通过：27 个文件，205 个测试。"
    },
    {
      "command": "npm --prefix frontend run test:browser:partition",
      "result": "通过：完整 246 = fast 222 + time-contract 24，两分区不相交。"
    },
    {
      "command": "npm --prefix frontend run test:browser -- scheduler --workers=4",
      "result": "通过：86 个 scheduler 测试；此后新增的最后一个恢复用例也已纳入完整 246 个测试验证。"
    },
    {
      "command": "定向 TDD 红绿验证",
      "result": "入口缺失、可读列表／预览缺失、管理页目录读取错误不可见均先观察到行为红灯，再实现并确认绿灯。"
    }
  ],
  "screenshots": [
    "frontend/test-results/schedulerTemplates-P2-P3-l-20757-r-the-exact-shared-identity/template-preview.png",
    "frontend/test-results/schedulerInteraction-P4-P3-d6d34-emplate-details-and-returns/narrow-template-details.png"
  ],
  "limitations": [
    "本票无未完成的自有验收项。P1 的空列表新增流程属于 02，未声明完整 P1 已交付。",
    "模板编辑后的最新内容往返属于 02；P36 主观体验验收不由本票声明通过。",
    "未单独运行完整后端回归：后端生产代码未修改，完整浏览器入口已构建并实际运行真实后端。",
    "依项目状态规则，票据保留 in-progress，待独立集成代理合并后关闭；本次未操作集成分支工作区、未自行合入、未运行 retro。"
  ]
}
