# 04: 脚本成功后批准，失败不批准

**What to build:** 脚本型 Bot 无需 Chat 或人工批准，只有本次真实 Bun 执行成功并通过结果校验才自动获批，交接仍由 manager 决定。

**Delivers:** P26、P27；P15、P25、P38 的 S1 脚本分区

**Test owner for:** P15、P25、P38（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** 01：普通工作 node 的批准与显式完成闭环

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 公开定义保存和预检拒绝脚本人工批准模式，合法自动配置保存读回；不静默改写冲突值，不产生无 Chat 可处理的人工等待（P25）。
- [ ] 真实 Bun 及实际文件分别验证成功、异常退出、显式 failure、结果形状非法、缺输出、重复／未声明 ID、缺失／不可读／目录假输出。仅正常退出、有效 success 和既有完整输出校验同时满足才批准；失败不批准且报告与真实部分效果一致（P26）。
- [ ] 脚本成功后自动报告并记录自动来源批准，但 node 尚未 completed，既无隐式下游派发也无 finish；公开 manager complete 后才交接（P27）。
- [ ] 合法 fail/retry 保留旧执行，新 execution 初始未批准，直到本次成功才批准；旧执行或旧输出不能放行新执行（P15）。
- [ ] 不创建或伪造 Task／Chat／Session、人工 Yes 或脚本恢复入口；报告保存／通知失败仍按原职责反馈，不回滚真实业务效果。
- [ ] 移除脚本旧报告策略约束断言，改测自动批准约束；保留输出、生命周期及 manager 完成回归，更新脚本表示和编写说明（P38）。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P15 | S1 脚本 retry 仅本次成功后批准 | 04 |
| P25 | S1 脚本定义拒绝人工、自动保存读回 | 04 |
| P26 | S1 真实 Bun 成功／异常／failure／缺失与非法输出及批准报告事实 | 04 |
| P27 | S1 脚本成功仍待 manager 显式 complete、无下游隐式派发 | 04 |
| P38 | 脚本旧报告策略断言 | 04 |

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 本仓检查

从本票 herdr-bot worktree 根运行；先设置指向本 worktree 的独立绝对 `CARGO_TARGET_DIR`。构建当前页面 `npm --prefix frontend run build`，以及当前后端与安全夹具 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`。

完成检查分别运行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。本票新增／更新的具体公开场景必须实际执行并记录用例选择器与证据；全局通过不能替代所认领的场景。涉及业务脚本时必须已有 Bun 并执行真实文件场景；页面票必须观察实际打包页面，类型检查不能替代。受控 Herdr／provider 只证明已标明的机制，不证明原生或真实模型业务。
