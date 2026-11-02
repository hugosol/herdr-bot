# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "01-map-named-inputsmd-059c7bf6-48b5-415b-b9d6-e4",
  "heads": {
    "herdr-bot": "e78e686a450a648e348589110ee55430fcd0cfa6"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "01-map-named-inputs",
  "result": "implemented-and-verified",
  "commit": "e78e686",
  "handoff": "本票实现已提交；worktree 可交由独立集成代理接管，未操作 dev 或其他票据／工作流报告。",
  "delivered": [
    "Map node 可选 inputs：按逻辑名声明 {source:\"file\",path} 或 {source:\"node\",node_id,output}；严格拒绝来源混用、缺字段和错类型。",
    "直接输入仅校验绝对路径表达，不检查文件存在、内容、授权或 zone 范围；未知来源 node 拒绝保存，失败保留原定义。",
    "输入引用不改变 blocked_by、不隐式启动；来源 node 被其他输入引用时拒绝删除，避免制造悬空定义。",
    "实际 dispatch 初始化保留原业务 input，并分别提供本次 userInput、直接文件地址及 manager-zone/.workflow/<Run ID> 中间资料位置；不复制背景、正文或上游聊天，不创建输出或每 node 目录。",
    "同步更新 scheduler/workflow 消费方、manager 初始化预期和三份长期协议文档；沿用现有 JSON 编辑与显式派发动作。"
  ],
  "criteria_evidence": [
    {
      "criteria": "P1/P2/P3：两类来源往返、跨卷／zone 外地址、独立依赖、保存读取不启动",
      "tests": [
        "named_inputs::named_inputs_round_trip_without_dependencies_or_implicit_launch"
      ]
    },
    {
      "criteria": "P1/P2/P3：非法来源、字段错类型、相对地址、未知 node 拒绝且原定义不变",
      "tests": [
        "named_inputs::invalid_named_inputs_preserve_original_map"
      ]
    },
    {
      "criteria": "P1：独立存储故障不替换原定义",
      "tests": [
        "named_inputs::named_input_storage_failure_keeps_original_definition"
      ],
      "evidence": "Windows 只读目标阻止真实发布，接口返回 node-write-failed；解除故障后 HTTP 读取仍是原定义。"
    },
    {
      "criteria": "P1：无文件输入 Map 可保存并显式派发，无强制文件生成",
      "tests": [
        "named_inputs::legacy_node_without_file_inputs_saves_and_dispatches"
      ]
    },
    {
      "criteria": "P3：编辑消费者维持来源引用有效",
      "tests": [
        "named_inputs::deleting_a_named_input_source_is_rejected_without_cascading"
      ]
    },
    {
      "criteria": "P4/P24：实际宿主初始化包含独立背景／业务 input／地址；排除敏感正文和聊天；不同 Run 的 manager-zone 资料位置可区分且不依赖 node zone",
      "tests": [
        "named_inputs::dispatch_receives_background_direct_addresses_and_run_scoped_material_location",
        "manager_trigger_and_status_follow_committed_identity"
      ],
      "evidence": "隔离真实 HTTP 保存、创建两个 Run、显式派发；外部 Herdr 记录型进程捕获实际 agent_prompt，并记录请求、结果及初始化文本。"
    }
  ],
  "verification": [
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch --test workflow_start -- --test-threads 2",
      "result": "105 passed，2 suites。"
    },
    {
      "command": "node scripts/test-backend.mjs --binary-jobs 2 --test-threads 2",
      "result": "完整后端通过：构建、全部发现的测试二进制及 doctest 成功；0 未派发。",
      "evidence": "C:\\Users\\hugos\\AppData\\Local\\Temp\\herdr-backend-tests-nSSwT3\\summary.json"
    },
    {
      "command": "npm --prefix frontend run build",
      "result": "通过 TypeScript 与 Vite 构建；首次因未安装依赖失败，npm ci 后通过。"
    },
    {
      "command": "npm --prefix frontend test -- tests/mapGraph.test.tsx tests/runDetails.test.tsx tests/runLaunchForm.test.tsx",
      "result": "3 files，6 tests passed；网络护栏通过。"
    },
    {
      "command": "node scripts/check-workflow-docs.mjs",
      "result": "17 documents，158 local links，0 errors。"
    }
  ],
  "tdd_evidence": "先观察 inputs 未知字段、未知来源 node 被错误接受、缺少 Run 背景、来源节点可错误删除等行为性红灯，再实现并转绿。已有存储故障保护与无输入行为直接验证，没有制造失败。",
  "earlier_failures": "早期默认线程定向运行有两个既有 5 秒夹具等待失败；限制测试线程后的定向与完整回归均通过。首次完整回归还发现 manager 初始化字面预期缺少新资料位置，已同步修正并完整复验通过。",
  "scope_boundaries": [
    "具名上游输出的运行解析属于 03；本票不猜测输出地址。",
    "真实 Chat 中的初始化与角色注入由 07 验收；本票证据是生产后端提交给受控外部宿主的实际输入，不冒充真实模型运行。",
    "票据状态按项目规则须实现、测试且合并后才可 closed；本票未改动票据状态。"
  ],
  "unresolved_ticket_items": []
}
