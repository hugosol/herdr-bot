# Retro — 07-refresh-from-filesystem-events.md

## Provenance

```json
{
  "feature": "workspace-file-browser",
  "ticket": "07-refresh-from-filesystem-events.md",
  "run_id": "9ea73e09-9c29-47db-8baa-c9ee2aef0cb6",
  "agent": "tdd",
  "agent_id": "07-refresh-from-filesystem-eventsmd-23dd8c8a-563",
  "session_id": "01a15461-d3ce-773c-9427-797a36631c0f",
  "session_file": "C:\\Users\\hugos\\.omp\\agent\\sessions\\--E--Developer-myhub-omp-bot--\\2026-10-19T12-18-55-818Z_01a15419-beca-778c-8f7a-0df59725b49e\\07-refresh-from-filesystem-eventsmd-23dd8c8a-563.jsonl",
  "parent_session_id": "01a15419-beca-778c-8f7a-0df59725b49e",
  "parent_session_file": "C:\\Users\\hugos\\.omp\\agent\\sessions\\--E--Developer-myhub-omp-bot--\\2026-10-19T12-18-55-818Z_01a15419-beca-778c-8f7a-0df59725b49e.jsonl",
  "workspace": "E:\\Developer\\myhub\\omp-bot",
  "execution_cwd": "C:\\Users\\hugos\\AppData\\Local\\Temp\\hyper-ticket-CXofvz",
  "tdd_status": "completed",
  "started_at": "2026-10-19T13:37:39.739Z",
  "tdd_finished_at": "2026-10-19T13:56:47.597Z",
  "retro_finished_at": "2026-10-19T13:57:46.521Z"
}
```

## Retrospective

# 复盘：07-refresh-from-filesystem-events

## 结论

本轮 TDD 实现、自动化验收与真实浏览器验收均完成，提交为 `dcd54ad`。最值得修复的是一处用户可见的重复错误文案；其次是真实浏览器验收缺少可复用配方，以及长套件验证的收尾时序。以下按严重度排列，并区分事实、推断与未验证项。

## 1. [中] 监听失效提示重复“自动更新已停止”

**事实**：真实浏览器输出的提示为“自动更新已停止：workspace 目录已不存在或无法访问，自动更新已停止：系统找不到指定的文件。……”。`frontend/src/workspaceWatch.ts` 的 `describeWatchProblem` 会加一次状态前缀，`backend/src/watch.rs` 的 `watched_identity` 错误文本里又带了一次。

**推断**：功能正确，但最关键的错误提示读起来像拼接故障；证据 `desktop-watch-error.png` 已记录该文本，本轮没有修正。

**建议**：只让一层负责“自动更新已停止”这个状态词。后端只返回原因（目录不存在、被替换、监听错误），前端统一加一次状态；或改为结构化 `{status, cause}`。在 `workspaceWatch.test.tsx` 增加精确文案断言，至少断言该短语只出现一次。

**验证**：前端测试 + 真实浏览器删除目录场景。

## 2. [中] 缺少真实浏览器/交付进程的可复用验收配方

**事实**：为完成 S1 验收，本轮手工完成了前端构建、临时 TOML、临时 workspace 与 Git 仓库、启动后端、在浏览器注册并打开 workspace；还通过一次失败调用才发现 `tab.run` 的隔离运行时里没有 `document`，应使用 `tab.evaluate`。这些步骤散落在工具调用里，没有沉淀到仓库。

**推断**：每个 S1 票据都会重复发现同一套启动、端口、临时仓库和浏览器选择器；这是可避免的导航与工具成本，也容易让不同轮次的证据口径不一致。

**建议**：在 `docs/agents/` 增加一页“真实浏览器验收”配方，或提供 `scripts/acceptance/browser`：固定构建与启动命令、临时配置模板、临时 workspace 与 Git 仓库创建、浏览器打开/注册/截图步骤，以及 `tab.run` 与 `tab.evaluate` 的区别。从 AGENTS.md 或票据模板放一条指针。

**验证**：新代理只读该文档即可复现本轮的 4 张截图。

## 3. [中] 长套件在最后一次编辑前启动，收尾还要重跑

