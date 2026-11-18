# 12: 双宿主执行隔离、恢复、并发与嵌套批准验收

**What to build:** 真实原生确认、会话恢复和嵌套管理都只批准正确的本次成果；等待一个节点确认时，同 Run 的其他合法工作仍可推进。

**Delivers:** P10、P11、P14、P16、P24、P28 的 S2 分区；P3 的原生恢复分区

**Test owner for:** P3、P10、P11、P14、P16、P24、P28（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** 02：批准的持久化、失败与执行隔离；03：子 manager 独立批准父节点整体成果；05：OMP／Pi 成果批准工具与自由报告

**Repositories:** herdr-bot, toolkit

**Status:** closed

## Acceptance criteria

- [x] 两宿主比较合法普通 node、独立 Chat、根 manager、无关联身份、其他 Run／node／旧 execution 及合法嵌套双角色，公开核对无串用；模型不选择身份（P10）。
- [x] 保持真实人工确认在途，公开合法 fail/retry 创建新 execution，再真实返回旧 Yes；新执行未批准，无新执行报告、完成或派发副作用。原生记录与新旧身份共同证明竞态顺序（P11）。
- [x] 实际结束 node 宿主进程并确认退出，新进程合法恢复同一 Chat；另实际结束并重启后端，核对持久批准与原执行模式，历史／HTTP 证明未重放业务输入。不引入旧 Run 接管（P14）。
- [x] 修改 Map 模式后原生恢复原 execution，工具仍采用冻结模式；新合法 retry 使用当前定义。只认领原生模式消费分区，S1 和页面测试分别归 02、06（P3）。
- [x] 批准后真实修改成果并再次报告，公开批准保持；不要求摘要匹配、不承诺已派发下游自动消费修改（P16）。
- [x] 真实确认保持在途时公开查询、报告并推进无依赖合法分支，各操作在确认结束前有界完成；记录单调时序、进程和 HTTP，不先释放确认再声称不阻塞（P24）。
- [x] 真实父子 Run 完成子流程，子 manager 在自动／人工父模式分别提交整体批准，核对工具双角色、冻结模式及原生 Yes；父 manager 满足报告门禁后另行 complete。内部批准不能代替父批准（P28；P29 S1 测试归 03）。
- [x] 复用既有原生会话设施，不编辑 Run、不用伪身份或受控成功冒充恢复。保存原生身份、确认、实际文件、公开读回及退出证据。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P3 | S2 原生恢复消费冻结模式 | 12 |
| P10 | S2 原生完整角色对比 | 12 |
| P11 | S2 真实在途确认返回旧 Yes | 12 |
| P14 | S2 原生 Chat 恢复及配合后端重启无输入重放 | 12 |
| P16 | S2 原生修改、报告及公开批准保持 | 12 |
| P24 | S2 真实确认在途时查询、报告和独立分支有界完成 | 12 |
| P28 | S2 双宿主父节点两模式工具／确认／公开批准 | 12 |

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 跨仓所有权与必需检查

本票为跨仓联调／验收票，owner 可修改所列两个仓库；拥有本票专项入口与覆盖用例，不只是收集其他票的日志。它与其他票共享普通实现槽位，按 blockers 首次合入即可调度；不等待它们的后台复盘，也不是新的统一最终 gate。

1. 在参与的 herdr-bot worktree 设置独立绝对 `CARGO_TARGET_DIR`，依次运行 `npm --prefix frontend run build`、`cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`、`node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。
2. 在参与的 toolkit worktree 使用已有 Bun 1.4.4／frozen-lock 依赖，依次运行 `bun run typecheck`、`bun run test`、`bun run check:integrations` 及受影响具名原生 dock。配对 dock 的 `HERDR_BOT_BINARY` 是本次安全壳，`HERDR_BOT_METADATA` 是参与 worktree 元数据；生产后端只用于隔离业务专项，不能代安全壳运行关机回归。
3. 本票实现或更新独立专项入口，并交付以下可复跑命令。从参与的 herdr-bot worktree 根运行 `python scripts/accept-node-approval-lifecycle.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$AUTHORIZED_MODEL_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。这些变量分别为两个实际票据 worktree、该 worktree 生产后端绝对路径、显式隔离依赖配置、显式授权模型配置及不存在的新证据目录；不得绑定协调仓旧产物或正常服务。入口复用已有验收设施，不新建产品 CLI／外部 seam；两仓源码、锁文件、产物及宿主安装身份都进入证据。
4. 专项必须逐项输出本票 coverage 的原始证据、独立预期和通过／未验证结果。任一仓检查失败、任一宿主或 coverage 缺失、实际模型／授权缺失或清理失败均不能标整票完成；退出非零，保留原始错误。受控机械故障明确标注，不混称真实模型业务。全局 suite、CLI fail-closed、单宿主成功或旧源码证据不替代本票专项。

