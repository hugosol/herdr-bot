# 停止 Map 与 graceful shutdown 生命周期

Type: grilling
Status: resolved
Assignee: none
Map: ../map.md
Blocked by: 03, 04
Modules: 流程控制；运行状态

## Handoff

- 已按运行状态机会话 Q66 用户“同意”确认本票不再需要；[范围处置 Lighthouse](../lighthouse/06-stop-lifecycle.md)为权威。以下 Question 保留原始问题，不再作为首版待办。
- 停止派发需求由 pause／resume 承接，不增加 stop、pausing、stopping、stopped，不保证活动 Chat 退出。原剩余范围与下一问题仅为历史背景。
- 依赖审查：运行事实与内存文件一致性、omp extension 接入与可信执行身份、运行启动与人工观察、跨模块设计收敛四张票据，原停止前置由本处置及运行状态机票据的暂停／恢复／成功结束输入替代；它们仍受其他未解决前置阻塞。追踪未发现新增缺口，详见 Lighthouse。
- 已确认的努力边界与确认来源见[地图 Notes](../map.md#notes)；不将本文待讨论方案当已批准契约。
- 下一问题：先明确停止请求被接受后立即保证什么，以及哪些事实成立后才进入终态。
- 剩余范围：决定停止新派发、通知节点、接收后续反馈、等待退出或保留现场、Run 结果及终态条件。区分成功完成、失败、中断、流程停止与外部 Chat 退出；不能假报全员停止。具体是否通知每个节点是未决问题，不默认承诺强制终止。
- 证据限制：仅有原交接计划、领域词汇及本轮讨论；未验证当前产品实现、外部宿主行为或真实运行。具体代码事实在工作本票时核查。

## Question

停止入口如何落实 graceful shutdown，活动节点与终态之间有什么契约？

决定停止新派发、通知节点、接收后续反馈、等待退出或保留现场、Run 结果及终态条件。区分成功完成、失败、中断、流程停止与外部 Chat 退出；不能假报全员停止。具体是否通知每个节点是未决问题，不默认承诺强制终止。

## Inputs

- [运行状态机、管理者工具与决策 seam](03-runtime-state-operations.md)
- [通用协作消息、交付与决策会话](04-collaboration-protocol.md)

- [第二阶段交接计划](../../workflow-map-runtime-tracer-implementation-plan.md)。其中暂停／重试等排除项须与本轮范围修订合并阅读。
- [领域词汇](../../../GLOSSARY.md)。

## Comments

- 所有候选接口、状态名称和会话组织方式均待 live grilling；本票不授权实现产品。
