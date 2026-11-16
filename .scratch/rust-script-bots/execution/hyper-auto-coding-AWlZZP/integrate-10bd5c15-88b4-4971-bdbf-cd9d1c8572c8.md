# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-01-execution-and-material-contractsmd-",
  "heads": {
    "herdr-bot": "975ca450b33f8b4ac9a6230135b633bc81794953"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "result": "本票原 worktree 集成验收通过；无遗留合并冲突，已同步首次集成提交。修复已提交到原开发分支，未操作集成工作区或直接合入 dev。",
  "repository": "herdr-bot",
  "branch": "hyper/rust-script-bots/01-execution-and-material-contra-ae5e46-d2c39b731408",
  "synchronized_commit": "5b7d9c02caf9e9449adb18ec4fe0f259c8be572b",
  "fix_commit": "975ca45",
  "fix": "交付 chief 预设已经使用 gpt-6.1-sol，重启回归仍断言旧 gpt-6-luna；更新独立字面预期并补充测试指引。未改动用户预设、运行代码或固定模型的原生专项要求。",
  "verification": [
    {
      "command": "bun install --cwd=frontend --frozen-lockfile",
      "outcome": "通过；依赖无变更。"
    },
    {
      "command": "npm --prefix frontend run build",
      "outcome": "通过；仅大 chunk 提示。"
    },
    {
      "command": "python scripts/test-script-contracts.py",
      "outcome": "通过：1 个测试。"
    },
    {
      "command": "npm --prefix frontend test",
      "outcome": "通过：34 个文件，253 项测试。"
    },
    {
      "command": "node scripts/test-backend.mjs",
      "outcome": "通过：构建、全部测试二进制及 doctest；0 个二进制未派发。"
    },
    {
      "command": "npm --prefix frontend run test:browser:build",
      "outcome": "通过：当前 worktree 页面、后端及安全关机壳构建成功；浏览器 363 passed、1 skipped。跳过项为既有文件符号链接场景 schedulerCrossOperation.spec.ts:176。"
    },
    {
      "command": "git add backend/tests/tasks.rs docs/agents/testing.md && git commit -m \"test: align chief preset regression with delivered model\"",
      "outcome": "成功提交 975ca45；2 files changed。"
    }
  ],
  "isolation": {
    "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-mvBk6o",
    "CARGO_TARGET_DIR": "C:/Users/hugos/AppData/Local/Temp/herdr-rust-contracts-01-mvBk6o-target"
  },
  "evidence": {
    "backend_summary": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-TCUsnt/summary.json",
    "command_log": "artifact://49"
  },
  "scope": "本票仅拥有公开表示细化及既有行为回归，只列 herdr-bot 一个仓库；未要求本票执行跨仓联调，不将这些检查外推为后续 Rust 业务或 S2/S3 的交付证据。",
  "blockers": []
}
