# 跨模块设计收敛与实施交接契约

Type: grilling
Status: resolved
Assignee: none
Map: ../map.md
Blocked by: 01, 02, 03, 04, 05, 06, 07, 08, 09
Modules: 全部六个 Module

## Handoff

- 2026-10-28 Traverse 审计恢复点：全部 10 张决策票及 Lighthouse 已遍历，依赖、复用模式、分层和相邻接口均已检查。审计两组决策缺口已由当前会话 Q1–Q4 及最终“同意”统一发布：修订见[字段权威](../lighthouse/02-map-definition-contract.md)，装配及完整 trigger 补充见[接入权威](../lighthouse/08-omp-tool-integration.md)，类型配置入口见[观察权威](../lighthouse/09-observation-experience.md)，交付检查见[交接权威](../lighthouse/10-spec-acceptance.md)。无未回答的审计问题；所有票维持 resolved，不另建汇总票。
- 确认来源：Q1“既然如此那就直接记录路径”；Q2“同意，一次性迁移即可”；Q3/Q4“同意”；用户随后同意按收口汇总统一发布、更新票据与地图并追踪。候选稳定 zone ID、Task 运行时旧格式兼容均未采用；原因分别是复用现有路径登记及一次性切换。
- 审计证据：backend/src/zones.rs:28、frontend/src/zoneApi.ts:7 仅提供 path；backend/src/tasks.rs:45 的旧 Task 无用途类型；chats.rs:209 的 compose_initial_input 提供初始 Skill 组合；TasksPanel.tsx 提供现有类型管理可复用入口。toolkit framework/core/startup.ts:63 按 ID 读取启用策略，core/config.ts:186 默认按 ID 目录定位配置。静态证据不证明新能力已实现。

- 已确认决定及逐项验收统一发布到[交接 Lighthouse](../lighthouse/10-spec-acceptance.md)。2026-10-28 用户明确要求“先将决策记录到ticket，完成wayfinder的结束工作，先不要发布spec.md”；本次不生成或修改 spec，不创建实施票据，不实现产品。
- 决策范围已收口，无待回答采访问题。下一步是用户授权后整理 spec 的模块接口、逐字段数据/操作/消息契约、HTTP 路由及配置键；这是尚未交付的整理工作，不宣称 schema 已发布或地图最终文档已全部完成。出现新的行为选择或冲突必须再讨论。
- manager-chat 启动过程仅日志、当前错误内存展示、重启后查日志的规则已在原[观察权威](../lighthouse/09-observation-experience.md#manager-chat-启动错误的展示边界)修订；不新增错误持久化或进度页，不取消 node 失败事实保存。
- Q6 替代最初长期文档引用 Lighthouse 的方案；最终长期文档须独立完整，删除临时材料后仍能阅读。Q2 替代原计划与本票旧问答/停止/独立放行清单，具体六组验收与四阶段顺序只在交接 Lighthouse 维护。
- Backtracer：本轮交接/验收意图由绑定、字段、状态、通信、故障、持久化、接入与观察提供；停止依赖由已确认处置和 pause/resume 替代。启动进度旧要求已按 Q4/Q7–Q9 修订，无剩余冲突；长期文档独立性由本票规定交付、接入提供运行文档与 Prompt 载体、故障票提供指南需求。未发现需新开决策票的缺口或层级违规；spec 的机械整理与真实能力验证不伪装成已完成。
- 证据限制：只做设计/静态核对；本轮阅读 chats.rs 创建状态、lib.rs 创建回执/查询及错误格式、config.rs 服务配置模式。未运行产品、测试、浏览器或真实 Agent，现有源码不是目标能力证明。

## Question

各模块契约是否足以形成不依赖实施者猜测的 spec，并覆盖已确认的边界场景？

新增交付检查问题：如何在最终 spec 与实施交接中纳入用户故障处理指南及管理者工具使用指引，并按真实工具名称、观察／Chat 入口核对其可操作性？用户已明确要求纳入，权威见[故障恢复与使用指引](../lighthouse/05-fault-pause-retry.md#用户指南与管理者工具使用指引)。本票决定最终交付组织与检查方式，不重新制定恢复政策，不将指南要求解释为新增恢复按钮或完整运维手册。

确定后续 spec 的组织、六组现行验收、使用指引、长期文档独立性和实施顺序；主链路为固定文件材料 A、B → C，涵盖提前交接、重复派发、错误归属、暂停人工介入、重试、写盘失败与重启只读。原问答、独立停止与放行要求按已发布范围替代，不重新纳入。制定真实 smoke 的环境前提与证据要求，不在本地图执行产品验收。用户已要求本次暂不发布 spec；逐字段契约整理留待后续授权。

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

- 2026-10-28 本轮确认索引：Q1 实施入口组织；Q2 六组新版验收；Q3 三类使用指引；Q5 固定路径文件 smoke；Q6 长期文档不得依赖临时 spec/Lighthouse；Q10 四阶段顺序，均获用户“同意”。完整决定仅维护在交接 Lighthouse。
- Q4 用户否定启动进度页；Q7 保留错误展示；Q8 拒绝 JSON 承载 manager-chat 启动错误历史；Q9 同意当前后端内存保存错误、刷新保留、重启查日志。被拒绝的进度字段与错误持久化候选不再作为待办。
- 最终用户明确要求记录决策并完成 wayfinder 结束工作，暂不发布 spec.md；因此本票 resolved 表示决策收口，不表示 spec、长期文档、产品或验收已交付。
