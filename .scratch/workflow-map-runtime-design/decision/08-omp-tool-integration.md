# omp extension 接入与可信执行身份

Type: grilling
Status: open
Assignee: none
Map: ../map.md
Blocked by: 01, 03, 04, 05, 06
Modules: Bot 执行；流程控制

## Handoff

- 建图阶段创建，尚未认领或解决，无 Lighthouse。
- 已确认的努力边界与确认来源见[地图 Notes](../map.md#notes)；不将本文待讨论方案当已批准契约。
- 下一问题：查明现有 Task 扩展加载与 omp 会话入口，再选择可信身份和上下文隔离方案；环境事实由工具核查而非询问用户。
- 剩余范围：工具注册落在 my-agent-toolkit extension；检查 oh-my-pi 的实际加载、会话隔离、消息继续和工具等待能力。明确凭据注入、生命周期、展示脱敏、请求鉴权及 API 契约。不能依赖模型自报任意 ID；不能用当前开发会话工具冒充产品能力。参考 hyper-auto-coding 的等待语义，不搬入编码领域流程。
- 证据限制：仅有原交接计划、领域词汇及本轮讨论；未验证当前产品实现、外部宿主行为或真实运行。具体代码事实在工作本票时核查。

## Question

如何在产品启动的 omp Chat 中加载真实工具，并可信地绑定运行角色与执行身份？

工具注册落在 my-agent-toolkit extension；检查 oh-my-pi 的实际加载、会话隔离、消息继续和工具等待能力。明确凭据注入、生命周期、展示脱敏、请求鉴权及 API 契约。不能依赖模型自报任意 ID；不能用当前开发会话工具冒充产品能力。参考 hyper-auto-coding 的等待语义，不搬入编码领域流程。

## Inputs

- [Bot、Task、Chat 与管理角色的绑定](01-execution-binding.md)
- [运行状态机、管理者工具与决策 seam](03-runtime-state-operations.md)
- [通用协作消息、交付与决策会话](04-collaboration-protocol.md)
- [节点故障、等待人工处理与重试](05-fault-pause-retry.md)
- [停止 Map 与 graceful shutdown 生命周期](06-stop-lifecycle.md)

- [第二阶段交接计划](../../workflow-map-runtime-tracer-implementation-plan.md)。其中暂停／重试等排除项须与本轮范围修订合并阅读。
- [领域词汇](../../../GLOSSARY.md)。

## Comments

- 所有候选接口、状态名称和会话组织方式均待 live grilling；本票不授权实现产品。
