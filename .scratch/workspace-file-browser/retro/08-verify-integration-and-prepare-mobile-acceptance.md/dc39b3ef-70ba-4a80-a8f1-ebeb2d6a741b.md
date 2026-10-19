# Retro — 08-verify-integration-and-prepare-mobile-acceptance.md

## Provenance

```json
{
  "feature": "workspace-file-browser",
  "ticket": "08-verify-integration-and-prepare-mobile-acceptance.md",
  "run_id": "dc39b3ef-70ba-4a80-a8f1-ebeb2d6a741b",
  "agent": "tdd",
  "agent_id": "08-verify-integration-and-prepare-mobile-accepta",
  "session_id": "01a154a7-66a2-77e3-b18a-a181d39f3cca",
  "session_file": "C:\\Users\\hugos\\.omp\\agent\\sessions\\--E--Developer-myhub-omp-bot--\\2026-10-19T12-18-55-818Z_01a15419-beca-778c-8f7a-0df59725b49e\\08-verify-integration-and-prepare-mobile-accepta.jsonl",
  "parent_session_id": "01a15419-beca-778c-8f7a-0df59725b49e",
  "parent_session_file": "C:\\Users\\hugos\\.omp\\agent\\sessions\\--E--Developer-myhub-omp-bot--\\2026-10-19T12-18-55-818Z_01a15419-beca-778c-8f7a-0df59725b49e.jsonl",
  "workspace": "E:\\Developer\\myhub\\omp-bot",
  "execution_cwd": "C:\\Users\\hugos\\AppData\\Local\\Temp\\hyper-ticket-YpcgHx",
  "tdd_status": "completed",
  "started_at": "2026-10-19T14:53:39.266Z",
  "tdd_finished_at": "2026-10-19T15:16:24.579Z",
  "retro_finished_at": "2026-10-19T15:17:34.885Z"
}
```

## Retrospective

# 复盘：ticket 08「完成跨流程验收与手机实机证据包」

## 结论摘要

本次交付最终完成：新增 3 个后端跨流程用例、4 个前端跨流程用例，全量后端 `286 passed / 34 suites`、前端 `21 files / 150 tests passed`，并用真实 Chromium + 隔离仓库/bare 远端产出 22 张 H1 证据截图和可复现场景脚本；提交为 `4f85ff5`，票据保持 `in-progress` 等待手机实机 H1。

值得改进的环境问题集中在四处：

1. **构建与测试没有互斥**：`npm run build` 与 `cargo test` 并发时，依赖 `backend/static` 的 mDNS 用例得到 404，制造了一次完整的假红。
2. **单文件 `rustfmt` 会越界格式化子模块**：`rustfmt backend/tests/workspace_integration.rs` 同时改写了 `tests/common/mod.rs` 与 `tests/common/sse.rs`；同时仓库当前并不是 `cargo fmt --check` 干净的。
3. **验证型票据缺少导航与快速/最终门禁约定**：需要自己从 1810 行 README 和大量测试文件中重建覆盖矩阵、测试命令与真实浏览器证据流程。
4. **Windows PowerShell 5.1 的脚本编码没有约定**：UTF-8 无 BOM + 中文注释直接解析失败，只能重写为 ASCII。

下面按严重度列出建议。事实、推断、未验证项分开标注。

## 事实基线

- 本次新增：
  - `backend/tests/workspace_integration.rs`（P42、P46、P3，`3 passed`）
  - `frontend/tests/integration.test.tsx`（P3、P41，`4 passed`）
  - `.scratch/workspace-file-browser/evidence/08-verify-integration-and-mobile-acceptance/`（22 张截图、2 个 PowerShell 场景脚本、H1 检查清单）
- 验证结果：
  - `cargo test --no-fail-fast` → `286 passed / 34 suites`，0 failed。
  - `npm --prefix frontend run test` → `21 files / 150 tests passed`。
  - `npm --prefix frontend run build` → 成功。
- 真实浏览器：
  - 窄屏 `390×844` 视口仿真，宽屏 `1280×900`；隔离仓库与 bare 远端；未执行真实关机、未推送业务远端。
  - P41 实测：首个本地事件 23ms 自动刷新；窗口内第二个事件 1.5s 后仍未自动显示；回到页面 81ms、手动刷新 56ms、重新进入 workspace 14ms 显示当前事实。
  - P42 实测：4 次本地事件后远端跟踪引用仍为旧值、`remoteCheckedAt=null`；显式检查远端后才更新。
