# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-02-create-bot-from-zonemd-bcce9eed-d1a",
  "head": "1aee15dd547e284a4f3fcc522bf99eec34642e92",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "passed",
  "ticket": "02-create-bot-from-zone",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-m9fXMx",
  "branch": "hyper/bot-template-creation/02-create-bot-from-zone-6b330b-11684352cd16",
  "head": "1aee15dd547e284a4f3fcc522bf99eec34642e92",
  "sync_check": {
    "merge_commit": "1aee15dd547e284a4f3fcc522bf99eec34642e92",
    "merge_parents": [
      "30a5baeaa7d36b7d68fa048a47eb3aa1e6fa4179",
      "a3031d310b403677bbeb8108b620afd67faa9b71"
    ],
    "integration_branch": "feature/create-bot",
    "integration_tip": "1aee15dd547e284a4f3fcc522bf99eec34642e92",
    "dev_head_equals_integration_tip": true,
    "tree_identical_to_integration": true,
    "conflict_markers_found": false,
    "git_diff_check": "clean (exit 0)",
    "unmerged_paths": 0,
    "worktree_clean": true
  },
  "global_tests": [
    {
      "command": "cargo test --no-fail-fast",
      "cwd": "backend/",
      "result": "exit 0; 325 passed; 0 failed; 3 ignored (mDNS, need --ignored); all test targets incl. bot_creation, herdr_config, bot_templates, zone_commit/zone_sync",
      "duration_seconds": 287
    },
    {
      "command": "npm test",
      "cwd": "frontend/",
      "result": "exit 0; 25 test files passed; 176 tests passed; 0 failed",
      "duration_seconds": 12
    },
    {
      "command": "npm run build",
      "cwd": "frontend/",
      "result": "exit 0; tsc --noEmit and vite build succeeded (extra check, not part of the gate)"
    }
  ],
  "fixes_applied": [],
  "commits_created": [],
  "not_run_by_scope": [
    "npm run test:browser (real Chrome / controlled-process smoke reserved for final independent gate)",
    "cargo test -- --ignored (real mDNS/firewall tests reserved for final gate)",
    "real Herdr/agent H1 manual run (ticket explicitly leaves it unverified)",
    "cross-ticket overall acceptance (final gate)"
  ],
  "blockers": [],
  "notes": [
    "Merge brought ticket 02 work (a3031d3) onto the ticket-01 integration state (30a5bae) with no conflicts; merged tree is byte-identical to feature/create-bot tip.",
    "All temporary test logs removed; git status --porcelain is empty.",
    "Root herdr-bot.toml retains the required explicit [herdr] workspace_name = 'bot-playground'.",
    "No repair was needed: the merged state passed both global test commands on the first run; the previously flaky zone timeout case passed."
  ]
}
