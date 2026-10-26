# Scheduler：Map 读取、共享节点模板与依赖图预览

Status: ready-for-agent
Label: ready-for-agent

## Problem Statement

用户需要把工作计划保存为结构明确的 JSON，并在 herdr-bot 主界面检查具体工作、参数和前置依赖。目前主界面只有 chat、bot、zone、server 入口，没有用于浏览服务端 Map 文件、查看依赖图或关联业务能力说明的入口。

用户希望未来由 LLM 生成 Map，但本阶段先人工提供完整文件，验证格式与展示闭环。node_template 应是跨 Map 共享的业务能力定义，而不是每张 Map 重复声明的模板，也不应等同于 Bot 或“发起 Chat”这一底层动作。用户需要先看具体工作，按需查看关联模板的业务说明；即使模板缺失，也应能看着依赖图定位问题。

## Solution

增加 scheduler 入口，读取服务所在电脑上指定目录中的 Map JSON，列出文件并提供只读依赖图、节点详情、按需展开的共享模板明细，以及显式重新加载和错误提示。

Map 保存具体 node、模板引用、参数和前置依赖；独立共享模板目录保存业务能力说明、输入输出说明和实现类型。首阶段所有业务内容均可人工编写，系统实际读取文件并显示，不硬编码 UI 示例，也不执行模板。

同时将 server 从顶层 tab 移到右上角按钮，打开同页独立内容页并可返回之前的 tab。提供可供未来 LLM 参考的 Map、共享模板样例与格式说明。

## User Stories

1. As a 主界面用户, I want 顶部在 zone 后显示 scheduler, so that 我能直接进入计划定义浏览功能。
2. As a 主界面用户, I want 打开或整页刷新仍默认进入 chat, so that 新功能不会改变既有默认入口。
3. As a 服务使用者, I want Map 属于整个服务而非某个 zone, so that 一张计划能够表达跨项目的工作。
4. As a 局域网访问者, I want 查看服务所在电脑上的同一组 Map, so that 不需要在浏览器所在设备另备文件。
5. As a 服务维护者, I want 配置 Map 与共享模板的存储目录, so that 能按实际环境组织定义文件。
6. As a 计划编写者, I want 每张 Map 保存为独立 JSON 文件, so that 可以手工编写、复制和修订。
7. As a scheduler 用户, I want 列表显示目录直接包含的 JSON 文件, so that 可以选择要检查的计划。
8. As a scheduler 用户, I want 目录没有 Map 时看到明确空态, so that 不会把没有文件误认为加载故障。
9. As a scheduler 用户, I want 文件或目录读取失败时看到错误, so that 能区分空目录与无法读取。
10. As a scheduler 用户, I want 点击文件后看到自动布局的依赖图, so that 不需要自己安排节点位置。
11. As a 计划检查者, I want 所有节点包括独立节点都被展示, so that 不会遗漏未连接的工作。
12. As a 计划检查者, I want B 依赖 A 时显示 A 到 B 的箭头, so that 能准确理解前置关系。
13. As a 计划检查者, I want 多前置、并行分支及汇合都正确展示, so that 能检查非线性的工作安排。
14. As a 图形查看者, I want 平移、缩放和适应画布, so that 能查看不同规模的图。
15. As a 计划检查者, I want 节点卡片显示标题和节点 ID, so that 能将界面内容与 JSON 对应。
16. As a 计划检查者, I want 点击 node 查看具体参数与前置依赖, so that 能核对本次工作的安排。
17. As a 计划检查者, I want 模板能力说明默认不展开, so that 不会与具体 node 的内容混淆。
18. As a 计划检查者, I want 通过模板明细按钮查看关联模板, so that 可以按需核对业务能力。
19. As a 能力维护者, I want 一个 node_template 被多个 Map 引用, so that 不必复制维护同一能力说明。
20. As a 计划编写者, I want 同一 Map 多个 node 使用同一模板但提供不同参数, so that 可以安排同类但不同目标的工作。
21. As a 能力维护者, I want use_when 表达适用需求而 description 表达业务能力和边界, so that 未来搜索与规划可以使用这些说明。
22. As a 计划检查者, I want 查看模板命名输入与输出的文本说明, so that 能理解能力预期使用什么、交付什么。
23. As a 计划检查者, I want 查看实现类型 chat 而不启动任何工作, so that 能了解实现类别而不会产生副作用。
24. As a 计划编写者, I want 参数按 JSON 内容原样表达并展示, so that 当前阶段不受尚未设计的参数契约约束。
25. As a 计划检查者, I want 缺失模板的节点仍保留在图中并被显式标记, so that 能借助图修复引用问题。
26. As a 计划检查者, I want 不可读取或无效模板有明确问题提示, so that 不会把没有明细误认为模板已验证。
27. As a 计划编写者, I want JSON 解析失败及非法依赖被指出, so that 不会信任被静默删节点或删边的图。
28. As a 计划编写者, I want 修改文件后刷新列表或重新加载, so that 能验证文件变更而不依赖服务重启。
29. As a 能力维护者, I want 重新加载后看到共享模板的当前说明, so that 修改一次即可影响引用它的 Map 的展示。
30. As a scheduler 用户, I want 新文件加载失败时不把旧图当作新结果, so that 能明确当前查看内容是否有效。
31. As a scheduler 用户, I want 一个坏文件不妨碍选择其他文件, so that 能继续检查其余计划。
32. As a 窄屏设备用户, I want 文件列表、图与详情仍可访问, so that 手机也能核对计划。
33. As a 键盘用户, I want scheduler 与右上角 server 入口可通过键盘操作, so that 导航不依赖鼠标。
34. As a server 使用者, I want 通过右上角按钮访问原有固定操作, so that 不会因导航调整丢失既有能力。
35. As a server 使用者, I want 返回进入 server 前的 tab, so that 能继续之前的工作。
36. As a 未来定义生成者, I want 完整 Map、共享模板示例与字段说明, so that 能生成同一种受支持格式，而非另造图形数据。

