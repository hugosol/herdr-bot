# Map 提示词整体优化：范围、标准与进度

## 下一会话从这里开始

用户决定以 Map 为单位，在新会话逐项优化提示词。本文件记录计划与已做工作，不表示全部优化或业务验收完成。先使用 writing-for-agents 技能；优化前核对当前文件，保留其他会话已有修改，不把本文的历史计数当作当前工作树快照。

对比基准：Git 提交 `78a6955a01507d5934b813bd635b28170718e6a6`（`workflow-nesting checkpoint`）。范围是该基准到当前文件，包含后续提交和工作区修改，不只看未提交 diff。初次调查时 HEAD 为 `6e6b8d7`；后续扫描已观察到其他提示词变化，下一会话应重新确认当前 HEAD 和实际差异。

已完成“独立修订既有 Bot 三指南”的局部精简、2026-11-09“创建并优化 Task＋Bot”三节点外层优化，以及本文件下述包装八节点与共享能力的文本／定义优化。真实模型行为验收仍未完成。逐项进度与证据见下文；本文件不授权自动启动全部优化、付费模型矩阵、修改用户部署或票据状态。

## 背景与原问题

主要业务改造是把原十节点“创建并优化 Task＋Bot”拆成三节点外层和八节点包装子流程：外层创建、优化 Task，再调用“为既有 Task 或 Map 包装完整 Bot”。包装能力可以独立使用，也可被其他 Map 嵌套调用。

三个指南编写 Bot／Task 在 checkpoint 时已经由创建与修订流程共享，不是这次才共享。此次统一了 Task／Map 双目标、共同 baseline、唯一 text、必要 handoff 和最终一次保存等契约。共享的是能力与材料协议，不是不同目标 Bot 的指南正文。

修订 Map 同步取消“先优化 Task.prompt”的前置步骤，改为只读核实共同基线；规划链支持发现和调用 Map 型 Bot。

问题证据入口：[三类阻塞与后续补充](20261107-ticket22-three-blockers-pause.md)。其中观察到：
- w1 在交付共同基线前索要未来三字段全文批准；补充阶段说明后仍无完整通过证据。
- 模型扩大为 manager 职责、把已授权目录中新建材料误判为未授权、手写错误 HTTP。
- 已观察到的 HTTP 400／403／409 拒绝不等于后端身份授权缺陷。
- 最新附录确认部分运行因 driver 缺 `reportConfirmationTimeoutMs` 导致 run／collect 工具未注入；那些运行不能作为模型职责理解偏差的证据。

不能把所有失败归因于提示词。首次输入、角色工具装配、report 原生 UI 和模型传输均有独立证据边界。提示词优化有效性必须通过实际行为验证，不能由规则存在或最后 completed 推断。

## 实际生效层与指引归属

Bot 的 `when`、`plan_guide`、`node_guide` 供规划者使用，不会自动注入执行 Chat。节点实际收到 Task 固定提示、node.input 和运行时指引。只修改 Bot 指南不能直接纠正节点执行行为。

| 层 | 权威职责 | 不应塞入的内容 |
| --- | --- | --- |
| Task.prompt | 稳定执行步骤、角色能力范围、适用分支、每步完成条件；支持独立 Chat | 某张 Map 的固定节点编号、兄弟调度安排、无关阶段执行清单 |
| Bot.when | 能力发现、选用分支与易误选边界 | 完整执行手册、安全检查流水账 |
| Bot.plan_guide | 影响工作安排的前提、依赖、用户参与和交接判断 | 执行者接口操作细节 |
| Bot.node_guide | 调用方本次要提供的业务目标、材料用途、选择、权限和额外验收 | 重抄 Task 稳定步骤、教授框架路由语法 |
| Map node.input | 本次目标、材料用途、修改范围、适用分支、交付要求与额外验收 | 通用执行手册、HTTP 身份协议、其他角色的动作 |
| input_rule/output_rule/blocked_by | 文件来源绑定、交付标识、依赖 | 用自然语言重复模拟结构化接线 |
| 共同业务材料契约 | 材料定义、安全写入、基线比较、保存与失败核实等跨能力规则 | 每个消费者都执行完整流程的暗示 |
| 运行时角色指引 | node／manager 身份、工具、报告策略、父子 Run 控制边界 | 由业务 input 自行赋予身份或绕过工具 |

