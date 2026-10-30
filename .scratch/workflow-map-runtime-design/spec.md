# omp Map 运行系统：需求与设计 Spec

Status: ready-for-agent
Label: ready-for-agent

## Problem Statement

用户已能在 scheduler 维护服务级 Map、node 及共享 Bot 能力定义，也能通过 Task 在 zone 中发起 Chat，但静态依赖图尚不足以支撑一次可观察、可交接、可人工恢复的实际流程。用户需要自行启动多个对话、跟踪前置工作、识别可交接结果并处理失败，容易混淆“Chat 已结束一轮”“报告已提交”和“节点工作已完成”。

缺少统一的运行事实及管理者工具时，跨 zone 的工作难以保持明确的执行关联，报告或通知失败容易被误判成工作失败，重试可能错误沿用旧执行结果。界面也需要区分当前执行、历史执行以及后端重启后不再接管的现场，而不是把静态 Map 或 Chat 在线状态当作运行状态。

本 spec 将已完成的 wayfinder 决策汇总为首版需求与设计输入，不宣称现有实现已具备这些能力，不以文档发布替代产品验收。

## Solution

用户从 scheduler 选择 Map，填写管理者 Task、zone 和可空的本次要求，发起一个独立 Run。管理者是直接由 Task 发起的 omp Chat，不要求先定义一个管理 Bot。它读取运行背景及记录，通过工具显式派发符合条件的 node、收取报告、记录完成、处置失败并结束 Run。

每次节点派发沿 node → Bot → Task 发起独立 omp Chat。节点使用自己的 zone，所有节点 Tab 进入管理者的实际 Herdr Workspace。节点只报告交接结果，管理者独占流程状态操作；后端校验身份、依赖和动作准入，保存事实，不替管理者自动选路。completed 同时表示交接完成和满足下游依赖，不要求 Chat 退出。

用户在 workflow 集中查看 Run、只读流程图、报告及执行历史，通过关联 Chat 的 Resume 进入现场。常见失败由人类核实或修复，再向管理者表达恢复、重试或手动完成意图。运行记录保留当前事实及历史，后端重启后旧 Run 只读，不自动接管。

## User Stories

### 定义与预设

1. As a 流程规划者, I want 区分 Map、node、Bot、Task 和 Chat, so that 静态工作、能力、预设和实际执行不会混为一谈。
2. As a 规划 Agent, I want 按阶段读取 Bot 的 when、plan_guide 和 node_guide, so that 能先筛选能力再规划和填写具体工作。
3. As a Bot 维护者, I want 先保存能力说明、稍后绑定 Task, so that 不必为未完成配置伪造执行预设。
4. As a Bot 维护者, I want 多个 Bot 可以显式共用一个 Task, so that 业务能力发现与执行预设能够分别维护。
5. As a 流程作者, I want 为 node 填写 Markdown input、登记的 zone 路径及前置依赖, so that 工作要求、执行位置和依赖关系明确。
6. As a 流程作者, I want 保存时拒绝格式错误、未知字段、缺失依赖和环, so that 不会保存内部结构无效的图。
7. As a 流程作者, I want 外部绑定未完善的合法定义仍可保存, so that 可以分步配置而不把“可保存”误当“可启动”。
8. As a Task 维护者, I want 在现有 Task 入口配置普通、manager 或 node 类型, so that 不需要额外维护一套角色配置。
9. As a Task 维护者, I want 角色固定扩展随类型确定且不能取消或混装, so that 合法角色 Task 始终取得对应工具。
10. As a 既有用户, I want Task 一次性迁移保留 ID、全部配置和历史 Chat 来源关联, so that 引入类型不会破坏现有预设与恢复路径。

### 发起与执行

11. As a 运行启动者, I want 从 scheduler 的 Map 发起页面选择管理者 Task 和 zone, so that 可以运行已有工作定义。
12. As a 运行启动者, I want 查看由管理者 zone 推导的 Workspace, so that 清楚本次对话将集中在哪里。
13. As a 运行启动者, I want 输入可空的 Markdown 本次要求, so that 同一 Map 能承接不同运行意图。
14. As a 运行启动者, I want 本次要求原文只交给管理者并保存在 Run 中, so that 不会被系统隐式改写为 node 输入。
15. As a 运行启动者, I want 启动前整图检查 Bot、Task、角色、agent 和 zone, so that 已知无效配置不会产生部分执行。
16. As a 运行启动者, I want 空 Map 启动被明确拒绝且不创建 Run 或 Chat, so that 空定义不会成为无意义运行。
17. As a 运行启动者, I want 同一个 Map 可以发起多个独立 Run, so that 不同执行不会共享运行状态。
18. As a 运行启动者, I want 创建 Run 后进入详情且刷新只读取已有 Run, so that 查看进度不会重复发起工作。
19. As a 管理者, I want 工作开始前真实 Chat 身份已经保存到 Run, so that 首次工具调用就有可靠归属。
20. As a 节点执行者, I want 每次派发使用独立原生 Chat、自己的 zone 和共同 Workspace, so that 上下文隔离与集中观察兼得。
21. As a Task 使用者, I want 首次启动及 retry 保留完整 trigger config, so that 模型、思考级别、初始 Skill 和固定指示不会因接入运行系统而丢失。
22. As a 管理者, I want 从快照查看 pending、阻塞原因、活动执行与结束条件, so that 能选择下一步而无需猜测文件状态。
23. As a 管理者, I want 明确列出 node 批量派发并获得逐项结果, so that 一个目标失败不会取消其他独立工作。
24. As a 管理者, I want 未满足前置依赖的派发被拒绝, so that 下游不会提前使用未交接结果。
25. As a 管理者, I want 重复操作返回已有事实而不重复创建执行或覆盖首次说明, so that 常规重复调用不会造成第二次效果。
26. As a 管理者, I want 各 Run 独立串行落实状态操作而节点 Chat 可并行工作, so that 操作一致性不牺牲独立工作的并行。

