# 10: 双宿主成功批准、自由报告与 chief 边界验收

**What to build:** 实际安装的 OMP 和 Pi 都能通过生产工具批准本次成果并自由报告，持久化失败不会被工具说成成功，chief 的原生确认不受 node report 改造影响。

**Delivers:** P2、P4、P6、P9、P12、P13、P18、P19、P23、P37 的 S2 分区；P5 的 Yes／非原生同意分区；P17 的双宿主报告分区；P38 的原生成功场景清理分区

**Test owner for:** P2、P5、P9、P12、P13、P17、P18、P19、P23、P38（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** 01：普通工作 node 的批准与显式完成闭环；02：批准的持久化、失败与执行隔离；05：OMP／Pi 成果批准工具与自由报告

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 两宿主分别派发自动模式普通 node，先公开读回未批准，再实际调用原生批准工具，核对工具结果、approved=true 及自动来源；不以配置或模型复述代替调用（P2、P4）。
- [ ] 两宿主人工模式均观察实际确认界面并真实选择 Yes；普通聊天同意和报告声称批准均不能替代。确认原文严格为“当前会话的成果是否已达到预期，可以交接并推进后续流程？”，无说明／报告／产物参数（P5 Yes／非原生同意、P6）。
- [ ] 无报告、无输出、无文件的合法执行在两宿主自动及人工模式分别批准；模型不提交 execution ID、report ID、正文摘要或版本（P9）。
- [ ] 两种模式、批准前后分别提交进度、问题、失败与成果报告，公开核对保存和通知，原生没有 report 确认；report-only 不批准，approve-only 不制造报告（P17、P18）。
- [ ] 隔离记录批准前后 Run 事实、manager 实际输入和完整外部调用：无批准通知、报告、自动重投、complete、dispatch 或 finish；正常 report 通知仍发生，显式 manager 操作才推进（P19、P23）。
- [ ] 采用 02 的独占真实存储故障，经两宿主原生工具保留持久化原错误，不能声称批准成功；公开仍未批准，后续 complete 拒绝。不得伪造后端响应（P12 S2）。
- [ ] 两宿主分别重复及并发调用批准，核对同一执行批准事实和运行／外部调用无重复业务效果；保留真实并发调用证据，不以串行重复代替并发，不固定私有存储布局（P13 S2）。
- [ ] 执行既有 chief 委派和报告回归，并在两宿主实际观察其原生确认仍存在，不扩大 chief 能力。删除本专项成功场景的旧 node report 确认预期（P37、P38）。
- [ ] 上述 S2 用例由本票唯一拥有，不重复认领 01／02 的 S1 用例；故障可用明确标注的受控 provider 驱动真实安装宿主，不据此声称真实模型完成业务判断。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P2 | S2 双宿主工具批准前后 | 10 |
| P4 | S2 双宿主自动工具及公开批准事实 | 10 |
| P5 | S2 双宿主 Yes、普通聊天与报告声称不能替代 | 10 |
| P6 | S2 双宿主确认原文及无说明参数 | 10 |
| P9 | S2 双宿主两模式无报告／输出及无身份参数 | 10 |
| P12 | S2 双宿主真实故障工具结果不得声称成功 | 10 |
| P13 | S2 双宿主重复／并发工具调用及外部效果 | 10 |
| P17 | S2 双宿主两模式、批准前后四类报告无确认 | 10 |
| P18 | S2 双宿主原生独立调用 | 10 |
| P19 | S2 manager 输入与实际原生调用无批准通知／重投 | 10 |
| P23 | S2 原生批准无隐式推进及实际外部调用 | 10 |
| P37 | 既有 chief 回归及 S2 双宿主委派／报告原生确认 | 10 |
| P38 | 原生成功场景旧 report 确认预期 | 10 |

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 跨仓所有权与必需检查

本票为跨仓联调／验收票，owner 可修改所列两个仓库；拥有本票专项入口与覆盖用例，不只是收集其他票的日志。它与其他票共享普通实现槽位，按 blockers 首次合入即可调度；不等待它们的后台复盘，也不是新的统一最终 gate。

1. 在参与的 herdr-bot worktree 设置独立绝对 `CARGO_TARGET_DIR`，依次运行 `npm --prefix frontend run build`、`cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`、`node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。
2. 在参与的 toolkit worktree 使用已有 Bun 1.4.4／frozen-lock 依赖，依次运行 `bun run typecheck`、`bun run test`、`bun run check:integrations` 及受影响具名原生 dock。配对 dock 的 `HERDR_BOT_BINARY` 是本次安全壳，`HERDR_BOT_METADATA` 是参与 worktree 元数据；生产后端只用于隔离业务专项，不能代安全壳运行关机回归。
3. 本票实现或更新独立专项入口，并交付以下可复跑命令。从参与的 herdr-bot worktree 根运行 `python scripts/accept-node-approval-report.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$AUTHORIZED_MODEL_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。这些变量分别为两个实际票据 worktree、该 worktree 生产后端绝对路径、显式隔离依赖配置、显式授权模型配置及不存在的新证据目录；不得绑定协调仓旧产物或正常服务。入口复用已有验收设施，不新建产品 CLI／外部 seam；两仓源码、锁文件、产物及宿主安装身份都进入证据。
4. 专项必须逐项输出本票 coverage 的原始证据、独立预期和通过／未验证结果。任一仓检查失败、任一宿主或 coverage 缺失、实际模型／授权缺失或清理失败均不能标整票完成；退出非零，保留原始错误。受控机械故障明确标注，不混称真实模型业务。全局 suite、CLI fail-closed、单宿主成功或旧源码证据不替代本票专项。

确认与无自动行为观察使用实际产品现有预算，记录配置与单调时钟窗口及理由，不改产品超时或重试策略。真实界面选择可自动驱动本票拥有的独占终端，但必须观察选中值、原生返回和公开事实，不能把按键写入成功当作 Yes。
