# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "03-stepped-template-editormd-033e99d7-841b-4b78-",
  "head": "fc2361340a0fd40ea375df3a31d0b7893eaa66ec",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "result": "本票实现及所属自动验收全部完成，已提交，可交由独立集成代理接管。未操作或合入集成分支。",
  "commit": "fc23613",
  "branch": "hyper/bot-template-extensions/03-stepped-template-editor-8a299b-8713386ed6b5",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-MQZFDe",
  "delivered": [
    "生产页面新增与编辑均采用“模板预设 → 扩展配置 → 确认保存”；往返保留完整草稿，导航不保存。",
    "保留所有既有字段编辑、四类命名字段插入及混合固定文字预览；确认摘要逐项显示全部字段、固定项、可选项及自动发现关闭。",
    "按当前 Agent 元数据提供扩展复选框；model-cost 独立显示且不可取消；切换 Agent 重置 model/thinking 并清空扩展。",
    "公开保存校验当前扩展适用性，拒绝跨 Agent、固定项、未知项和重复选择；模板仅持久保存 extensions 稳定标识数组。",
    "重新打开编辑读取当前模板及元数据；真实磁盘写入失败明确反馈且不破坏旧模板。README 与现有测试消费者已同步。"
  ],
  "acceptance_evidence": [
    {
      "criteria": "P1–P6、P12、P14、P16 展示、P18",
      "test": "frontend/browser-tests/steppedTemplates.spec.ts：三步新增和编辑保留完整草稿，摘要与公开持久结果一致，删除不关闭既有 Bot",
      "proves": "真实页面完整生命周期、字段与多行草稿、四类命名插入和独立预览、逐字段摘要、导航后独立读取不变；删除前已创建 Bot，随后外部调用和既有对象保持不变。"
    },
    {
      "criteria": "P14、P17",
      "test": "同文件：切换 Agent 清空扩展且切回不恢复，新模板零选明确保存为空",
      "proves": "OMP 两项 → Pi → OMP 后无隐藏选择恢复，model/thinking 为默认，零选保存读回为空。"
    },
    {
      "criteria": "P12–P15、P16 保存边界、P19、P25 稳定标识持久化",
      "test": "同文件：公开保存六种扩展组合并在独立客户端和真实重启后保留稳定标识",
      "proves": "公开选项符合独立字面集合；OMP 四组合、Pi 两组合均保存；非法选择拒绝；持久文件无映射副本；真实重启后逐标识读取一致。"
    },
    {
      "criteria": "P25 扩展展示与保存校验、P26 扩展重新编辑与保存",
      "test": "同文件：扩展元数据在同一进程重新编辑和保存时生效，只持久保存稳定选择",
      "proves": "同进程修改标签和入口后重新打开显示新标签；旧页面保存遇到最新适用性变化被拒绝，修复后保存成功。"
    },
    {
      "criteria": "P4、P19、P20",
      "test": "同文件：真实磁盘写入故障明确失败且重读和重启保留旧模板与扩展",
      "proves": "真实目录阻断临时文件写入，页面无成功反馈，草稿保留；独立读取与真实重启后旧模板及扩展选择不变。"
    }
  ],
  "verification": [
    {
      "command": "backend: cargo build --bins",
      "result": "通过"
    },
    {
      "command": "backend: cargo test --test bot_templates --test dynamic_catalog --lib",
      "result": "81 通过"
    },
    {
      "command": "frontend: npm run build",
      "result": "通过"
    },
    {
      "command": "frontend: npm test",
      "result": "26 个文件，184 个测试通过"
    },
    {
      "command": "frontend: npm run test:browser -- steppedTemplates.spec.ts botTemplates.spec.ts dynamicCatalog.spec.ts mobileWorkflow.spec.ts lanAccess.spec.ts",
      "result": "13 个真实浏览器测试通过"
    }
  ],
  "tdd": "先观察公开 extensions 保存接口缺失导致 400、步骤界面缺失导致失败，再完成实现并通过对应用例。其余已满足的标准通过真实场景补充验收。",
  "remaining": "本票范围无未完成项。真实 Agent 扩展加载及 P16/P25/P26 启动覆盖由 04 负责；完整失效修正与尺寸/真机专项不属于本票，未据此声称已通过。",
  "ticket_status": "已勾选全部验收条件并记录证据；依项目规则，独立集成合入前保留 in-progress，合入后方可 closed。"
}