### 报告、完成与等待

27. As a 节点执行者, I want 仅提交报告正文、由系统确定归属与接收者, so that 不需要填写 Run 或角色身份。
28. As a 节点执行者, I want 报告描述结果地址与使用限制, so that 管理者和下游知道如何使用产物。
29. As a 节点执行者, I want 结果可交接时报告、随后继续不影响结果的收尾, so that 下游不必等待 Chat 退出。
30. As a 管理者, I want 报告不自动完成节点, so that 流程推进仍由显式工具动作决定。
31. As a 管理者, I want complete 同时满足下游依赖, so that 无需另一个放行动作。
32. As a 管理者, I want collect 返回快照及按 node 汇总的最新适用报告标识, so that 可以结构化地处理交接。
33. As a 管理者, I want 查看或收取报告后未完成待办仍保留, so that 阅读不会被误认为已经落实交接。
34. As a 管理者, I want collect 不依赖 Herdr 正文投递成功, so that 辅助通知失败不阻塞已保存的交接事实。
35. As a 管理者, I want 无活动执行且无新通知时 collect 返回原因, so that 不会无限空等而忘记派发或结束。
36. As a 管理者, I want 有界等待、取消和网络错误被明确区分, so that 超时不会自动改变运行状态或伪造快照。
37. As a 节点执行者, I want report 区分保存结果与通知结果, so that 不会仅因通知失败重复提交报告。
38. As a 管理者, I want 在全部节点 completed 后显式 finish, so that 成功结束不依赖 Chat 是否全部退出。

### 故障与人工处理

39. As a 人类处理者, I want 暂停阻止新执行但保留已有执行反馈, so that 可以处理现场而不必终止所有 Chat。
40. As a 管理者, I want resume 仅恢复派发资格、不自动派发, so that 恢复后仍由我选择动作。
41. As a 人类处理者, I want 区分派发失败与执行失败, so that 能选择核实、恢复或重试而不任意修改状态。
42. As a 人类处理者, I want 核实派发已创建时关联实际 Chat、未创建时恢复 pending, so that 不确定结果不会直接导致重复启动。
43. As a 管理者, I want recover 保留原 Chat、retry 发起独立新 Chat, so that 状态恢复与真正重新执行不被混淆。
44. As a 人类处理者, I want 知道重试不撤销旧执行副作用, so that 在启动新执行前处理重复写入等风险。
45. As a 管理者, I want 保留失败和旧执行记录、禁止旧报告冒充新执行依据, so that 历史不会因重试被抹掉或误用。
46. As a 人类处理者, I want 真实可交接的执行失败节点能够由管理者手动完成, so that 常见人工修复无需通用状态后门。
47. As a 下游执行者, I want 手动完成时仍由人工补齐结果地址和限制, so that 状态完成不会被误认为材料已经送达。
48. As a 管理者, I want 错误明确说明已提交状态或已启动 Chat 等部分效果, so that 不把部分生效误当成毫无效果。

### 观察、记录与维护

