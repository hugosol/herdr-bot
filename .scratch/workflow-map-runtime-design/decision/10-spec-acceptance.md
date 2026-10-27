# 跨模块设计收敛与实施交接契约

Type: grilling
Status: open
Assignee: none
Map: ../map.md
Blocked by: 01, 02, 03, 04, 05, 06, 07, 08, 09
Modules: 全部六个 Module

## Handoff

- 建图阶段创建，尚未认领或解决，无 Lighthouse。
- 已确认的努力边界与确认来源见[地图 Notes](../map.md#notes)；不将本文待讨论方案当已批准契约。
- 下一问题：逐项对照原计划和本轮范围修订，检查接口生产者与消费者是否一致，列出尚缺的设计输入。
- 剩余范围：汇总引用各 Lighthouse，不复制成第二套政策权威。形成 spec.md 的模块接口、数据／操作／消息契约与验收场景；涵盖 A、B → C、问答、提前交付、重复派发、错误归属、暂停人工介入、重试、停止、写盘失败与重启中断。制定真实 smoke 的环境前提与观察证据，不在本地图执行产品验收。检查未决或冲突并回到原票，不静默裁决。
- 证据限制：仅有原交接计划、领域词汇及本轮讨论；未验证当前产品实现、外部宿主行为或真实运行。具体代码事实在工作本票时核查。

## Question

各模块契约是否足以形成不依赖实施者猜测的 spec，并覆盖已确认的边界场景？

汇总引用各 Lighthouse，不复制成第二套政策权威。形成 spec.md 的模块接口、数据／操作／消息契约与验收场景；涵盖 A、B → C、问答、提前交付、重复派发、错误归属、暂停人工介入、重试、停止、写盘失败与重启中断。制定真实 smoke 的环境前提与观察证据，不在本地图执行产品验收。检查未决或冲突并回到原票，不静默裁决。

## Inputs

- [Bot、Task、Chat 与管理角色的绑定](01-execution-binding.md)
- [Map、Bot 与运行配置的契约边界](02-map-definition-contract.md)
- [运行状态机、管理者工具与决策 seam](03-runtime-state-operations.md)
- [通用协作消息、交付与决策会话](04-collaboration-protocol.md)
- [节点故障、等待人工处理与重试](05-fault-pause-retry.md)
- [停止 Map 与 graceful shutdown 生命周期](06-stop-lifecycle.md)
- [运行事实与内存文件一致性](07-persistence-consistency.md)
- [omp extension 接入与可信执行身份](08-omp-tool-integration.md)
- [运行启动、只读图与人工观察路径](09-observation-experience.md)

- [第二阶段交接计划](../../workflow-map-runtime-tracer-implementation-plan.md)。其中暂停／重试等排除项须与本轮范围修订合并阅读。
- [领域词汇](../../../GLOSSARY.md)。

## Comments

- 所有候选接口、状态名称和会话组织方式均待 live grilling；本票不授权实现产品。