运行时装配参考 `backend/src/runs/startup.rs`，报告策略控制段在 `backend/src/scheduler/mod.rs`。实际角色工具与其提示词在独立 my-agent-toolkit 仓库，本轮没有完成该仓提示词审查。manager Task `task-18e2c2827c484128-0` 在初次对比中未变，固定 prompt 为空；不要据此推断实际管理者没有运行时指引。

## 优化标准

### 1. 分层与单一来源

每条指引都应有明确接收者和权威位置。删除重复之前，确认执行者能实际到达承接规则：Task 已包含，或存在明确触发条件的文档指针。Bot 指南、依赖边、父 Chat 和未声明文件都不能作为隐式承接。

稳定规则留 Task／共同契约，node.input 只写本次差异。服务地址对已有固定环境的 Task 不重复填写；通用替换者没有固定业务服务时，input 仍须提供实际服务与接口参考。

### 2. 按阶段组织，而非累加禁令

优先写当前允许动作和可检查的完成条件；保留必要硬边界，避免反复激活未来阶段。全流程委托是当前阶段的范围依据，不自动成为当前 Chat 的执行清单。

w1 基线交付不依赖未来三字段全文批准或最终保存；编写者确认所属完整正文；保存者核实整批后保存；manager 负责完成、派发与父子交接。

### 3. 指针与渐进披露

使用 writing-for-agents 的信息层级：当前步骤内联；共用定义集中；仅特定分支需要的细则通过有条件章节指针读取。指针说明何时读、读哪节、用于什么动作，避免泛指“遵守整份契约”让无关阶段淹没当前职责。

先强化指针和完成条件，再考虑拆文件。没有必要不要新建一套平行协议；也不要只是逐句缩短旧文而保留错误的信息结构。

### 4. 授权分清对象与范围

分别保留：修订委托、材料文件写入、各字段全文批准、最终 Bot 保存、manual report 原生 UI 发送确认。流程到达、报告确认或 can_complete 都不能代替保存授权。

配置只读与交接材料可写应并列清楚：明确授权在某目录新建本次材料时，可以选择无冲突的新文件名；目录存在本身不是授权。文件边界、链接安全和防覆盖要求不因精简而削弱。

创建流程另有表达优化节点，必须审查“编写时批准的版本”与“最终保存版本”的关系；不凭全局授权推断当前优化者能直接保存，也不把未经确认的语义变化当作表达整理。

### 5. 共享能力与完整性

一个共享 Task／Bot 只优化一次，同时检查全部调用 Map 和独立 Chat 分支。不能分别为创建图与修订图复制出两套稳定规则。

每个节点核对：输入足够、材料直接提供、输出用途明确、当前完成条件可检查、下一阶段可消费。保持真实目标、revision、非目标、完整读回、部分效果和禁止盲重试等要求；省略正文中的细节不等于取消执行约束。

### 6. 验证与完成声明

先做定义检查：JSON、真实 Bot／Task 绑定、材料来源、输出、依赖、报告策略、文档章节指针。避免提示词全文快照或关键词测试；它们不能证明模型遵循。

行为验收另外记录：真实输入进入原生历史、实际角色工具可用、材料原生读取、真实批准、文件字节、HTTP 副作用和阶段顺序。先窄场景反馈，再覆盖 Task／Map、有／无材料及独立 Chat；不盲重试挑成功。

未经真实模型验证只能标为“文本／定义优化完成，行为未验证”。遵守 `docs/agents/testing.md`、相关验收文档与用户模型授权；交接记录要求后续真实调用使用 `openai-codex/gpt-6-luna`，执行前核实当前授权与隔离依赖，不读取用户凭据或自动运行付费矩阵。