49. As a 运行观察者, I want workflow 按创建时间倒序列出全部 Run 和完成数量, so that 能找到当前及历史工作。
50. As a 运行观察者, I want 只读图同时用状态文字和视觉标记区分 node, so that 不依赖颜色或 Chat 在线状态判断流程。
51. As a 运行观察者, I want 节点详情展示工作定义、阻塞或失败原因、当前报告和执行历史, so that 能定位下一步人工处理。
52. As a 人类处理者, I want 从管理者、当前节点和历史执行进入关联 Chat 的 Resume, so that 可以沿真实对话处理现场。
53. As a 运行观察者, I want manager 启动错误刷新后仍可见且不伪装成进度页, so that 能理解启动失败及部分效果。
54. As a 运行观察者, I want 只在可见页面轮询、支持手动刷新并保留查询失败前的内容, so that 观察错误不会伪装成空运行列表。
55. As a 历史查看者, I want 明确知道图来自当前 Map、定义不可用时不补造历史图, so that 不把现有配置当历史快照。
56. As a 历史查看者, I want 后端重启后旧 Run 标为未接管、仍可查看事实和尝试 Resume, so that 不误以为恢复 Chat 就恢复了 Run 管理。
57. As a 维护者, I want 当前状态、状态历史和报告分开保存, so that 查询当前现场不依赖历史重放。
58. As a 维护者, I want 文件损坏、残缺追加及部分提交明确报错, so that 系统不以空数据、自动截断或假回滚隐藏问题。
59. As a Agent 使用者, I want Task 职能、工具使用指引和本次工作信息分开提供, so that 提示词职责明确且不维护多份手册。
60. As a 用户, I want 与实际界面一致的故障指南, so that 不必手填工具参数或直接改运行文件。
61. As a 扩展维护者, I want 复用 toolkit 的公共 HTTP、Prompt 与 adapter 能力, so that 业务协议不侵入 framework 或依赖 connect-bot 的开关。
62. As a 验收者, I want 真实 omp Agent 完成固定文件的并行汇合链路, so that 不能用模拟调用或手改记录冒充端到端交付。
63. As a 长期维护者, I want 删除临时设计材料后仍有完整的需求、契约和指南, so that 维护不依赖历史会话或临时票据。

## Implementation Decisions

以下只记录已批准的职责、行为及现有边界，不新增 seam。HTTP 路由拼写、未固定字段与配置键的最终形式、测试 seam 及其承诺覆盖留给后续契约工作；不能以命名整理引入新行为。

### 领域与职责

- Map 是服务级静态工作定义，node 是其中的具体工作安排，Bot 是共享业务能力定义，Task 是发起 Chat 的可复用预设，Chat 是实际原生对话。Run 是 Map 的一次执行；workflow 只是界面入口名。
- 既定职责包括定义管理、流程控制、Chat 执行接入、报告与 collect、运行持久化、观察与人工入口。复用现有 scheduler 定义编辑、Task 管理、zone 发起 Chat 和 Chat Resume，不新建平行预设或会话系统。
- 管理者独占状态操作；节点只报告。LLM 选择动作，后端检查并落实。决策调用者可替换的边界已决定，但首版只交付管理者 Chat，不建设策略插件平台或第二套状态执行逻辑。
- 不建立 Bot 运行实例对象，不把管理者伪装成业务 node。Task 与 Chat 数据不额外保存 Run 归属；当前归属以 Run 的真实 Chat 关联为准。

### 静态格式与 Task 类型

| 对象 | 已确认契约 |
| --- | --- |
| Map | 保留 title、description、按标识组织的 nodes；移除 schema_version；不含运行状态、管理者配置或重试策略 |
| node | 保留 title、bot_id、blocked_by；以字符串 input 替代 parameters；必填 zone 路径引用，沿用现有登记与发起规则，不新增稳定 zone ID |
| Bot | id、name、when、plan_guide、node_guide，以及可暂缺的 task_id；移除 use_when、description、inputs、outputs、implementation 和 schema_version 的旧结构 |
| Task | 增加普通、manager、node 用途类型；不是 agent 类型或执行身份；完整保留 trigger config 与 env config 的职责 |
| 本次启动配置 | 管理者 Task、zone、推导出的实际 Workspace，以及可空 user_input，属于 Run 而非 Map/Bot |

- when 服务筛选，plan_guide 服务规划，node_guide 服务 node 填写；三份独立文本可重复、可空，Markdown 章节不转成程序校验规则。Bot 说明不注入执行 Chat。
- node.input 支持 Markdown，承载本次工作、交付与交接要求，允许引用尚未生成的材料；不建设插值、参数类型系统、outputs 或动态材料绑定。Task.prompt 承载固定业务背景和限制。
- 保存严格检查未知字段、类型、必需字段、同图依赖存在性与无环性；外部 Bot/Task/zone 引用不完整不阻止合法定义保存。启动则整图检查有效绑定、正确角色、omp 和有效登记目录，不静默选择默认值，不审查业务质量。空 Map 可保存但不能启动。
- zone 引用保存目录路径，取消登记后不能启动，同路径重新登记后重新执行整图校验；不保证目录搬迁自动跟随。旧决策中的 zone_id 称呼不构成稳定 ID 承诺。
- 新 Task 默认普通类型；manager/node 仅允许 omp，前后端均拒绝非法组合。普通类型保留既有 agent 选择。
- manager 固定包含 manager-chat，node 固定包含 node-chat，普通类型不加载两侧角色扩展；角色扩展不进入可选列表、不允许取消或混装，类型切换随之改变固定角色扩展。其他可选扩展按既有规则处理；运行启动不偷偷补齐扩展。
- Map/Bot 存量仅有示例，按唯一新格式重建，不留旧格式兼容。已有 Task 一次性显式补为普通类型，保留 ID、名称、全部配置及历史来源关联，不按名称或 prompt 猜角色，不以运行时缺省兼容代替迁移。
- 平时配置可编辑、后续启动使用新配置；首版假设 Run 期间配置不变，不提供快照、版本锁、变更检测、编辑锁或动态加载保证。

