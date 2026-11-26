# Suite Chat lifecycle acceptance

Set `$TESTER` and `$BOT` and follow the isolation and evidence rules in the [testing guide](agents/testing.md).

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-suite-lifecycle` owns P12/P13/P28/P29/P30/P31/P32 and
P35's **real concurrent startup consumption**. P35 assembly/publication tests
remain separate. The approved boundaries are public Task/Suite/Chat/workflow HTTP
(S2), isolated deployment/startup and restart (S3), and installed native agents
launched by real Herdr (S4). This is not a replacement for either repository's
full regression suite or for the other Suite resource/MCP/mode acceptance.

## Inputs and safety

Build and run product regressions from `$BOT`. The wrapper binds `$BOT` before
the scenario, regardless of the shell's current directory. Both worktrees, the
backend binary, the dependency-only paths file, and a new evidence directory are
mandatory. There is no installed-toolkit, other-worktree backend, user deployment,
or paths.local fallback. `--scenario all` (default) independently runs `baseline`,
`suite-lifecycle`, `roles-native`, `roles-suite`, `native-resume`, `auth-native`
and `concurrency`, each with both installed hosts. An earlier failure does not
prevent later independent scenarios from being attempted. `--host pi|omp` and individual scenarios provide narrower
feedback; they do not certify the complete eight-promise matrix. The summary
keeps every promise explicitly unverified until both hosts' owning scenarios and
process cleanup succeed. A missing dependency or failed observation is nonzero,
not a skip. Business actions are not retried. A narrowed successful slice is
reported in `selectedScenarioSucceeded`; the overall `passed` flag and exit code
remain unsuccessful while required host/scenario coverage is incomplete.

The current tester filters `approve` out of its role-tool comparison. The
`roles-native` and `roles-suite` results do not establish that nodes expose
`approve` or that other roles lack it, even if their report and recovery checks
pass. Keep that part of P13 unverified.

The paths file contains only read-only native dependencies. The two integration
roots deliberately name **individual installed Herdr extension files**, not agent
configuration/auth directories. The runner hashes and loads these files; it does
not inspect adjacent user data. They are copied as source into the test's ambient
extension discovery fixture for source-less recovery, never as credentials.

```toml
[roots]
herdr = 'C:/installed/herdr'
pi_package = 'D:/node_modules/@earendil-works/pi-coding-agent'
omp_package = 'D:/node_modules/@oh-my-pi/pi-coding-agent'
pi_state_extension = 'D:/readonly-integrations/herdr-agent-state.ts'
omp_state_extension = 'D:/readonly-integrations/herdr-omp-agent-state.ts'
```

The installed `pi` and `omp` PATH launchers must actually resolve to those package
roots. The runner does not install programs, change PATH, elevate privileges,
modify developer mode, patch Herdr/Pi/OMP, use a source CLI/SDK-assembled agent,
replace a native tool manager, or redirect the product's whole agent directory.
The **test process environment** isolates HOME/USERPROFILE/APPDATA/LOCALAPPDATA,
XDG and native agent storage; that is fixture ownership, not a product Suite
configuration feature. Inherited provider credentials/proxies are removed.

Neither native model configuration contains an apiKey. OMP's custom-provider
schema additionally requires an explicit `auth: none` declaration when the key
is not in models.yml. This only admits the custom loopback model: OMP's native
resolver still prefers the stored credential to its anonymous sentinel. The
external endpoint independently requires the exact random native-store key;
an unauthenticated request cannot pass. Pi reads the same test-owned credential
through its native auth.json format. Native auth APIs seed fresh data; no user
auth store is copied or linked.

The toolkit deployment is made from the supplied worktree's source, excluding
runtime owner-access records. Its workflow endpoint is changed only in the
owned deployment copy to the dynamically bound backend. Source/deployment hashes
are retained. No other source copy is silently substituted. Custom Suite
`extensions` are concrete source entries passed directly to native hosts; no
extension-package links or loader manifests are generated. Module dependencies
and assets are read by the native host from their original source directory.
Each Suite has one `harness` and a flat `extensions` array. Use separate Suites
for OMP and Pi, even when they select the same source files. Skills reference
source paths directly and need no generated links or symlink privileges. New
native processes consume the current Suite declarations.

## Build, repository checks and invocation

Prepare frontend dependencies with `bun install --cwd=frontend --frozen-lockfile` and toolkit
dependencies with `bun install --frozen-lockfile` using its declared Bun version.
Use a Cargo target directory inside this herdr-bot worktree. Build frontend
assets before process-level backend checks; the backend embeds source identity.
The runner verifies the Cargo depfile/source timestamps, binary hash, frontend
freshness, both worktree revisions/diffs/file hashes, native package/launcher
identity, installed integration hashes and host versions. This is local build
provenance, not a reproducible-build attestation.

```powershell
$TOOLKIT_WT = 'D:/worktrees/toolkit'
$ORIGINAL_TOOLKIT_ROOT = 'D:/sources/original-toolkit-checkout'
$ISOLATED_PATHS = 'D:/acceptance/lifecycle-paths.toml'
$EVIDENCE_DIR = 'D:/acceptance/lifecycle-run-unique'
Set-Location $BOT
$env:CARGO_TARGET_DIR = Join-Path $BOT 'backend/target'
bun install --cwd=frontend --frozen-lockfile
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bins --example safe_shutdown_shell
node scripts/test-backend.mjs
npm --prefix frontend test
npm --prefix frontend run test:browser:build