## Map 范围清单

下表基于对实际 Map→Bot→Task 引用的扫描，不只看 Map 文件是否修改。

| Map／文件（均位于 meta-data/workflow-maps） | 相对 checkpoint 的提示词范围 | 当前优化进度 |
| --- | --- | --- |
| 为既有 Task 或 Map 包装完整 Bot：`create-bot-wrapper.json` | 新增八节点图；空白创建者、三编写者、表达优化者、保存者的 Task 与 Bot 指南；包装 Bot | 文本／定义优化完成；通用优化者沿用既有优化，真实模型行为未验证 |
| 创建并优化 Task＋Bot：`create-bot-task-workflow--bc-61ce48b80a4c.json` | 十节点变三节点；w1/w2/wrap_bot input，Task 创建者、通用优化者直接保存分支及包装调用 | 外层文本／定义优化完成；W2 已恢复 optimize-prompt，专用优化者已删除；包装进度见上一行，真实模型行为未验证 |
| 独立修订既有 Bot 三指南：`enhance-existing-bot--bc-301b25d9f8d6.json` | 五节点；w1 换为基线核实能力，三编写者与保存者共享，新增修订包装 Bot | 局部精简完成；整体优化和行为验收未完成 |
| `plan_workflow.json` | 两 Task.prompt、两 Bot.plan_guide、outline.input；拓扑仍为两节点 | 整体待做 |
| Tibo监测后排查C盘：`tibo-monitor-then-c-drive-inspection.json` | Map 本身未改；其 Tibo-monitor Task.prompt 与 Bot.node_guide 有变更 | 独立外围项，待做 |

另有未被当前 Map 引用的“能力创建者 · Bot＋Task”Task，其 prompt 有修改，单列检查。旧独立 create-bot 不因新包装能力出现而自动改绑或迁移。

## 建议优化顺序与共享对象

### A. 共享能力层

先统一三个编写者、通用优化者、通用保存者与 `docs/business-materials.md`。这是包装图与修订图的共同依赖，不要在两个 Map 会话重复改造。

| Bot ID | Task ID | 用途 |
| --- | --- | --- |
| `write-bot-when` | `task-18e3404478db1ef8-2` | when 编写与确认 |
| `write-bot-plan-guide` | `task-18e340545eb481bc-3` | plan_guide 编写与确认 |
| `write-bot-node-guide` | `task-18e340f5287462ec-4` | node_guide 编写与确认 |
| `optimize-prompt` | `task-18e31d34797f1464-3` | 表达优化；创建图三路复用 |
| `replace-text` | `task-18e3415003d8cd00-5` | 最终保存；创建与修订共享 |

共享定义路径：`meta-data/tasks.json`、`meta-data/bots/*.json`。Bot 文件名不总等于 ID，按 JSON id 定位，不猜文件名。三个编写 Task 在本次局部精简前已包含较详细的信息筛选规则；保留其他会话修改，不用 checkpoint 内容覆盖当前稿。

### B. 包装 Map

空白创建者 `create-blank-bot` → `task-18e33fdc55177d1c-1`。
拓扑：create_blank → 三 write_* → 各自 optimize_* → save_bot。create_blank 产共同 baseline/handoff；编写和优化消费者直接取得 baseline；save_bot 消费 baseline＋三组最终 text/handoff，产 saved/handoff。

重点：既有目标与新建 Bot 的边界、批准版本与优化版本、三分支材料、包装 Bot 的调用说明。参考 `docs/create-bot-wrapper.md`。

### C. 修订 Map

w1：`prepare-bot-revision` → `task-revise-bot-baseline-v1`；w2–w4 是三个共享编写者；w5 是 replace-text。
拓扑：w1 → w2/w3/w4 → w5。修订在编写 Chat 内整理确认，没有独立表达优化节点。

结合共享能力优化结果复核全部 input，再检查 `meta-data/bots/prepare-bot-revision.json`、`meta-data/bots/revise-bot-guides.json` 的三份指南与真实执行能力一致。参考 `docs/revise-bot-guides.md`。

