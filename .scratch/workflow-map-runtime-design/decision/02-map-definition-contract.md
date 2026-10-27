# Map、Bot 与运行配置的契约边界

Type: grilling
Status: open
Assignee: none
Map: ../map.md
Blocked by: 01
Modules: 定义管理

## Handoff

- 建图阶段创建，尚未认领或解决，无 Lighthouse。
- 已确认的努力边界与确认来源见[地图 Notes](../map.md#notes)；不将本文待讨论方案当已批准契约。
- 下一问题：列出现有格式能表达与不能表达的需求，再决定信息归属；先确定策略的声明位置，不提前确定重试动作算法。
- 剩余范围：逐项覆盖依赖、输入、命名交付、执行绑定、管理配置和重试策略归属；定义版本、严格未知字段校验、迁移与不可运行定义的表达。不要把运行态加入静态 Map。重试策略是否写入 Map 是用户明确保留的未决问题。
- 证据限制：仅有原交接计划、领域词汇及本轮讨论；未验证当前产品实现、外部宿主行为或真实运行。具体代码事实在工作本票时核查。

## Question

当前 Map JSON 是否覆盖本轮需求？哪些信息属于 Map、Bot、Task 或本次 Run，哪些需要改变现有格式？

逐项覆盖依赖、输入、命名交付、执行绑定、管理配置和重试策略归属；定义版本、严格未知字段校验、迁移与不可运行定义的表达。不要把运行态加入静态 Map。重试策略是否写入 Map 是用户明确保留的未决问题。

## Inputs

- [Bot、Task、Chat 与管理角色的绑定](01-execution-binding.md)

- [第二阶段交接计划](../../workflow-map-runtime-tracer-implementation-plan.md)。其中暂停／重试等排除项须与本轮范围修订合并阅读。
- [领域词汇](../../../GLOSSARY.md)。

## Comments

- 所有候选接口、状态名称和会话组织方式均待 live grilling；本票不授权实现产品。