- 一次假红：首次全量 `cargo test` 与 `npm run build` 并发运行，`mdns_conflict`（2 例）与 `mdns_failure`（1 例）因入口返回 `HTTP 404` 失败；构建完成后单独重跑 `3 passed`。
- 一次格式化越界：`rustfmt --edition 2024 backend/tests/workspace_integration.rs` 修改了 `backend/tests/common/mod.rs` 与 `backend/tests/common/sse.rs`；这些改动已用 `git checkout --` 还原，未进入提交。
- `cargo fmt --manifest-path backend/Cargo.toml -- --check` 当前会在多个既有文件（`backend/examples/safe_shutdown_shell.rs`、`backend/src/commit.rs`、`backend/src/config.rs`、`backend/tests/workspace_watch.rs` 等）报告格式差异。
- `frontend/package.json` 只有 `dev` / `build` / `preview` / `test` / `test:watch`，没有 lint 脚本；未检查 CI 配置，因此不能断言仓库没有 CI 门禁。

## 建议（按严重度）

### 1. 给“构建静态页面 + 后端测试”建立单一顺序门禁，并隔离静态目录依赖

**事实**：`mdns_conflict` 与 `mdns_failure` 会请求 `/` 并期望打包页面；`npm run build` 会替换 `backend/static`。本次两者并发时得到 404，串行重跑通过。

**推断**：`ServeDir` 在 `vite build` 替换静态目录的窗口内找不到 `index.html`，于是返回 404；这是构建/测试互斥问题，不是 mDNS 回归。

**建议**：

- 增加一个顶层 `check` 命令（脚本或 `justfile`），固定顺序为：
  1. `npm --prefix frontend run build`
  2. `cargo test --no-fail-fast --manifest-path backend/Cargo.toml`
  3. `npm --prefix frontend run test`
- 在测试侧把“页面入口”改成不依赖可写构建产物：测试启动时指向一个只读夹具静态目录，或让 mDNS 用例只断言监听/服务可用而不读取真实打包页面。
- 若短期不隔离，至少在 README/AGENTS 的测试说明中写清“构建与测试不得并发”，并让顶层命令成为唯一推荐入口。

**预期效果**：消除整轮假红；agent 不需要靠重跑判断 404 是环境还是产品问题。

**证据状态**：失败与重跑均是本次实际观察；静态目录替换是推断，未通过注入构建延迟或检查 `ServeDir` 日志进一步验证。

### 2. 约束单文件格式化的边界，并明确 rustfmt 基线

**事实**：`rustfmt` 会沿着 `mod common;` 格式化子模块；本次因此改写了两个无关文件。仓库当前的 `cargo fmt --check` 又不干净，无法直接作为门禁。

**建议**：

- 在 AGENTS 或测试文档中写明：只格式化新文件时使用
  `rustfmt --config skip_children=true <file>`（已验证该参数对本次新文件可产生干净结果）。
- 或者把格式检查收敛为“只检查变更文件”，并固定 rustfmt 版本；否则现有全仓 `cargo fmt --check` 会一直红。
- 在提交前增加 `git diff --name-only` 的机械检查，确认格式化没有触碰预期之外的文件。

**预期效果**：避免把无关格式化带进票据提交，也避免“格式门禁永远红 → 没人看”的 sediment。

**证据状态**：越界修改与 `--check` 失败均为实际观察；CI 是否已固定 rustfmt 版本未验证。

### 3. 增加 `docs/agents/testing.md`（或等价测试导航），并在 AGENTS.md 放一条指针

**事实**：本次为确定“01–07 的自动检查是否都覆盖、该跑哪些套件、真实浏览器证据放在哪”，需要从 1810 行 README、`contract.md` 覆盖表、`backend/tests/*` 文件名与 `frontend/tests/*` 用例名中重建；没有一份面向 agent 的测试/验收导航。

**建议**：新增一份短的测试导航文档，至少包含：

