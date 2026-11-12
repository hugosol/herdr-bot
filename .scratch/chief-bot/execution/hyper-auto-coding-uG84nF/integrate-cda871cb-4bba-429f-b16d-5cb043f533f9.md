# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-17-durable-report-receiptsmd-ba29d102-",
  "heads": {
    "herdr-bot": "4bac986ec80fdfc9583aeb5c18cf9eac93390875"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "17-durable-report-receipts.md",
  "status": "集成验收通过",
  "repository": "herdr-bot",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-3WERJr",
  "branch": "hyper/chief-bot/17-durable-report-receipts-bbcb24-91332fa5e5a1",
  "head": "4bac986ec80fdfc9583aeb5c18cf9eac93390875",
  "synchronization": "已同步首次集成提交；无遗留合并冲突，最终工作区干净。无需修改或新增提交，未操作集成工作区。",
  "commands": [
    {
      "command": "git status --short --branch && git log -3 --oneline",
      "outcome": "退出 0；原开发分支正确，HEAD 为 4bac986，包含实现提交 606149e；无未提交文件。"
    },
    {
      "command": "export CARGO_TARGET_DIR='C:/Users/hugos/AppData/Local/Temp/hyper-ticket-3WERJr/backend/target' && node scripts/test-backend.mjs && npm --prefix frontend test && npm --prefix frontend run test:browser:build",
      "outcome": "整体退出 0。后端 75 targets、构建、doctest 全通过；chief_delegation 25/25，包括全部 7 个 receipts 用例。前端 34 文件、253 测试通过。浏览器 356 通过、1 个既有符号链接用例 skipped；前后端均使用本票 worktree 构建产物。"
    },
    {
      "command": "sha256sum backend/Cargo.lock frontend/bun.lock",
      "outcome": "退出 0，锁文件身份已记录。"
    },
    {
      "command": "git status --short --branch && sha256sum backend/target/debug/herdr-bot.exe backend/target/debug/herdr-fixture.exe backend/static/index.html",
      "outcome": "退出 0；最终工作区干净，构建产物身份已记录。"
    }
  ],
  "evidence": {
    "completeOutput": "artifact://763",
    "backendSummary": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-mVLq1H/summary.json",
    "receiptTests": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-mVLq1H/binary-016.stdout.log",
    "browserResults": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-3WERJr/frontend/test-results/",
    "sha256": {
      "backend/Cargo.lock": "ecf45bd9047d8772dd8c3abddee0f503e843f91230cc12a00725f593246bb36f",
      "frontend/bun.lock": "1712c4c2006b8de357902c6c6828e5237977676d1c5bccd0a0939bca5e77581c",
      "herdr-bot.exe": "647ed173c05c8b61eb9a78414834cb1ddb633f9ab6b4db77b59e185c1cf6a74f",
      "herdr-fixture.exe": "8e092178f34e3dd9e18d4db898ea89da067cf9bec8223b15c7d44f023c472641",
      "backend/static/index.html": "782d6e8009bbbb779b908ec45a8eb18b7c5e505b360410e32241f439b3b0b4d7"
    }
  },
  "coverage": "本票仅列 herdr-bot，拥有 S2 持久化、并发及故障回归；无本票独占跨仓原生模型验收。没有实际模型调用；不把受控 CLI 或本轮通过外推为 19 的双宿主恢复或 20 的原生交接验收。",
  "blockers": []
}
