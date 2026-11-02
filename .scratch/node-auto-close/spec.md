# Node 完成后自动关闭 Herdr Tab 与报告策略编辑

Status: ready-for-agent

Approved contract: [contract.md](contract.md) — 接缝、约束审查及 Full automation 验收模式已确认。

## Problem Statement

用户在 scheduler 中安排 Map node，并通过 workflow 中的 Run 执行工作。manager 接受节点交付后，节点关联的 Herdr Tab 仍然保留，需要用户逐个手动关闭。用户希望对具体 node 选择完成后自动关闭，减少终端清理操作，同时不能让 Herdr 关闭失败或响应迟缓阻塞业务流程。

节点已有 `report_policy`，但用户还需要在 scheduler 节点编辑器中直接选择报告提交策略，而不是仅通过定义数据维护。

## Solution

在 scheduler 节点编辑器增加默认关闭的 `auto_close` 开关，并用单选圆点呈现 `report_policy` 的两个选项，标签直接使用英文值 `auto` 和 `manual`。

节点派发时将 `auto_close` 固定到本次节点执行。manager 的 `complete` 操作成功使该节点进入 `completed` 状态后，如果本次执行启用了自动关闭，就非阻塞地触发一次 `herdr tab close <tab_id>`，目标仅限本次执行关联的 Herdr Tab。关闭结果只记录后端日志，不增加 workflow 前端结果显示，不改变业务完成状态，不重试、不补偿。

关闭 Tab 会终止其中的终端及相关进程，不是隐藏界面，也不是删除 Chat 历史。编辑器应说明这一后果。

## User Stories

1. As a Map 编辑者, I want 在每个 node 上独立设置 `auto_close`, so that 不同工作可以采用不同的终端保留选择。
2. As a Map 编辑者, I want 在 scheduler 节点编辑器中直接操作自动关闭开关, so that 不需要手工编辑定义文件。
3. As a Map 编辑者, I want 新节点默认不自动关闭, so that 未主动选择时不会终止节点终端。
4. As an 现有 Map 用户, I want 未配置 `auto_close` 的旧节点继续保留 Tab, so that 既有工作流不会突然发生破坏性行为变化。
5. As a Map 编辑者, I want 保存并重新打开节点后保留两个配置值, so that 编辑器操作确实改变可复用的工作定义。
6. As a Map 编辑者, I want 在开关旁理解关闭会终止终端及相关进程, so that 能知情选择自动关闭。
7. As a Map 编辑者, I want 用互斥的单选圆点选择 `auto` 或 `manual`, so that 报告策略可以直接查看和切换。
8. As a Map 编辑者, I want 报告策略选项标签直接显示英文值, so that 界面与定义数据中的值一致。
9. As a node Chat 用户, I want `auto` 保持结果可交接时报告的现有语义, so that 新编辑入口不改变报告行为。
10. As a node Chat 用户, I want `manual` 继续要求在本 Chat 获得用户确认后才发送报告, so that 补充和更正报告也遵守既有确认约定。
11. As a Run 用户, I want 自动关闭以 manager 确认节点完成为依据, so that 提交报告或结束一轮对话不会提前终止终端。
12. As a Run 用户, I want 节点派发时固定本次自动关闭设置, so that 运行中修改 Map 不会意外关闭已启动的终端。
13. As a Run 用户, I want 后续新派发使用当时的 Map 设置, so that 更新定义能应用到新的节点执行。
14. As an 历史 Run 用户, I want 缺少新字段的旧执行按不自动关闭处理, so that 历史记录不会产生新的终止动作。
15. As a Run 用户, I want 完成操作和下游推进不等待 Herdr 关闭返回, so that 终端清理不拖延业务流程。
16. As a Run 用户, I want Herdr 不可用或关闭失败时节点仍保持完成, so that 已验收的工作不会因清理失败回退。
17. As a Run 用户, I want 重复完成请求不会重复关闭 Tab, so that 一次完成仅产生一次关闭尝试。
18. As a Run 用户, I want 刷新页面和服务重启不补发关闭, so that 系统不产生延迟的意外终止动作。
19. As a Run 用户, I want 关闭目标仅限本次节点执行关联的 Tab, so that manager、其他节点和旧执行的终端不会被误关。
20. As a Chat 用户, I want 完成后主动恢复的 Chat 不被再次自动关闭, so that 可以查看或继续使用对话历史。
21. As an 运维者, I want 通过后端日志了解关闭尝试及失败原因, so that 排查问题无需引入新的业务状态或前端结果面板。
22. As a workflow 用户, I want 前端不增加自动关闭成功或失败反馈, so that workflow 仍专注于业务运行事实。
23. As a Run 用户, I want 无法可靠确定关联 Tab 时不猜测关闭目标, so that 配置自动关闭不会影响无关终端。
24. As a Run 用户, I want Herdr 要求额外确认时不被自动绕过, so that 原有关闭保护仍然有效。

## Implementation Decisions

