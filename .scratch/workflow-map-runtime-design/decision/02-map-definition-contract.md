# Map、Bot 与运行配置的契约边界

Type: grilling
Status: resolved
Assignee: none
Map: ../map.md
Blocked by: 01
Modules: 定义管理

## Handoff

- 完整决定已发布至[字段契约 Lighthouse](../lighthouse/02-map-definition-contract.md)。本轮 Q30 汇总后用户明确“同意，请发布”；本票全部问题已决定，无待恢复的采访问题，不授权产品实现。
- 恢复入口：先读 Lighthouse。它替代原部分结果中关于 inputs、zone 格式、版本及迁移的未决描述；各项确认来源与被替代方案保留在相应主题中，不维护第二份政策清单。
- 关键理由：Bot 文档按 Agent 使用阶段独立提供，因此允许重复；绑定与说明可分步配置，因此未绑定可保存；用户确认只有示例，因此不设计历史迁移。自动规划编排不在本票范围，交接和合法重复执行仍消费协作、状态机及故障票据。
- 下一张可开始：[运行状态机、管理者工具与决策 seam](03-runtime-state-operations.md)，从完成报告、交付与放行的关系开始；[通用协作消息、交付与决策会话](04-collaboration-protocol.md)也在 frontier。本次不认领它们。
- 证据：静态读取 backend/src/scheduler.rs 的 WorkflowMap、MapNode、Bot、BotImplementation，观察到旧字段和占位执行类型；未验证目标 schema 已实现，未运行产品、宿主或真实 Agent 验收。input 与 zone_id 的宿主接口精度由接入票据核查。
- Backtracer：以本次字段结果和绑定 Lighthouse 为来源核对现有问题覆盖；阶段化能力说明与字段职责由本票提供，自动规划过程按用户确认不纳入目的地。Task/独立 Chat/Workspace 前提由绑定决定提供；实际接入由接入票据提供；完成与依赖准入、重复执行合法性由状态机和故障票据提供；交接由协作票据提供；运行事实由持久化票据提供；启动与观察由观察票据提供；示例与跨模块实施契约由收敛票据承接。未发现需要新增票据或用户裁决的缺口／上游冲突，未改写其他票据。
- 依赖结果：本票完成只满足故障、持久化、观察及收敛票据的字段前提，不解除它们其他未完成依赖。地图的循环排除项按本次授权收窄，未取消任何已有前提；新执行身份、会话复用、迟到反馈和下游影响仍未决定。

## Question

当前 Map JSON 是否覆盖本轮需求？哪些信息属于 Map、Bot、Task 或本次 Run，哪些需要改变现有格式？

逐项覆盖依赖、输入、命名交付、执行绑定、管理配置和重试策略归属；定义版本、严格未知字段校验、迁移与不可运行定义的表达。不要把运行态加入静态 Map。重试策略是否写入 Map 是用户明确保留的未决问题。

## Inputs

- [Bot、Task、Chat 与管理角色的绑定](01-execution-binding.md)

- [第二阶段交接计划](../../workflow-map-runtime-tracer-implementation-plan.md)。其中暂停／重试等排除项须与本轮范围修订合并阅读。
- [领域词汇](../../../GLOSSARY.md)。

## Comments

- 2026-10-27：用户完成字段与职责 grilling，在 Q30 汇总后授权发布。字段、校验、版本和示例处置以 Lighthouse 为准；未采用的候选不构成契约。状态、通信及宿主接口继续由相应票据决定，本票不授权实现产品。
