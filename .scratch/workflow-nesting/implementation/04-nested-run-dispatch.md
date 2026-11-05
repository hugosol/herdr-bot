# 04: 派发 Map 型节点并建立执行级父子关联

**What to build:** 父 manager 派发 Map 型节点时，使用该节点的 manager、业务输入和 zone 创建独立子 Run，并以持久执行关联授权唯一的双角色 Chat。

**Delivers:** P04, P05, P06, P10, P11, P12, P21, P22（只认领下表覆盖项；完整 promise 等全部所属项通过）

**Test owner for:** P04, P05, P06, P10, P11, P12, P21, P22 的本票覆盖项；其他覆盖项 owner 见下表。

**Blocked by:** 02: 派发消费者时解析并检查实际文件；03: 配置两类 Bot 目标并验证可调用关系

**Repositories:** herdr-bot

**Status:** ready-for-agent

依据：[批准契约](../contract.md)；[规格](../spec.md)。契约 Approval 已获批准，本轮 24 票修订方案由用户授权发布。发布不代表实现、自动验收或用户部署操作已完成。

## Acceptance criteria

- [ ] 两组独立字面配置核对节点指定的 manager Task、zone 和原始业务文字；Bot 不固化 manager，子 Map 内部节点保留各自 zone。
- [ ] 同 Map 两次派发获得不同 child／execution；父子关系属于具体 execution，公开详情可读且不复制或展开父图。
- [ ] 父节点解析后的本地输入 ID 按同名传入子接口；多层同名按所属 Run 解析，空输入与可选缺席合法。
- [ ] 父可选项缺席进入必需子接口时明确拒绝并指出缺项，不绕过必需约束，且无该子工作启动。
- [ ] 父 execution 与 child manager 的真实 Chat 构成唯一合法组合；服务端 run/collect 定位 child，report 定位对应父 execution，不开放模型身份选择器。
- [ ] 保留同 Run 角色混用、无关联、多重非法关联、来源 Task／agent 不一致和 Session 碰撞拒绝。
- [ ] 对应宿主装配 manager-chat 与 node-chat；与 12 协调公开语义，不擅自引入新的 Task 类型或一律放宽角色唯一性。
- [ ] 启动最小成功路径保留真实阶段和已发生效果，07 扩展确定性阶段故障；不宣称跨文件事务或自动回滚。
- [ ] 所有受影响调用方与既有行为检查同步切换，删除被本次替代的死路径，不引入旧字段别名或以兜底吞掉失败。
- [ ] 更新所属长期使用／运行／验证说明，记录真实执行的入口及证据边界；临时 ticket／证据不作为长期文档唯一依赖。

## 覆盖分区与测试所有权

| Promise | 本票覆盖项／实现贡献 | Contract 测试 owner | 其余归属 |
|---|---|---|---|
| P04 | 两组公开启动配置与运行事实 | 04 | 原生输入及来源：14 |
| P05 | 外层与内部 zone 保持 | 04 | 实际目录／文件来源：13 |
| P06 | 独立 child 与唯一父 execution，公开详情 | 04 | 真实角色一致性：14 |
| P10 | 嵌套空接口及可选缺席传递 | 04 | 根：01；普通节点：02；实际消费：13 |
| P11 | 父可选缺席不能绕过子必需项 | 04 | 根接口：01；真实不可读：02 |
| P12 | 父本地 ID 到 child 同名及多层绑定 | 04 | 本层：02；实际材料：13 |
| P21 | 持久关联授权、非法碰撞及拒绝 | 04 | 原生身份接线：14 |
| P22 | 同名 node／多 Run／旧执行的服务端归属规则 | 04 | 真实工具调用：14 |

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