### 启动与输入

- 每个 Run 绑定一个管理者 Chat；同一 Map 可有多个 Run。人类选择 manager 类型 omp Task 和 zone，Workspace 从该 zone 目录名推导，保存实际 Workspace 标识。
- 全部 node Tab 使用管理者实际 Workspace，但各自使用定义中的 zone。不可退回当前或默认 Workspace；目标不可用如实失败。
- 正常顺序是整图校验、初始化三份运行记录、启动管理者 Chat、取得原生 session_id、保存关联、再提交初始指示。node dispatch 和 retry 遵守相同身份与提交顺序。关联保存失败不提交指示，已经启动的外部 Chat 不假称回滚。
- 首版每次派发和 retry 发起独立原生 Chat。用户确认 omp 未开启 autoResume；不传恢复参数或 --session-dir，不改变原生记录位置，也不以新 Tab 证明原生上下文隔离。
- 完整 Task trigger config 保留：model/thinking 用于启动，初始 Skill、固定 prompt 和本次 input 复用现有组合规则。Resume 不重放 trigger config，需要新进程时仍使用来源 Task 的最新 env config，遵循既有恢复契约。
- Task.prompt 提供固定职能；extension 通过公共 PromptDeclaration 的 first-enabled-run 注入工具指引；本次 input 提供工作信息与路径。工具 description 不能替代工具使用指引。
- manager input 提供 Map、独立运行说明文档、三份记录的位置及 Run ID；末尾独立段落追加非空 user_input 原文。空值不追加，不修改 Task.prompt，不把完整 Map 或说明正文重复注入。
- node input 提供自身标识/名称、node.input 和运行说明及记录位置；不注入 Bot 文档、不自动传播 user_input、不复制整张 Map。各 Chat 能按绝对路径读取所需 zone 外文档及记录是环境前提，不扩大浏览器文件浏览权限，不增设启动可读性门禁。

### 状态与管理者工具

Run 状态为 running、paused、completed；node 状态为 pending、running、completed、failed。ready、阻塞原因和结束条件由当前状态与静态依赖计算，不另存生命周期。node.running 不表示模型正在输出；completed 表示可交接且满足依赖，不表示 Chat 退出。报告、轮次结束、进程退出都不能自动完成节点。

manager 注册 run 和 collect，node 注册 report。run 的 action 及参数只接受各自必要部分；调用者身份不由模型填写。

| run action | 参数及准入 | 效果 |
| --- | --- | --- |
| status | 无业务参数；可信管理者 | 返回快照，终态仍可查询 |
| dispatch | node_ids；Run running；逐项 pending 且全部前置 completed | 串行启动并返回逐项结果；拒绝不改状态，启动失败记 failed |
| complete | node_id、reason；Run 未结束；节点 running，或人工处理后可交接的执行阶段 failed | 转 completed 并满足依赖；不自动派发或伪造报告 |
| fail | node_id、reason；Run 未结束、节点 running | 记执行阶段 failed，保留 Chat，不自动暂停、重试或终止 Chat |
| reconcile_dispatch | node_id、核实结果、reason；确认已创建时附真实 Chat 引用；仅派发失败 | 已创建则补关联并恢复 running，未创建则恢复 pending，不创建 Chat；不确定保持 failed |
| recover | node_id、reason；执行阶段 failed，人工确认原 Chat 可继续 | 恢复 running，保留原 Chat 与失败历史，不重新启动或修复现场 |
| retry | node_id、reason；Run running、节点 failed；派发不确定先核实 | 发起独立新 Chat，成功转 running，失败仍 failed，保留旧执行与失败事实 |
| pause | Run running | 转 paused，阻止新执行，不等待已有 Chat 收尾 |
| resume | Run paused | 转 running，不自动派发 |
| finish | Run running 或 paused，全部 node completed | Run completed，不要求 Chat 退出 |

- 同 Run 状态操作串行，批量派发占用入口直到整个批次完成；后续 pause 等待批次，不中途撤回已启动目标。不同 Run 不共用该串行入口，节点工作仍可并行。
- 单目标拒绝或失败不阻止批次剩余目标，不回滚已启动者。正常已记录的派发不重复启动；重复 complete/fail/pause/resume/finish 不重复生效或覆盖首次事实。无效请求解释原因，pending 不可直接完成。
- 状态动作返回结果与同一串行边界内生成的快照；后续操作仍重新检查，不把旧快照当授权。
- paused 仍可报告、完成、记录事实及不启动 Chat 的核实/recover；retry 必须先 resume。普通失败不自动终结 Run。
- 终态拒绝状态变更，重复 finish 返回已有结果，查询仍可用。不提供任意 setStatus、JSON patch 或直接编辑运行文件的操作入口。
- 正常路径管理者应主动推进可执行工作，而非有 running 就空等；不内置固定工具链、业务阶段或后台选路，也不强制 manager 继续一轮。

