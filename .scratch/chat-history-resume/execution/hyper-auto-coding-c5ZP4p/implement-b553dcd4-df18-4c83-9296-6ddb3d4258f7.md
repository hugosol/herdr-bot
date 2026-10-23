# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "04-preserve-history-workspacemd-9d344fa9-0feb-4b",
  "head": "705302ebd28adfc46cd376e8bfdee6386e3b9523",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "04-preserve-history-workspace",
  "implementation": "完成本票实现并提交，worktree 可交独立集成。未操作集成分支、未修改其他票据或工作流报告。",
  "commit": "705302e",
  "branch": "hyper/chat-history-resume/04-preserve-history-workspace-92bbf1-10144a1f16f7",
  "delivered": [
    "正常发起 Chat 收录已确认的原 Workspace ID；connect-bot register.data.originalWorkspaceId 可选上报已知位置，不推断未知位置。已知原位置不被重复收录覆盖。",
    "原 Workspace 跨重启保留，按稳定 ID 恢复，支持原位置更名；确认删除后才回退唯一配置默认 Workspace，页面与 HTTP 均说明回退。查询失败或无法判定存在性时停止，不创建或启动。",
    "定位其他位置、原位置恢复、删除后回退均不写历史。旧格式无需迁移，可正常读取和 Resume；登记删除不影响历史或原生内容。",
    "已更新 docs/integration-boundaries.md 的兼容与位置不变说明。"
  ],
  "acceptance_evidence": {
    "P7": "history_workspace.rs：connect_history_preserves_original_workspace_across_restart_and_resumes_there、normal_launch_and_connect_keep_literal_identity_set_and_original_location、locating_elsewhere_keeps_original_location_and_native_content；真实临时持久文件、新应用实例与外部创建目标观察。",
    "P8_deleted_original": "deleted_original_falls_back_without_overwriting_history_even_after_restart；historyWorkspace.spec.ts 实际点击交付页面，观察 w-other 原位置及删除后 w-target 默认位置和回退文字。",
    "P9_original_query_failure": "original_workspace_query_failure_never_falls_back_or_starts，覆盖命令失败及缺失 Workspace ID 的响应，外部仅出现 agent_list/workspace_list，无创建或启动。",
    "P21_owned_subset": "old_format_history_remains_readable_and_resumable_without_rewriting；normal_launch_and_connect_keep_literal_identity_set_and_original_location 使用独立字面身份集合 native-full-001/native-full-002，验证同名同 cwd、重复收录、注销及重启；不宣称 P21 整体完成。",
    "red_green": "首次运行 original workspace 用例在公开历史字段断言失败：Null != w-other；实现后通过。其余已满足分支通过针对性验收，未制造人为失败。"
  },
  "verification": [
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test history_workspace --test session_history --test chat_resume_pi --test pi_zone_history -- --nocapture",
      "result": "25 passed, 0 failed"
    },
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test history_workspace --test omp_zone_history --test history_pagination -- --nocapture",
      "result": "18 passed, 0 failed（本票 6 项重复运行）"
    },
    {
      "command": "npm --prefix frontend run build && cargo build --manifest-path backend/Cargo.toml",
      "result": "前端 TypeScript/Vite 与后端构建均通过"
    },
    {
      "command": "npm run test:browser -- historyWorkspace.spec.ts chatResumePi.spec.ts（cwd frontend）",
      "result": "网络策略检查通过；真实 Chrome 页面 2 passed, 0 failed"
    }
  ],
  "screenshots": [
    "frontend/test-results/historyWorkspace-Resume-re-5ba06-s-deleted-original-fallback/original-workspace-restored.png",
    "frontend/test-results/historyWorkspace-Resume-re-5ba06-s-deleted-original-fallback/deleted-original-fallback.png"
  ],
  "integration_notes": [
    "03 将 restore_pi 泛化为 restore_history；合并时保留该泛化，并使用本票 HerdrClient.resume_workspace(selected.original_workspace_id.as_deref()) 返回的 workspace 与 fallback_reason。已通知 03。",
    "docs/integration-boundaries.md 本票原位置说明与 03 的 Pi/OMP 术语扩展需语义合并。",
    "本票验收项全部勾选；按仓库规则，Status 在实际合入前保持 in-progress，合入后由集成方关闭。"
  ],
  "unverified_or_outside_owned_scope": [
    "受控 Herdr 加真实浏览器不替代真实 Pi/OMP 历史正文及用户焦点人工验收 H-Pi/H-OMP。",
    "Pi/OMP 恢复后正常重新接入及重启的组合去重检查由票据 07 唯一负责。"
  ]
}
