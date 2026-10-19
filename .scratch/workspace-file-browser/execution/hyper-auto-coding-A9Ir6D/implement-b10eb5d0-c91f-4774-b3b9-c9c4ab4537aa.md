# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "07-refresh-from-filesystem-eventsmd-23dd8c8a-563",
  "head": "dcd54ad114dbcf4de66394da498bd3871b6e34ab",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "completed",
  "ticket": "07-refresh-from-filesystem-events",
  "commit": "dcd54ad workspace-file-browser: refresh from filesystem events",
  "delivered": [
    "Backend module backend/src/watch.rs: per-workspace real filesystem listener (notify 8) watching the workspace root recursively plus linked-worktree external Git metadata, per-workspace event coalescing gate (immediate when idle, one trailing flush per window, bounded under continuous writes), generation/health state, and a single SSE endpoint GET /api/workspaces/watch shared by all observers.",
    "Startup config [workspaces] refresh_interval_seconds (default 10, legal 1..=u32::MAX); missing section/field keeps default, zero/negative/float/string/mistyped keys fail startup with a field-specific error.",
    "Frontend workspaceWatch.ts real SSE feed with complete-snapshot recovery, health/refresh frames, interrupted-connection degradation, useWorkspaceWatch hooked to the file-list tab, auto re-read of directory/file/Git status and list badges, and explicit '自动更新已停止' / '可能已过期' banners; manual refresh, workspace entry/return, and all existing query paths are never throttled.",
    "Listener health detects missing/replaced workspace root and external Git metadata identity every min(interval, 5s), reports an error once, and never auto-restarts or claims recovery on the same connection.",
    "README, omp-bot.toml, and module table updated; real-browser evidence (screenshots + notes) added under .scratch/workspace-file-browser/evidence/07-refresh-from-filesystem-events/."
  ],
  "criteria_evidence": {
    "real plain-directory add/modify/delete auto-updates": "backend/tests/workspace_watch.rs::external_plain_directory_create_modify_delete_refreshes_browsing (real process, real fs, real SSE); real Chromium saw auto.txt appear 12ms after external creation without clicking refresh (desktop-auto-refresh.png).",
    "real Git branch/pending + metadata": "same file::external_git_branch_and_worktree_writes_refresh_branch_and_pending_status and ::linked_worktree_external_git_metadata_updates_branch; real Chromium showed 分支：browser-feature-evidence 20ms after external git switch without refresh (desktop-git-branch-refresh.png).",
    "per-workspace merge/throttle, 10s default, immediate first refresh, trailing flush, no infinite postponement": "watch::gate_tests (5 tests: window start/end, dense/continuous writes, new window after flush); ::default_config_reports_ten_second_interval_at_runtime; ::two_observers_share_one_throttled_refresh_sequence (one shared generation sequence, one trailing flush); ::manual_read_inside_refresh_window_returns_current_facts_immediately.",
    "multiple observers not amplifying": "::two_observers_share_one_throttled_refresh_sequence asserts both connections receive the same generation for immediate and trailing refreshes; backend owns one watcher/gate per workspace.",
    "config default/override/legacy/invalid": "backend/src/config.rs config tests: legacy defaults, explicit 25/1 accepted, zero/negative/wrong-type rejected; runtime snapshot test verifies 10s default reaches the delivered process.",
    "listener failure visible, no false recovery, manual query real": "::listener_failure_is_reported_and_manual_queries_still_read_current_facts (real delete then recreate; health error, manual 404 then 200 current.txt, no refresh/health-live afterwards); frontend workspaceWatch tests for watch-error/watch-stale banners; real Chromium showed the error banner and manual refresh read current.txt while the banner stayed visible (desktop-watch-error.png, desktop-manual-refresh-after-failure.png).",
    "no periodic fetch / local-only": "implementation only watches local paths and emits refresh notifications; no remote/fetch call or timer was added to any watcher or frontend data source.",
    "throttle does not take over deliberate queries": "::manual_read_inside_refresh_window_returns_current_facts_immediately; frontend manual-refresh test runs while listener is failed; existing entry/return effect remains immediate and bypasses the watch generation path."
  },
  "verification_run": [
    "cd backend && cargo test (full suite) — passed; last background snapshot showed all integration suites including workspace_browse 8, workspace_git_status 7, workspace_registry 7, workspace_watch 6, doc-tests 0.",
    "cd backend && cargo test --test workspace_watch — 7 passed after adding the manual-read bypass test.",
    "cd frontend && npm test — 17 files, 115 tests passed.",
    "cd frontend && npm run build (tsc --noEmit && vite build) — passed.",
    "Real delivered process + managed Chromium: external create auto-refreshed in 12ms, external branch switch in 20ms, listener failure banner in 44ms, manual refresh after failure showed current.txt with error banner retained.",
    "Fault injection: disabling the health tick made the listener-failure test time out; removing external Git metadata watching made the linked-worktree test time out; disabling trailing flush made the multi-observer test time out; all restored and re-run green."
  ],
  "unverified_or_out_of_scope": [
    "No merge or push performed; ticket status file left unchanged for the integration agent.",
    "Full window-interleaving tests across operations/diff/Commit/Sync are assigned to ticket 08 and were not attempted here.",
    "No mobile real-device check applies to this ticket's P39/P40/P52/P54 coverage; no real shutdown was executed.",
    "Silent watcher-thread death with an unchanged workspace identity is not independently detectable; the reproducible failure covered is path deletion/replacement plus notify error callbacks."
  ]
}
