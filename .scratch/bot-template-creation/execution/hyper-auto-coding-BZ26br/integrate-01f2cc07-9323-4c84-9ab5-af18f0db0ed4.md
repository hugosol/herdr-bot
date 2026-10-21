# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-01-shared-bot-templatesmd-fcd3fa21-cc5",
  "head": "30a5baeaa7d36b7d68fa048a47eb3aa1e6fa4179",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "passed",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-dzuMHF",
  "branch": "hyper/bot-template-creation/01-shared-bot-templates-f0b11e-11684352cd16",
  "head": "30a5baeaa7d36b7d68fa048a47eb3aa1e6fa4179",
  "sync_check": {
    "dev_head_equals_integration_tip": true,
    "integration_branch": "feature/create-bot",
    "tree_identical_to_integration": true,
    "conflict_markers_found": false,
    "git_diff_check": "clean (exit 0)",
    "unmerged_paths": 0
  },
  "global_tests": [
    {
      "command": "cargo test --manifest-path backend/Cargo.toml",
      "cwd": "repo root",
      "result": "305 passed; 0 failed; 3 ignored (mDNS, require --ignored); 37 suites",
      "duration_seconds": 261
    },
    {
      "command": "npm --prefix frontend run test",
      "cwd": "repo root",
      "result": "24 test files passed; 173 tests passed; 0 failed",
      "duration_seconds": 11
    }
  ],
  "fixes_applied": [],
  "commits_created": [],
  "final_worktree_clean": true,
  "not_run_by_scope": [
    "npm --prefix frontend run test:browser (real Chrome / real-process smoke reserved for final independent gate)",
    "cargo test -- --ignored (real mDNS/firewall tests reserved for final gate)",
    "frontend build / cross-ticket acceptance (final gate)"
  ],
  "blockers": []
}
