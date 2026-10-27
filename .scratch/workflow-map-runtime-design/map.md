# Map 运行系统曳光弹需求与设计地图

Label: wayfinder:map

## Destination

以现有第二阶段计划为基础，落定 omp 首版通用 Map 运行系统的需求、模块职责与接入契约，形成可供后续实施的 spec／设计文档。完成条件是关键设计问题已明确、契约可供实施者使用；本次不实现产品或执行真实 Agent 验收。

## Notes

- 基线：[第二阶段交接计划](../workflow-map-runtime-tracer-implementation-plan.md)。它是设计输入，不是已实现能力证明；本次确认的范围变化见下文，不能机械沿用旧文的排除项。
- 每次会话遵循 wayfinder；grilling 票据使用 grilling 与 domain-modeling。发布决定使用 lighthouse，随后使用 backtracer。一次会话最多解决一张非研究票据。
- 术语遵循 [GLOSSARY](../../GLOSSARY.md)：Map 是静态定义，Bot 是业务能力定义，Task 是发起预设，Chat 是实际对话。管理角色不等于 Bot 定义。
- 本次只是 charting：下列内容是用户确认的努力边界与讨论恢复上下文，不是已解决的设计票据。各票据仍须产出自己的 Lighthouse 权威；Decisions so far 暂为空。

### 已确认的努力边界

确认来源：本轮用户明确表示“我的目标是产出设计文档……我们只讨论需求与设计”。设计应覆盖原计划中的边界情形，因为它们可能影响底层设计；原计划对暂停／重试／人类介入的排除项按本轮补充收窄。

- 首版只接入 omp；如需注册工具，在 `E:/Developer/myhub/my-agent-toolkit` 中通过 extension 实现。参考宿主代码 `E:/Developer/myhub/oh-my-pi`，参考协作机制 `E:/Developer/myhub/oh-my-wayfinder/extensions`，不搬入 Git／worktree 编码业务阶段。
- Chat 管理者选择流转动作；运行系统落实动作、保存事实并执行权限和操作约束，不内置失败后的自动选路策略。流转决策保留 seam，以后可接入其他模式。
- 首版无额外业务质量校验。用户原话：“基本上只要是node能正常传达完成信号即可，不会有额外的校验。”完成、交付和放行如何机械衔接仍由对应票据决定。
- extension 必须覆盖节点故障后的暂停、重试、结束操作，不仅预留空入口。自动策略与可调用执行能力分开；不因旧计划禁止自动重试而删掉管理者调用重试的能力。
- 暂停等待人类主动监控与处理；不建专门人工审批／通知／接管系统。人类处理后给管理者 Chat 发消息，由管理者通过工具继续、重试或完成。暂停阻止新的派发，已有执行仍可反馈；恢复与“直接完成”的精确定义尚未确定。
- 首版提供停止 Map 的工具入口，以 graceful shutdown 为设计方向；是否通知活动节点、等待多久及终态语义留待决定。不把 Run 停止等同于所有外部 Chat 已退出。
- 通用协作模块覆盖本次 Run 参与者之间的协作消息，包括指示、问题、回复、进度、交付通知和执行反馈；不接管完整 Chat transcript、模型流式输出或宿主心跳。用户最后一轮明确确认此范围。
- 运行内核保持业务无关；首版执行接入限定 Task → omp Chat，不建设任意宿主插件平台。

### 暂定方案与未决输入

- 暂定由管理者 Chat 直接运行 Map，不强制经过管理 Bot；用户要求 Bot／Task／Chat 绑定关系确定后回顾此方案。
- 重试策略是否属于 Map 未决定；新执行身份、原 Chat 复用、旧反馈处理也未批准，不将先前建议当作结论。
- 判断在当前管理者会话还是另开会话未决定。
- 用户要求逐项讨论：Map JSON 覆盖性；管理者工具与状态流转；运行文件与内存一致性；chat 类型 Bot 的 node → Bot → Task → Chat 链；通用消息协作模块。下方均有对应票据。

## Decisions so far

尚无已完成票据或已发布 Lighthouse；不把建图阶段的暂定方案当作已决契约。

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

- 各模块契约落定后，重审跨模块故障场景，届时才能明确是否出现本轮尚未识别的缝隙；已有暂停、重试、停止问题不重复列为 fog。
- 明确实际接入约束后，评估是否需要新的外部事实研究或轻量原型票据；目前不预设研究结论，也不运行真实验收。

## Out of scope

- 产品实现、实施票据执行、真实 Agent 运行验收：由后续实施阶段完成，本地图提供设计与验收契约。
- LLM Plan、运行中改图、循环、定时调度、自动故障恢复及重启自动续跑。
- 专门的人工审批／通知／接管产品功能；不排除人类通过已有 Chat 参与故障处理。
- 非 Chat 执行 Adapter、非 omp 宿主实现、通用插件市场和业务质量校验平台。
- 完整聊天基础设施、transcript 汇总、通用事件总线平台及分布式多写入者运行存储。