Set-Location $TOOLKIT_WT
bun install --frozen-lockfile
$env:OMP_BASELINE_RECORD_ROOT = $ORIGINAL_TOOLKIT_ROOT
$env:HERDR_BOT_REPO = $BOT
$env:HERDR_BOT_BINARY = Join-Path $env:CARGO_TARGET_DIR 'debug/examples/safe_shutdown_shell.exe'
$env:HERDR_BOT_METADATA = Join-Path $BOT 'meta-data/task-options.toml'
bun run typecheck
bun run check:integrations
bun run test

$env:LIFECYCLE_TOOLKIT_WORKTREE = $TOOLKIT_WT
$env:HERDR_BOT_WORKTREE = $BOT
python "$TESTER/scripts/accept-suite-lifecycle.test.py"
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-suite-lifecycle `
  --toolkit-worktree "$TOOLKIT_WT" `
  --backend "$env:CARGO_TARGET_DIR/debug/herdr-bot.exe" `
  --paths-file "$ISOLATED_PATHS" --scenario all --host all `
  --evidence-dir "$EVIDENCE_DIR"
```

For this matrix, OMP selects skill collections and Pi selects individual skill
directories. These are fixture selections, not limits on OMP's supported sources. `--timeout` bounds an
observation (default 90 seconds, maximum 300), not a retry budget. Evidence output
must not already exist. Each scenario has its own deployment/native storage,
named Herdr server, dynamic loopback model/backend ports and processes. stdout
and stderr are continuously drained. Success and failure stop the owned server
and process tree, await service exits, and independently confirm held Windows
process handles have exited. Original errors and cleanup errors are separate;
cleanup failure invalidates coverage. Test credentials are not command arguments,
headers in evidence, or logs; credential stores are removed after confirmed exit.

## Independent expectations and observations

| Coverage | Scenario/oracle |
| --- | --- |
| P12 | Each host starts ordinary Chats with and without Suite, loads its real shipped model-cost entry, executes `/budget help` locally, and observes `detail`, `holiday`, `clear` without another model request. The real installed Herdr state integration supplies native identity. Pi's shipped fixed pi-suite consumes real selected resources. Role entries are exercised, not substituted by observer tools. |
| P13 | Use the same Task independently and as a dispatched node. The expected workflow tools are normal `{}`, manager `{run,collect}`, and node `{approve,report}`. Observe actual native calls and public workflow HTTP, with approval and reporting recorded separately. Within the creating backend, manager and node identity and execution IDs survive native process exit and explicit Resume without replay. After a real backend restart, the original Run remains read-only. Native role loading and identity remain, role actions return `manager-unassociated` or `node-unassociated`, `managed=false`, and saved Run and report facts stay unchanged. Resume cannot take over a Run or dispatch another execution. |
| P28 | Model definitions contain no apiKey. Native auth is seeded using installed hosts' public native auth-storage APIs, with random test-owned data, then every real model request must authenticate with that single key (only match booleans are retained). Same/different Suite startup overlaps. Two native credential writers update independent provider rows in the same default native store; native readback must preserve both rows and the launch credential. No absent `--agentDir` argument is used as evidence. |
| P29 | The observer reads the effective native model and thinking level; a separate installed native SessionManager opens the actual native history and reads original identity/branch. Histories must stay under the test's native session root. Both native model configuration formats are independently reread unchanged. |
| P30 | Literal provider/model `lifecycle-owned/lifecycle-probe`, thinking `high`, initial Skill body `LIFECYCLE-INITIAL-SKILL-463`, fixed text `LIFECYCLE-FIXED-172`, input `LIFECYCLE-INPUT-298`, and `Lifecycle {chat.id}` naming are independent oracles. Native user history contains the initial Skill and each fixed/input marker exactly once. Suite resources contain no replacement trigger. |
| P31 | Initial Suite A consumes A. Saving A selects B for future launches. The source Task then selects Suite B, whose definition is independently changed to C. After backend restart, original-identity resume must consume C, not A/B; changed model/thinking/Skill/prompt/name trigger fields must not replay or rename. Before any explicit resumed user prompt, the native branch is identical; afterward exactly that one new user message exists. Genuine native histories also recover through a legacy no-source catalog fixture and actual Task deletion plus a same-name decoy, with native default discovery and explicit extension degradation, not a guessed Suite. |
| P32 | A live A Chat remains the same PID with no new model request or before-agent-start event across save/read barriers and a 1.25-second quiet observation; an explicit subsequent user turn consumes A, while a later launch consumes B. This does not disable native reload or freeze third-party source bytes; only the Suite definition is changed in this scenario. |
| P35 | Both same-Suite and different-Suite pairs run real native hosts. A documented native session-start handler blocks before the installed state integration publishes readiness. The first public creation must still be `starting` or `submitting`; a second launch must actually consume both complete sentinels before the first is released. Finally the first also consumes its complete set. A/B/C skill and package-asset literals are independently declared; mixed/missing sets fail. No file-coordination lock may extend over that external startup effect. |

`auth-native` holds the first authenticated **model request**, then launches the
second host and releases only after both native credential-resolved requests have
arrived. Arrival/release timestamps independently prove overlap. This does not
hold TUI startup; the stricter external-start gate remains specific to P35.
`native-resume` also calls an ambient-only tool and checks literal
`NATIVE-DISCOVERY-ONLY-784`, so a mistaken fixed-only fallback cannot pass merely
because its reported degradation text says native discovery. OMP stores initial
Skill invocation as a user-attributed `custom_message`; that native record and
its exact original `/skill:lifecycle-initial` arguments are counted as user input,
including when checking that recovery did not replay it. Pi's native user-message
representation is observed separately, not forced onto OMP's history schema.

A controlled loopback model chooses tool calls; native tools read the actual
selected Skill and package-relative asset and execute the real role tools. This
proves lifecycle/resource/protocol mechanisms, not a real model's autonomous
business decisions. The fixed observer only uses documented native events and
read-only extension APIs. It does not monkey-patch any internal collaborator.

The top-level `evidence.json` maps coverage to each host/scenario's evidence;
per-scenario records retain actual requests/results, native model wire, resource
outputs, native history and hashes, observation events, terminal output,
source/binary/native versions, owned process snapshots and exit confirmations.
The CLI safety test proves only missing-backend refusal/evidence integrity at S3;
it does not certify S4 promises. A global green suite, nonempty log, zero process
exit or manifest without actual consumption never marks a native item verified.