### 报告、collect 与 Herdr 通知

- report 只接收正文，系统生成报告标识、时间和来源/目标/执行关联，固定通知同 Run 管理者。正文说明结果位置与限制，不建立 Delivery、命名产物表、产物复制或路径业务校验。
- 先保存报告，成功后更新索引和通知，再独立尝试 Herdr 正文输入。保存失败不投递、不形成待办；保存成功但投递失败不撤销、不自动补投、不暂停、不仅为此重复 report。
- 每次重复报告产生新记录；待办按 node 汇总最新适用报告，仅当前状态允许完成的节点形成可操作待办。读取、通知提交或 collect 检查不消除待办，completed 后不再列为待办。新执行不使用旧执行报告作为依据。
- 暂停不阻止已有关联节点报告，completed 后报告不重新打开节点或 Run；后端重启旧 Run 的只读限制优先。适用性仍由后端检查，不让报告越权推进状态。
- Herdr 单向输入只是辅助通知，正文标出 node 来源与报告标识；不保证原生 Agent 认证、严格时序、安全边界注入、打断 collect 或已阅读。提交成功只证明接入层确认的阶段，manager 可按标识读取报告记录。
- collect 无必填业务参数，返回结束原因、最新快照、按 node 汇总的待办及报告标识，不重复正文。暂停/结束直接返回；未检查通知立即触发检查；无人等待时保留通知，连续通知可合并，检查后的新通知留给下次。
- 无新通知且无 running 节点时不无限等；否则可以等待，不额外要求存在可派发 node 就必须立即返回。通知检查与待办完成是两件事。
- 默认等待 90 分钟、可配置，HTTP 预算略长于服务端等待期限；不复用 connect-bot 短上报预算。到期返回原因与现场，不自动 failed 或 pause。
- 工具中断或 Chat 结束取消请求，后端清理等待者。取消/网络失败不保证快照、不伪造正常结果、不自动重试。collect 等待不持有文件锁或 Run 状态操作入口。

### 扩展、身份与公共能力

- 在 my-agent-toolkit 中交付同一业务目录内的 manager-chat、node-chat 两份独立自动扩展声明及薄 omp 出口，不提供手动模式。两侧共享必要实现，不共享 Chat 身份、内存或连接。
- manager/node 消费 common，common 不反向依赖角色侧，foundation 不依赖 business；不为目录对称创建空层。业务协议保留在扩展，HTTP、Prompt 和宿主会话能力通过 framework/interface 与 adapter 使用，入口不承载业务。
- 两角色 ID 均接入现有自动启用策略并显式选择策略；仅有入口文件不算完成装配。业务配置显式定位 endpoint 与请求预算，以 config.path 和稳定 configBaseDir 定位，不依赖 node cwd 或两个角色 ID 默认推导出的目录。具体配置组织与键名沿用 framework 机制，不另建配置框架。
- extension 从 adapter 获取当前原生 session_id 并自动携带；后端每次解析 Run、角色、node 与当前执行。不按名称、目录、Tab 或 Task 猜归属，无关联或无法确定归属则拒绝。
- 首版运行工具请求只接受同机来源，使用后端关联校验而不签发独立凭据。session_id 不是秘密，明确接受同机进程冒用风险；这不是强认证或沙箱，不改写既有会话上报 API 的信任 ADR。
- 复用公共 HTTP，补充调用方取消支持；不复制客户端，不把 Run 协议放入 framework 或 connect-bot 私有实现。运行扩展不依赖 connect-bot 启用开关；connect-bot 不被当成现成的反向收信通道。
- 不增加归属/联系人查询、普通发送、广播或 extension 接收连接。活动期间原生会话切换的自动检测与重绑定不在范围。

### 持久化与错误语义

已决定采用业务无关文件层与每 Run 三份记录，属于既定边界，不在此扩展为新存储架构。

| 记录 | 职责 |
| --- | --- |
| run.json | 完整当前状态：Run 身份、Map 引用、创建时间、管理者 Task/zone/Workspace、user_input 原文、可空管理者关联、Run/node 状态、当前执行关联、必要失败阶段/原因及完成说明 |
| history.jsonl | 追加执行与状态历史，保存旧执行、失败、恢复及成功状态变化的操作标识、时间、操作者、动作、目标、前后状态和说明 |
| messages.jsonl | 追加报告，保存标识、类型、时间、来源、目标、执行归属和正文，不记录投递成功或阅读事实 |

