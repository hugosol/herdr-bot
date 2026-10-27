# Bot、Task、Chat 与管理角色的绑定

Type: grilling
Status: resolved
Assignee: none
Map: ../map.md
Blocked by: none
Modules: Bot 执行；定义管理

## Handoff

- 本票决定已发布至[绑定 Lighthouse](../lighthouse/01-execution-binding.md)。用户在 Q23 收口后明确“可以，请发布”；本票无剩余待决定问题，不授权产品实现。
- 跨票确认的字段语义仅发布至[字段契约 Lighthouse（部分）](../lighthouse/02-map-definition-contract.md)；目标票正文、状态、Assignee 和剩余问题保持不变。字段完整性不能视为已确认。
- 下一 ready question 属于[Map、Bot 与运行配置的契约边界](02-map-definition-contract.md)：在已确认的字段归属下检查完整性，再决定字段格式、严格校验与存量迁移；先读其部分 Lighthouse，不从旧建图恢复点重新争论已确认归属。
- 关键取舍：Bot 服务能力发现与正确调用，不是可复用 node 配置；业务背景与限制由 Task.prompt 管理，不另注入 Bot prompt；node 的具体输入和位置在定义时确定。不引入 Bot 运行实例、逻辑位置绑定或配置快照保证。旧候选与变更理由见 Lighthouse Discussion。
- 证据：静态读过 scheduler.rs 的 Bot/MapNode 和 lib.rs 的 CreateChatRequest/WorkspaceTarget；当前格式仍缺目标执行绑定。未运行产品或验证宿主独立 Chat、指定 Workspace 能力。

### Publication trace

- 追溯范围：本票绑定权威及字段契约部分权威；上游无已发布决策阻塞。对照地图、GLOSSARY、Task/Bot 命名 ADR 与旧交接计划，并读取相关未决票 Question/Handoff。
- 意图信号与提供者：能力发现、输入填写、格式迁移由[字段契约](02-map-definition-contract.md)覆盖；独立 Chat、可信关联及宿主目标能力由[omp 接入](08-omp-tool-integration.md)覆盖；管理者启动、配置选择与统一 Workspace 观察由[启动与观察](09-observation-experience.md)覆盖；提前拒绝及派发失败分别由[运行状态与操作](03-runtime-state-operations.md)、[故障与重试](05-fault-pause-retry.md)覆盖。
- 模式与依赖追溯：Task → Chat、zone/Workspace 分离不改变命名 ADR；跨 zone 材料访问、input 中上游引用由[协作协议](04-collaboration-protocol.md)覆盖；Chat ID 尚未取得、流程事实与 Chat 状态分离由状态、接入及[持久化](07-persistence-consistency.md)共同覆盖；旧 Chat 历史及迟到反馈属于故障票。提供者未决不表示能力已具备。
- 冲突对照：旧交接计划强制管理 Bot、配置快照及额外执行引用清单不能继续作为批准要求；本轮明确取代的范围见绑定权威。持久化／观察票中的“快照”是待决题目而非先前批准的保证，开始时须消费新的配置不变前提。普通历史 Chat 恢复语义未被修改。
- 字段语义原属另一层，已由用户按文件名确认并在字段契约权威部分发布；无重复政策。字段具体形状、完整性及迁移仍留在原票。
- 追溯结果：未发现需要新建票据的无主问题或待用户裁决的上游决策冲突。精确 Workspace 定位、独立原生会话与 ID 关联均是已有接入问题的未验证前提，不据此宣称已实现；本次不产生额外研究票或改写下游 Handoff。

## Question

如何从 node 找到 chat 类型 Bot、解析 Task 与环境并发起独立 Chat？管理者是否直接以 Task 启动，还是也需要 Bot？

明确 Bot／Task／Chat 的职责、引用关系、配置来源与覆盖顺序、缺失引用处理、快照范围与隔离要求。回顾管理者不经过 Bot 的暂定方案；首版只支持 chat，不建设多 Adapter 平台。

## Inputs

- [第二阶段交接计划](../../workflow-map-runtime-tracer-implementation-plan.md)。其中暂停／重试等排除项须与本轮范围修订合并阅读。
- [领域词汇](../../../GLOSSARY.md)。

## Comments

- 建图时的候选不自动构成决定；本次已确认范围以关联 Lighthouse 为准。未决定的状态、协作、格式和接入细节由对应票据继续讨论；本票不授权实现产品。