确认与无自动行为观察使用实际产品现有预算，记录配置与单调时钟窗口及理由，不改产品超时或重试策略。真实界面选择可自动驱动本票拥有的独占终端，但必须观察选中值、原生返回和公开事实，不能把按键写入成功当作 Yes。

## Comments

- 2026-11-18：用户确认专项单独验证工具，不要求验证模型自主编排调用。嵌套驱动可逐项发送明确用户请求，仍由真实模型调用安装宿主原生工具；不得以直接 HTTP 写入、受控模型返回或伪造 UI 替代。保留身份、冻结模式、原生 Yes、内部成果不替代整体批准、批准不隐式完成以及父 manager 独立 complete 的契约断言。原自主编排失败证据保留，不据此重试原失败决策或改写历史成功标记。
- 2026-11-18：原 worktree `C:/Users/hugos/AppData/Local/Temp/hyper-ticket-5s1ANw` 的 `lifecycle12-combined-native-exact/evidence.json` 已合成完整八格，双宿主 P3/P10/P11/P14/P16/P24/P28 均 verified；原始源码 companion 以记录的 revision/diff 重建并核对字节 SHA256，未改写原始结果。证据均位于该 worktree `.scratch/node-result-approval/evidence/`。
- 2026-11-18：`lifecycle12-final-acceptance/evidence.json` 的完整检查仍 passed=false。前后端构建、前端测试、浏览器测试、toolkit integrations、具名 node-report/nested-role docks、生命周期设施测试均通过。失败为后端 `zone_commit::configured_timeout_stops_git_and_its_children_then_releases_the_repository`（终止后仍 409 git-write-in-progress）、toolkit typecheck 缺少锁定的 TypeScript Windows 平台包、toolkit `model-cost-persistence.test.ts` 两个用例未等到预期 consumption 文档。toolkit 全量实际覆盖244个文件，1930 pass/2 fail，无漏跑文件；后端因失败未运行 doctest。随后以 `bun install --frozen-lockfile --force` 修复本地依赖安装，单独 `bun run typecheck` 已通过（93 files、8 declarations、0 diagnostics）。未重跑或忽略剩余三个失败测试，未修改 Git／成本业务实现，未将原失败 ledger 改绿；尚未提交合并 dev。
- 2026-11-18：用户授权修复上述三个失败用例。Git 修复将 Windows Git 进程树在执行前纳入独占 Job Object，并在超时后确认活动进程数归零才释放写资格；原 commit 与 sync 超时回归通过，日志为 `lifecycle12-final-acceptance/check-git-timeout-repair.log`。与 OMP Git 工具更新按独立问题处理。成本持久化仅修复测试同步：已观察到两次请求数据完整但归属同一 Round，saveVersion=1；先核对第一轮精确持久化事实，再发送第二次请求，保留归档／估算／重启断言及原期限。该文件4/4及typecheck通过，未修改成本生产代码。修复后完整两仓检查及新后端产物的双宿主原生矩阵正在 `lifecycle12-repaired-full/` 运行，最终结果仍待收集。
- 2026-11-18 完成：`lifecycle12-certified/evidence.json` 为 passed=true，八个原生场景齐全，OMP/Pi 的 P3/P10/P11/P14/P16/P24/P28 全部 verified，cleanupErrors=[]。10项仓库检查均成功：9项在产物／安装／路径／源码／命令选择核对后复用，当前生命周期设施测试重新执行成功。原 Pi network timeout 记录保留，用户授权的单独重试成功；未将原失败改绿。
- 两仓已合入 dev：herdr-bot `9b24bb0fcc`（Git进程树修复）、`3de9ddcf3e`（生命周期验收）；toolkit `04f1fc2b2e`（成本持久化测试同步）、`8ebd368626`（真实原生身份故障caller及文档）。通过临时 LF 集成工作区消除提交工具的 CRLF 归属误判；保留 dev 的 handoff 支持并接入 lifecycle，两个原预检冲突的意图均保留。集成后15项生命周期离线回归、3项安装表单观察器回归、handoff/lifecycle CLI边界检查及 toolkit typecheck/authoring 均通过。原 worktree、未提交自动生成元数据和全部原始证据保留；旧工作流快照不再作为重派 ticket12 的依据，不代表其他票或旧工作流已 finish。