- 每 Run 独立目录，存储根沿用项目数据配置，不锁定绝对路径。三份记录初始化全部成功后才启动 Chat，但不声称跨文件初始化原子。
- 当前查询不依赖历史重放；内存采用已提交状态。通知已检查位置、等待者和期限仅在内存，报告索引从已保存消息派生，不设 collect 游标文件。
- 通用文件层按调用者声明的文件集合、固定锁顺序协调排他访问，普通读取也遵守；不相交集合可并行。它提供读取、创建、整体替换和追加，不认识 Run 或 JSON 业务。
- 单文件整体更新写同目录临时文件、必要同步后替换，不先删除正式文件；追加不复制完整历史。Windows 的真实替换与故障行为必须验证，不宣称断电安全。
- 文件锁只覆盖实际读改写，不跨外部启动、投递或 collect 等待。Run 业务串行与文件锁是不同约束；Agent 直接读记录只是查看，不保证跨文件一致快照，动作仍通过工具校验。
- 状态先保存当前记录，失败不更新内存或追加成功历史；成功后更新内存再追加历史。历史失败明确“状态已提交、历史记录失败”，不回滚或声称未生效。
- 已有必要文件缺失、状态不可解析或记录损坏明确报错，不重建为空。已有 JSONL 尾部残缺时停止追加，由人工处理，不自动截断。
- 不提供跨文件事务、备份回滚、启动事务恢复或多进程写协议。错误可能留下部分提交，外部 Chat 启动也不可由文件层回滚；不承诺崩溃或关联保存失败窗口的完整幂等。
- 错误沿既有后端机制传播，补充阶段与已发生效果，HTTP 明确返回、界面不显示成功或自动重试。通常中止当前调用，不持续封锁 Run 或停服务；批量逐项处理等已有明确分支继续有效。
- 投递诊断记录 Run、报告、目标 Chat、结果及原因，不重复正文，不新增业务投递文件，不因日志失败撤销或重投报告。
- 后端重启后旧 Run 仅供读取，不重新接管、不修改旧状态，不推断 running 应转 failed/completed。拒绝旧 Run 的状态修改、report 和 collect；未接管是服务上下文事实，不新增持久 Run 状态。

### 常见故障与人工职责

- 人类核实或修复文件、环境和 Chat，通过管理者 Chat 表达意图；自然语言消息不直接改状态，不要求用户填写工具参数。
- 派发不确定先核实；recover 是原 Chat 的状态恢复，retry 是独立新执行。二者都不能隐含现场回滚、旧 Chat 终止或副作用撤销。
- 执行阶段 failed 手动 complete 必须真实可交接，不是跳过工作；状态动作不制造报告、不补发材料、不作额外质量校验。人工通过已有 Chat 补齐结果地址与限制，不假设有 manager→node 工具。
- 用户指南说明查看失败、Resume、核实/修复、表达恢复意图及副作用；manager 指引覆盖全部 action、collect、核实与交接责任；node 指引覆盖 report 时机、地址/限制、报告不等于完成及仅投递失败不要重发。

### 界面与查询

- chat 右侧新增 workflow 顶层入口。scheduler 的选中 Map 提供 + 发起页面，不在 zone 另设 Run 入口；不新增运行名称，以 Map 标题与创建时间识别、Run ID 定位。
- 发起页选择 manager 类型 omp Task、zone，只读显示推导 Workspace，提供可空 Markdown user_input；不临时覆盖 node 绑定、zone 或 input。校验失败留在发起页，取得 Run ID 后进入详情。
- 列表跨 Map、按创建时间倒序，显示标题、时间、Run 状态和完成数/总数，不增加分组、搜索、筛选或列表操作。
- 详情为概况、图、选中节点详情。图外提供 manager Resume，本次要求可展开、空值显示未提供。图复用现有布局和依赖方向，不把 manager 画成 node。
- 图使用状态文字与边框/填充/标记组合：pending 弱化及虚线，running 强调，completed 成功标记，failed 错误警示；选择外圈独立，不加动画，不以 Chat 在线状态代替流程状态。
- 节点详情展示 Bot/Task、zone、input、依赖、当前状态与阻塞/失败原因、完成说明、当前 Chat、当前报告正文/时间、折叠历史和旧 Chat Resume。无关联明确显示，不隐式创建执行。
- 观察只读，不新增 pause/resume/retry/complete 按钮或前端状态机。Resume 复用实际定位/恢复及错误反馈，不改变 node 状态；旧 Run Resume 不接管 Run，失败/未知如实展示。
- manager 启动过程只写日志，无进度页或新启动生命周期。启动错误与部分效果保留于当前后端内存供详情查询，刷新保留；重启后不重建页面错误，只查诊断日志，不增加日志浏览 API 或业务错误历史。node 失败仍按运行契约持久化。
- 列表/详情默认每 5 秒轮询、服务级可配置，无界面设置。仅可见 workflow 页面轮询，隐藏/离开暂停、恢复可见立即查询，不重叠请求，支持手动刷新。失败保留上次内容并显示更新失败与最后成功时间。
- 图注明来自当前 Map，不是历史快照。定义缺失、不可解析或节点无法可靠对应时不补造图，保留可读取的状态、要求、报告、历史和 Chat，必要时以保存标识显示名称。旧 Run 醒目标记历史现场/未接管。

