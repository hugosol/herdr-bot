# Initial Suite native acceptance

`python scripts/accept-initial-suites.py` owns P11's exploration OMP 18,
exploration Pi 18 and implementation OMP 11 scenarios, P42's actual
`hyper-auto-coding` command, and P44's complete two-repository delivery manifest
and public-entrypoint consistency. The approved interfaces are isolated
startup/deployment (S3), existing Task/Suite/Chat HTTP (S2), and installed native
hosts launched by real Herdr (S4). No product test-only endpoint is added.

## Dependencies and preparation

Run from the explicit herdr-bot worktree with a separately supplied toolkit Git
worktree containing the delivered Pi Suite extension. Only herdr-bot is modified
by this acceptance ticket. Dependencies must already be installed. Do not
install/elevate/change PATH, patch Herdr/OMP/Pi, use a source CLI/SDK-built agent,
replace the native tool manager, copy credentials or use existing user Chats.

Build this worktree's frontend and backend according to the repository testing
instructions. The runner checks the Cargo depfile, source timestamps, frontend
freshness, binary hash, normal PATH launchers and their installed package roots.
Do not substitute another worktree's binary or static assets.

```powershell
$env:CARGO_TARGET_DIR = Join-Path $HERDR_BOT_WT 'backend/target'
Set-Location $HERDR_BOT_WT
npm --prefix frontend ci
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-paths
python scripts/accept-initial-suites.test.py
```

Supply a **dependency-only** paths file. Each root is an existing read-only
program, source skill collection or individual installed Herdr integration;
never point it at a user auth/config directory. Resolve roots using the delivered
`herdr-paths` binary, not a second TOML/root-reference implementation.

```toml
[roots]
herdr = 'C:/installed/herdr'
pi_package = 'D:/node_modules/@earendil-works/pi-coding-agent'
omp_package = 'D:/node_modules/@oh-my-pi/pi-coding-agent'
pi_state_extension = 'D:/readonly-integrations/herdr-agent-state.ts'
omp_state_extension = 'D:/readonly-integrations/herdr-omp-agent-state.ts'
wayfinder = 'D:/sources/oh-my-wayfinder'
wayfinder_skills = '${roots.wayfinder}/skills'
matt_pocock_skills = 'D:/sources/matt-pocock/skills/skills'
# The actual directory spelling can differ from the native skill name.
expert_translate = 'D:/sources/expert-tranlate'
```

Choose explicit Git baselines immediately before this feature's delivery changes
in each participating repository. The manifest records every delivered changed
file (including uncommitted/untracked source), deleted files, current hashes,
contributing commits and production diff, excluding disposable `.scratch` data.
Do not choose HEAD merely to conceal earlier contributions. Both baselines must
be ancestors of their respective current source revisions. This evidence is not
a new workflow scheduling gate or a claim that unrelated checks passed.

```powershell
$previous = $env:HERDR_BOT_MANUAL_SYMLINKS
$env:HERDR_BOT_MANUAL_SYMLINKS = '1'
try {
  python scripts/accept-initial-suites.py `
    --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" `
    --herdr-bot-base "$HERDR_BOT_BEFORE_SUITES" --toolkit-base "$TOOLKIT_BEFORE_SUITES" `
    --backend "$env:CARGO_TARGET_DIR/debug/herdr-bot.exe" `
    --paths-file "$ISOLATED_DEPENDENCIES" --scenario all `
    --evidence-dir "$NEW_EXCLUSIVE_EVIDENCE_DIR"
} finally {
  $env:HERDR_BOT_MANUAL_SYMLINKS = $previous
}
```

## Independent native observations

The independent name/source literals live in the acceptance runner, not in the
assembler or an expectation recomputed from `suites.json`. The delivered
`meta-data/suites.json` is copied **byte-for-byte** into each isolated deployment;
its roots resolve to the real approved skill sources and an owned copy of the
supplied toolkit. That copy excludes live runtime access records. Only its
connect-bot/workflow endpoint configuration is overridden to this run's dynamic
loopback backend; the original source is untouched. The centralized MCP name is
retained, pointing to a controlled external protocol server, not a replacement
native MCP manager. This test does not claim real codebase-memory service quality.

A test-owned fixed extension registers an observation command using the native
public extension API. Its `pi.getCommands()` snapshot is the host's actual loaded
skill/extension command set and source metadata. This includes manual-only
`expert-translate`, which a model-prompt-only check could miss. Exact equality
rejects missing, duplicate, renamed and extra skills. Each discovered source is
independently resolved against the approved wayfinder-first/matt-pocock-second
literal and hashed. The directory `expert-tranlate` is not renamed into a skill
alias; the native name remains `expert-translate`. `to-tickets` is required;
`to-ticket` is not accepted. Config existence or the copied definition alone
never certifies P11.

Implementation OMP must first expose exactly one **actually registered**
extension command named `hyper-auto-coding`. The runner then sends
`/hyper-auto-coding` with no slug through Herdr's native pane input and requires
the real handler's `用法: /hyper-auto-coding <slug>` response. This branch precedes
spec reads, workflow/Phase 2, worktree creation, TDD and retro. An initial prompt
string or registration list alone is not P42 evidence. No user task is executed:
any model request or task `.scratch` output fails acceptance.

P44 separately retains both complete delivery inventories, build/source identity,
installed Herdr/OMP/Pi package hashes before and after, ordinary launcher/package
identity and the OS-observed installed CLI process under owned Herdr. Product
diffs reject host patch/vendor delivery and recognized internal patch/SDK
reconstruction patterns. These consistency checks do not make a missing native
entrypoint pass: all three real scenarios and cleanup are required for P44.

## Isolation, results and limits

Every scenario owns fresh HOME/USERPROFILE/APPDATA/LOCALAPPDATA/XDG/native state,
configuration, generated resources, dynamic loopback ports, a named Herdr server
and processes. Individual installed state-extension files are loaded read-only;
no adjacent credentials/settings/history are read. Process stdout/stderr are
continuously consumed. Normal and failed launches stop owned services and await
their exits, with native/terminal descendants observed through Windows process
handles. Original failures and cleanup failures remain separate in evidence.

The evidence directory must not already exist. Store raw runs under this
feature's disposable evidence area and exclude only that run directory locally;
do not commit native histories, credentials or raw logs. `evidence.json` reports
all five coverage entries as unverified until their complete owning observation
and cleanup succeeds. `--scenario exploration-omp|exploration-pi|implementation-omp`
is narrower feedback and cannot yield full acceptance. `--timeout` bounds each
observation (default 90s, maximum 300s), not a retry budget. Scenarios are attempted
independently; one failure does not prevent the remaining scenarios.

Missing skill sources, Pi public factory flag/version incompatibility, Windows
true-symlink denial (including OS error 1314), host dependency failures, a missing
native response or cleanup failure are nonzero and explicitly unverified. Never
substitute junctions, copied skills, source-CLI success, historical prototypes,
configuration counts or unrelated green tests. The S3 CLI regression proves
foreign-backend evidence fails closed; it is not an S4 success claim. Complete
backend/frontend regressions supplement, but cannot replace, this native matrix.
