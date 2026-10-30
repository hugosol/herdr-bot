# 通用协作消息、交付与决策会话

Type: grilling
Status: resolved
Assignee: none
Map: ../map.md
Blocked by: 01
Modules: 协作与交付

## Handoff

- 2026-10-28 用户在 Q37 确认接入部分归属后，明确同意发布并在追踪无阻塞时解决本票。[通信 Lighthouse](../lighthouse/04-collaboration-protocol.md)为本票完整权威；本票无剩余采访问题，不授权产品实现。
- 上游修订：[运行状态机 Lighthouse](../lighthouse/03-runtime-state-operations.md)已在原主题替代独立完成／放行方案。依据为用户“completed直接就代表‘已放行’就可以了”及 Q3 确认管理者独占状态操作；本票不另维护状态政策。
- 跨票部分发布：Q35 明示文件归属并获同意的[同目录存储决定](../lighthouse/07-persistence-consistency.md)，Q37 明示文件归属并获确认的[HTTP 统筹与 connect-bot 调查要求](../lighthouse/08-omp-tool-integration.md)。两张目标票正文、Status、Assignee 与其他未决范围保持不变。
- 已处置的原 Question 分支：不建回复关联／线程、独立 Delivery 或输入材料自动绑定、专门历史查询工具、结构化失败报告；交付正文和地址由 Agent 决定，只用提示词引导。自动输入在既有关联 Chat 内处理，不另开决策会话。记录位置不改变各自 zone。具体文件格式、extension 布局及接入 API 交给相应提供者，不作为本票候选实现。
- 被替代方案及原因：仅交接报告唤醒已由用户选择所有消息可打断取代，以贴近业务；collect 仍仅管理者使用。发送重发限制、Run 结束禁通信、角色历史读取权限均因用户要求简单而撤回。单一运行 extension 建议未批准；SSE 未锁定。不会把这些早期候选发布成要求。
- Backtracer 追踪：已核对绑定、字段、状态机与停止处置。直接通信、自动输入和正常交接由本票提供；可信身份、HTTP、90 分钟等待及 connect-bot 调查由接入票提供；同目录记录、通知检查边界、可靠写入由持久化票提供；新执行身份、迟到报告及依赖重用由故障票提供；查看现场由观察票提供，跨模块闭环由交接票复核。后者未完成不代表能力已实现。停止票是处置证据，不被当作进程恢复提供者。
- 追踪结论：独立放行的上游冲突已获用户明确修订并发布；未发现需新建票或另加问题的未覆盖缺口。观察票原文“独立展示”和历史输入中的独立放行提法属于被替代设计输入，恢复时应消费现行状态机权威，不是并存政策；不批量改写下游 Handoff。原地图两片 fog 尚未获得新的具体问题，保留。下一 frontier 为[节点故障、等待人工处理与重试](05-fault-pause-retry.md)，本会话不认领。
- 证据：静态读取 omp 的 subagent／IRC 提示词、IrcBus 与 IrcBridge，hyper-auto-coding 的 collect，以及当前后端 POST／SSE 路由。具体路径和观察限度见通信 Lighthouse。未执行产品、真实 Agent 或 connect-bot 底层核查；本次只校验发布文件和 tracker 关系。

## Question

Run 参与者之间如何统一传递消息、交付引用与反馈，并让等待中的管理者或执行者继续？

覆盖寻址、消息类型、问题回复关联、输入交付绑定、稳定交付引用、消息游标、有界 collect、无人等待时的保留、Chat 空闲后的实际投递与唤醒。明确消息保存／送达／处理的区别，以及消息与运行命令的边界。判断在当前会话还是单独会话由本票决定。排除 transcript、流式输出和宿主心跳。

## Inputs

- [Bot、Task、Chat 与管理角色的绑定](01-execution-binding.md)

- [第二阶段交接计划](../../workflow-map-runtime-tracer-implementation-plan.md)。其中暂停／重试等排除项须与本轮范围修订合并阅读。
- [领域词汇](../../../GLOSSARY.md)。

## Comments

- 建图时列举的候选已按本轮确认收口或交接；现行决定以 Handoff 链接的 Lighthouse 为准。具体接入与存储实现仍待对应票据，本票不授权实现产品。
