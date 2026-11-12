# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-01-shared-report-persistencemd-4503bc8",
  "heads": {
    "herdr-bot": "ce57169d9427a985a158be3b64a3676a67b96df2"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "completed",
  "ticket": "01-shared-report-persistence.md",
  "repository": "herdr-bot",
  "branch": "hyper/chief-bot/01-shared-report-persistence-c0b421-91332fa5e5a1",
  "head": "ce57169d9427a985a158be3b64a3676a67b96df2",
  "commits": [
    "8f9c7e5 fix: clear shared report integration regression gates",
    "ce57169 fix: restore global browser acceptance contracts"
  ],
  "synchronization": {
    "integration_commit": "2722c8ecca6b2f8ff761df2ee8958634196fe2a5",
    "ancestor_check_exit": 0,
    "remaining_conflicts": false,
    "worktree_clean": true
  },
  "changes": [
    "修复两个完整后端回归的过时 input_files 全对象断言，保留数量、顺序、身份、必需性及公开保存/预检行为。",
    "修复隐藏 Run 列表提前请求目录、应用标题可访问语义、重复选择相同 Task 导致预览失效、Chat 历史背景区别。",
    "更新全浏览器回归中过时的分组目录、Map 默认字段、原生报告确认、输入就绪及窄屏导航夹具/断言；更新既有测试说明。"
  ],
  "verification": [
    {
      "command": "npm --prefix frontend run build",
      "exit": 0,
      "outcome": "本 worktree 静态页面构建成功；保留大 chunk 警告。"
    },
    {
      "command": "CARGO_TARGET_DIR='C:/Users/hugos/AppData/Local/Temp/hyper-ticket-avGLz7/backend/target' node scripts/test-backend.mjs",
      "exit": 0,
      "outcome": "全后端构建、全部测试二进制及 doctest 通过，0 未派发；包含节点报告与嵌套回归。",
      "evidence": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-FwSYy7/summary.json"
    },
    {
      "command": "npm --prefix frontend test",
      "attempt": 1,
      "exit": 1,
      "outcome": "30 失败；原始失败保留 artifact://106。"
    },
    {
      "command": "npm --prefix frontend test",
      "attempt": 2,
      "exit": 0,
      "outcome": "34 文件、253 用例通过。"
    },
    {
      "command": "CARGO_TARGET_DIR='C:/Users/hugos/AppData/Local/Temp/hyper-ticket-avGLz7/backend/target' npm --prefix frontend run test:browser:build",
      "attempt": 1,
      "exit": 1,
      "outcome": "25 失败、327 通过、1 跳过；原始失败 artifact://120。"
    },
    {
      "command": "npm --prefix frontend test",
      "attempt": 3,
      "exit": 0,
      "outcome": "最终状态34 文件、253 用例通过；jsdom scrollTo 警告保留。"
    },
    {
      "command": "CARGO_TARGET_DIR='C:/Users/hugos/AppData/Local/Temp/hyper-ticket-avGLz7/backend/target' npm --prefix frontend run test:browser:build",
      "attempt": 2,
      "exit": 0,
      "outcome": "最终状态构建通过，352 通过、1 既有文件符号链接用例跳过，无失败。",
      "evidence": "artifact://140；artifact://152"
    },
    {
      "command": "git status --short --branch && git rev-parse HEAD && git merge-base --is-ancestor 2722c8ecca6b2f8ff761df2ee8958634196fe2a5 HEAD",
      "exit": 0,
      "outcome": "当前原开发分支，工作区干净，集成提交已同步。"
    }
  ],
  "scope": {
    "repositories": [
      "herdr-bot"
    ],
    "cross_repository_acceptance": "本票未要求独立跨仓专项；P21完整交付归14，P36/P37两宿主原生验收归16，不外推为已通过。",
    "real_model_calls": 0,
    "integration_workspace_operations": 0,
    "retro_calls": 0
  },
  "identities": {
    "node": "v24.18.0",
    "bun": "1.4.4",
    "cargo": "1.98.1",
    "sha256": {
      "backend/Cargo.lock": "ecf45bd9047d8772dd8c3abddee0f503e843f91230cc12a00725f593246bb36f",
      "frontend/bun.lock": "1712c4c2006b8de357902c6c6828e5237977676d1c5bccd0a0939bca5e77581c",
      "backend/src/report_delivery.rs": "8b6f71b96dc5a525c5084291089f84a3580239b34eec6b0c4f739ee34ea9c8cb",
      "backend/src/runs/reports.rs": "d2633e8cd78af8c2566abfec3e9e1430c832950e63dc723965e9b608c77981f3",
      "backend/static/index.html": "260f2eecb0d3038fd9eb84587aff9fbb3030b05c6f8e0daee3815e869aacc09b",
      "backend/target/debug/herdr-bot.exe": "1e38531c13c6f537db8a731ab004a19bd4dbf68871a2b7b542cc96769c8cec38",
      "backend/target/debug/herdr-fixture.exe": "9f9701dd1a99924cb790a202ec0e0892c386a2b2e941b7162f7ea25c619d3e46",
      "backend/target/debug/examples/safe_shutdown_shell.exe": "863192e1748a317841910c912f7e8c61042894d344540256ef39fdc8624f0860"
    }
  },
  "blockers": []
}
