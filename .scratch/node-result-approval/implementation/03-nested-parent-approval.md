# 03: 子 manager 独立批准父节点整体成果

**What to build:** 真实子 Run 的 manager 同时以父 node 执行者身份，在子流程完成后独立提交整体批准；内部工作完成不会代替父层交接。

**Delivers:** P29；P10、P28、P38 的 S1 嵌套分区

**Test owner for:** P10、P28、P38（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** 01：普通工作 node 的批准与显式完成闭环

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 通过真实父子 Run 和双向执行关联解析角色；run/collect 仍定位子 Run，批准和外报只定位其真实父 execution，不允许调用者选择其他归属（P10、P28）。
- [ ] 父节点自动／人工模式分别冻结；子流程未完成时拒绝整体批准，完成后按父模式批准。后端测试只验证 S1，人工 Yes 的原生证据由 12 写测试（P28）。
- [ ] 完成实际子节点和子 Run 后，父 execution 仍为未批准；未经独立整体批准不能凭内部批准完成父节点。整体批准后仍由父 manager 满足报告等既有条件并显式 complete（P29）。
- [ ] 旧子 manager 的角色不随父节点 retry 指向新 execution；缺失、错误、多重或跨 Run 关联不得误授权，旧子 Run 事实不改写（P10）。
- [ ] 删除本票拥有的嵌套外报发送审批旧断言，保留子 finish、最新报告、输出绑定、父层完成与通知失败回归；同步更新双角色说明（P38）。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P10 | S1 嵌套双角色关联与归属 | 03 |
| P28 | S1 父子关联、子流程完成条件与父层独立门禁 | 03 |
| P29 | S1 实际子节点和子 Run 完成不批准父 execution | 03 |
| P38 | 嵌套旧外报审批断言 | 03 |

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 本仓检查

从本票 herdr-bot worktree 根运行；先设置指向本 worktree 的独立绝对 `CARGO_TARGET_DIR`。构建当前页面 `npm --prefix frontend run build`，以及当前后端与安全夹具 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`。

完成检查分别运行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。本票新增／更新的具体公开场景必须实际执行并记录用例选择器与证据；全局通过不能替代所认领的场景。涉及业务脚本时必须已有 Bun 并执行真实文件场景；页面票必须观察实际打包页面，类型检查不能替代。受控 Herdr／provider 只证明已标明的机制，不证明原生或真实模型业务。
