# 06: Map 配置与 Run 批准事实页面

**What to build:** 用户可在真实 Map 页面配置成果批准，并在 Run 页面分清当前定义、本次或历史执行采用的模式以及已经发生的批准事实。

**Delivers:** P36；P1、P3、P17、P25、P30、P38 的 S3 通用页面分区

**Test owner for:** P1、P3、P17、P25、P30、P38（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** 01：普通工作 node 的批准与显式完成闭环；02：批准的持久化、失败与执行隔离；04：脚本成功后批准，失败不批准

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 真实打包页面保存并重开普通 node 的省略默认、显式自动和人工模式；独立公开读回核对持久定义，不以控件选中代替保存事实（P1）。
- [ ] 同 execution 期间修改 Map，页面分别展示当前配置和冻结模式；retry 后旧／新执行事实各自正确。批准前后及自动／人工来源可区分，自动批准不写成用户 Yes（P3、P36）。
- [ ] 脚本界面只提供自动模式，冲突草稿明确拒绝或要求显式修正；结束节点显示不适用，无伪造 approved=true、无 node 批准等待（P25、P30）。
- [ ] 删除旧 report_policy 控件、展示及其专属测试，更新混合页面场景的有效职责，保留保存冲突、草稿保护和报告／执行展示回归（P17、P38）。
- [ ] 经真实浏览器、当前构建页面及独占生产后端验证，保留截图、同刻公开定义／Run JSON 和外部调用记录；页面观察不产生业务重派。不存在新独立审批页面或私有 UI seam（P36）。
- [ ] 更新用户配置与运行查看说明；不从当前 Map 补造旧 Run 的批准事实，不新增旧运行兼容或批量改写。发布节点职责配置的页面消费测试由 14、15 拥有，不在本票重复认领 P34。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P1 | S3 真实页面配置 | 06 |
| P3 | S3 当前定义／执行事实区分 | 06 |
| P17 | S3 删除页面旧开关 | 06 |
| P25 | S3 脚本页面固定自动 | 06 |
| P30 | S3 结束节点配置／状态不适用 | 06 |
| P36 | S3 通用打包页面与独立公开读回：前后、历史／当前、模式／来源及特殊节点 | 06 |
| P38 | 页面旧审批控件／展示断言 | 06 |

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 本仓检查

从本票 herdr-bot worktree 根运行；先设置指向本 worktree 的独立绝对 `CARGO_TARGET_DIR`。构建当前页面 `npm --prefix frontend run build`，以及当前后端与安全夹具 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`。

完成检查分别运行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。本票新增／更新的具体公开场景必须实际执行并记录用例选择器与证据；全局通过不能替代所认领的场景。涉及业务脚本时必须已有 Bun 并执行真实文件场景；页面票必须观察实际打包页面，类型检查不能替代。受控 Herdr／provider 只证明已标明的机制，不证明原生或真实模型业务。
