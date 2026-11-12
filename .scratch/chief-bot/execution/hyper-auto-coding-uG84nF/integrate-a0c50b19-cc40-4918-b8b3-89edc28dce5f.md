# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-16-accept-chief-reportsmd-f68d8dd7-d64",
  "heads": {
    "herdr-bot": "1ff7c629ba6fc174ef44114bfffac7997eca5819",
    "my-agent-toolkit": "cfd220d6410fd92a7188c3b171932b866eff79f1"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "passed",
  "ticketClosed": true,
  "heads": {
    "herdr-bot": "1ff7c629ba6fc174ef44114bfffac7997eca5819",
    "my-agent-toolkit": "cfd220d6410fd92a7188c3b171932b866eff79f1"
  },
  "worktreesClean": true,
  "result": "解决 toolkit 遗留合并冲突并提交；修复报告背景遗漏、呈现时越界读取交付物、评价器引文证据及原生测试 peer 路由。两宿主全部 P18/P19/P20/P22/P28/P36/P37 通过。",
  "commands": [
    {
      "command": "npm --prefix frontend run build",
      "exit": 0
    },
    {
      "command": "cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell",
      "exit": 0
    },
    {
      "command": "node scripts/test-backend.mjs",
      "exit": 0
    },
    {
      "command": "npm --prefix frontend test",
      "exit": 0,
      "passed": 253
    },
    {
      "command": "npm --prefix frontend run test:browser:build",
      "exit": 0,
      "passed": 360,
      "skipped": 1
    },
    {
      "command": "bun run typecheck",
      "exit": 0
    },
    {
      "command": "bun run test",
      "exit": 0,
      "passed": 1926,
      "failed": 0
    },
    {
      "command": "bun run check:integrations",
      "exit": 0
    },
    {
      "command": "bun test tests/harness-dock/native-chief-resend.test.ts tests/harness-dock/chief-map-unavailable.test.ts tests/harness-dock/chief-role.test.ts",
      "exit": 0,
      "passed": 14
    },
    {
      "command": "bun test tests/harness-dock/native-chief-report.test.ts",
      "exit": 0
    },
    {
      "command": "python scripts/accept-chief-reports.test.py",
      "exit": 0
    },
    {
      "command": "python scripts/accept-workflow-codex.py --scenario reports --hosts omp,pi [完整绝对路径参数见证据]",
      "exit": 0,
      "actualModel": "gpt-6-luna",
      "actualRequests": 59
    }
  ],
  "acceptance": {
    "hosts": [
      "omp",
      "pi"
    ],
    "casesPerHost": [
      "success",
      "failure",
      "blocked",
      "capability"
    ],
    "quietWindowSecondsPerCase": 46,
    "ownedProcessExitConfirmations": 20,
    "cleanupErrors": []
  },
  "retainedFailures": "所有早期评价器、peer、idle recap 清理及 Pi 越界 read 失败均保留；最终通过发生于对应修复之后。",
  "evidence": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-k4pZ72/.scratch/chief-bot/evidence/16-integration/final-report.json",
  "fullAcceptanceEvidence": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-k4pZ72/.scratch/chief-bot/evidence/16-integration/reports-report-only/business/evidence.json",
  "handoff": "已确认所属进程退出并直接释放独占 browser/native/dock 窗口给19。未操作集成工作区或直接合入dev。",
  "blockers": []
}