### D. 创建外层，再规划链

外层 w1：create-task → `task-18e31d30fe69a51c-2`；w2：optimize-prompt → `task-18e31d34797f1464-3`，仅保义优化并按授权直接保存；wrap_bot 调用 create-bot-wrapper。专用交互优化者已按用户决定删除，以下初次优化记录保留为历史，不作为当前绑定。
重点：完整委托、目标配置与后续授权交接，不复制子图执行手册；子 Run finish、父节点 report、父 manager complete 分开。

规划链：outline-planning → `task-18e318b13b71a8bc-0`；plan-to-map → `task-18e319d73777fc6c-1`。重点：Task／Map 能力选用、单节点嵌套调用、输入输出映射、本次 manager／zone；只有外层根 Run 启动配置可延后，不能据此省略 Map 调用节点自身配置。参考 `docs/planning-deliverables.md`。

### E. 外围项

Tibo-monitor Task：`tpl-18e0ceac0a89f9bc-0`，Bot：`tibo-monitor`。按自身业务链优化，不套用三指南材料流程。
独立能力创建者 Task：`task-18e315be1cf0ef10-0`，Bot：`create-bot-task`。已知基准差异主要为当前 Task API 的 suite 等接口更新；保留独立创建／复用职责。

## 已完成的局部精简：准确边界

本会话实际修改：
- `meta-data/tasks.json`：仅 `task-revise-bot-baseline-v1.prompt`。
- `meta-data/workflow-maps/enhance-existing-bot--bc-301b25d9f8d6.json`：仅 w1–w5 的 input。
- `docs/revise-bot-guides.md`：补充提示词归属说明。

具体结果：
- w1 Task 改为“核实委托与目标 → 形成共同基线 → 交付材料”，按阶段指向共同契约章节；明确未来全文批准不阻塞基线交付，并区分配置只读与授权材料写入。
- w1.input 只说明 request/materials 用途和本次文件交付；本图输入名不固化进可独立使用的 Task。
- w2–w4.input 只选择对应字段的修订、同一 Chat 内整理确认以及 text/handoff 文件交付；稳定核实和安全步骤由已有共享 Task 承接。
- w5.input 保留真实服务、三组来源与字段映射、整批替换范围和契约入口；删除指挥子管理者 finish/report 的段落。父层交接已有修订包装 Bot 与调用文档承接，不属于保存节点。
- 未修改任何 Bot、其他 Task、图拓扑、input/output 规则、report_policy 或 auto_close。

当时的字符统计：w1 Task 1732→1073；五节点 input 合计 3158→798。数字只记录该次修改，不是后续优化目标，也不是质量证明。

已实际执行一次内存中的 JSON／引用检查：逐一解析五节点 Bot→Task，验证上游输出和 Run 命名输入可解析，检查共同契约所引用章节存在，并与编辑前对象比较确认仅上述 prompt/input 字段变化。结果通过。检查没有新增永久测试文件。

未执行真实模型验收，也未运行后端完整回归。不能据此声称职责偏差、授权理解或完整五节点业务已修复。

仍待完成：共享能力／共同契约优化、两个修订相关 Bot 指南审查、共享变化后的完整链复核、真实模型阶段与授权验证。

## 2026-11-09：创建 Task＋Bot 外层初次优化（绑定已由下节取代）

本轮范围为用户选定的三节点外层及其两个执行 Task／Bot；包装八节点作为接口依赖核对，不认领其整体优化。已确认当前目录存在 `herdr-bot` codebase-memory-mcp 索引。对比 checkpoint 并以编辑前当前对象作为非目标保持基线，保留当前 Suite、宿主、模型与其他会话修改。