### 交付顺序与长期文档

依次交付单节点真实闭环、并行依赖汇合、常见故障恢复、完整观察与交付收口。四阶段不是可选范围，工具指引和必要错误反馈随能力交付，不全部留到最后。

最终长期文档完整承接需求、模块职责、接口/数据契约、状态规则、限制及重要取舍；用户指南、运行背景说明和 extension 指引各满足其读者。长期文档、提示词和代码注释不得必须依赖本临时 spec、Lighthouse 或票据；删除临时材料后仍能理解、使用和维护系统。

## Testing Decisions

### 既定验收层次与仓库惯例

- 真实 Agent 主链路必须实际取证；确定性状态边界和故障检查补齐边缘情形，页面验证实际界面，不要求每个边界重复昂贵 Agent 全流程。具体 seam、承诺分组和覆盖归属由 `/to-contract` 选择，本 spec 不提前分配测试 seam。
- 既有后端集成测试使用隔离 TestServer，前端逻辑/组件使用现有 Vitest 入口，浏览器交付验收使用构建后的 Playwright 入口。遵守项目测试指引，不绕过 npm 网络护栏。
- 普通自动化只监听数值回环地址，各用例独占存储、临时 cwd、动态端口和必要 Herdr 副本；子进程持续消费输出并在失败/成功路径清理。不得操作用户现有对话或产物。
- LAN/mDNS、真实符号链接和关机遵守显式手动验收约束，不因测试环境默认授权。涉及 Windows 文件替换的保证在目标平台验证，不能靠模拟 API 宣称成立。
- 实际 Herdr/omp、角色 Task/扩展、有效 zone、共同 Workspace、所需路径可读及可用模型配置是主链路前提；遵守 omp 未开启 autoResume、运行期间配置不变、同机信任假设。缺少真实环境时不得以替身宣称通过。

### 六组验收范围

**一、真实 Agent 主链路。** 使用验收专用目录中的固定 A、B → C Map，不依赖不稳定外部服务。A/B 各生成预定的不同文件并 report 地址和限制；C 的 input 预先写好两个地址，实际读取两份材料形成结论文件。观察真实 manager 的 run/collect、A/B 并行独立 Chat、显式 complete 后 C 才能派发、全部完成后 finish。至少一项前置在报告后仍收尾，验证无需等 Chat 退出。保存真实工具结果、三份记录、身份关联和页面证据；人工干预须记录，不能用手改 JSON 代替。

**二、定义、启动与操作约束。** 覆盖严格新格式、内部图校验与外部引用可保存/不可启动之别、空 Map、zone 注销与同路径重登记、Task 类型及前后端角色扩展拒绝、一次性 Task 迁移保留全部配置与历史关联。验证双角色自动启用、稳定配置定位、不同 node cwd 下指引可得；首次启动和 retry 保留初始 Skill。验证初始化/关联失败不提交指示、已发生外部副作用明确暴露、依赖拒绝、普通重复操作、批次逐项结果、串行 pause、角色越权及终态约束。

**三、报告、collect 与通知。** 覆盖保存失败不投递/不形成待办、保存成功但 Herdr 失败仍可 collect、重复报告追加、按 node 汇总最新适用报告、检查不清待办、旧报告不作新执行依据、通知保留/合并、防空等、暂停/结束即时返回、到期原因及取消清理。观察 manager 工作中、空闲和 collect 等待时的真实 Herdr 表现，不将其提升为严格投递或阅读保证。网络错误不得伪造快照、自动重试或改变状态。

**四、常见故障与人工恢复。** 覆盖 pause 禁新执行但可报告/完成、resume 不派发、paused 拒绝 retry；派发不确定核实后的两条恢复路径；原 Chat recover 与新 Chat retry 的区别及失败历史保留；人工完成不伪造报告或自动补齐交接。依据真实界面与工具检查用户和 Agent 指引可操作，不扩展自动恢复、已完成节点返工或下游失效传播。

**五、持久化与重启。** 验证当前状态不依赖历史重放、状态保存失败不更新内存、历史追加失败明确部分提交、锁不跨外部等待、Windows 安全替换、不先删正式文件、文件损坏/残缺 JSONL 停止追加。重启旧 Run 只读且不改原状态，拒绝状态操作/report/collect，Resume 不接管。manager 启动错误刷新保留、重启不重建，不误取消 node 失败持久化。

**六、观察与文档。** 实际验证 Task 类型配置、scheduler 发起、workflow 列表/图/节点详情、当前/旧 Chat Resume、user_input 只读、状态非仅颜色、5 秒可配置轮询、可见性与无重叠请求、查询失败保留内容、历史定义降级、未接管提示。验证实际 Chat 获得工具指引及路径，长期文档独立承接契约，移除临时材料后无必要依赖。

