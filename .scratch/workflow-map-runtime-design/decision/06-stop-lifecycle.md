# 停止 Map 与 graceful shutdown 生命周期

Type: grilling
Status: open
Assignee: none
Map: ../map.md
Blocked by: 03, 04
Modules: 流程控制；运行状态

## Handoff

- 建图阶段创建，尚未认领或解决，无 Lighthouse。
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
