# Map 运行系统曳光弹需求与设计地图

Label: wayfinder:map

## Destination

以现有第二阶段计划为基础，落定 omp 首版通用 Map 运行系统的需求、模块职责与接入契约，形成可供后续实施的 spec／设计文档。完成条件是关键设计问题已明确、契约可供实施者使用；本次不实现产品或执行真实 Agent 验收。

## Notes

- 基线：[第二阶段交接计划](../workflow-map-runtime-tracer-implementation-plan.md)。它是设计输入，不是已实现能力证明；本次确认的范围变化见下文，不能机械沿用旧文的排除项。
- 每次会话遵循 wayfinder；grilling 票据使用 grilling 与 domain-modeling。发布决定使用 lighthouse，随后使用 backtracer。一次会话最多解决一张非研究票据。
- 术语遵循 [GLOSSARY](../../GLOSSARY.md)：Map 是静态定义，Bot 是业务能力定义，Task 是发起预设，Chat 是实际对话。管理角色不等于 Bot 定义。
- 下列努力边界来自建图阶段；已发布决定以 Decisions so far 链接的 Lighthouse 为权威。部分发布不代表源票全部完成，也不代表产品能力已实现。

### 已确认的努力边界

确认来源：本轮用户明确表示“我的目标是产出设计文档……我们只讨论需求与设计”。设计应覆盖原计划中的边界情形，因为它们可能影响底层设计；原计划对暂停／重试／人类介入的排除项按本轮补充收窄。

- 首版只接入 omp；如需注册工具，在 `E:/Developer/myhub/my-agent-toolkit` 中通过 extension 实现。参考宿主代码 `E:/Developer/myhub/oh-my-pi`，参考协作机制 `E:/Developer/myhub/oh-my-wayfinder/extensions`，不搬入 Git／worktree 编码业务阶段。
- Chat 管理者选择流转动作；运行系统落实动作、保存事实并执行权限和操作约束，不内置失败后的自动选路策略。流转决策保留 seam，以后可接入其他模式。
- 首版无额外业务质量校验。用户原话：“基本上只要是node能正常传达完成信号即可，不会有额外的校验。”完成与放行已按[运行状态机修订](lighthouse/03-runtime-state-operations.md)合并，交接报告和 collect 消费[通信契约](lighthouse/04-collaboration-protocol.md)。
- extension 必须覆盖节点故障后的暂停、重试、结束操作，不仅预留空入口。自动策略与可调用执行能力分开；不因旧计划禁止自动重试而删掉管理者调用重试的能力。
- 暂停等待人类主动监控与处理；不建专门人工审批／通知／接管系统。人类处理后给管理者 Chat 发消息，由管理者通过工具继续、重试或完成。暂停阻止新的派发，已有执行仍可反馈；恢复与手动完成消费[故障恢复原则](lighthouse/05-fault-pause-retry.md)及其引用的状态机动作契约。
- 停止新派发的需求由 pause／resume 承接，不再提供独立 stop 或 graceful shutdown 生命周期；不保证活动 Chat 退出。Q66 已明确修订原停止入口要求，权威见[停止生命周期范围处置](lighthouse/06-stop-lifecycle.md)。
- 协作范围已按接入会话收窄为节点交接报告、manager collect 与 Herdr 单向正文通知；通用双向消息/广播后置，现行范围见[通信契约](lighthouse/04-collaboration-protocol.md)。
- 运行内核保持业务无关；首版执行接入限定 Task → omp Chat，不建设任意宿主插件平台。

### 暂定方案与未决输入

- 管理角色的绑定回顾已完成，见下方绑定决策；不再作为暂定方案。
- 重试策略字段与循环范围已由[字段契约](lighthouse/02-map-definition-contract.md)收口；原 Chat 恢复、新 Chat 重试及旧报告不得冒充新执行依据已由[故障恢复原则](lighthouse/05-fault-pause-retry.md)收口；保存职责已由[持久化契约](lighthouse/07-persistence-consistency.md)确定，身份与工具接入已由[接入契约](lighthouse/08-omp-tool-integration.md)收口。
- 报告通知采用 Herdr 通用输入，不承诺原生 IRC 语义；collect 不依赖正文投递，详见[通信契约](lighthouse/04-collaboration-protocol.md)。
- 用户要求逐项讨论：Map JSON 覆盖性；管理者工具与状态流转；运行文件与内存一致性；chat 类型 Bot 的 node → Bot → Task → Chat 链；通用消息协作模块。下方均有对应票据。

## Decisions so far

- [Bot、Task、Chat 与管理角色的绑定](lighthouse/01-execution-binding.md)：能力发现与执行分层、直接启动管理者、独立 Chat、统一 Workspace 及启动检查已确认；配置不变与用户确认 omp 未开启自动续接为本版前提，Workspace 从 manager zone 推导。
- [Map、Bot 与运行配置的契约边界](lighthouse/02-map-definition-contract.md)：阶段化 Bot 文档、Task 绑定、node 输入与位置、保存／启动校验及版本和示例处置已确定；管理配置、重试策略与循环范围边界已收口；Traverse 已收口 zone 路径引用、Task 一次性类型迁移及保存／角色扩展约束。

