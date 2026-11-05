# 03: 配置两类 Bot 目标并验证可调用关系

**What to build:** 用户可在主界面保存 Task 型、Map 型或未绑定 Bot；节点可声明 manager Task，公开预检检查合法调用关系，删除 Map 时保护能力引用。

**Delivers:** P01, P02, P07, P37

**Blocked by:** None (can start immediately)

**Repositories:** herdr-bot

**Status:** ready-for-agent

依据：[批准契约](../contract.md)；[规格](../spec.md)。契约 Approval 已获批准，本轮 24 票修订方案由用户授权发布。发布不代表实现、自动验收或用户部署操作已完成。

## Acceptance criteria

- [ ] 普通 normal Task 与 Map 两种目标分别保存、预览和重载；双绑定拒绝且原定义不变，manager Task 不能成为普通 Task 型目标。
- [ ] 未绑定 Bot 可保存；未绑定、失效目标或不合法 manager 配置在运行前明确拒绝且无宿主启动。不得静默改绑或按隐含优先级执行。
- [ ] Map 型节点指定 manager Task，Bot 本身不固化 manager；页面与严格公开定义形状同步支持，保留 normal／manager 职责区分。
- [ ] 公开预检接受 A→B→C 及不同节点重复引用 B；拒绝 A→A、A→B→A 并指出调用链，无启动副作用。
- [ ] 循环检查按当前调用路径判断，不以全局已访问集合误拒绝合法重复；不展开子图或引入定义快照系统。
- [ ] 被 Bot 引用 Map 删除拒绝并定位引用；无引用可按既有路径删除，取消／拒绝不删除、不自动解绑。
- [ ] 引用扫描覆盖损坏、不可读定义及既有链接安全边界；无法可靠核实不能视为无引用。
- [ ] 所有受影响调用方与既有行为检查同步切换，删除被本次替代的死路径，不引入旧字段别名或以兜底吞掉失败。
- [ ] 更新所属长期使用／运行／验证说明，记录真实执行的入口及证据边界；临时 ticket／证据不作为长期文档唯一依赖。

## 覆盖分区与测试所有权

| Promise | 本票覆盖项／实现贡献 | Contract 测试 owner | 其余归属 |
|---|---|---|---|
| P01 | 合法双类型、互斥拒绝及页面重载 | 03 | 完整 |
| P02 | 草稿可存、无效目标不启动 | 03 | 完整 |
| P07 | 多层／重复正例及直接／间接循环拒绝 | 03 | 完整 |
| P37 | 引用保护、无引用删除及扫描失败 | 03 | 完整 |

表中 owner 是相应覆盖项的唯一测试编写与维护者。一个用例可以支持多个 promise，但不能由两票重复认领；promise 必须在全部覆盖项通过后才完整交付。enabling 票的单仓机制／公开接线检查不替代上述 contract 覆盖。

## Worktree checks 与运行烟测

在本票 herdr-bot worktree 根执行，遵循仓库测试、网络及临时资源规范：

- 使用独立 `CARGO_TARGET_DIR`；确保本 worktree 静态页面已由 `npm --prefix frontend run build` 生成。需要受控宿主时构建本 worktree 的 herdr-fixture／安全壳，不使用真实关机程序。
- 开发定向入口 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch`，并按实际变更覆盖启动／定义／删除／恢复所属既有目标，不以删旧行为测试换绿。
- 完成时运行一次 `node scripts/test-backend.mjs`。
- 前端 `npm --prefix frontend test`；真实页面入口 `npm --prefix frontend run test:browser:build`，实际观察页面行为。时间承诺标签变化时核对 `npm --prefix frontend run test:browser:partition`。
- 实际启动隔离页面与后端，通过用户表单／公开 HTTP 执行本票成功和拒绝路径，观察页面、持久读回与宿主副作用。

用独立字面预期、真实临时文件及实际持久读回核对；失败／不可读必须观察真实效果，不只 mock 返回错误。不改私有源码文本作测试，不固定提示词措辞或内部锁结构。长期回归只覆盖消费者可见的边界、状态和副作用。

每个场景拥有可变目录及动态数值回环端口；持续消费进程输出，成功失败均终止并等待所属任务／进程，保留原错误及清理错误。不读用户凭据、不连接用户 Chat／daemon、不修改用户业务定义，不执行真实关机、提权、LAN 或 mDNS。

## 完成标准

上表本票覆盖项、全部验收条件和所属仓库检查均有实际通过证据后才能关闭。Full automation 无人工验收豁免；真实依赖缺失如实保留阻塞，不以提示词存在、配置文件、可编译 scaffold、受控成功或单次最终状态宣称完整交付。