## Implementation Decisions

### 已确认的领域层级

- Map 是服务级的静态工作定义，不属于 zone，可以组织未来在不同 zone 中开展的工作。
- node_template 是跨 Map 共享、相对稳定的业务能力定义。例子是“查询天气”“估算 Codex 额度重置时间”，而不是以通用“发起 Chat”承载所有业务语义。
- node 是某张 Map 中的一项具体工作定义，不是运行实例。它引用模板并提供本次参数；同一模板可被同一或不同 Map 的多个 node 使用。
- Bot 沿用现有定义：用于发起 Chat 的可复用预设。node_template 不等于 Bot；Chat、subagent 或固定代码属于能力的执行实现层。zone 可以作为某种实现需要的参数，不是所有 node 的固定字段。
- 本阶段只固化前置依赖，不把它与执行路由或材料传递混同。图可展示不等于定义完整，更不等于可以执行。

### JSON 格式

- 字段名统一使用 snake_case，格式版本字段为 schema_version，当前版本为整数 1。
- Map 包含 schema_version、title、description、nodes；不包含 node_templates，也不嵌入模板副本。
- nodes 是以 Map 内节点 ID 为 key 的对象，不再在值中重复保存节点 ID。
- node 包含 title、node_template_id、parameters、blocked_by。parameters 是命名参数对象，值为直接给定的 JSON 值；blocked_by 是同一 Map 内前置节点 ID 的数组。
- 共享模板包含 schema_version、id、title、use_when、description、inputs、outputs、implementation。
- 模板 id 是独立身份，Map 以 node_template_id 引用它，不依赖文件名。
- use_when 是适用需求与场景的文本；description 是泛化的业务能力、预期结果与重要边界的文本，不以启动系统资源的指令替代业务说明。
- inputs、outputs 均为“名称到说明文本”的对象，不引入 type、required 或参数验证表达式。outputs 表达预期交付，不保存真实结果。
- implementation 包含 type。后端使用 Rust 枚举表达实现类型，当前仅有 Chat，JSON 值为 chat；以后增加实际类型时再扩展枚举。不接受未知值后静默回退为 chat。
- 目前不声明具体执行配置，也不据 implementation.type 创建 Chat 或执行任何代码。
- 不把 session ID、运行状态、执行尝试、审批记录或实际产出写入 Map 或模板。