修改对象：
- `meta-data/tasks.json`：仅 `task-18e31d30fe69a51c-2.prompt` 与 `task-18e3454a33cecffc-6.prompt`。按核实、确认、保存读回、交付组织阶段及完成条件；保留当前 Suite/API、真实身份、共享影响、运行禁写、非目标保持、部分效果核实和禁止盲重试等约束。优化者在同一 Chat 使用 writing-for-agents，不另开表达优化 Chat。
- `meta-data/bots/bot-1793309187825217800-58200-2.json`、`bot-1793353937334260800-25960-12.json`：只调整 when／plan_guide／node_guide，分别承担选用、安排、本次业务输入。移除执行手册、框架接线教学及互相矛盾的变量替换说明，不更改框架规则。
- 外层 Map：只调整 description 与 w1／w2／wrap_bot.input。稳定检查由可实际到达的 Task 承接；保留自包含 Task 交接、w2.handoff 兼作 request、授权范围及父子交接边界。
- `docs/create-bot-wrapper.md`：增加“外层提示词归属”，区分外层 Task 配置 handoff 与包装内部 baseline／text／handoff。两个 Task 仅按需读取共同材料契约的文件安全章节，不把 Bot 正文协议套到外层交接。

授权按阶段分开：未来指南全文批准不阻塞 Task 创建；w2 交接前确认包装必需委托与权限；已有批准记录来源和范围，不推断最终 Bot 保存授权。manual 报告由运行时原生界面批准本次 payload，不能用业务确认代替。

共享影响：扫描当前全部 Map／Bot，create-task、existing-task-prompt-optimizer 和 create-bot-wrapper 的直接调用方只有本外层。两个 Task 的独立 Chat 分支同步适用新固定 prompt。包装的空白创建者、三个编写者、通用表达优化者、保存者这六个共享 Task 及其 Bot、包装 Map、共同材料契约均未修改；旧独立 create-bot／create-bot-task 和其他 Map 不变。

实际验证：运行一次内存定义检查，解析当前 JSON，遍历外层＋包装共 11 节点的 Bot→Task／Map 真实绑定、manager 类型、材料来源、依赖无环、子输入必填覆盖、save_bot 到父节点 saved／handoff 输出对应、报告策略及章节指针。与本轮编辑前对象逐项比较，确认只有上述 prompt／指南／description／input 变化，所有 Task 非目标配置、其他 Bot／Map 和八节点接线保持不变。结果通过；无新增永久测试或校验脚本。

未验证：真实 HTTP 保存、原生角色工具、模型遵循、授权交互及完整业务链。未启动真实模型验收、付费矩阵、用户业务或部署迁移。结论仅为三节点外层文本／定义优化完成，不能宣称完整 Task＋Bot 创建行为通过；包装内部优化和真实模型验收仍按独立范围推进。

## 2026-11-09：W2 恢复通用优化者

用户确认 W1 已负责业务澄清与全文确认，W2 改回 `optimize-prompt`。删除无其他 Map 调用的专用 Bot `existing-task-prompt-optimizer` 及 Task `task-18e3454a33cecffc-6`；只改仓库定义，不清理用户部署、Chat 或历史证据。

W1 明确记录后续保义优化／保存授权。W2 直接消费创建配置中的 prompt 与确认结论，不重新 grilling，保存前按 `docs/create-bot-wrapper.md` 的“外层 Task.prompt 直接保存”核实真实目标、共享影响、运行禁写、当前 API 与非目标保持。有效授权下的纯表达变化直接推进；语义变化、缺授权或冲突保持阻塞。报告策略仍为 manual、auto_close=false，不随 Bot 改绑改变。

通用 Task 只澄清已有两分支：共同 Bot 基线分支仍交付 text／handoff；直接保存分支接受完整配置中的原文，按平台是否提供 revision 核实，交付 saved／handoff。W2 新增 saved 输出保存完整最新 Task；handoff 仅保存身份、需求、授权、核对结论及包装委托，继续直接绑定子 request。包装自行读取目标，既有八节点接线与三路材料权限不变。

同步 outer_create 公开接口回归、真实外层验收入口与 19／24 当前清单；24 只拥有通用优化者 Task.prompt 直接保存分支，21 仍拥有 Bot 指南材料配对。旧初次优化结果与历史迁移记录保留，不再代表当前专用 Task 存在。通用 Task 实际宿主为 Pi、模型 gpt-6.1-sol，W2 随真实绑定使用它，不复制旧专用 Task 的 OMP 配置。

