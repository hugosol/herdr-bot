# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "15-file-integrity-coordinationmd-72a6116b-5122-4",
  "head": "33a6f43c35034074d3e847363b897a0b654e0b47",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "15-file-integrity-coordination",
  "result": "实现与本票确定性验收完成，已提交，worktree 可交独立集成代理接管；未操作或合入集成分支。",
  "commit": "33a6f43",
  "branch": "hyper/workflow-map-runtime-design/15-file-integrity-coordination-d1211f-9156471a482f",
  "changes": [
    "统一运行记录的严格 JSON/JSONL 读取与追加：缺失、损坏、残尾错误包含文件路径，不重建、不截断；dispatch/retry 历史追加也检查已有记录。",
    "Run 详情检查历史损坏，不通过历史重放当前事实；无法读取运行身份记录时如实返回存储错误，不伪装成身份未关联。",
    "复用既有固定顺序集合锁和同目录替换，迁移报告及状态历史追加调用；保留部分提交反馈和独立 Run 推进。",
    "新增 7 项真实 HTTP/文件契约检查，并更新 docs/workflow-runtime.md 的故障处置、角色指引、保证范围及重复验证入口。"
  ],
  "acceptance": {
    "P47.01": {
      "result": "通过",
      "evidence": [
        "integrity_missing_and_corrupt_required_records_are_never_recreated：三类必要文件缺失、当前状态不可解析/缺字段、报告损坏/残尾，核对错误与原字节。",
        "integrity_corrupt_history_is_rejected_without_changing_any_record：历史非法 JSON、非对象记录及残尾，详情明确拒绝。",
        "integrity_dispatch_never_appends_to_damaged_history_after_external_wait：外部等待中损坏历史，拒绝追加并保留已提交状态/已发生 Chat 效果。",
        "integrity_message_and_transition_appends_preserve_damaged_logs：报告与状态转换追加拒绝损坏记录，不投递未保存报告。"
      ]
    },
    "P48.01": {
      "result": "通过目标 Windows 文件与受控外部等待分区",
      "evidence": [
        "integrity_windows_replace_failure_keeps_formal_record_and_next_replace_succeeds：真实 Windows 文件句柄不共享 DELETE，替换失败后正式路径及旧句柄字节保持；释放句柄后显式替换成功。",
        "integrity_concurrent_reads_observe_complete_records_during_replacements：80 次详情读取与 40 次替换并发，记录完整、历史准确。",
        "integrity_external_start_keeps_same_run_readable_and_disjoint_run_writable：实际外部启动 gate 期间同 Run 可读、另一 Run 可提交 pause，保留 100ms 有界负向观察及外部调用记录。",
        "完整 workflow_dispatch 回归同时执行既有完整批次并发、延迟报告投递、collect 等待期间读取/状态更新检查。"
      ]
    }
  },
  "verification": [
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch <定向测试过滤> -- --nocapture",
      "result": "先观察到历史损坏返回成功、dispatch 向残尾继续追加、文件错误缺少路径、运行身份扫描吞掉文件错误等真实 red；分别修复后 green。"
    },
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture",
      "result": "60 passed，0 failed；输出 artifact://707。"
    },
    {
      "command": "npm --prefix frontend ci && npm --prefix frontend run build",
      "result": "成功。"
    },
    {
      "command": "node scripts/test-backend.mjs --binary-jobs 2",
      "result": "完整后端回归通过，build、全部测试二进制及 doctest 通过，0 binaries not dispatched，用时 256.8s。",
      "logs": "C:\\Users\\hugos\\AppData\\Local\\Temp\\herdr-backend-tests-OePTI0",
      "summary": "C:\\Users\\hugos\\AppData\\Local\\Temp\\herdr-backend-tests-OePTI0\\summary.json"
    }
  ],
  "limitations": [
    "外部等待用隔离记录型 Herdr 副本提供确定性时序；不宣称原生 Herdr/omp、模型行为或角色 Prompt 实际注入已验证。",
    "未新增或承诺跨文件事务、断电安全、多进程写入保证。",
    "未修改其他票据或工作流报告；本票尚未合入，因此未将票据状态标为 closed。"
  ],
  "unresolved_owned_criteria": []
}
