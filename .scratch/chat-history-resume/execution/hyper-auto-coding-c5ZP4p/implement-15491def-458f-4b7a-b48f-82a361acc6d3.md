# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "03-resume-omp-chatmd-e7d62d29-f668-4f8b-8e91-acd",
  "head": "97f347862dbb523ace341e92b648d80729a45a7b",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "03-resume-omp-chat",
  "result": "本票生产实现与自动验收完成，已提交；worktree 可交由独立集成代理接管。未操作集成分支工作区、未自行合入、未调用 retro。",
  "commit": "97f3478",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-MN6O1D",
  "delivered": [
    "将 Pi 恢复流程推广为单一 restore_history 流程，OMP 复用先查找、默认 Workspace 解析、创建、原生身份与实际 cwd 确认、最终聚焦及权威结果。",
    "OMP 使用当前配置对应的 agent 类型与固定集成，原生参数为 --resume，不使用 --restore，不提交输入、不重新应用 Bot 模型或思考设置。",
    "优先使用身份一致的可用文件；仅 ID 或文件不可用时明确加载保存的同一原生 ID。错误文件、不可用目录、缺失集成、启动失败、错误／缺失身份、错误实际 cwd 和聚焦失败均不能返回恢复成功。",
    "更新 README 与现有集成边界说明；迁移旧定位测试中已过时的 OMP not-found 预期。"
  ],
  "acceptanceEvidence": [
    {
      "criteria": "P6、P10、P11、P13、P14、P15、P22 的 OMP 自动分项",
      "tests": "backend/tests/chat_resume_omp.rs::omp_file_resumes_in_historical_cwd_without_bot_overrides_or_input_then_focuses",
      "proves": "真实 HTTP 驱动生产 CLI adapter；观察完整阶段、独立原生文件、精确 --resume 参数与固定集成、真实含空格目录及子进程实际 cwd、无输入／Bot 覆盖、确认身份后聚焦。"
    },
    {
      "criteria": "P10",
      "tests": "omp_id_only_and_unavailable_file_resume_only_the_selected_native_id",
      "proves": "仅 ID 与不可用文件两种样例均只加载保存的精确原生 ID。"
    },
    {
      "criteria": "P10、P11、P12",
      "tests": "omp_unusable_sources_or_cwd_fail_without_creating_a_target",
      "proves": "错误会话文件、缺失 cwd 或必要集成明确失败且不创建目标。"
    },
    {
      "criteria": "P6、P11、P12、P22",
      "tests": "omp_requires_same_native_identity_actual_cwd_and_confirmed_focus",
      "proves": "创建／启动失败、错误／缺失身份、错误实际 cwd、失败／未确认聚焦不能冒充成功，不重启副本或提交输入。"
    },
    {
      "criteria": "复用先查找与禁止绕过失败",
      "tests": "omp_existing_target_or_lookup_failure_never_starts_another_process",
      "proves": "已有匹配只定位；查询失败、无效响应与已有目标聚焦失败不启动副本。"
    },
    {
      "criteria": "S1 页面验收及 OMP 自动结果反馈",
      "tests": "frontend/browser-tests/chatResumeOmp.spec.ts",
      "proves": "真实 Chrome 操作交付页面，确认成功与 HTTP 结果一致；错误原生身份和查询失败有可见反馈，无新输入或额外聚焦。"
    }
  ],
  "verification": {
    "red": "cargo test --test chat_resume_omp -- --nocapture：实现前观察 outcome 为 not-found 而非 restored，行为断言失败。",
    "green": [
      "OMP 各垂直切片定向测试均通过。",
      "cargo test --test chat_resume --test chat_resume_pi --test chat_resume_omp：19 passed，0 failed。",
      "cargo build --bins：通过。",
      "npm ci --no-audit --no-fund：完成。",
      "npm run build：通过。",
      "npm run test:browser -- chatResumeOmp.spec.ts：1 passed；网络策略检查通过。"
    ],
    "transientFailure": "曾将浏览器运行与 Cargo 测试并行，Windows 已运行的 herdr-bot.exe 导致 Cargo 拒绝访问；浏览器退出后顺序执行相关 HTTP suites，19 项全部通过。",
    "screenshots": [
      "frontend/test-results/chatResumeOmp-OMP-history--cb1cc-poses-failed-native-loading/omp-resume-confirmed.png",
      "frontend/test-results/chatResumeOmp-OMP-history--cb1cc-poses-failed-native-loading/omp-resume-failed.png"
    ]
  },
  "remaining": [
    "真实 OMP 原生身份、历史内容、焦点、无输入及未自动继续中断任务仍由票据 07 的 H-OMP 准备并交用户确认；本票没有把受控 Herdr 参数记录当作真实原生加载证明。",
    "票据保持 in-progress：按项目规则 closed 要求合入；自动验收清单已勾选，等待独立集成。"
  ],
  "integrationNotes": [
    "04 已提供 commit 705302e；独立集成时保留本票 restore_history(state, selected, kind)，将其 Workspace 查询迁移为 resume_workspace(selected.original_workspace_id.as_deref())，并使用返回的 fallback_reason；不要重新建立 OMP 独立恢复流程。",
    "05 的实例级执行 guard 应继续包裹 post_resume，覆盖 Pi／OMP 两者。"
  ]
}