本次只生成文档，不报告上述产品测试或真实链路已经通过。

## Out of Scope

- 本次 spec 发布不实现产品、不执行实施票据，不完成 `/to-contract` 的 seam 选择和承诺覆盖批准。
- 自动生成 Map/LLM Plan、规划过程编排、运行中改图、专门循环系统、定时调度、声明式自动重试或后台选路。合法既有动作组合不被额外禁止，但不据此新增返工机制。
- 非 omp 执行、非 Chat Adapter、通用宿主插件平台、Git/worktree/TDD/自动集成业务阶段。
- Bot 文档自动注入执行提示词、变量插值、动态节点传参、user_input 自动下发、Delivery 对象、产物绑定/校验/快照及额外业务质量审查。
- 独立放行、stop/graceful shutdown 及 pausing/stopping/stopped 生命周期、强制终止或等待所有 Chat 退出；pause/resume 仅承接停止新派发需求。
- manager→node、同伴消息、广播、普通双向协作、extension 接收长连接、原生 IRC 等价保证、强制续轮、联系人/归属查询工具。
- 通用人工审批、通知/接管产品、恢复按钮、任意状态修改、完整运维平台、checkpoint、已完成节点返工及下游失效传播。
- 强认证、独立凭据、会话切换自动重绑定、--session-dir 迁移、完整崩溃窗口幂等。
- 配置快照、多格式运行时兼容、稳定 zone ID、目录搬迁自动跟随、历史定义自动还原。
- 跨文件事务、自动回滚/修复、断电恢复保证、多进程写入平台、数据库引擎、全仓库文件层迁移、Chat transcript 汇总。
- 后端重启后自动或手动重新接管旧 Run，或通过 Resume 隐式恢复 Run 管理。
- 启动进度页、管理者启动错误业务持久化、日志浏览入口、额外运行命名、Run 列表搜索/筛选/分组。

## Further Notes

### 发布与下一步

2026-10-28 用户明确请求生成 spec，解除此前“先记录决定、暂不发布 spec”的文档发布限制；不扩大为产品实施授权。所有决策票保持原 resolved 状态及原确认历史，本 spec 不重新认领或重开它们。

已完成 `/to-contract` 并发布[获批运行契约](contract.md)：承诺、四个观察 seam 与 Rapid iteration 验收责任由该契约持有，不在本 spec 复制。下一步执行 **`/to-tickets .scratch/workflow-map-runtime-design/contract.md`**。HTTP 路由、逐字段序列化形式和配置键仍需按已批准契约及现有惯例整理，不能宣称本 spec 已发布可直接对接的完整 wire schema。若实施发现新行为选择、冲突或 seam 不成立，先修订契约，再调整受影响票据。

### 依据与现状

术语遵循 [领域词汇](../../GLOSSARY.md)；尊重 [Task 命名决定](../../docs/adr/0012-rename-bot-preset-to-task.md)、[Bot 命名决定](../../docs/adr/0013-rename-node-template-to-bot.md)、[zone 命名决定](../../docs/adr/0009-rename-workspace-to-zone-for-herdr.md)及[会话写入同机边界](../../docs/adr/0004-session-write-boundary-same-host.md)。本次是运行能力及格式演进，不恢复旧术语或旧别名，不更改既有普通 Chat Resume 语义。

本次静态核对当前定义结构仍有 schema_version、node.parameters 与旧 Bot implementation；Task 已分离 trigger/env 但尚无本设计的用途类型；Chat 初始输入已有 Skill、固定 prompt、本次 input 的组合；scheduler 已有图与定义编辑可复用。上述仅为实现基线，不是新运行能力证明。测试入口与隔离约束见[测试指引](../../docs/agents/testing.md)。

设计溯源：

- [Bot、Task、Chat 与管理角色的绑定](lighthouse/01-execution-binding.md)
- [Map、Bot 与运行配置的契约边界](lighthouse/02-map-definition-contract.md)
- [运行状态机、管理者工具与决策 seam](lighthouse/03-runtime-state-operations.md)
- [通用协作消息、交付与决策会话](lighthouse/04-collaboration-protocol.md)
- [节点故障、等待人工处理与重试](lighthouse/05-fault-pause-retry.md)
- [停止 Map 与 graceful shutdown 生命周期的范围处置](lighthouse/06-stop-lifecycle.md)
- [运行事实与内存文件一致性](lighthouse/07-persistence-consistency.md)
- [omp extension 接入与可信执行身份](lighthouse/08-omp-tool-integration.md)
- [运行启动、只读图与人工观察路径](lighthouse/09-observation-experience.md)
- [跨模块设计收敛与实施交接契约](lighthouse/10-spec-acceptance.md)

这些链接提供临时决策来源，不豁免最终长期文档独立完整的交付要求。
