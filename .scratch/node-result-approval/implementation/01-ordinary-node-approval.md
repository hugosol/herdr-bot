# 01: 普通工作 node 的批准与显式完成闭环

**What to build:** Map 设计者可配置普通工作 node 的成果批准模式；node 可独立批准本次成果、随时报告，manager 只有在批准和既有交接条件同时满足时才能显式 complete。

**Delivers:** P21、P22；P1、P2、P3、P9、P17、P18、P19、P23、P30、P38 的 S1 基础分区

**Test owner for:** P1、P2、P9、P17、P18、P19、P23、P30、P38（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** None (can start immediately)

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 公开定义保存／读取独立验证省略默认 true、显式 true、显式 false；null、字符串、数字、对象等非法类型拒绝且原定义不变（P1）。
- [ ] 普通 node 派发后 approved=false，即使 auto_approved=true 也不提前批准；批准模式随真实 execution 冻结，模式、事实及自动／人工来源分别表达（P2、P3）。
- [ ] 通过 S1 增加批准能力和角色上下文，身份来自真实会话关联，不让模型选择 Run、node、execution 或收件人；人工 Yes 的原生交互由工具票承担，不把普通 HTTP 调用当作原生确认凭证。向后续票记录实际公开协议与错误语义，不预建私有测试 seam。
- [ ] 无报告、文件、输出和成果说明的合法执行可以批准；不添加 report ID、摘要或成果版本前提，不冻结正文（P9）。
- [ ] 已有可交接报告但未批准时 complete 拒绝，无 completion 或下游派发；批准后其余条件齐备才成功。已批准仍分别拒绝无报告、最新 canComplete=false、约定交付缺失；较早 true 不能覆盖最新 false（P21、P22）。
- [ ] report-only 不改变批准，approve-only 不制造报告／通知；报告允许在批准前后保存和通知。批准不自动 complete、dispatch、finish 或重投，记录真实外部调用对比（P17–P19、P23）。
- [ ] 结束节点仍按原成功发起／显式 finish 规则，无批准门禁，不伪造 approved=true（P30）；chief 确认职责不改。
- [ ] 删除后端普通节点旧 report_policy 审批机制及专属断言，混合用例改测仍有效的报告保存、通知故障、输出与完成职责（P38）。同步更新本票拥有的调用方、夹具及长期协议说明；无旧 Run 升级、字段别名、静默补批准、历史删除或批量改写。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P1 | S1 默认／true／false／非法类型保存读取 | 01 |
| P2 | S1 派发初始未批准 | 01 |
| P9 | S1 无报告／无产物公开批准 | 01 |
| P17 | S1 报告保存／通知及失败职责 | 01 |
| P18 | S1 report-only／approve-only 事实正交 | 01 |
| P19 | S1 批准无通知、正常 report 通知 | 01 |
| P21 | S1 未批准 complete 拒绝、无完成／下游效果，批准后合法成功 | 01 |
| P22 | S1 无报告／最新 false／缺交付及批准后最新 false 的完成拒绝 | 01 |
| P23 | S1 公开批准无 complete／dispatch／finish | 01 |
| P30 | S1 结束节点发起／finish 豁免、无伪批准 | 01 |
| P38 | 后端普通 node 旧报告审批断言 | 01 |

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 本仓检查

从本票 herdr-bot worktree 根运行；先设置指向本 worktree 的独立绝对 `CARGO_TARGET_DIR`。构建当前页面 `npm --prefix frontend run build`，以及当前后端与安全夹具 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`。

完成检查分别运行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。本票新增／更新的具体公开场景必须实际执行并记录用例选择器与证据；全局通过不能替代所认领的场景。涉及业务脚本时必须已有 Bun 并执行真实文件场景；页面票必须观察实际打包页面，类型检查不能替代。受控 Herdr／provider 只证明已标明的机制，不证明原生或真实模型业务。