实际验证：`CARGO_TARGET_DIR=E:/Developer/myhub/herdr-bot/backend/target cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch outer_create -- --nocapture` 通过（1 passed），覆盖真实公开定义保存／读回、可选材料预检及缺失拒绝，无业务 Run 或宿主调用。一次内存检查通过：验收 Python 语法、实际 catalog/task_ids 选择、全部 Map 的 Bot／Task／Map 引用及材料来源、W2 saved／handoff 接口、专用对象缺席与章节指针；meta-data／backend／scripts／docs 无旧专用 ID 或文件名引用。历史迁移清单及本文历史记录有意保留。真实模型业务与已授权时不重复澄清的行为仍未验证，未启动付费矩阵或用户业务。

## 2026-11-09：通用优化者去业务化

按用户指出的上下文污染精简：通用 Task.prompt 删除 Bot 三指南、共同 baseline/text/handoff 分工、固定输出名称及业务操作禁令清单，只保留保义优化、有效确认、显式材料、安全文件交付和获准直接保存的条件分支。技能调用由 Task.skill=writing-for-agents 承担；按用户进一步要求，Task.prompt 已删除重复的技能调用指示，也不复述技能手册。通用 Bot 的 plan_guide/node_guide 同步改为调用方提供原文、约束、交付要求及按需平台／材料协议，不内置 Bot 业务。

W2.input 删除“本次不采用 Bot 三指南”等反向说明及与 Task 重复的规则，只明确 task_created 中的原文与依据、真实 Task 保存目标、按阶段读取的平台章节及 saved／handoff 用途。Task API 安全约束仍由“外层 Task.prompt 直接保存”承接；包装三路所需完整基线核实、语义确认、唯一正文和只写材料权限，已由各自现有 input 及共同材料契约承接，本轮未修改包装节点。独立调用通用优化者时，由调用方显式提供适用的业务协议和材料，而非默认加载 Bot 业务。

字符数仅作变更记录：Task.prompt 1542→724，W2.input 622→346。保留当前文件中的 W2 report_policy=auto、auto_close=true；前节 manual／false 是此前切换时的历史值，不再作为现状。使用文档改为引用实际报告策略，不缓存具体设置。

验证：内存 JSON／对象比较通过，修改限于通用 Task.prompt、Bot 两份指南及 W2.input，其他配置、四个调用方的材料绑定和包装定义保持不变，阶段章节指针有效。`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch outer_create -- --nocapture` 使用当前 worktree 的独立绝对 CARGO_TARGET_DIR，1 passed。真实模型保义、材料消费及无重复澄清行为未验证。

## 2026-11-09：Skill 与 Task 去重

对照 writing-for-agents 后，通用 Task 删除表达／组织方法、背景筛选及正文去重等 Skill 已承担的写作指示；保留 Skill 未定义的业务保义范围、有效授权、输入／交付契约、文件安全、配置保存与失败核实。具体完成条件仍属于本 Task，不因 Skill 要求“写清完成条件”而删除。prompt 从删除技能调用后的 702 字符减至 539 字符；Task.skill 仍为 writing-for-agents。

实际检查：JSON 解析及编辑前后对象比较通过，仅该 Task.prompt 改变，技能绑定与其他配置不变。未运行真实模型，不把文本去重视为行为验收通过。

## 2026-11-09：Task 保存规则归位

用户指出包装文档承载 Task 写入方法会混淆只读目标边界。已将原“外层 Task.prompt 直接保存”移至 `docs/usage.md` 的“仅修改 Task.prompt”，移除外层专属措辞与输出命名；包装文档仅链接 Task 保存规则，不再持有其副本。W2 保存指针和独立验收输入同步更新；W2 的包装 request 仍单独指向包装文档“调用接口”。以上历史章节中的旧保存地址已被本次迁移取代。