- [运行状态机、管理者工具与决策 seam](lighthouse/03-runtime-state-operations.md)：completed 已修订为交接完成并解除下游依赖，移除独立放行；管理者仍显式操作，保留串行派发、故障核实、暂停及成功结束，并补齐最小人工恢复动作。
- [通用协作消息、交付与决策会话](lighthouse/04-collaboration-protocol.md)：首版保留结构化交接报告、管理者 collect 与 Herdr 单向输入；后置通用双向协作，collect 不依赖投递。
- [节点故障、等待人工处理与重试](lighthouse/05-fault-pause-retry.md)：人工处理现场，管理者通过工具恢复、重试或完成；状态与交接分别补齐，纳入用户指南和管理者工具指引，不穷举复杂故障路径。
- [运行事实与内存文件一致性](lighthouse/07-persistence-consistency.md)：三文件布局、按文件协调及部分提交已确定，重启只读旧 Run；补充本次 user_input 原文保存、启动后不编辑。
- [omp extension 接入与可信执行身份](lighthouse/08-omp-tool-integration.md)：孪生扩展、身份关联、启动顺序与三个工具已收口；manager input 末尾追加用户要求，不传递到 node；Traverse 补齐自动启用策略、配置定位及完整 Task trigger 的交接覆盖。
- [运行启动、只读图与人工观察路径](lighthouse/09-observation-experience.md)：scheduler 发起、workflow 只读观察与 Resume 已确定；管理者启动过程仅日志、错误在当前后端内存供详情展示，重启后查日志，不显示启动进度；Task 类型配置复用现有管理入口。
- [跨模块设计收敛与实施交接契约](lighthouse/10-spec-acceptance.md)：六组验收、固定文件主链路、独立长期文档、使用指引和四阶段实施顺序已确认；Traverse 审计及补充验收已收口；用户要求暂不发布 spec.md。

## Decision tickets

以下仅为问题导航；frontier 以 decision 文件的 Status 与 Blocked by 为准。

- [Bot、Task、Chat 与管理角色的绑定](decision/01-execution-binding.md)
- [Map、Bot 与运行配置的契约边界](decision/02-map-definition-contract.md)
- [运行状态机、管理者工具与决策 seam](decision/03-runtime-state-operations.md)
- [通用协作消息、交付与决策会话](decision/04-collaboration-protocol.md)
- [节点故障、等待人工处理与重试](decision/05-fault-pause-retry.md)
- [停止 Map 与 graceful shutdown 生命周期](decision/06-stop-lifecycle.md)
- [运行事实与内存文件一致性](decision/07-persistence-consistency.md)
- [omp extension 接入与可信执行身份](decision/08-omp-tool-integration.md)
- [运行启动、只读图与人工观察路径](decision/09-observation-experience.md)
- [跨模块设计收敛与实施交接契约](decision/10-spec-acceptance.md)

## Not yet specified

当前决策票均已 resolved。2026-10-28 Traverse 全图审计发现的两组决策缺口已由用户逐项确认并发布，配套交接遗漏已纳入原权威及验收；无剩余待决批次或可见 fog。恢复与证据见[跨模块设计收敛与实施交接契约的 Handoff](decision/10-spec-acceptance.md#handoff)。最终交接决定见[跨模块设计收敛与实施交接契约](lighthouse/10-spec-acceptance.md)。

2026-10-28 用户明确暂不发布 spec.md：本轮结束的是决策流程，不宣称最终 spec 已交付。下一步待用户授权后按已发布决定整理具体接口/schema、配置键和实施交接文档；若整理发现新行为选择或冲突，再进入决策。长期文档须独立承接最终契约，不依赖将删除的临时材料。真实产品实现与行为验证仍属于后续实施。

## Out of scope

- [停止 Map 与 graceful shutdown 生命周期](decision/06-stop-lifecycle.md)：独立停止设计不再需要，暂停承接停止新派发需求；[确认处置与依赖审查](lighthouse/06-stop-lifecycle.md)。依赖本票的持久化、接入、观察、交接票据改为消费运行状态机的暂停／恢复／成功结束输入；现有依赖行保留处置追踪，不将本票 resolved 当作 graceful shutdown 已设计完成，其他未解决前置仍阻塞。

- 产品实现、实施票据执行、真实 Agent 运行验收：由后续实施阶段完成，本地图提供设计与验收契约。
- LLM Plan（包括自动规划与 node 填写的过程编排）、运行中改图、专门建设循环能力、定时调度、自动故障恢复及重启自动续跑；循环排除项的收窄与合法动作组合边界见[字段契约](lighthouse/02-map-definition-contract.md)。
- 专门的人工审批／通知／接管产品功能；不排除人类通过已有 Chat 参与故障处理。
- 非 Chat 执行 Adapter、非 omp 宿主实现、通用插件市场和业务质量校验平台。
- 完整聊天基础设施、transcript 汇总、通用事件总线平台及分布式多写入者运行存储。
- 首版通用双向消息、同伴通信、广播、extension 接收长连接和原生 IRC 保证；[通信范围修订](lighthouse/04-collaboration-protocol.md)。不增加强制续轮、会话切换处理、独立凭据或 --session-dir 存储迁移，见[接入范围](lighthouse/08-omp-tool-integration.md)。
- 跨文件事务／自动回滚、后端重启后重新接管旧 Run、全仓库文件操作迁移：范围及接受的风险见[持久化契约](lighthouse/07-persistence-consistency.md)。
