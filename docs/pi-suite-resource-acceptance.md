# Pi Suite resources and native MCP acceptance

黑盒验收代码已迁至独立项目 `herdr-bot-tester`。本页验收命令使用显式设置的 `$TESTER` 与 `$BOT`，不要求两个项目相邻。产品构建与自身回归仍从产品根运行。

```powershell
$TESTER = 'E:/Developer/myhub/herdr-bot-tester'
$BOT = 'E:/Developer/myhub/herdr-bot'
```

验收设施的 `scripts/` 路径现属于验证项目。参数、授权、覆盖及非零退出约束不变。完整入口见验证项目 `README.md`。

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-suite-pi-resources` owns the Pi resource/MCP matrix at
S2 (public HTTP), S3 (deployment/startup), and S4 (native agents launched by real
Herdr). It also consumes the same selected declaration with installed OMP as a
cross-host resource control; that does not replace OMP's separately owned tests.
It never uses an SDK, source CLI, internal monkey patch, replacement MCP manager,
user session, credential copy, junction or copied-resource fallback.

## Prerequisites and commands

Use two explicit participating Git worktrees, installed Herdr/Pi/OMP on their
normal PATH launchers, Python 3.11+, Node.js, and the toolkit's required Bun
version. Prepare repository dependencies according to each repository's testing
instructions. Build this backend with a worktree-private Cargo target directory,
including its public path resolver. Build the current frontend, not a different
worktree's static assets:

```powershell
$TOOLKIT_WT = 'D:/worktrees/toolkit'
$ISOLATED_PATHS = 'D:/acceptance/pi-resource-paths.toml'
$EVIDENCE_DIR = 'D:/acceptance/pi-resource-run-unique'
$env:CARGO_TARGET_DIR = Join-Path $BOT 'backend/target'
Set-Location $BOT
bun install --cwd=frontend --frozen-lockfile
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-paths
```

Supply a dedicated dependency-only TOML file, never the user's business/config
roots or authentication directory. These roots are read-only installed program
locations, not alternative implementations:

```toml
[roots]
herdr = 'C:/installed/herdr'
pi_package = 'D:/node_modules/@earendil-works/pi-coding-agent'
omp_package = 'D:/node_modules/@oh-my-pi/pi-coding-agent'
```

The runner resolves this file through the `herdr-paths.exe` built beside the
specified backend. It checks the backend's Cargo depfile names the supplied
source root and that the binary does not predate current Rust sources/manifest/
lockfile. It retains those source hashes, binary hash, tracked source hashes,
revision/diff, native launcher/package paths and host versions. This is local
build provenance, not a cryptographic reproducible-build attestation. Package
arguments must match what the installed PATH launchers actually load. There is
no fallback to another worktree, installed toolkit or user deployment.

```powershell
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-suite-pi-resources `
  --toolkit-worktree "$TOOLKIT_WT" `
  --backend "$env:CARGO_TARGET_DIR/debug/herdr-bot.exe" `
  --paths-file "$ISOLATED_PATHS" --scenario all --evidence-dir "$EVIDENCE_DIR"
```

`--evidence-dir` must not already exist. `--scenario` selects one named scenario
for narrower feedback; only `all` attempts the complete owned matrix. No scenario
is silently skipped, and failures do not prevent independent scenarios from
being attempted. Selected skills and extensions load directly from their source
paths without creating symlinks. Missing dependencies, startup failure,
mismatched results and cleanup failure return
nonzero. The runner does not install programs, elevate privileges, modify PATH,
read real authentication, enable LAN/mDNS, run real shutdown or operate existing
user Chat/daemon processes.

## Independent observations