验证：新保存章节与包装接口指针可解析、唯一保存章节归属正确；JSON 比较确认只改 W2.input，验收 Python 语法通过。本次仅迁移文档归属及指针，不改变保存逻辑，未运行真实模型或业务流程。

## 2026-11-09：包装八节点与共享能力优化

范围以当前文件为准；开始时 HEAD 为 `1288d1acb8d28ff98e0cd2ee9af96ec48289e797`，已确认当前仓库存在 `herdr-bot` 索引。保留 checkpoint 之后及工作区的其他修改，不重新改造已完成的外层。

修改对象：
- `meta-data/tasks.json`：仅空白创建者 `task-18e33fdc55177d1c-1`、三个编写者 `task-18e3404478db1ef8-2`／`task-18e340545eb481bc-3`／`task-18e340f5287462ec-4`、保存者 `task-18e3415003d8cd00-5` 的 prompt；对应五个 Bot 仅改 when／plan_guide／node_guide。按核实、当前工作、确认和交付设完成条件，保留三字段各自的信息筛选规则；Skill 调用沿用现有配置。
- `meta-data/workflow-maps/create-bot-wrapper.json`：仅 description 与八个 input；`meta-data/bots/create-bot-wrapper.json`：仅三指南。固定服务不在已内置服务的创建／编写节点重复，通用优化者与保存者仍显式取得真实服务及业务协议。
- `docs/business-materials.md`：改为按职责读取，新增“共同基线消费”和“表达优化”；保留已有章节指针。集中承接完整配置／真实 revision 比较、独立 Chat 无基线分支、材料分工与文件安全；明确授权目录中新建材料可选择无冲突文件名，目录存在不等于授权。基线交付不等待未来全文批准；最终 text 必须有仍有效的确认覆盖，语义变化或本次要求最终版本批准时重新确认。
- `docs/create-bot-wrapper.md`：用职责章节索引承接执行规则；包装 Bot 的规划指南指向“调用接口”，保存节点不再指挥 manager。父子 finish／report／complete 边界仍由调用接口及外层承接。

共享影响：三个编写者与保存者同时供包装和修订 Map 使用；修订现有 input 已显式提供共同 baseline、所属字段、同 Chat 整理确认及保存协议，保持不改。空白创建者只被包装 Map 引用。通用 `optimize-prompt` Task／Bot 完全不改，其四个调用为包装三路与外层 W2；三路 Bot 专属规则由本次 input 的章节指针承担。保存者同样去除内置 Bot 专用协议，独立 Chat 通过本次显式协议选择业务分支。外层 Map、修订 Map、旧独立创建入口和其他 Map 定义保持不变。

验证：一次内存检查通过，解析全部五张 Map 的 Bot→Task／Map 绑定、子输入必填覆盖、manager 类型、材料来源与依赖无环，核对章节存在；与编辑前对象比较，确认修改仅限上述文本字段，所有 Task 非目标配置、八节点接线及报告策略保持不变。使用本 worktree 的绝对 `CARGO_TARGET_DIR=E:/Developer/myhub/herdr-bot/backend/target` 执行 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch create_bot_wrapper -- --nocapture`，1 passed；验证公开定义与材料接口，不证明模型遵循。无新增永久测试或临时脚本文件。

结论：包装及本轮共享能力的文本／定义优化完成。未启动真实模型、付费矩阵、用户业务或部署迁移；Task／Map、有／无材料、独立 Chat 配对、有效批准沿用和完整八节点行为仍未验证。修订图整体优化不在本轮认领范围。

## 每次新会话的交接要求

先确认当前实际修改和本次 Map 范围；列出其引用的共享 Task／Bot 及其他调用方。读取对应当前提示词和必要文档，按上述归属决定保留、删除或移动，避免在未核实承接的情况下删约束。

完成后更新本文对应进度，并记录：修改文件与对象、规则归属调整、共享影响范围、实际执行的验证及结果、未验证项。若只完成文本层，保持行为验收未完成；若发现工具装配或驱动问题，另记原因，不用业务提示词掩盖。
