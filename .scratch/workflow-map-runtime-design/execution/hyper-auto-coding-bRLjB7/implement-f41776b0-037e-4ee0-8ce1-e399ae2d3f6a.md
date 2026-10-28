# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "10-collect-wait-cancelmd-daa87869-0728-40d7-947d",
  "head": "d8288c4064afb53e594cfa4754053e22024a4778",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "本 worktree 可执行部分已完成并提交；外仓/真实角色验收缺口仍在，不宣称票据全部验收通过",
  "commit": "d8288c4",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-mqti20",
  "delivered": [
    "新增 POST /api/workflow/manager/collect：返回 notification、paused、completed、no-running、timeout 独立原因、当前快照及按 node 汇总的报告标识，不重复报告正文。",
    "保存成功后、Herdr 投递前发布内存通知；无人等待仍保留、连续通知可合并，检查不清除未完成待办；检查后的新报告留给后续检查。",
    "等待由 HTTP 响应体持有，断开释放等待者，不派生脱离请求的后台任务；等待不占文件锁或 Run 操作入口。晚期读取失败中止传输，不伪造正常快照。",
    "增加稳定配置 scheduler.collect_wait_seconds，默认 5400 秒，验证正整数；配置实际控制后端期限。",
    "更新长期运行说明、角色部署/错误处理指引及配置样例。未修改其他票据、工作流报告或共享外仓。"
  ],
  "criteria_evidence": {
    "P25.02": "扩展 report_save_failure_has_no_message_pending_item_or_host_delivery：真实只读文件保存失败后，collect 等完整 1 秒返回 timeout/空待办，无通知或外部投递。",
    "P26.02/P28.03/P31.01": "collect_retains_merges_notifications_before_failed_delivery_and_keeps_pending_work：两份无人等待时保存的报告保留/合并通知，外部投递仍阻塞时先完成 collect；重复检查保留待办；之后新通知唤醒下一次等待；150ms 外部调用观察无补投。",
    "P30.01": "collect_without_active_execution_returns_reason_and_body_free_snapshot、collect_wait_releases_on_pause_and_last_handoff_and_completed_is_immediate，加通知及期限用例分别验证五类原因；ready 不强制唤醒，pause/complete 不被等待锁阻塞。",
    "P32.01": "collect_configured_expiry_and_tcp_disconnect_leave_facts_and_next_notification_intact 核对默认 5400 秒及实际 1 秒到期、不修改三份运行记录。实际调用方 HTTP 长预算部分未验证。",
    "P33.01/P68.05（后端分区）": "上述用例实际读取响应头后关闭三条 TCP，150ms 无记录/外部调用变化，后续请求仍收到新通知；collect_late_read_failure_breaks_transport_without_fake_snapshot_or_state_changes 验证晚期真实文件损坏导致不完整传输、零伪造正文及恢复后的正常新请求。",
    "P60.01": "未通过：独立 toolkit 公共取消能力及实际角色调用不存在于本 worktree，未复制客户端或冒充 S3。",
    "P67.02": "非默认后端期限已实际验证；公共 toolkit 调用方更长 HTTP 预算尚未实现/验证。"
  },
  "verification": [
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch collect_without_active_execution_returns_reason_and_body_free_snapshot -- --exact --nocapture",
      "result": "先观察 404 red，接线后 green。"
    },
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch collect_retains_merges_notifications_before_failed_delivery_and_keeps_pending_work -- --exact --nocapture",
      "result": "先观察 no-running 而非 notification 的行为 red，实现后 green。"
    },
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch collect_configured_expiry_and_tcp_disconnect_leave_facts_and_next_notification_intact -- --exact --nocapture",
      "result": "先观察配置未接线导致 3 秒超时的行为 red；正确实现 green。临时将等待移至脱离连接的后台任务，测试检测旧等待者错误消费新通知：timeout != notification，确实失败；变异已撤销，最终完整回归通过。"
    },
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch collect_ -- --nocapture",
      "result": "6 passed。"
    },
    {
      "command": "node scripts/test-backend.mjs",
      "result": "首次因缺少 frontend 构建产物失败，39 个二进制未派发、doctest 未执行；未将该轮算作通过。"
    },
    {
      "command": "npm --prefix frontend run build",
      "result": "首次缺少 tsc；执行 npm --prefix frontend ci --ignore-scripts --no-audit --no-fund 安装锁定依赖后，构建成功。"
    },
    {
      "command": "node scripts/test-backend.mjs",
      "result": "修复静态资产前提后完整通过：build、全部派发二进制及 doctest 均 passed，0 binaries not dispatched，113.9 秒。"
    }
  ],
  "evidence_paths": {
    "final_backend_summary": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-EzjwaY/summary.json",
    "initial_prerequisite_failure": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-aQMfvI/summary.json",
    "mutation_failure": "artifact://400",
    "durable_guide": "docs/workflow-runtime.md：收取报告、等待与取消"
  },
  "unresolved": [
    "按 Main 明确批准的交付边界，未修改共享 my-agent-toolkit。实际 entrypoints/omp-auto 仅有 codebase-tools.ts、connect-bot.ts，缺少 manager-chat/node-chat。",
    "实际 toolkit 公共 HttpRequestOptions 只有 method/headers/body/timeoutMs/proxy，没有调用方取消信号。因此 P33.01/P60.01 的公共调用取消、工具中断/Chat 结束取消及真实角色证据未完成。",
    "P32.01/P67.02 的实际角色长 HTTP 预算及 P68.05 的角色网络失败/取消无自主重试、错误反馈仍需外仓实现和隔离真实 Herdr/omp 验收；后端 TCP/受控 adapter 证据不替代这些验收。"
  ],
  "integration_notes": [
    "已正常提交，可由独立集成代理接管；未自行合入集成分支。",
    "snapshot 与 reports::summary 新增 include_reports 参数：既有调用 true，collect false，避免序列化后再丢弃报告正文。",
    "Runs 新增 signals 注册表，save_run 和成功 report 保存发布变化；与票11操作串行改动集成时需保留通知发布顺序及 collect 等待不持有操作入口的边界。"
  ]
}
