# 14: 运行记录读取与部分提交故障

**What to build:** 调用者能够分别读取当前状态、执行历史与报告；写当前状态失败时不采用未提交事实，历史追加失败时明确知道状态已经生效。

**Delivers:** P43, P44, P46

**Blocked by:** 10 — collect 通知保留、有界等待与取消；13 — retry 独立新执行与报告适用性

**Status:** in-progress

## 契约与范围

依据：[approved contract](../contract.md) 与其继承的 [spec](../spec.md)。P 编号与 S1–S4 的含义以 contract 为准；本票不重新定义 promise 或新增观察 seam。

完善已有运行路径的提交语义，不另建平行存储。三类记录独立可读，当前查询不靠历史重放；旧执行、失败、恢复、完成说明保留。失败只中止当前调用或按批次规则处理，不自动持续封锁 Run。

仅交付本票的覆盖分区；共享 promise 不因某一分区通过就算整体完成。实现前沿是所有列出的阻塞票均完成的票，不按编号强制全串行。

## 验收标准

- [x] **P43.01**（本票编写并执行的契约检查）：注入真实可重复当前状态写入失败，联合检查返回、后续查询、磁盘及成功历史：内存和成功历史不采用未提交状态。
- [x] **P44.01**（本票编写并执行的契约检查）：在当前状态提交后使历史追加失败，核对已提交状态、缺失的新历史与明确部分生效错误；不回滚，后续合法调用不被持续封锁。
- [x] **P46.01**（本票编写并执行的契约检查）：独立准备及真实操作产生的当前状态、执行历史、报告分别可读；查询不靠历史重放，必要原因/说明与旧执行保留；通知派生索引不成为第四份业务记录。
- [x] 完成上述能力所需调用者、错误反馈和角色/用户指引；不留占位实现、假成功、运行时旧格式兼容或未接线入口。
- [x] 保留本票真实入口的可重复验证证据及检查结果；相关长期说明随能力更新，不只更新临时票据。

## 验证与证据

对实际状态动作注入可重复文件写入故障，联合观察 HTTP/工具返回、后续查询和磁盘；另在状态提交后使历史追加失败，再执行后续合法调用。

遵守仓库测试指引：复用真实 HTTP TestServer、现有前端/npm 浏览器入口及 toolkit 公共 interface；页面能力观察实际交付页面，宿主能力需要真实 Herdr/omp 证据。受控 adapter 只证明确定性故障/时序，不证明模型或原生宿主行为。

普通检查只用数值回环、独占临时数据/cwd/动态端口与必要 Herdr 副本，持续消费并清理所属子进程；不操作用户对话或产物，不绕过 npm 网络护栏。LAN/mDNS、真实符号链接和关机仍遵守显式人工授权规则。

自动化预期取自契约、独立字面样例或预先算定结果，不用被测实现重算预期。负向自动行为检查注明有界观察窗口并保留外部调用证据；取消/错误不得伪造快照或自动重试。真实环境缺失如实报告阻塞，不以替身或跳过宣称通过。

## Comments

本 worktree 已实现并按 S2/S4 验证，待宿主排队合入集成分支；未合入前维持 `in-progress`，不把 agent 完成等同于 `closed`。

### 当前开发分支的独立集成验收（2026-10-29）

- 接收集成提交 `d7ce8a31a7e680f0eb7fb8daed1237a578478feb` 后，解决 `backend/tests/workflow_dispatch.rs` 的追加模块冲突，同时保留本票 `records` 与角色安全票 `security`，不删除任何一侧覆盖。
- `npm --prefix frontend run build` 通过，提供后端测试必需的静态资源；`npm --prefix frontend test -- --maxWorkers=2` 通过，27 个测试文件、205 项测试全部成功，npm 网络护栏通过。
- 完整后端运行使用仓库允许的低并发入口，外层和目标内部均串行：`node --input-type=module -e "process.env.CARGO_TARGET_DIR='C:/Users/hugos/AppData/Local/Temp/hyper-ticket-hj8MCI-ticket14-preserved/target-ticket14'; const {main}=await import('./scripts/test-backend.mjs'); process.exitCode=await main(['--binary-jobs','1','--test-threads','1','--output-dir','C:/Users/hugos/AppData/Local/Temp/hyper-ticket-hj8MCI-ticket14-preserved/integration-global-tests']);"`。
- 本轮完整后端 build、全部 binary 与 doctest 均通过，0 个目标未派发，用时 750.9 秒；`workflow_dispatch` 为 53 passed，包含 records 与 security；此前失败的 `zone_commit` 本轮亦通过。没有修改或放宽其断言；本轮低并发通过不证明此前失败根因已定位或并发条件下问题已修复。
- 本轮汇总和逐目标原始日志保留于 `C:/Users/hugos/AppData/Local/Temp/hyper-ticket-hj8MCI-ticket14-preserved/integration-global-tests/`，不改写下列历史失败证据。按此次接管范围未执行浏览器/实机 smoke、复杂脚本或跨票据整体验收，它们留给最后独立门禁。

