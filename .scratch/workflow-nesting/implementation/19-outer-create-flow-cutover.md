# 19: 旧创建 Task 与 Bot 流程调用新包装能力

**What to build:** 原创建 Task＋Bot 流程保留创建及优化 Task 的外层职责，随后使用真实目标身份和显式文件输入调用新包装子流程。

**Delivers:** enabling: unblocks P45, P46

**Blocked by:** 17: 发布为已有 Task 或 Map 包装 Bot 的能力

**Repositories:** herdr-bot

**Status:** ready-for-agent

依据：[批准契约](../contract.md)；[规格](../spec.md)。契约 Approval 已获批准，本轮 24 票修订方案由用户授权发布。发布不代表实现、自动验收或用户部署操作已完成。

## Acceptance criteria

- [ ] 外层保留 Task 创建、Task.prompt 优化，再调用 17 的 Map 型 Bot，不把创建 Task 固化进可复用包装能力。
- [ ] 明确子 manager、zone、业务要求、命名输入及输出；外层使用真实 report 交付和显式 complete 边界。
- [ ] 修正受影响 Task 创建／优化链中的已退役 Task API 字段及失效契约指示，使用当前合法选项和公开接口，不增加兼容别名。
- [ ] 仅迁移选定链路；旧 create-bot-task、create-bot 独立能力保持原职责，其他 Map 不自动替换。
- [ ] 通过公开定义写读及预检核对接线和作用范围，真实外层调用由 24 拥有。
- [ ] Task 创建者及通用优化者（task-18e31d34797f1464-3）的 Task.prompt 直接保存分支，交 24 负责独立／workflow 配对；21 仍负责通用优化者的 Bot 指南材料分支，不重复用例。
- [ ] 所有受影响调用方与既有行为检查同步切换，删除被本次替代的死路径，不引入旧字段别名或以兜底吞掉失败。
- [ ] 更新所属长期使用／运行／验证说明，记录真实执行的入口及证据边界；临时 ticket／证据不作为长期文档唯一依赖。

## 覆盖分区与测试所有权

| Promise | 本票覆盖项／实现贡献 | Contract 测试 owner | 其余归属 |
|---|---|---|---|
| P45 | 外层受影响 Task 兼容贡献 | 24 | 独立／workflow 配对：24 |
| P46 | 外层调用及旧入口保持实现贡献 | 24 | 完整真实外层调用：24 |

表中 owner 是相应覆盖项的唯一测试编写与维护者。一个用例可以支持多个 promise，但不能由两票重复认领；promise 必须在全部覆盖项通过后才完整交付。enabling 票的单仓机制／公开接线检查不替代上述 contract 覆盖。

## Worktree checks 与运行烟测

在本票 herdr-bot worktree 根执行，遵循仓库测试、网络及临时资源规范：

- 使用独立 `CARGO_TARGET_DIR`；确保本 worktree 静态页面已由 `npm --prefix frontend run build` 生成。需要受控宿主时构建本 worktree 的 herdr-fixture／安全壳，不使用真实关机程序。
- 开发定向入口 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch`，并按实际变更覆盖启动／定义／删除／恢复所属既有目标，不以删旧行为测试换绿。
- 完成时运行一次 `node scripts/test-backend.mjs`。
- 在隔离后端通过公开保存、读取及预检实际消费本票定义／材料接线，检查合法与拒绝结果；对共享业务 Task 运行最小实际调用，保留材料读写与授权结果。该烟测不替代指定跨仓票的完整真实模型验收。

用独立字面预期、真实临时文件及实际持久读回核对；失败／不可读必须观察真实效果，不只 mock 返回错误。不改私有源码文本作测试，不固定提示词措辞或内部锁结构。长期回归只覆盖消费者可见的边界、状态和副作用。

每个场景拥有可变目录及动态数值回环端口；持续消费进程输出，成功失败均终止并等待所属任务／进程，保留原错误及清理错误。不读用户凭据、不连接用户 Chat／daemon、不修改用户业务定义，不执行真实关机、提权、LAN 或 mDNS。

## 完成标准

上表本票覆盖项、全部验收条件和所属仓库检查均有实际通过证据后才能关闭。Full automation 无人工验收豁免；真实依赖缺失如实保留阻塞，不以提示词存在、配置文件、可编译 scaffold、受控成功或单次最终状态宣称完整交付。