- `auto_close` 属于 Map 的 node 定义，是可选布尔字段，缺省为 `false`；不从 Bot 或 Task 继承。读取、编辑和保存支持该字段，遵守现有定义编辑与校验方式。
- scheduler 节点编辑器提供 `auto_close` 开关；`report_policy` 使用两个互斥单选圆点，标签严格为 `auto`、`manual`，默认沿用现有 `auto`。
- `report_policy` 既有语义不变：它约束报告提交时机，不新增后端 report/complete 门禁，不改变 manager 的交接验收职责。`manual` 的用户确认发生在节点 Chat 中，不增加审批按钮。
- 节点派发时把 `auto_close` 记录在本次节点执行中。执行开始后修改 Map 不热更新它；恢复沿用该执行设置；新的独立派发或 retry 使用当时的 Map 设置。旧执行缺字段视为 `false`，无需为了启用功能改写历史执行。
- 沿用现有术语与状态契约：`complete` 是 manager 操作，`completed` 是完成后的节点状态。本功能不新增名为 `complete` 的状态，也不改变现有 complete 的权限、前置条件和完成说明要求。
- 只有成功提交的节点完成转换才有资格触发关闭。提交报告、agent 停轮、等待用户输入、进入失败或取消状态、Run 的整体 finish 均不是独立关闭触发器。既有规则允许 manager 完成的节点仍沿用该规则，不额外收窄可完成状态。
- 自动关闭启用时，使用本次节点执行可靠关联的 Tab ID 调用一次 Herdr 关闭命令。不得扫描其他 Tab、用名称猜测目标或改关 manager、其他节点、旧执行的 Tab。
- 业务完成先成立，关闭动作不等待返回才允许 complete 响应或下游推进；不得让执行关闭命令的等待占住业务推进所需资源。具体异步组织方式留待 `/to-contract` 及实现设计。
- manager 完成决定即允许立即关闭，无需等待 agent 空闲、退出或输出收尾文字；不引入宽限期和二次完成条件。
- 一次完成转换最多触发一次关闭尝试。重复 complete 不触发；页面读取、刷新、服务重启和恢复 Chat 不触发。这里不是跨崩溃的可靠投递或 exactly-once 保证。
- Herdr 失败、不可用、目标缺失或返回 `confirmation_required` 均不撤销完成、不阻塞业务，也不自动重试或绕过额外确认。目标无法可靠确定时不执行关闭。
- 关闭尝试及结果仅由后端日志承载，包含足以定位相关 Run/node/执行或已知 Tab 的上下文。不得把错误记录为关闭成功；不增加持久关闭结果字段、业务状态、workflow 结果展示或前端通知。
- 关闭最后一个 Tab 可能连带关闭 Herdr Workspace；沿用 Herdr 自身的关闭及确认语义，不新增本项目的确认流程。关闭不会删除原生 Chat 历史。
- 保持既有“节点执行角色来自执行上下文”的领域规则：此功能不改变 Task 类型、Chat 来源或恢复执行身份。
- 本规格不决定模块划分、接口接缝或测试接缝。既有 scheduler 定义编辑、节点派发、manager 完成流程及 Herdr 调用能力是相关现状，不在此指定新的抽象或执行框架。

## Testing Decisions

- 现有 scheduler 定义 CRUD 与浏览器验收可作为编辑、保存、重读及非目标定义保持不变的先例；已有报告策略默认值、非法值与执行期间固定策略的检查可作为配置行为先例。
- 现有 manager 完成逻辑区分实际状态转换与重复操作的无变更结果；后续契约阶段应沿用这一事实，不为关闭功能重新定义完成语义。
- 验证须遵守仓库测试指引：前端检查经 npm 脚本保留网络护栏；交付页面通过真实浏览器表面验证，前端单元检查不能替代交付界面验收。
- 自动化会话与进程场景使用测试拥有的隔离数据、临时目录、回环端口及受控 Herdr 替身或测试实例，不关闭用户正在运行的 Tab，不操作真实用户对话。
- 本次只产出规格，未运行功能测试或真实关闭命令；参考调查也没有证明真实 Tab 关闭或恢复的端到端行为。
- 测试接缝、接缝承载的承诺、覆盖分配及具体测试设计统一留给 `/to-contract`，本阶段不预定。

## Out of Scope

- 自动重试、后台重试队列、持久任务投递、服务重启补偿或跨崩溃 exactly-once 保证。
- 自动关闭的前端成功/失败提示、workflow 关闭结果、持久关闭状态及新的业务状态。
- 等待 agent 空闲、优雅退出协议、延迟关闭、宽限期或关闭前二次确认流程。
- 批量清理、全局扫描、关闭 manager、其他节点或旧执行 Tab，以及 Workspace 清理策略。
- 自动绕过 Herdr 的 `confirmation_required`。
- 自动恢复已关闭 Tab、启动替代 Tab、恢复进程、删除 Chat 历史或改变现有 Chat 恢复机制。
- Bot/Task 层面的自动关闭默认值或继承规则。
- 改变报告审批语义、增加审批按钮、把报告提交等同于节点完成。
- 更改 manager 的 complete/finish 权限、完成条件或下游调度规则。
- 决定实现/测试接缝，或在规格阶段实施产品功能。

## Further Notes

- 本规格综合本次对话中 Q1–Q9 的确认及后续修正。用户明确取消 workflow 节点关闭结果展示，只要求后端日志；报告策略选项最终确定为英文值 `auto`、`manual`。
- 接受的可靠性边界：服务可能在保存完成事实后、实际发出关闭命令前退出，因此 Tab 可能保留；不通过恢复扫描补关。清理失败无需用户处理才能继续业务。
- 领域依据为根目录 `GLOSSARY.md`；既有“普通与节点 Task 合并，节点角色由执行上下文确定”的 ADR 继续适用，本功能不改变该决策。
- Herdr 参考调查：[关闭 tab、进程生命周期与会话恢复](../herdr-tab-close-and-session-resume.md)。调查说明关闭命令、进程终止后果和额外确认边界，但不等同于本功能已经验证完成。
- 当前报告策略契约见 [scheduler 文档](../../docs/scheduler.md)，测试安全与执行约束见 [测试指引](../../docs/agents/testing.md)。
- 下一步：对本规格执行 `/to-contract`，在该阶段选择接缝并批准承诺；本规格标记 `ready-for-agent` 不表示实现或验收已完成。
