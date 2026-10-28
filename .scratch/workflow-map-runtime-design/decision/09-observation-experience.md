# 运行启动、只读图与人工观察路径

Type: grilling
Status: resolved
Assignee: none
Map: ../map.md
Blocked by: 02, 03, 04, 05, 06
Modules: 观察与人工控制

## Handoff

- 2026-10-28 用户对 Q21/Q22 明确“同意”，授权发布并完成本票；[观察 Lighthouse](../lighthouse/09-observation-experience.md)持有完整界面设计与确认来源。无剩余待访谈问题，不授权产品实现。
- 本轮新增 user_input 已按用户文件归属确认分别发布到[持久化权威](../lighthouse/07-persistence-consistency.md#当前状态内容与派生数据)与[接入权威](../lighthouse/08-omp-tool-integration.md#提示词文档与启动-input)。本票消费它们，不建立第二套保存或输入政策；原票状态、claim 与 Handoff 不变。
- 被替代分支：启动弹窗改为发起页面；原无额外输入建议被 user_input 取代；原 3 秒轮询改为默认 5 秒且服务级可配置。旧定义快照、独立放行展示与投递业务记录不恢复。用户明确 user_input 只给管理者、不做后续逻辑；节点传播不是待补机制。
- Backtracer：已追踪本次 user stories 的独立运行意图、状态观察、Chat 介入及历史区分；绑定/字段提供 Task/zone/Workspace，状态/通信/故障提供动作语义与报告，持久化/接入提供原文、运行记录、执行历史与真实 Chat 关联。停止依赖消费既有范围处置，由暂停/恢复/finish 替代，不要求 graceful shutdown。
- 图复用、Resume、服务级轮询所需的启动/查询/配置接口和实际可操作性由[跨模块设计收敛与实施交接契约](10-spec-acceptance.md)现有模块契约与验收范围覆盖，不新增问题或修改其 Handoff。Map 历史不可还原按 Q19 明确降级，Resume 不接管 Run 按 Q20 约束，均无未裁决冲突。没有新的未覆盖 gap 或待跟进批次。
- 下一张 frontier 为[跨模块设计收敛与实施交接契约](10-spec-acceptance.md)，需消费最新 Lighthouse，而非旧票中的独立放行、双向通信和停止候选；本轮未认领。当前票全部范围已定，真实运行能力仍待后续实施验证。
- 证据限制：静态阅读 SchedulerPanel.tsx、ChatCreationDialog.tsx、ChatPanel.tsx 相关段及领域/权威文档，未运行产品、测试、浏览器或真实 Agent。现有图和 Resume 仅是复用线索，不是新增运行界面的验收证明。

## Question

用户如何发起并观察一次 Run，识别暂停或故障，并通过关联 Chat 介入？

决定发起入口与配置、运行列表与只读图、节点现场和报告/执行历史、管理者/节点 Chat 导航，以及刷新不重复启动。复用现有图与 Resume，不做独立前端状态机或专门人工审批。原“快照图、节点执行与放行独立展示、消息交付错误详情”按既有无定义快照、completed 合并交接与依赖解除、投递仅诊断日志的权威收窄，不是剩余未答范围。

## Inputs

- [Map、Bot 与运行配置的契约边界](02-map-definition-contract.md)
- [运行状态机、管理者工具与决策 seam](03-runtime-state-operations.md)
- [通用协作消息、交付与决策会话](04-collaboration-protocol.md)
- [节点故障、等待人工处理与重试](05-fault-pause-retry.md)
- [停止 Map 与 graceful shutdown 生命周期](06-stop-lifecycle.md)：范围处置，不要求独立停止输出。
- [运行事实与内存文件一致性](07-persistence-consistency.md)
- [omp extension 接入与可信执行身份](08-omp-tool-integration.md)
- [第二阶段交接计划](../../workflow-map-runtime-tracer-implementation-plan.md)：历史输入，以发布权威为准。
- [领域词汇](../../../GLOSSARY.md)

## Comments

- 用户先明确 workflow 顶层入口、Map + 发起页面、运行图和节点下方详情，以及图外/节点的 Resume。Q6–Q9 确认无额外运行名称、空 Map 拒绝、创建后进入详情、倒序列表。
- Q5 用户要求本次 user_input；Q10/Q11 确认可空 Markdown、追加管理者输入、保存及只读展示；Q12 原话“只作为manager-bot的输入，不作任何后续逻辑”。Q21 按文件名确认原权威修订。
- Q13–Q20 确认布局、状态整体样式、节点报告/历史、5 秒可配置轮询、定义缺失简单降级与旧 Chat Resume；Q19 用户原话“不做太多版本控制和容错机制”，未批准新快照或自动修复。
- 最终 Q21/Q22 用户“同意”，本次统一发布、追踪并解决决策票；不创建实现票据或执行产品验收。
