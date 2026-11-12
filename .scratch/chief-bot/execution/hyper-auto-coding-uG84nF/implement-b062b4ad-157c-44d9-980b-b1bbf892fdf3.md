# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "04-exclusive-chief-authoritymd-b50e2e34-babe-4af",
  "heads": {
    "herdr-bot": "8af7506d345e82beb0774a717f25172717fce86f"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "implemented_ready_for_independent_integration",
  "commit": "8af7506",
  "repository": "herdr-bot",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-fv9Ss8",
  "delivered": [
    "持久化、后端级唯一 chief 占位；既有创建、恢复和登记入口共同约束，不按 zone/Task 分区。",
    "确认原生身份后绑定；登记离线保留期间继续占位，注销或扫描移除后失权；不强制终止旧进程。",
    "明确启动拒绝释放未绑定占位；外部效果不确定时保留占位，跨后端重启禁止再次启动。",
    "提供当前资格判断及显式 claim 注销；更新现有使用说明和测试指引。"
  ],
  "public_interface": {
    "GET /api/chief": "{current:null|{claimId,creationId?,sessionId?,phase}}；phase 为 starting/resuming/awaiting-registration/registered/uncertain。未绑定或 uncertain 均继续占位。",
    "POST /api/chief/qualify": "输入 {sessionId}；仅当前 registered 身份返回 {current:{...}}，否则 409 chief-not-current。消费者须在实际授权点重新判断。",
    "POST /api/chief/release": "输入当前 {claimId}；返回 {current:null,released:{...}}，保留原不确定事实；陈旧 claim 返回 409。不会终止进程或删除原生 Chat。",
    "existing_operations": "POST /api/chats、/api/chats/resume、/api/sessions/events 共同执行资格约束；竞争返回 409 chief-occupied，不排队。后端重启清空登记但保留启动占位、可信来源与撤销事实；被替代身份不会因重启复权。"
  },
  "acceptance_evidence": {
    "P32": "p32_real_restart_serializes_live_registration_and_resume_before_external_completion：真实隔离后端终止与重启；外部查询门闩期间竞争登记/恢复；唯一权威与明确拒绝；再次重启验证旧身份不复权。附带受控 t=11/t=41 秒生命周期、恢复新进程及 qualify 正负例。",
    "P33": "p33_definite_start_failure_frees_slot_but_uncertain_external_start_survives_restart：明确启动拒绝后合法重试；readiness timeout 保留 uncertain，真实重启后无第二次外部启动；陈旧注销拒绝、明确解除当前 claim 后可再次发起。",
    "owned_test_result": "2 passed；最终完整后端运行中的 binary-015.stdout.log 同样通过。",
    "limits": "外部 Herdr 为受控 CLI，证明 S2 协议与故障处理，不证明真实 OMP/Pi 加载或模型职责；未进行模型调用。P29/P30 浏览器全验收与 P31 原生交接仍分别归 07、20。"
  },
  "verification": [
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test chief_authority -- --nocapture",
      "exit": 0,
      "result": "2 passed；进程启动/退出及 cleanup=[] 见 artifact://222。"
    },
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test chat_resume_environment unreadable_task_storage_or_catalog_fails_without_start_or_repair -- --nocapture",
      "exit": 0,
      "result": "1 passed；修复新增角色解析提前读取导致的既有 Resume 错误 envelope 回归，原始失败保留。"
    },
    {
      "command": "node scripts/test-backend.mjs",
      "exit": 1,
      "result": "最终运行 chief_authority、Resume 与生命周期等通过；workflow_dispatch 中 create_bot_wrapper/revise_bot_guides 两例因 input description 预期差异失败。原始日志：C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-VqotGy；初次回归日志：herdr-backend-tests-biwNeD。"
    },
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test zone_browse --test zone_commit --test zone_create --test zone_diff --test zone_git_status --test zone_integration --test zone_portability --test zone_reader --test zone_registry --test zone_sync --test zone_watch",
      "exit": 0,
      "result": "补跑完整入口因失败未派发的 11 targets：86 passed，artifact://244。"
    },
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --doc",
      "exit": 0,
      "result": "0 doctests。"
    },
    {
      "command": "npm --prefix frontend run test:browser:build -- --workers=4",
      "exit": 1,
      "result": "构建通过；327 passed、25 failed、1 skipped，原始输出 artifact://242，错误上下文在 frontend/test-results。涉及既有 scheduler 分组/文案/字段、mobileWorkflow 预览、节点策略等消费者差异，未为全量变绿修改范围外预期。"
    },
    {
      "command": "bun install --cwd frontend --frozen-lockfile --linker isolated --backend copyfile --force（独占 BUN_INSTALL_CACHE_DIR）",
      "exit": 0,
      "result": "修复本地依赖缺失 picocolors 后执行完整浏览器检查；原始构建失败保留 artifact://234。"
    }
  ],
  "identities": {
    "source": "8af7506；独立绝对 CARGO_TARGET_DIR 为本 worktree/backend/target，静态页由本 worktree 构建。",
    "rustc": "1.98.1 (48a229cea 2026-09-01)",
    "node": "v24.18.0",
    "bun": "1.4.4",
    "sha256": {
      "backend/Cargo.lock": "ecf45bd9047d8772dd8c3abddee0f503e843f91230cc12a00725f593246bb36f",
      "frontend/bun.lock": "1712c4c2006b8de357902c6c6828e5237977676d1c5bccd0a0939bca5e77581c",
      "herdr-bot.exe": "a47b8cc96f0e8fa91da531c37c789f3f06f8d7584bf6663fdde8befdeb67ef3d",
      "herdr-fixture.exe": "bf923abc37ea75193d0a71f4c95cad1edb3e380f9bb03768349e623e321431a4",
      "safe_shutdown_shell.exe": "ee4e16d1d41219a12a9e97b862ec5d389d32f020b58ddac75b1563696e515785",
      "backend/static/index.html": "7b1afc39c50e3ded7b62fc8fccc6c47aac2b613ec50ce75c5116e2e7f8388355"
    }
  },
  "handoff": "实现已提交，所属进程已结束，无后台检查继续运行。按 Main 决定保留全量失败证据，交自动集成代理处理全局检查及必要修复；未合入 dev，未修改其他票据或工作流报告，不将本次 invocation 视为票据已 merged/closed。"
}
