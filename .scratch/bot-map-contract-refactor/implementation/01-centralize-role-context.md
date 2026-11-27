# 01: 集中既有角色说明与消息装配

**What to build:** 先把现有 Task、Run 和 chief 路径的通用角色正文、运行事实格式化、词典及消息组合集中到既有 prompts 模块，保持调用者可观察行为，为后续协议切换提供单一维护入口。

**Delivers:** enabling: unblocks P1, P10, P33, P34, P36

**Blocked by:** None (can start immediately)

**Repositories:** bot

**Status:** ready-for-agent

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [ ] 迁移实际拼接逻辑及其调用者，不保留散落正文、仅转发的第二入口或重复角色政策；复用批准的既有模块组织决定。
- [ ] 调用者仍拥有身份判定、状态准入、材料解析与 Chat 启动；不把这些职责搬入提示词模块。
- [ ] 保持独立 Task 的初始 Skill、固定设定、用户输入、根/子管理者差异、chief 路径、结束节点例外与真实恢复关联。
- [ ] 词典仍仅作用于既定类型字段，每条消息采用同一快照；不新建模板引擎、角色框架或运行时正文部署依赖。
- [ ] 这是保行为 prefactor，不提前实现新业务政策；后续 05 负责新语义的公开提交内容测试。

## 检查与证据

保行为检查：从本票 bot worktree 执行 `cargo check --manifest-path backend/Cargo.toml --all-targets`；记录原始结果。若发现既有公开行为变化，应修正 prefactor，而不是改旧期望来认可它。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 01 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。

本票属于当前自动开发队列，完成自身适用实现与自动检查后即可交付；不等待 11–14，也不代做它们。保留真实失败/部分效果，更新本票范围文档及证据。