| Scenario | Coverage and independent oracle |
| --- | --- |
| `selected` | P6/P7/P9/P10: one logical declaration, both native hosts; Pi runs in two distinct cwd directories. Central root and declaration-relative sources contain spaces. Exactly `selected-relative` and `selected-root` are advertised, despite user/project/wrong-cwd/unselected/new-source sentinels. Native read consumes body, script and asset literals. A real extension factory imports sibling TS and a local dependency and reads an `import.meta.url` asset; expected `PACKAGE-ASSET-482:64` is the worked example 19×3+7. True generated skill links, absence of extension-package links, and unchanged source bytes are checked. Both hosts receive the concrete `relative package/extension/probe.ts` source entry; sibling modules, node_modules and assets load natively from the original directory, without a generated extension manifest. The same Pi launch calls builtin native and added Suite MCP endpoints. |
| `append` | P17/P19: an initially empty project calls native `NATIVE-FILE-113` and centralized `SUITE-FILE-997`; no project MCP file is written. |
| `same-name` | P20: a native file entry beats the same-named registered Suite entry; the losing endpoint has zero initializes/calls. |
| `equivalent` | P20: equivalent URLs under different names retain both Pi servers; each native tool name is called, producing two connections and two reads of the same independent file. Pi's public MCP guide states that the same URL under different names retains separate sign-ins; OMP's URL-based deduplication is not imposed on Pi. |
| `disabled` | P20: a disabled file entry beats an enabled Suite entry; zero connections/calls to that endpoint, plus native disabled status. |
| `empty` | P21: explicit empty Suite MCP list still calls the native file server. |
| `unreadable-skill`, `unreadable-extension` | P46: after successful preview, a Windows exclusive file handle makes SKILL.md or the concrete extension entry truly unreadable (not an arbitrary package asset pre-scan); public launch rejects with the resource identity, no model request and unchanged Herdr tab list. |
| `generation-failure` | P46: after preview a normal file blocks the owned output directory; launch refuses instead of continuing empty. |
| `incompatible` | P46: after preview, the owned persisted Pi Task is explicitly changed to reference an existing OMP Suite; Pi startup rejects before Herdr/model invocation. Suite harness remains immutable. |
| `unreachable` | P46: a syntactically valid native MCP endpoint is unreachable. Normal startup and the working native MCP call still happen; real tool failure and native `/mcp` error/state are retained, not a new all-ready gate. |

MCP endpoints are test-owned HTTP protocol servers that read independent files;
they replace only the external remote service. Pi's builtin MCP, connection
handling, override/equivalence/disable rules, tools and `/mcp` UI remain real.
Initialize/call counts are observed at those endpoints, not at an internal wrapper.
The controlled loopback model chooses predetermined tool calls; this verifies
resource/protocol mechanisms, not model autonomy or model output quality.

Every scenario retains actual HTTP/model requests and tool results, native
terminal/status/history, final artifacts, wire responses, and owned process exit
confirmation. Both stdout/stderr are continuously drained. Primary errors and
cleanup failures are separate. The summary marks a coverage item verified only
when the entire scenario and cleanup succeeded. Nonempty logs, a valid generated
manifest, an unrelated green suite, or a successful CLI configuration test are
not S4 consumption evidence.

Missing centralized MCP reference coverage is intentionally not duplicated: use
`backend/tests/suites/suite_references.rs` test
`missing_central_mcp_is_readable_but_save_and_launch_are_refused` (P18/P46, owner 02).

## Regression and complete delivery checks

The runner's S3 fail-closed regression does not launch agents:

```powershell
$env:HERDR_BOT_WORKTREE = $BOT
$env:SUITE_ACCEPTANCE_TOOLKIT = $TOOLKIT_WT
python "$TESTER/scripts/accept-suite-pi-resources.test.py"
```

From the herdr-bot worktree, record each result separately:

```powershell
Set-Location $BOT
node scripts/test-backend.mjs
npm --prefix frontend test
npm --prefix frontend run test:browser:build
```

From the toolkit worktree, use the prescribed Bun and frozen dependencies. Set
all interop inputs explicitly; `HERDR_BOT_BINARY`/`HERDR_BOT_METADATA` do not set
the separate workflow fixture's source root:

```powershell
Set-Location $TOOLKIT_WT
$env:HERDR_BOT_REPO = $BOT
$env:HERDR_BOT_BINARY = Join-Path $env:CARGO_TARGET_DIR 'debug/examples/safe_shutdown_shell.exe'
$env:HERDR_BOT_METADATA = Join-Path $BOT 'meta-data/task-options.toml'
# If temporary worktrees do not share the recorded sibling baseline checkouts:
$ORIGINAL_TOOLKIT_ROOT = 'D:/sources/original-toolkit-checkout'
$env:OMP_BASELINE_RECORD_ROOT = $ORIGINAL_TOOLKIT_ROOT
bun install --frozen-lockfile
bun run typecheck
bun run check:integrations
bun run test
```

These global checks supplement the scenario matrix; they do not replace it.
The public `--pi-suite-config` factory channel and installed Pi CLI regression
live in toolkit's `extensions/pi-suite/README.md` and `tests/pi-suite/`.