**事实**：完整 `cargo test` 在后台运行约 275 秒；期间又加入了 `manual_read_inside_refresh_window_returns_current_facts_immediately` 测试，完成后不得不重跑 `cargo test --test workspace_watch`（7 passed）。前端全量测试也在 act 修正前后各跑了一次。

**推断**：这不是正确性问题，但会造成“全量通过”的证据与最终工作树不完全对应，并消耗长反馈周期。

**建议**：把“冻结编辑 → 运行唯一一组最终验证 → 提交”写成收尾清单；循环内只跑相关测试，全量套件只在最后运行一次。可以在仓库根提供一个 `check` 脚本统一后端与前端命令（本轮未发现这样的入口）。

**验证**：检查最终提交时间与最后一次全量测试的输入版本一致。

## 4. [中低] 票据状态没有随实现流转

**事实**：票据文件在本轮结束时仍为 `Status: ready-for-agent`，没有写入 `in-progress` 或 `closed`；`docs/agents/triage-labels.md` 定义了这两个实现状态。本轮按用户指示没有修改其他票据，也没有明确被要求改自己的状态。

**推断**：实现代理与宿主/集成代理之间对“谁负责状态”存在未写明的分工；即使宿主会处理，交付说明里也缺少这一项。

**建议**：在票据模板或 AGENTS.md 写明：实现代理开工时置 `in-progress`，合并后由谁置 `closed`，以及是否允许实现代理在未合并时置 `closed`。把它放进 handoff 清单。

**未验证**：宿主是否已经在集成阶段更新该状态。

## 5. [低] MCP 大纲规则在后续大文件读取中没有持续执行

**事实**：本轮先用 `get_file_outline` 查看了 `config.rs`、`lib.rs`、`git.rs`、`WorkspaceBrowser.tsx` 等，但之后直接读取了 `backend/tests/common/mod.rs`、`frontend/src/sseSessionFeed.ts` 等超过 100 行的文件（多为按范围读取），没有再取大纲。该规则来自本轮宿主指令，不是仓库 AGENTS.md。

**推断**：规则对“已知路径、测试辅助文件”的收益低于成本；另一方面，`config.rs` 的 76 条大纲输出也占用了不少上下文。严格执行与完全不执行之间缺少分级。

**建议**：把规则细化为“首次读取未知大文件前”，并允许 `search_graph` 或 `get_code_snippet` 作为等价路径；或对测试辅助文件、已知入口文件豁免，避免重复大纲。

**验证**：无需代码变更；观察后续轮次的工具调用即可。

## 6. [低] `cargo fmt --check` 在基线上不干净

**事实**：运行 `cargo fmt --check` 时，`backend/examples/safe_shutdown_shell.rs` 与 `backend/src/config.rs` 等既有内容已有 rustfmt 差异；本轮新代码也有少量排列差异。为避免无关改动，没有运行 `cargo fmt`。

**推断**：如果仓库期望 rustfmt 作为门禁，基线未清理会让该检查失去信号；如果不期望，运行它只是噪声。

**建议**：二选一：接入 CI 或预提交 rustfmt 并清理基线；或明确记录“本项目不强制 rustfmt”，从验收命令中移除。若 review 代理会检查格式，则只格式化本票改动文件。

**未验证**：本轮没有检查 CI 与预提交配置。

## 7. [低] 浏览器验收调用可合并，测试辅助可复用

**事实**：真实浏览器步骤被拆成多次 eval 调用（打开、注册、打开 workspace、截图、删目录、重建、手动刷新）；一次 `tab.run` 误用 `document` 导致失败。新前端测试最初产生 React `act` 警告，后来用一个局部 `emit` 包装解决；既有 `monitoring.test.tsx` 仍有同类警告。

**推断**：单次脚本化验收与共享的假 SSE 帧发送辅助能减少调用数与警告噪声，尤其在本轮接近 200 次请求软预算时。

**建议**：把浏览器流程写成脚本；在 `tests/` 提供共享的 `act` 包装，或让 fake event source 的 `emit` 自带 `act`。优先级低，但可降低未来 S1 票据的成本。