- **命令矩阵**：快速循环（单个 `--test` / `npx vitest run <file>`）与最终门禁（全量后端、全量前端、build）。
- **承诺 → 套件映射**：P3/P41/P42/P46 等对应哪个测试文件；01–07 各自新增的套件与证据目录。
- **真实浏览器证据流程**：如何启动隔离场景、如何保存截图、窄屏/宽屏视口、已知的 viewport 共享与截图超时处理。
- **禁止事项**：构建与测试并发、单文件 rustfmt 越界、真实关机/业务远端推送。

在根 `AGENTS.md` 加一条类似“写测试、跑验收或准备浏览器证据前，读 `docs/agents/testing.md`”的指针。当前 AGENTS.md 只有三条指针，体量很小，增加这一条不会造成上下文负担。

**预期效果**：后续集成/验收票直接按图索骥，减少探索与漏跑；也把本次新建的 H1 场景脚本变成可发现的资产。

**证据状态**：导航缺失是本次实际体验；该文档能否显著缩短后续耗时是推断，未做对照测量。

### 4. 为“验证型票据”定义快速循环、突变证明与最终门禁

**事实**：本次新增的跨流程用例在实现已存在的前提下直接通过；没有对每条新拥有的承诺做临时突变来证明测试能抓到违例。全量后端套件约 285 秒，本次跑了两次（一次基线、一次最终）。最终提交前没有在最后一次格式化/还原之后重跑全量。

**建议**：

- 在 TDD/验收流程中为“验证型票据”增加固定步骤：
  1. 先写/跑针对性用例（快速循环）；
  2. 对每条本票唯一拥有的承诺做一次临时突变（例如：让 watch 刷新调用 `check-remote`、去掉局域网来源校验、让回到页面不刷新），确认对应用例失败，再还原；
  3. 全部改动与格式化完成后，只跑一次全量门禁。
- 把“最终全量必须在最后一次编辑之后”写成完成条件，避免出现本次这种“全量通过 → 仅空白格式化 → 只跑针对性用例”的证据时间差。

**预期效果**：提高跨流程用例的判别力证据；减少全量套件的重复成本；最终报告的全量结果与提交树严格对应。

**证据状态**：未做突变是事实；“突变能抓住违例”未验证，但既有票据（01–07）曾用同样方法，属于可复用的项目实践。

### 5. 统一 PowerShell 脚本编码规则，或改用 `pwsh`

**事实**：Windows PowerShell 5.1 将 UTF-8 无 BOM 的 `.ps1` 按 ANSI 读取，中文注释与字符串导致解析错误；改写成 ASCII 后脚本一次通过。

**建议**（任选其一）：

- 在仓库约定：`.ps1` 必须是 ASCII-only，或保存为 UTF-8 with BOM；在 `docs/agents/testing.md` 与脚本模板中注明。
- 增加一个轻量检查：扫描 `*.ps1` 是否含非 ASCII 字节且无 BOM。
- 若环境允许，统一用 `pwsh`（PowerShell 7，默认 UTF-8）运行证据脚本，并在 README 标注版本要求。

**预期效果**：避免证据脚本在用户/CI 环境出现“本机能跑、换机解析失败”的编码问题。

**证据状态**：解析失败与 ASCII 重写成功均为实际观察；该编码规则是否适用于仓库其他脚本未全面检查。

### 6. 把当前行为与历史变更记录分开，或至少在 README 顶部加导航

**事实**：README 已到 1810 行，混合了当前使用说明、配置、测试命令、接口说明和从 v1 到本票的连续变更记录；本次查找“当前 workspace 行为”“测试命令”“mDNS 失败原因”时主要靠 `grep` 标题与分段阅读。

**建议**：

- 把 `## 变更记录：…` 系列移到 `docs/changelog.md` 或 `.scratch/<feature>/` 证据索引，README 只保留当前行为与一个指向变更记录的指针；或
- 在 README 顶部增加一个简短的目录/导航块，明确“当前行为 / 配置 / 测试 / 变更记录 / 人工验收”的入口。

**预期效果**：降低新 agent 的检索成本，减少把历史记录误当当前行为的风险；也让 H1 交接文档更容易被找到。

**证据状态**：检索成本是本次实际体验；“拆分后更快”是推断，未测量。

