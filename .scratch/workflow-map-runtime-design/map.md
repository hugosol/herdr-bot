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
- 通用协作模块覆盖本次 Run 参与者之间的协作消息，包括指示、问题、回复、进度、交付通知和执行反馈；不接管完整 Chat transcript、模型流式输出或宿主心跳。用户最后一轮明确确认此范围。
- 运行内核保持业务无关；首版执行接入限定 Task → omp Chat，不建设任意宿主插件平台。

### 暂定方案与未决输入

- 管理角色的绑定回顾已完成，见下方绑定决策；不再作为暂定方案。
- 重试策略字段与循环范围已由[字段契约](lighthouse/02-map-definition-contract.md)收口；原 Chat 恢复、新 Chat 重试及旧报告不得冒充新执行依据已由[故障恢复原则](lighthouse/05-fault-pause-retry.md)收口；保存职责已由[持久化契约](lighthouse/07-persistence-consistency.md)确定，精确可信身份表示仍由接入票据落实。
- 通信在既有关联 Chat 内自动输入与继续，不为消息另开决策会话；工具注册与宿主注入由接入票落实，见[通信契约](lighthouse/04-collaboration-protocol.md)。
- 用户要求逐项讨论：Map JSON 覆盖性；管理者工具与状态流转；运行文件与内存一致性；chat 类型 Bot 的 node → Bot → Task → Chat 链；通用消息协作模块。下方均有对应票据。

## Decisions so far

- [Bot、Task、Chat 与管理角色的绑定](lighthouse/01-execution-binding.md)：能力发现与执行分层、直接启动管理者、独立 Chat、统一 Workspace 及启动检查已确认；配置不变是本版前提。
- [Map、Bot 与运行配置的契约边界](lighthouse/02-map-definition-contract.md)：阶段化 Bot 文档、Task 绑定、node 输入与位置、保存／启动校验及版本和示例处置已确定；管理配置、重试策略与循环范围边界已收口。

- [运行状态机、管理者工具与决策 seam](lighthouse/03-runtime-state-operations.md)：completed 已修订为交接完成并解除下游依赖，移除独立放行；管理者仍显式操作，保留串行派发、故障核实、暂停及成功结束，并补齐最小人工恢复动作。
- [通用协作消息、交付与决策会话](lighthouse/04-collaboration-protocol.md)：两类结构化消息、IRC 式自动输入、管理者专用 collect、合并检查与交接待办、可配置 90 分钟等待及提示词边界已确定。
- [节点故障、等待人工处理与重试](lighthouse/05-fault-pause-retry.md)：人工处理现场，管理者通过工具恢复、重试或完成；状态与交接分别补齐，纳入用户指南和管理者工具指引，不穷举复杂故障路径。
- [运行事实与内存文件一致性](lighthouse/07-persistence-consistency.md)：三文件布局、业务无关文件层、按文件加锁及提交顺序已确定；接受跨文件部分提交，统一传播错误，后端重启只读旧 Run。
- [omp extension 接入与可信执行身份（部分）](lighthouse/08-omp-tool-integration.md)：确定 herdr-bot 统筹 HTTP 通信，并先调查 connect-bot 复用；传输细节与扩展组织仍未决。

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

- 各模块契约落定后，核对常见故障恢复的跨模块契约是否一致；不重新展开已收窄的复杂故障枚举，已有暂停、重试、停止问题不重复列为 fog。
- 明确实际接入约束后，评估是否需要新的外部事实研究或轻量原型票据；目前不预设研究结论，也不运行真实验收。

## Out of scope

- [停止 Map 与 graceful shutdown 生命周期](decision/06-stop-lifecycle.md)：独立停止设计不再需要，暂停承接停止新派发需求；[确认处置与依赖审查](lighthouse/06-stop-lifecycle.md)。依赖本票的持久化、接入、观察、交接票据改为消费运行状态机的暂停／恢复／成功结束输入；现有依赖行保留处置追踪，不将本票 resolved 当作 graceful shutdown 已设计完成，其他未解决前置仍阻塞。

- 产品实现、实施票据执行、真实 Agent 运行验收：由后续实施阶段完成，本地图提供设计与验收契约。
- LLM Plan（包括自动规划与 node 填写的过程编排）、运行中改图、专门建设循环能力、定时调度、自动故障恢复及重启自动续跑；循环排除项的收窄与合法动作组合边界见[字段契约](lighthouse/02-map-definition-contract.md)。
- 专门的人工审批／通知／接管产品功能；不排除人类通过已有 Chat 参与故障处理。
- 非 Chat 执行 Adapter、非 omp 宿主实现、通用插件市场和业务质量校验平台。
- 完整聊天基础设施、transcript 汇总、通用事件总线平台及分布式多写入者运行存储。
- 跨文件事务／自动回滚、后端重启后重新接管旧 Run、全仓库文件操作迁移：范围及接受的风险见[持久化契约](lighthouse/07-persistence-consistency.md)。
