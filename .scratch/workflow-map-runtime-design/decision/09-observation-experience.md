# 运行启动、只读图与人工观察路径

Type: grilling
Status: open
Assignee: none
Map: ../map.md
Blocked by: 02, 03, 04, 05, 06
Modules: 观察与人工控制

## Handoff

- 建图阶段创建，尚未认领或解决，无 Lighthouse。
- 已确认的努力边界与确认来源见[地图 Notes](../map.md#notes)；不将本文待讨论方案当已批准契约。
- 下一问题：先确定用户选择 Map 与运行环境的启动路径，再用暂停与重试历史验证信息层级。
- 剩余范围：明确启动入口与配置、快照图、节点执行与放行的独立展示、消息交付错误详情、管理者／节点 Chat 入口、多次执行历史、刷新不重复启动。复用现有静态图；不做独立前端状态机或专门人工审批。用户未最终选择从 Map 还是 zone 启动，不将此前建议视作批准。
- 证据限制：仅有原交接计划、领域词汇及本轮讨论；未验证当前产品实现、外部宿主行为或真实运行。具体代码事实在工作本票时核查。

## Question

用户如何发起并观察一次 Run，识别暂停或故障，并通过关联 Chat 介入？

明确启动入口与配置、快照图、节点执行与放行的独立展示、消息交付错误详情、管理者／节点 Chat 入口、多次执行历史、刷新不重复启动。复用现有静态图；不做独立前端状态机或专门人工审批。用户未最终选择从 Map 还是 zone 启动，不将此前建议视作批准。

## Inputs

- [Map、Bot 与运行配置的契约边界](02-map-definition-contract.md)
- [运行状态机、管理者工具与决策 seam](03-runtime-state-operations.md)
- [通用协作消息、交付与决策会话](04-collaboration-protocol.md)
- [节点故障、等待人工处理与重试](05-fault-pause-retry.md)
- [停止 Map 与 graceful shutdown 生命周期](06-stop-lifecycle.md)

- [第二阶段交接计划](../../workflow-map-runtime-tracer-implementation-plan.md)。其中暂停／重试等排除项须与本轮范围修订合并阅读。
- [领域词汇](../../../GLOSSARY.md)。

## Comments

- 所有候选接口、状态名称和会话组织方式均待 live grilling；本票不授权实现产品。
