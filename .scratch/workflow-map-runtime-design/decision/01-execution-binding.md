# Bot、Task、Chat 与管理角色的绑定

Type: grilling
Status: open
Assignee: none
Map: ../map.md
Blocked by: none
Modules: Bot 执行；定义管理

## Handoff

- 建图阶段创建，尚未认领或解决，无 Lighthouse。
- 已确认的努力边界与确认来源见[地图 Notes](../map.md#notes)；不将本文待讨论方案当已批准契约。
- 下一问题：先明确 Bot 的业务说明与 Task 的执行配置各自拥有哪些信息，再讨论管理角色是否需要 Bot。
- 剩余范围：明确 Bot／Task／Chat 的职责、引用关系、配置来源与覆盖顺序、缺失引用处理、快照范围与隔离要求。回顾管理者不经过 Bot 的暂定方案；首版只支持 chat，不建设多 Adapter 平台。
- 证据限制：仅有原交接计划、领域词汇及本轮讨论；未验证当前产品实现、外部宿主行为或真实运行。具体代码事实在工作本票时核查。

## Question

如何从 node 找到 chat 类型 Bot、解析 Task 与环境并发起独立 Chat？管理者是否直接以 Task 启动，还是也需要 Bot？

明确 Bot／Task／Chat 的职责、引用关系、配置来源与覆盖顺序、缺失引用处理、快照范围与隔离要求。回顾管理者不经过 Bot 的暂定方案；首版只支持 chat，不建设多 Adapter 平台。

## Inputs

- [第二阶段交接计划](../../workflow-map-runtime-tracer-implementation-plan.md)。其中暂停／重试等排除项须与本轮范围修订合并阅读。
- [领域词汇](../../../GLOSSARY.md)。

## Comments

- 所有候选接口、状态名称和会话组织方式均待 live grilling；本票不授权实现产品。
