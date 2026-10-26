# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "02-template-editingmd-2b275cac-5b96-495b-89e6-a6",
  "head": "c65015fb4801aed3be32d76408097817cc4b6569",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "implemented_and_committed; ready_for_independent_integration",
  "ticket": ".scratch/scheduler-definition-management/implementation/02-template-editing.md",
  "commit": "c65015f",
  "branch": "hyper/scheduler-definition-management/02-template-editing-bf83b0-3a96c9da721e",
  "delivered": [
    "同 Tab 新增／修改共享模板：独立 ID、只读已有身份、不含顶层 id 的真实 JSON 草稿、保存后直接进入最新预览。",
    "天气草稿使用文档 JSON 的编译期打包内容作为单一来源；运行时无需读取开发仓库文档，打开／放弃不安装模板或依赖。",
    "生产 HTTP 保存完成严格校验、全局身份冲突检查、并发创建协调、安全文件名、原文件位置保留及旧版本检查；真实文件写入失败保留草稿。",
    "返回、切换 tab／server、应用刷新及浏览器刷新保护已修改草稿；返回来源时重新读取 Map，保留有效 node 选择，诊断已移除来源。",
    "更新 README、样例管理说明及 scheduler 术语描述；未修改其他票据或工作流报告。"
  ],
  "acceptance_evidence": [
    {
      "criteria": "P1（空态新增）、P6、P8、P9、P11、P28",
      "test": "schedulerTemplateEditing.spec.ts — empty template list creates an independent-ID editable bundled draft and persists only on save",
      "evidence": "独立天气业务字面量、空 ID、草稿放弃、模板与 Map 文件清单、路径语义 ID、哨兵文件及真实保存重读。"
    },
    {
      "criteria": "P6、P10、P12、P28",
      "test": "editing loads only real content, keeps immutable identity and original filename, and previews the saved result",
      "evidence": "非天气真实内容、只读 ID、原始文件名与身份关联、生产 HTTP 重读及即时最新预览。"
    },
    {
      "criteria": "P7、P14、P17（校验失败）",
      "test": "invalid template content cannot bypass validation and keeps the original ID and raw draft",
      "evidence": "页面及生产创建／修改请求覆盖语法、顶层 id、版本、各必需字段、错误类型、未知字段／实现类别；原文件字节不变，合法空文本与空对象可保存。"
    },
    {
      "criteria": "P13、P17（身份冲突）",
      "test": "global duplicate and concurrent same-ID creation reject without overwriting definitions or drafts",
      "evidence": "真实并发创建只有一个成功；重复及歧义身份不能覆盖定义，页面保留原始草稿。"
    },
    {
      "criteria": "P18、P17（旧版本冲突）",
      "test": "two clients and external modification or removal reject stale template drafts and require reload",
      "evidence": "两个真实页面及外部修改／移除分别使旧提交失败；较新文件不被覆盖，ID 与原始草稿保留并提示重新加载。"
    },
    {
      "criteria": "P17（受控写入失败）",
      "test": "a controlled filesystem write failure preserves the ID and raw JSON until retry succeeds",
      "evidence": "真实 Windows 目录 ACL 拒绝文件创建，页面与生产请求均失败且不改文件；恢复权限后可保存。"
    },
    {
      "criteria": "P19",
      "test": "modified template drafts confirm return, tab and server navigation, app refresh and browser refresh",
      "evidence": "各退出动作分别取消和确认；取消保留草稿，确认执行原动作；原生刷新只核对 beforeunload 行为。"
    },
    {
      "criteria": "P4、P30",
      "test": "saving a shared capability preserves live source selection and updates all references, while removed origins are invalidated",
      "evidence": "同 Map 两个 node 与跨 Map 引用均读取新版能力，Map 文件不变、不生成模板副本；有效选择保留，删除来源 node／Map 后明确失效。"
    }
  ],
  "verification": [
    {
      "command": "npm --prefix frontend run test:browser -- schedulerTemplateEditing.spec.ts --workers=1",
      "result": "最终 8/8 通过；包含加强后的已有文件校验保护与 Map 依赖文件清单断言。"
    },
    {
      "command": "npm --prefix frontend test",
      "result": "27 个测试文件、205 个测试全部通过。"
    },
    {
      "command": "npm --prefix frontend run test:browser:build -- --workers=2",
      "result": "前后端构建及网络护栏通过；全量浏览器 253 通过、1 失败。本票 8 项全部通过。失败为 botExtensionLaunch.spec.ts 启动服务时回环端口 53877 已被占用（Windows 10048）。"
    },
    {
      "command": "node scripts/test-backend.mjs",
      "result": "构建通过；后端回归失败于未改动的 zone_sync::configured_timeout_stops_the_running_command_and_keeps_effective_pull_state：预期 200，实际 git-write-in-progress 409。该目标 17 通过、1 失败；入口因此未执行 doctest。"
    },
    {
      "command": "TDD 定向 red → green",
      "result": "观察到缺失“新增模板”入口的 red；严格校验测试分别暴露缺失字段路径诊断及错误接受 implementation 位置数组，修复后均转绿。原生取消刷新测试改用浏览器重载命令，避免等待被用户取消的 load 事件。"
    }
  ],
  "integration_notes": [
    "后续 scheduler 写入应复用 backend/src/scheduler.rs 的 pub(crate) static DEFINITION_COMMIT，锁覆盖最终检查至实际提交。",
    "模板生产接口：GET /api/scheduler/templates/new；POST /edit {id}；POST /save {id,content:原始JSON字符串,revision:null或读取版本}。",
    "本票验收复选项已完成；按仓库规则，Status 保留 in-progress，独立集成合入后再标 closed。未操作集成分支工作区或自行合入。"
  ],
  "unresolved": [
    "本票功能及所属自动验收无未完成项。",
    "全项目回归不能报告全绿：上述浏览器端口占用、zone_sync 回归失败及未运行 doctest 已如实保留，未越出票据修改无关代码。",
    "尚未执行独立集成及合并；交出当前 worktree 供集成子代理接管。"
  ]
}
