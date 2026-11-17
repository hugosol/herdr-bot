# 02: 批准的持久化、失败与执行隔离

**What to build:** 同次执行的批准可以持久恢复，而失败写入、旧会话、迟到确认和 retry 都不会把批准错误地应用到新工作；等待或故障不阻塞同 Run 的其他合法操作。

**Delivers:** P12、P13；P3、P10、P11、P14、P15、P16、P24、P38 的 S1 隔离分区

**Test owner for:** P3、P10、P11、P12、P13、P14、P15、P16、P24、P38（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** 01：普通工作 node 的批准与显式完成闭环

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 经公开派发后修改 Map，原执行及其合法恢复仍采用冻结模式；合法 fail/retry 创建新身份并采用独立指定的当前模式。新执行 approved=false，旧批准和报告不继承；旧事实保留（P3、P15）。
- [ ] 通过真实普通 node、独立 Chat、根 manager、无关联、其他 Run／node／旧执行比较身份；非法或陈旧身份不能修改当前执行。嵌套双角色业务由 03 唯一拥有（P10）。
- [ ] 将旧执行批准请求保持在途，公开合法 retry 后才允许旧提交结束；新 execution 仍未批准，无新执行副作用。此处是 S1 竞态，不冒充原生 UI（P11）。
- [ ] 用独占真实存储故障使批准持久化失败，保留原错误，不返回批准成功；公开读回仍未批准，其后 complete 仍拒绝。不得 mock 成功响应或通过编辑运行记录制造批准（P12）。
- [ ] 同次重复及并发批准只留下同一批准事实，无重复业务报告、通知、完成、派发或 finish；核对 Run 和实际外部调用，不固定存储布局（P13）。
- [ ] 实际结束并确认后端进程退出，启动不同 PID 后公开读回同次 execution 批准事实及模式；不重放业务输入、不接管旧 Run 或新增恢复权限（P14）。
- [ ] 批准后真实修改材料并追加报告，批准保持，最新报告仍独立影响 complete；不增加摘要匹配、自动撤销、同步或重派（P16）。
- [ ] 批准相关等待不长期占用 Run 操作资源；用公开查询、报告及另一合法分支操作验证有界返回，记录单调时序。真实原生等待证据归 12（P24）。
- [ ] 替换本票拥有的旧策略冻结／恢复／retry 专属断言，保留仍有效的执行隔离、失败与报告回归；更新持久化及错误说明（P38）。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P3 | S1 冻结、配置修改、恢复与新 retry 模式 | 02 |
| P10 | S1 普通／非法／跨 Run、node、旧 execution 身份 | 02 |
| P11 | S1 在途提交与 retry 竞态 | 02 |
| P12 | S1 真实持久化故障、原错误、公开未批准及 complete 拒绝 | 02 |
| P13 | S1 重复／并发批准及业务副作用记录 | 02 |
| P14 | S1 实际后端重启持久读回 | 02 |
| P15 | S1 普通 retry 新执行未批准、不继承 | 02 |
| P16 | S1 真实材料修改与追加报告批准保持 | 02 |
| P24 | S1 批准相关等待的同 Run 并发操作 | 02 |
| P38 | 后端冻结／恢复／retry 旧策略断言 | 02 |

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 本仓检查

从本票 herdr-bot worktree 根运行；先设置指向本 worktree 的独立绝对 `CARGO_TARGET_DIR`。构建当前页面 `npm --prefix frontend run build`，以及当前后端与安全夹具 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`。

完成检查分别运行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。本票新增／更新的具体公开场景必须实际执行并记录用例选择器与证据；全局通过不能替代所认领的场景。涉及业务脚本时必须已有 Bun 并执行真实文件场景；页面票必须观察实际打包页面，类型检查不能替代。受控 Herdr／provider 只证明已标明的机制，不证明原生或真实模型业务。