- P43.01：`records::current_write_failure_preserves_queries_disk_and_success_history` 对 pause/fail/complete 注入真实 Windows 只读当前文件，联合检查 HTTP 错误、故障期间 status、磁盘及成功历史，并在恢复写权限后执行合法动作。首次即绿，证明已有提交语义，无制造无关 red。
- P44.01：`records::history_failure_reports_committed_completion_and_allows_next_action` 与 `records::batch_history_failure_explains_each_committed_target_without_stopping_batch` 分别注入历史只读，核对完成/批次/失败 retry 已提交状态、缺失历史及下一次合法动作。两项先在缺少明确部分生效 code 的断言处 red，再补齐共享反馈转 green；保留 `historyError` 并新增 `code=history-append-failed/stateCommitted=true/message`。
- P46.01：`records::independent_records_read_current_facts_without_replaying_history` 独立字面样例与 `records::operated_records_preserve_old_execution_recovery_completion_and_reports` 真实 HTTP 操作均通过；核对 current/history/report 独立读取、旧执行/失败/恢复/完成说明、旧报告不支配新执行、collect 不改三文件及无第四业务文件。此前行为已有，首次即绿。
- 上述五项均分别通过 `cargo test --manifest-path backend/Cargo.toml --target-dir .scratch/target-ticket14 --test workflow_dispatch records::<测试名前缀> -- --nocapture` 实际执行。原始工具输出分别为 `artifact://517`、`artifact://525`、`artifact://539`、`artifact://546`、`artifact://550`。
- 长期说明更新在 `docs/workflow-runtime.md` 的“独立读取与部分提交”：包括真实读取入口、每项部分生效反馈、用户/角色责任、可重复命令与证据边界。没有新增存储、日志浏览 API 或故障专用公开入口。
- 全后端入口首次在 `herdr_config` 因本 worktree 缺少已构建 `backend/static/index.html` 中止，39 目标未派发、doctest 未执行；完整日志 `.scratch/ticket14-verification/summary.json`。已执行 `npm --prefix frontend ci --offline` 及 `npm --prefix frontend run build`，补齐真实构建前置后重新运行完整后端：build 通过，55 个 binary 通过、1 个失败、5 个未派发，doctest 未执行。`workflow_dispatch` 全部 **50 passed**（含本票 5 项），日志 `.scratch/ticket14-verification-prepared/binary-050.stdout.log`。全局失败是未修改的 `zone_commit::configured_timeout_stops_git_and_its_children_then_releases_the_repository` 等待 10 秒仍未观察到 index.lock/staging 的断言；该目标 18 passed/1 failed，日志 `binary-053.stdout.log`。未重跑掩盖此失败，不声称已证明它在改动前存在；保留为集成回归未通过项。汇总 `.scratch/ticket14-verification-prepared/summary.json`。完整命令：`node --input-type=module -e "import path from 'node:path'; process.env.CARGO_TARGET_DIR=path.resolve('.scratch/target-ticket14'); const {main}=await import('./scripts/test-backend.mjs'); process.exitCode=await main(['--binary-jobs','2','--output-dir','.scratch/ticket14-verification-prepared']);"`。
- 本票确定性故障只替换外部 Herdr CLI，TCP 与文件操作真实；不宣称原生 Herdr/omp、外部 toolkit 角色工具或公共 Prompt 已验证。这些不是本票 P43/P44/P46 的 S2/S4 分区证据的替代物。
- 拒收后的交接修复：保留实现提交 `e7123aa` 与以上全部验证，不重复执行已完成检查。为交出干净 worktree，原 `.scratch/target-ticket14/`、`.scratch/ticket14-verification-prepared/`、`.scratch/ticket14-verification/` 已原样移至 worktree 外 `C:/Users/hugos/AppData/Local/Temp/hyper-ticket-hj8MCI-ticket14-preserved/` 下的同名目录，没有删除产物或改写日志。上文命令及日志内绝对路径是当时运行记录；现在应从该保留目录读取对应证据。票据既有验收记录一并提交，状态仍待独立集成后关闭。
