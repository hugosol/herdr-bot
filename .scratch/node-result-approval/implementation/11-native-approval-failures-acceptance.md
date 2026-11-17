# 11: 双宿主批准拒绝、取消、超时与无 UI 验收

**What to build:** 用户未在原生确认中明确批准时，节点始终可以继续工作，不会被标为失败，也不会自动再次弹出确认。

**Delivers:** P7、P8；P5 的非 Yes 分区；P38 的原生异常场景清理分区

**Test owner for:** P5、P38（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** 01：普通工作 node 的批准与显式完成闭环；05：OMP／Pi 成果批准工具与自由报告

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

## Acceptance criteria

- [ ] OMP 和 Pi 各自覆盖 No、Esc／原生取消、真实时钟超时及原生 UI 不可用四类结果。保留界面／原生请求及真实返回或异常，不能直接 mock 业务确认结果（P5 非 Yes、P7）。
- [ ] 每种结果核对工具反馈、公开 approved=false 和 node 未因确认失败成为 failed；实际继续修改材料、发送报告并读回，再由主动新请求重新确认，证明可继续工作（P7）。
- [ ] 每个拒绝／异常返回后持续观察原生调用、确认请求及公开状态，无系统自动新确认或批准；根据已有确认／传输／调度预算说明有界窗口并记录实际持续时间，不以即时断言或延长产品超时凑通过（P8）。
- [ ] 区分系统自动重试和窗口结束后测试主动发起的新请求；人工模式报告自由发送，不把重新请求批准变成报告前提。
- [ ] 删除本专项异常场景旧 report 发送审批预期，改测成果批准拒绝和继续工作；报告不得因 No／取消／超时而被拦截（P38）。
- [ ] 无 UI 可用受控 provider 配合真实安装宿主制造机械故障，必须标明边界；不以受控结果冒充真实模型选择，不以单宿主抵消另一宿主。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P5 | S2 双宿主 No／取消／超时／无 UI | 11 |
| P7 | S2 双宿主四类非批准结果、非失败、继续工作与主动再请求 | 11 |
| P8 | S2 四类结果后有依据的无自动重试窗口 | 11 |
| P38 | 原生异常场景旧 report 确认预期 | 11 |

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 跨仓所有权与必需检查

本票为跨仓联调／验收票，owner 可修改所列两个仓库；拥有本票专项入口与覆盖用例，不只是收集其他票的日志。它与其他票共享普通实现槽位，按 blockers 首次合入即可调度；不等待它们的后台复盘，也不是新的统一最终 gate。

1. 在参与的 herdr-bot worktree 设置独立绝对 `CARGO_TARGET_DIR`，依次运行 `npm --prefix frontend run build`、`cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`、`node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。
2. 在参与的 toolkit worktree 使用已有 Bun 1.4.4／frozen-lock 依赖，依次运行 `bun run typecheck`、`bun run test`、`bun run check:integrations` 及受影响具名原生 dock。配对 dock 的 `HERDR_BOT_BINARY` 是本次安全壳，`HERDR_BOT_METADATA` 是参与 worktree 元数据；生产后端只用于隔离业务专项，不能代安全壳运行关机回归。
3. 本票实现或更新独立专项入口，并交付以下可复跑命令。从参与的 herdr-bot worktree 根运行 `python scripts/accept-node-approval-failures.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$AUTHORIZED_MODEL_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。这些变量分别为两个实际票据 worktree、该 worktree 生产后端绝对路径、显式隔离依赖配置、显式授权模型配置及不存在的新证据目录；不得绑定协调仓旧产物或正常服务。入口复用已有验收设施，不新建产品 CLI／外部 seam；两仓源码、锁文件、产物及宿主安装身份都进入证据。
4. 专项必须逐项输出本票 coverage 的原始证据、独立预期和通过／未验证结果。任一仓检查失败、任一宿主或 coverage 缺失、实际模型／授权缺失或清理失败均不能标整票完成；退出非零，保留原始错误。受控机械故障明确标注，不混称真实模型业务。全局 suite、CLI fail-closed、单宿主成功或旧源码证据不替代本票专项。

确认与无自动行为观察使用实际产品现有预算，记录配置与单调时钟窗口及理由，不改产品超时或重试策略。真实界面选择可自动驱动本票拥有的独占终端，但必须观察选中值、原生返回和公开事实，不能把按键写入成功当作 Yes。