### 7. 票据模板区分“agent 侧验收”与“人工 H1”，避免全勾选与 in-progress 并存

**事实**：本票的 10 条验收项在 agent 侧全部完成后被勾选，但票据同时保持 `in-progress` 等待手机实机；我额外加了一行说明“勾选只表示 agent 侧完成”。这种表达对后续读者仍有歧义。

**建议**：

- 在实现票模板中增加独立字段，例如 `Agent-side acceptance: complete / H1: pending`；
- 或者把人工验收项保留为未勾选，另建“agent 证据已完成”小节；
- 在 `triage-labels.md` 中明确：`in-progress` 可以表示“agent 工作完成、等待人工确认”，但不能被当作 `closed`。

**预期效果**：状态与勾选语义一致，集成/评审 agent 不会误判本票已完全关闭。

**证据状态**：本次票据的实际状态是事实；模板改动的效果未验证。

### 8. 把真实浏览器证据流程做成可复用脚本/清单，减少工具试错

**事实**：本次在 `browser` 工具上遇到并解决了：

- `wait_until: 'networkidle'` 不被接受，需用 `'load'`；
- `page.screenshot({path})` 在 35 行变更列表页面多次 30s 超时，先调用 `tab.screenshot({silent:true})` 解冻后再保存成功；
- 打开 1280 宽的新 tab 会把已存在的 390 宽 tab 的 `window.innerWidth` 改成 1258（视口似为共享实例级），导致一张“窄屏”截图实际是宽屏，必须关闭宽 tab 并重新 `emulate`；
- Puppeteer 子集没有 `locator().nth`，改用 `page.evaluate` 点击第 N 个元素。

**建议**：

- 在测试导航文档中记录这些坑与推荐写法；
- 或提供一个小型证据脚本（打开页面、设置视口、截图到指定目录、必要时先解冻），作为 H1/验收票的公共工具；
- 明确“每个 tab 截图前重新断言 `window.innerWidth`”作为证据完整性检查。

**预期效果**：减少截图超时、视口串台和证据重拍；让证据包更可复现。

**证据状态**：上述现象均为本次实际观察；超时的根因（idle freeze / renderer 状态）未验证，不能断言是工具缺陷。

### 9. 低优先级：把“考虑 codebase-memory-mcp”改成条件式规则

**事实**：本次是全新 worktree，未建立 codebase-memory 索引；我评估后直接用 `grep`/`read`/`find`，没有调用 codebase-memory-mcp。现有指令“调用 grep 或 search 前，考虑是否使用 codebase-memory-mcp 更合适”在本会话中没有改变行为。

**建议**：改成可执行的条件式规则，例如“如果当前仓库已在 codebase-memory 中有索引，优先 `search_graph`；否则用 `grep`/`find`，不为一次性检索新建索引”。否则它更像一条 no-op，持续占用全局指令预算。

**预期效果**：减少无动作的全局指令；让 agent 在真正有索引的仓库里才投入索引工具。

**证据状态**：本次未使用该 MCP 是事实；“改成条件式会更好”是推断，未做 A/B。

## 未验证与不补造

- 未检查仓库 CI 配置与 pre-commit hook 的现状；只确认了 `frontend/package.json` 没有 lint 脚本、`cargo fmt --check` 当前不干净。
- 未在最终提交后再次运行全量后端套件；全量结果来自格式化前的同一行为树，格式化仅涉及新测试文件的空白变化，之后针对性用例通过，但严格说最终树没有“提交后全量”证据。
- 未做新用例的突变验证；因此“这些用例能抓住对应违例”只有既有票据的方法论支持，本票没有独立证据。
- 未验证 `ServeDir` 404 的直接原因、浏览器截图超时的直接原因、共享视口是否属于工具设计；这些均为基于现象与重跑的推断。
- 真实手机、软键盘、触摸与 H1 全部项目未验证；本报告不把 390×844 视口仿真当作实机结论。
- 未检查 H1 场景脚本在另一台 Windows 机器或 PowerShell 7 下的行为；只在本次 Windows PowerShell 5.1 环境实际运行过 `prepare-scenario.ps1`，`start-server.ps1` 未单独执行（直接运行了同一 exe 与配置）。