### 存储与读取

- Map 与共享模板分别存放在两个服务端目录中，均可配置；相对路径遵循项目既有的进程工作目录语义。具体默认目录作为交付位置记录在 Further Notes。
- 每张 Map 一个 JSON 文件，每个模板一个 JSON 文件；仅扫描各目录直接包含的 JSON 文件，不递归扫描。
- 模板内容以真实文件读取和关联展示，不以 UI 内置 mock 替代。人工样例是示例数据，不是已有执行能力的证据。
- 重新加载读取当前 Map 和当前共享模板内容；本轮不保存模板快照，不引入版本锁定。
- 没有可用文件时明确呈现空态；读取失败不能伪装为空目录。
- 浏览只读，不提供任意本地路径读取或根据参数读取文件的能力；JSON 文本不作为 HTML 或代码执行。

### 依赖图、详情与错误

- blocked_by 是依赖连线的唯一来源，不另保存手写 edges；B 的 blocked_by 包含 A 时画 A → B。
- 依赖采用无环图；检查未知前置节点、自依赖和依赖环。箭头不表示无条件启动、材料自动传递或任何已经实现的等待状态。
- 图自动布局并支持平移、缩放、适应画布；独立节点不丢失，不被错误连接。
- 节点卡片先显示 node 标题和 ID。选择 node 后显示其参数与前置关系；关联模板内容在进一步点击模板明细后才展示。
- 模板明细展示标题、适用场景、能力说明、输入输出说明及实现类型。字符串按文本展示，结构化参数使用可读的格式化 JSON。
- 不校验 parameters 是否匹配模板输入，不校验参数对应的 zone、Bot 或其他资源是否存在，也不验证实际交付。
- 缺失模板时保留图、节点与边，在对应节点、详情和页面问题提示中明确说明缺失的模板 ID。模板不可读或格式不受支持也明确报告，不补造说明；不因辅助模板内容不可用而删除可展示的依赖图。
- Map JSON 无法解析、图所需结构不合法或依赖非法时，明确指出问题，不以静默删除节点或边的方式呈现有效图。
- 选择或重新加载失败时，不把上次成功的图和详情呈现为本次成功结果；清除当前展示或清楚区分错误，沿用已确认的优先方案清除当前图。
- 显式刷新列表与重新加载构成修改反馈闭环，不增加自动文件监听。

### 导航与示例

- 顶部 tab 顺序为 chat、bot、zone、scheduler。打开与整页刷新默认 chat；保留可访问的键盘导航。
- server 从 tab 移到右上角按钮，打开同一浏览器页面中的独立内容页，不使用新窗口或狭窄下拉菜单；提供返回先前 tab 的入口，保留现有固定操作及安全反馈。
- 桌面可并排展示列表、图和详情，窄屏使用适合的上下布局，保持各内容可访问。
- 提供独立 Map 与共享模板样例及格式说明。完整样例覆盖相同模板不同参数、并行分支、汇合和独立节点，并清楚声明示例能力未实现执行。
- 文档示例不自动混入服务的用户数据；按说明复制到配置目录后可预览。
- 更新领域词汇与使用说明，记录 Map、node_template、node 的区别及 server 入口变更，不改写历史 ADR。
- 本 spec 不选择模块拆分、内部接口、HTTP 路由、图形库、布局算法或测试 seam；这些留给 /to-contract。

## Testing Decisions

