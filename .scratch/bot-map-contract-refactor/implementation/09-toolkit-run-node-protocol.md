# 09: OMP/Pi 管理者与节点工具适配

**What to build:** OMP/Pi 的管理者和节点工具采用批准的新公开协议及共同业务政策，使实际执行者能准备输入、结束 Run、报告进展并交接父层成果。

**Delivers:** enabling: unblocks P7, P15, P18, P19, P20, P21, P33, P34, P35, P37

**Blocked by:** None (can start immediately)

**Repositories:** toolkit

**Status:** ready-for-agent

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [ ] 按批准契约适配 run/prepare_inputs/finish、map_output.files、公开读回与节点报告，保留既有操作参数适用命名和整批替换语义，不创造新登记工具。
- [ ] 普通 Task 节点 outputs、子 Map 正式成果来源和无文件 can_complete=false 中途报告区分；批准、报告、complete、finish 及通知分别表达，不自动推进。结束节点不加载普通节点报告职责。
- [ ] 根/子管理者与 OMP/Pi 入口共享对应批准正文；扩展只维护工具指引，基本流程事实仍由后端供给。子管理者保留内部管理/父节点交接两层职责，不要求管理者读取 Map 专属业务标准。
- [ ] 同步 schema、工具/参数 description、初始化及动态 guidance，不只修改 init；保留身份、原生 approve 与真实恢复关联，不以提示词绕过后端拒绝。manager 的 chief 报告工具指引复用 10 所负责的唯一来源，不另维护正文；可按批准接口独立实现，不制造 09/10 循环依赖。
- [ ] 工具包本地协议测试采用独立请求/回执字面样例，检查实际公开工具入口而非源码快照；它们不冒充跨仓或原生安装验收。同步工具包使用说明。

## 检查与证据

在本票 toolkit worktree 运行 `bun run typecheck`、`bun run check:integrations` 及受影响 workflow-chat 测试文件（包括 manager/node shared logic 和 node-report 对应 case）。涉及 dock 的变动运行对应具名 dock，使用同次构建的安全壳；新协议的跨仓 case 唯一归 11，真实安装归 12/13。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 09 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。

本票属于当前自动开发队列，完成自身适用实现与自动检查后即可交付；不等待 11–14，也不代做它们。保留真实失败/部分效果，更新本票范围文档及证据。
