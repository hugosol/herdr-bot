# 07: chief 委派、结构化报告与接替读取

**What to build:** chief 沿用 Task/Map 委派、预览与确认，保存本次文件要求；执行者交付结构化成果，报告独立持久并通知当前合法 chief，接替者不依赖原会话记忆。

**Delivers:** P22, P23, P24, P25, P26, P27, P28, P29, P34, P57

**Blocked by:** 02 Bot 新格式、编辑与定义消费者整体切换; 06 公开成果 finish 与父子 Run 显式交接

**Repositories:** bot

**Status:** ready-for-agent

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [ ] 委派可选 output_rule，省略空列表，HTTP/持久为 outputRule；参与预览、变化核对、确认/取消并随原委派保存。只支持既有 Task/Map，不增加 script 或 Task 统一静态接口。
- [ ] Task 实际提交内容包含本次要求；成功报告的 outputs 必须齐全合法可读，失败/受阻可以无文件或保留有效部分成果。
- [ ] Map 要求只能是公开输出同名子集，正式文件地址取自已完成 Run，不接受地址覆盖；Map 自己仍须交齐全部公开必交项。未 finish 可无文件报告失败/受阻；不对所有 success 增加统一 finish 门禁，不为 chief_report 增加 can_complete/进度分类。
- [ ] 保留 background/execution/result/summary/deliverables/pendingDecisions 等原说明，outputs 单独保存，不解析文字 deliverables。独立及 Map 委派消息沿用同一批准说明，不从工具可见性授予角色。
- [ ] 报告先持久保存再尝试通知当前合法 chief；保存/通知分别反馈，失败不撤销、不自动重投或重新委派。显式重发沿用原生确认；新 chief 可按权限读取陌生报告及真实文件，不依赖原 chief 在线，不猜测 ID 含义。
- [ ] P29 本票拥有预览后变化/取消拒绝、Task 输出拒绝、Map 成果来源拒绝，以及保存或外部通知部分效果 case。P27 chief 通知失败与有限窗口外部调用由本票唯一维护。
- [ ] P57 本票拥有隔离清理后恢复/登记当前 chief 与旧委派、报告职责、旧 Run 管理资格拒绝的产品对照；全部 Chat 原生历史与实际材料保留。实际清理归手动 14。
- [ ] 同步公开协议、报告读回和系统说明；不硬编码能力建设入口，不扫描积压或自主后续派发。跨仓工具确认行为由 11 验证，真实安装由 13 验证。

## 检查与证据

从本票 bot worktree 运行 `cargo test --manifest-path backend/Cargo.toml --test chief_delegation --test chief_authority` 的对应委派、报告、receipts、恢复/权限 case；对保存/通知失败保留独立记录与测试指引要求的观察窗口，不改产品超时来制造通过。必要的恢复调用者 case 归入原 target，记录精确选择器。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 07 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。

本票属于当前自动开发队列，完成自身适用实现与自动检查后即可交付；不等待 11–14，也不代做它们。保留真实失败/部分效果，更新本票范围文档及证据。