- 已核对现有前端的导航与键盘操作测试，以及通过真实服务、独立元数据文件验证重新读取的浏览器场景，可作为既有测试方式的参考，不照搬其具体接口或断言。
- 项目已有 React 前端、Rust 后端、Vitest 组件与逻辑检查，以及 Playwright 真实浏览器验收。沿用项目现有测试入口与隔离约定，不额外引入另一套测试框架。
- 实施交付必须实际打开页面，观察文件选择、图形方向与布局、参数与模板明细、缺失模板提示、重新加载及 server 返回行为。仅编译或仅逻辑测试不证明视觉展示正确。
- 实际运行数据使用测试拥有的临时目录与显式配置，不读取、覆盖或修改用户真实 Map、模板、Bot、zone 或 Chat 数据。
- 真实服务场景沿用项目现有安全测试服务与数值回环地址，独占可变数据及动态端口；不启动真实 Chat，不触发真实关机或真实固定脚本副作用。需要覆盖既有危险操作时使用项目安全壳或记录型替身。
- 前端及浏览器检查通过 npm 入口保留网络护栏；后端完整回归使用项目规定入口。源码变化后的浏览器验收先构建真实产物。
- 关注用户可见的行为、错误与状态切换，不新增仅验证复制、内部接线、源码文本或 mock 回声的测试。
- 本次仅发布 spec，未修改业务代码、未运行产品或测试，不宣称功能已经实现。测试 seam、各 seam 的承诺及覆盖映射由 /to-contract 决定。

## Out of Scope

- LLM 生成、搜索或修改 Map，以及 bot_search 的实现、召回策略、上下文加载和工具注册。
- 第二阶段 Plan 的具体步骤、提示策略、草稿生命周期及确认机制。
- 模板图形化管理、编辑、搜索、自动导入或完整共享模板平台。
- 启动 Chat、subagent、固定代码或其他节点实现；Chat 是当前唯一声明的实现类型，不是本轮执行能力。
- 参数类型体系、必填性或资源有效性校验、输入输出契约执行及实际结果校验。
- 执行路由、条件分支、材料绑定与跨 zone 消息传递。
- Dispatch、Collect、Finish、调度、运行状态、重试、并发、恢复、审批及完成门禁。
- node/node_template 的额外业务类型体系、多实现选择、模板版本锁定和快照。
- 运行中改图、自动文件监听、图形编辑、拖拽修改语义及独立节点文件库。
- 将输入参数当作文件路径读取其内容，或引入任意本地文件浏览权限。

## Further Notes

### 发布与后续流程

- 本文发布于本地 issue tracker：`.scratch/scheduler-map-preview/spec.md`，标记 `ready-for-agent`。
- 已批准的承诺与 seam 见 [contract](./contract.md)，验收模式为 Rapid iteration；人工体验验收仅保留 contract 的 P33。
- 下一步运行 `/to-tickets .scratch/scheduler-map-preview/contract.md`。contract 的批准不表示业务实现或验收已完成；后续确认的约束细节以 contract 为准。

### 已确认的交付位置

- Map 默认目录：`.data/workflow-maps/`。
- 共享模板默认目录：`.data/node-templates/`。
- 参考样例及格式说明：`docs/example/`，分别提供 Map 与共享模板文件。
- 上述目录是已确认的用户配置与文档位置，不用于预先决定实现源码位置或模块边界。

### 来源与优先级

- 需求起点：`.scratch/llm-driven-workflow-definition-design.md` 与 `.scratch/workflow-map-and-plan-implementation-plan.md`。
- 本 spec 以随后用户确认的讨论为准：撤回 Map 内嵌 node_templates 的提议；共享模板外置；输入输出仅文本描述；不做参数契约校验；缺失模板保留图；完成条件与审批字段不进入本轮。
- 旧文档中的节点完整任务字段、未知输出引用校验、完成约定和审批说明，不再作为本轮必须实现内容。当前材料绑定未定义，不应擅自补回旧字段。
- 当前仓库主界面仍是 chat / bot / zone / server 四 tab，尚无本 spec 声明的 scheduler 成果。
- 术语沿用根 GLOSSARY.md 及 Bot / Chat ADR：node_template 是新的业务能力概念，不是把现有 Bot 再次命名为模板。既有 zone 文件边界与静态内容安全原则不因 scheduler 而放宽。
- 当前 JSON 是可扩展的第一阶段静态展示格式，不承诺未来无需有意识的格式演进，也不以完整展示声称模板可执行。
