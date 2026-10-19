# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "04-review-final-content-diffsmd-b994a532-f505-44",
  "head": "9cff66253266d4f0805df2a56d334c988bb71978",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "04-review-final-content-diffs",
  "status": "implemented and verified; ticket branch ready for integration; ticket status set to in-progress (not merged)",
  "commits": [
    "b01d01c workspace-file-browser: review final content diffs",
    "ba47e24 workspace-file-browser: record ticket 04 implementation status",
    "9cff662 workspace-file-browser: distinguish remote comparison in diff test"
  ],
  "delivered": {
    "backend": "GET /api/workspaces/diff reads HEAD (empty tree when no first commit) as the old side and the final local file as the new side, merging staged and unstaged content; supports added/deleted/renamed (oldPath) and conflicted changes. Both sides independently apply preview_limit_bytes before loading content; binary/too-large sides set complete:false with explicit reasons. Both new and old paths go through the shared workspace root-boundary validation, including links, junctions, traversal, absolute paths, and root replacement. Uses only git ls-tree/cat-file and read-only browse reads.",
    "frontend": "Change list is selectable; desktop shows left list + right diff, default side-by-side; narrow shows unified diff with explicit back and exact list-scroll restore. Selection, diff, and scroll survive internal browse/changes view and top-level tab round trips. Diff rows are rendered as text nodes only; over-limit/binary changes remain visible, colored, and counted."
  },
  "acceptance": [
    {
      "criterion": "HEAD→final local content, not index→worktree or remote→local; added/deleted reviewable",
      "evidence": "backend/tests/workspace_diff.rs::diff_reads_head_to_final_content_without_touching_index_or_files (HEAD beta vs index STAGED vs final STAGED/FINAL; differing refs/remotes/origin/main; added/deleted/rename); frontend tests for added/deleted; Chromium old side beta"
    },
    {
      "criterion": "No-first-commit repository uses empty tree; eligible files shown as green additions; no git init",
      "evidence": "backend/tests/workspace_diff.rs::empty_repository_compares_against_an_empty_tree; existing added-green frontend coverage; no git-init entry point added"
    },
    {
      "criterion": "Files and index semantics unchanged around read/diff/rename review",
      "evidence": "same test records index/file snapshots; Chromium evidence independently hashed .git/index, ls-files --stage, cached/unstaged diffs and all file bytes (unchanged: true)"
    },
    {
      "criterion": "New public diff entry enforces root boundary incl. outside, traversal, symlink, junction, path replacement, and oldPath",
      "evidence": "backend/tests/workspace_diff.rs::diff_entry_enforces_the_workspace_root_boundary (real junction and real symlink created; root replaced by junction; oldPath traversal; unregistered workspace; closed query surface; no secret leaked)"
    },
    {
      "criterion": "Diff text only plain text; real browser proves HTML/script does not execute; no suffix whitelist",
      "evidence": "Chromium: literal <script>/<img> in diff, diff innerHTML escaped, window.__pwned undefined, no img element; binary content in a .txt file is classified by content; frontend test diffReview.test.tsx"
    },
    {
      "criterion": "Per-side content limit (default 1 MiB), exact boundary, both over-limit combinations, no full diff, binary reason",
      "evidence": "backend/tests/workspace_diff.rs::diff_sides_apply_the_preview_limit_independently (32-byte exact, old>new-small, old-small>new, both binaries); Chromium 4,950,000 > 1,048,576 for each side and NUL binary reason; no diff-view rendered"
    },
    {
      "criterion": "Over-limit/binary changes stay visible, colored, counted in both views",
      "evidence": "frontend diffReview test asserts change list and browse tree rows retain data-change/color and count 变更（7）; Chromium 变更（23） unchanged with limit/binary rows visible"
    },
    {
      "criterion": "Real wide browser: left list/right content, default side-by-side, selection and scrolling, screenshots",
      "evidence": "Chromium 1024×800 evidence directory 04-review-final-content-diffs; 23-item list scrollHeight 789 > clientHeight 518, scrollTop 260 with selected diff; screenshots desktop-*.png"
    },
    {
      "criterion": "Real narrow browser: default unified diff, progressive entry, explicit back, list scroll restore",
      "evidence": "Chromium 390×844: data-mode=unified, unified -beta/-gamma/+STAGED/+FINAL, 返回变更列表, scrollTop 260 restored exactly; narrow-*.png"
    },
    {
      "criterion": "Context preserved across internal views and top-level tabs (selection, expansion, scroll)",
      "evidence": "frontend diffReview test switching browse/changes and sessions/files keeps aria-current, diff and scrollTop 260; Chromium verified scrollTop 260 before/after"
    }
  ],
  "verification": {
    "commands": [
      "cargo test --test workspace_diff → 4 passed",
      "cargo test --test workspace_git_status → 7 passed",
      "cargo test --no-fail-fast → 222 passed / 30 suites, 0 failed",
      "npm test → 16 files / 113 tests passed",
      "npm run build (tsc --noEmit && vite build) → success",
      "cargo fmt --check on changed files (project has pre-existing formatting deviations; new code checked separately)"
    ],
    "mutation_checks": [
      "Temporarily used index objects as the old side: HEAD→final test failed with old=STAGED vs expected beta; reverted, 4 passed",
      "Temporarily rendered diff despite complete:false: over-limit/binary tests failed; reverted, 8 passed",
      "Pure-function property check: 2000 random line-diff cases reconstructed both sides exactly"
    ],
    "note": "cargo test --no-fail-fast was run on the final product code; the later commit only strengthened workspace_diff.rs with a differing remote-tracking ref, re-verified with cargo test --test workspace_diff → 4 passed."
  },
  "evidence": ".scratch/workspace-file-browser/evidence/04-review-final-content-diffs/ (README + 10 screenshots)",
  "limitations": [
    "Wide/narrow browser evidence is headless Chromium; narrow is viewport emulation. Real-phone touch/diff-scroll/back feel remains human-led H1 for ticket 08.",
    "Commit, Sync, review-version revalidation, file-event refresh and conflict recovery remain out of scope for this ticket.",
    "No remote service was contacted; remote comparison is ruled out via a local differing refs/remotes/origin/main and HEAD assertions."
  ],
  "worktree": "clean; only ticket 04 files, its evidence, README and implementation files changed; integration branch untouched; evidence backend stopped"
}
