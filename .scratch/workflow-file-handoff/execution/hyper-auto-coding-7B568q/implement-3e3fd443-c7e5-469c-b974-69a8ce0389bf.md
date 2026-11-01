# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "03-current-execution-input-resolutionmd-2f6646c7",
  "heads": {
    "herdr-bot": "773892b91790c5c2a21d5b09cb324024e43c25b4"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "implemented-tested-committed; awaiting independent integration",
  "commit": "773892b",
  "commitMessage": "Resolve workflow file inputs from current execution reports",
  "implementation": [
    "派发初始化直接从本 Run 的持久报告按来源 node、当前 execution 和输出名解析地址；按名保留省略项，不回退旧 execution，不解析正文或扫描目录。",
    "缺失输出显示逻辑输入名及来源，仍沿用既有依赖和显式派发规则；保留 userInput 和业务 input。",
    "沿用 messages.jsonl 唯一权威记录，无新增 registry、查询工具、输入模板语言或脚本执行器。"
  ],
  "changedFiles": [
    "backend/src/runs/startup.rs",
    "backend/tests/workflow_dispatch.rs",
    "backend/tests/workflow_dispatch/input_resolution.rs",
    "docs/workflow-runtime.md"
  ],
  "acceptanceEvidence": [
    {
      "criteria": "AC1–3；P5/P9/P11",
      "test": "input_resolution::saved_named_inputs_resolve_per_run_and_accumulate_report_deltas",
      "proves": "公开 HTTP 保存 Map、创建两个 Run、重试来源，交错提交当前/旧执行和其他 node 的同名输出及误导正文；实际初始化严格匹配独立地址预期。A=a1/B=b1 → A=a2 → 省略 outputs 得到 A=a2/B=b1，并与报告接口及持久记录一致。"
    },
    {
      "criteria": "AC3–5；P4/P6/P9",
      "test": "input_resolution::retry_never_falls_back_and_missing_outputs_do_not_block_explicit_dispatch",
      "proves": "重试前后实际地址不同；旧执行迟到报告不能补全当前缺失名字。依赖满足后下游仍创建并初始化，明确 unresolved，不改变 blocked_by 或撤销上游完成；保留背景和业务 input，无隐式下游启动。"
    },
    {
      "criteria": "AC4/6；P6/P11/P13",
      "test": "input_resolution::downstream_resolves_saved_outputs_while_notification_is_pending_and_then_fails",
      "proves": "真实通知被外部 gate 阻塞时，已保存输出已可用于完成来源并显式初始化下游；之后通知失败不撤销登记。初始化、报告响应、详情与持久报告一致；150ms 窗口无自主重投或派发。"
    },
    {
      "criteria": "AC5/7/8；P4/P23/P24",
      "test": "input_resolution::shared_consumers_and_business_summary_receive_original_external_addresses_only",
      "proves": "公开 Map 定义驱动共享地址及业务汇总输入；两个消费者得到相同原地址，汇总节点另获业务 node 登记的地址。manager/node/产物目录分离，独立正文、原生上游聊天及二进制文件敏感标记不进入初始化；文件不移动/复制，缺失汇总文件不由框架生成，仅保留三份业务记录。"
    },
    {
      "criteria": "AC9",
      "evidence": "更新实际初始化消费方 startup.rs、workflow_dispatch 行为测试和 docs/workflow-runtime.md 长期解析文档。"
    }
  ],
  "verification": [
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch input_resolution::saved_named_inputs_resolve_per_run_and_accumulate_report_deltas -- --exact --nocapture",
      "result": "TDD：修正测试请求字段后，观察到实际初始化为 unresolved 而非预期登记地址的行为红；实现后通过。"
    },
    {
      "command": "新增其余三个 input_resolution 测试的逐项定向运行",
      "result": "全部通过。"
    },
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_start -- --nocapture",
      "result": "11 passed。"
    },
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture",
      "result": "106 passed，包含最终增强的双 Run/重试/迟到报告组合。"
    },
    {
      "command": "node scripts/test-backend.mjs",
      "result": "首次因 worktree 缺少 backend/static/index.html 失败；执行 npm --prefix frontend ci 和 npm --prefix frontend run build 补齐既定前提后再次执行，全量通过，build/binaries/doctest 均 passed，0 binaries not dispatched。"
    }
  ],
  "evidenceLocations": {
    "behavioralRed": "artifact://206",
    "actualInitializationAndIndependentAddressComparisons": "artifact://219",
    "startupVerification": "artifact://217",
    "fullBackendSummary": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-qXtW3D/summary.json",
    "initialMissingStaticAssetsFailure": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-b2YNkn/summary.json"
  },
  "limitations": [
    "本票受控外部宿主证据只证明实际初始化参数；未宣称真实原生 Chat 收到地址（07）或真实业务内容消费/汇总字节证明（08）通过。",
    "尚未合入 dev；未改票据状态或工作流报告，closed 应待独立集成后决定。"
  ],
  "handoff": "实现已提交，worktree 交由独立集成代理接管；本票无剩余实现事项。"
}
