# Suite：场景工具集与 Task 会话环境配置

Status: ready-for-agent
Labels: ready-for-agent

本文按本轮已确认的最终决策整理；后来的明确决定取代讨论中已放弃的方案。不表示产品实现、迁移或全部运行验收已经完成。

Contract: [已批准的 Suite 契约](contract.md) — 四个接缝、约束审视和 Full automation 验收模式均已确认；具体承诺与验收覆盖以契约为准。

## Problem Statement

用户目前可以在 Task 中选择可选扩展，但一个完整的工作场景还依赖 skills、MCP、提示词模板、语言服务及扩展运行模式。用户需要在多个位置重复配置，难以将“代码库探索”“代码实现”等场景作为可复用的会话环境进行选择，也难以判断一次发起 Chat 最终获得了哪些能力。

用户希望在一台 server 上集中定义 MCP，再按 Task 选择额外能力，而不必为每个项目添加配置。已有项目和 agent 全局 MCP 仍应按原生规则工作；不能为了接入 suite 而丢失原生工具、资源、提示词或管理能力。

OMP 和 Pi 的资源加载入口不同。用户不希望维护两套 suite，也不允许修改 herdr、OMP、Pi 源码。直接替换整个 agent 配置目录会连带改变认证、模型和会话存储，不符合只调整场景资源的目标。

当前还存在需要保留的业务约束：Task 是可复用预设，Chat 是实际对话；初始指示和环境资源不同；固定扩展、workflow 执行角色、原生会话身份与恢复行为不能因本功能改变。既有 Task 必须按用户确认的映射完成一次性迁移，不能靠名称猜测、自动导入全部默认资源或长期保留两套选择机制。

## Solution

引入 Suite（场景工具集）：为某类场景预设的一组可复用资源及明确的能力设置。每个 Task 可以不选 suite，或者选择一个适用于目标 agent 的 suite；不支持叠加多个 suite，也不引入继承和覆盖顺序。

用户通过 herdr-bot 图形界面管理 suite。自定义配置文件是权威定义，界面与程序均使用该定义，不另存一份独立数据库配置。MCP 定义先通过集中配置文件维护，suite 界面从中选择并校验引用，不建设独立 MCP 编辑或进程托管界面。

程序根据 suite 配置及集中 TOML 路径根，生成指定位置的资源目录、链接和宿主所需配置。生成目录是可重建中间产物，不是用户手工维护的事实来源。资源实体继续复用原有文件，不复制每份 skill 或 extension 的源码。

发起 Chat 或按既有规则恢复为新进程时，系统读取当前 suite，装配适用资源并经 herdr 原有能力启动 agent。固定扩展继续按当前 agent 和执行角色规则加载；模型、思考级别、初始 Skill 调用、初始消息和 Chat 命名仍由 Task 的 trigger config 管理。

**最终资源策略如下：**

| 资源或行为 | OMP | Pi |
| --- | --- | --- |
| Skills | 选择 suite 声明的技能，不让默认发现扩大技能集合 | 使用原生显式资源选择，排除常规默认技能发现 |
| 可选 extensions | 所选 suite 的入口与既有固定扩展 | 所选 suite 的入口与既有固定扩展，另默认加载 pi-suite |
| MCP | 原生增量加载 suite 配置，保留项目、全局及原生允许的其他来源 | pi-suite 追加注册，保留原生 MCP 与用户、可信项目配置 |
| MCP 重复、优先级与禁用 | 使用 OMP 原生行为，不自行合并或强制 suite 优先 | 使用 Pi 原生行为，文件同名配置可优先于扩展注册 |
| Suite 本地 prompt templates | 跳过，不模拟为其他功能 | 保持原生可调用文本模板语义 |
| LSP | 增量加载，保留原生默认及用户、项目配置优先级 | 跳过，不新增 Pi LSP 实现 |
| Codemode | 不承诺同名设置或模拟 Pi 行为 | suite 明确选择普通 on 或 only，并覆盖原模式 |
| 无 suite | 不应用 suite 资源及模式覆盖 | pi-suite 仍固定加载，但不追加服务器或接管 codemode |

MCP 空清单在两个宿主中都表示“不追加 MCP”，不是禁用原有 MCP。Skills 的选择要求与 MCP 的增量语义分别成立，不得为过滤技能而误关原生 MCP 来源。

## User Stories

1. As a Task user, I want to select one suite for a Task, so that I can reuse a complete scenario environment instead of selecting resources repeatedly.
2. As a Task user, I want to leave the suite unselected, so that ordinary Tasks can continue without suite-specific overrides.
3. As a Task user, I want the product to reject multiple suite selections, so that resource precedence and mode merging remain unambiguous.
4. As a suite author, I want several Tasks to reference the same suite, so that I can maintain their shared environment once.
5. As a suite author, I want to create, inspect, edit, and delete suite definitions through the graphical interface, so that I do not have to build resource links manually.
6. As a configuration maintainer, I want graphical edits to be persisted in the authoritative configuration file, so that file-based configuration and the interface do not diverge.
7. As a suite author, I want to declare concrete skill, extension, and prompt resource locations, so that I can reuse existing resources without a separate universal registry.
8. As a server administrator, I want machine-specific absolute paths centralized in TOML roots, so that resource definitions do not duplicate local installation paths.
9. As a suite author, I want the generated directory location to be configurable, so that deployment layout is not hard-coded in the application.
10. As a suite author, I want the application to generate resource links and entry manifests, so that both agents can consume the same logical definition through their supported loaders.
11. As a skill author, I want a skill's scripts, references, and assets to remain available, so that linking its entry does not break its workflow.
12. As an extension author, I want relative imports and bundled assets to keep working, so that an extension remains functional after directory-based assembly.
13. As a Task user, I want undeclared default skills excluded when using a suite, so that the advertised skill set matches the selected scenario.
14. As a suite author, I want required skill dependencies explicitly included, so that wrapper skills and conditional workflow steps can find the skills they invoke.
15. As a suite author, I want new files appearing in a source collection not to expand an existing suite automatically, so that its declared skill selection stays intentional.
16. As a Task user, I want fixed extensions preserved independently of suite choices, so that session reporting and baseline integrations remain available.
17. As a workflow user, I want manager and node role extensions to follow execution context, so that suite selection does not alter the role of a Chat.
18. As a Task user, I want only compatible suites offered for the chosen agent, so that I do not accidentally select an OMP-only suite for Pi.
19. As an API client, I want incompatible suite selections rejected on save and launch, so that interface filtering is not the only compatibility check.
20. As a suite author, I want one suite definition to cover both supported agents, so that different loading mechanisms do not require duplicate user configurations.
21. As a Pi user, I want the LSP portion of a shared suite skipped, so that unsupported LSP does not prevent the other resources from loading.
22. As an OMP user, I want suite-local prompt templates skipped without replacing them with system instructions, so that unsupported template loading does not silently change meaning.
23. As a Pi user, I want prompt templates to remain callable text templates, so that selecting a suite does not execute them as initial messages.
24. As a server administrator, I want MCP connection definitions maintained once in a central file, so that individual projects do not need duplicate configuration.
25. As a suite author, I want to choose MCP definitions from that central catalog, so that suite configuration contains references rather than repeated connection details.
26. As a suite author, I want missing MCP references detected when saving and launching, so that external file edits cannot silently remove required capabilities.
27. As a Task user, I want invalid references displayed explicitly, so that I can correct the configuration instead of receiving an unexplained partial environment.
28. As an existing-project user, I want original project and global MCP configurations retained, so that introducing a suite does not disable working integrations.
29. As a Task user, I want MCP duplicates and precedence handled by the agent's native rules, so that herdr-bot does not invent another merge policy.
30. As a Task user, I want an empty suite MCP list to mean no additional servers, so that existing native MCP behavior remains intact.
31. As an OMP user, I want suite MCP servers handled by the native manager, so that tools, resources, prompts, notifications, server instructions, and management functions retain their native integration.
32. As a Pi user, I want suite MCP servers registered through the public extension API, so that native connection management and existing file-defined servers continue to work.
33. As a Pi user, I want pi-suite loaded as a fixed extension, so that every applicable Task can use suite features without separately selecting the adapter.
34. As a Pi user without a suite, I want pi-suite to leave native MCP and codemode behavior alone, so that installing the adapter does not change unrelated Chats.
35. As a suite author, I want an on/only control in the suite's Pi configuration area, so that I can choose how the model accesses tools.
36. As a Pi user, I want the selected suite's codemode mode to override the prior mode in either direction, so that ordinary mode does not accidentally inherit a global only setting.
37. As a Pi user, I want bottom-level tools to remain callable from codemode only, so that hiding direct declarations does not remove the actual capabilities.
38. As an OMP-only suite author, I want Pi-specific controls omitted from that suite's editor, so that the interface does not imply unsupported behavior.
39. As an authenticated agent user, I want existing credentials and their concurrency behavior preserved, so that suite selection does not require re-login or duplicate OAuth state.
40. As a Chat user, I want model configuration and native conversation storage preserved, so that changing scene resources does not move or fork my history.
41. As a Task user, I want model choice, thinking level, initial skill, initial message, and naming behavior unchanged, so that suite configuration remains an environment concern.
42. As a returning Chat user, I want a new process to use the current environment of its source Task, so that recovery follows the existing latest-environment rule without replaying initial instructions.
43. As a user of an already-running Chat, I want suite edits not to trigger active reconfiguration, so that saving a definition does not unexpectedly interrupt the conversation.
44. As a suite maintainer, I want deletion blocked while Tasks still reference the suite, so that removing a definition does not silently unbind those Tasks.
45. As a suite maintainer, I want the referencing Tasks shown when deletion is blocked, so that I can change or remove the references deliberately.
46. As a resource owner, I want cleanup limited to generated artifacts rather than linked source data, so that suite maintenance cannot delete the real skills or extensions.
47. As a concurrent Chat user, I want simultaneous launches using the same or different suites not to observe partially generated resources, so that each startup gets a coherent environment.
48. As a Windows user, I want path spaces, separate installation roots, and link failures handled explicitly, so that configuration does not depend on one directory layout or hidden privilege assumptions.
49. As an OMP developer, I want Rust, TypeScript, Java, and Python LSP definitions in the initial suites, so that appropriate installed language services can be discovered for the current project.
50. As a developer, I want unavailable language-server executables distinguished from working LSP support, so that a declared language is not presented as an already-running service.
51. As an existing Task owner, I want the approved one-time mapping applied by stable Task identity, so that migration does not guess intent from a mutable display name.
52. As an existing Task owner, I want IDs, trigger settings, Bot references, and historical source associations preserved, so that migration does not create duplicate presets or break existing workflows.
53. As an operator, I want migration completion recorded and partial failures handled explicitly, so that restarts do not duplicate generated definitions or silently discard data.
54. As a hyper-auto-coding Task owner, I want its implementation suite to include the extension that supplies its initial command, so that the migrated Task still has that capability.
55. As a Tibo-monitor Task owner, I want this phase to leave it without a suite and defer web-access integration, so that the implementation follows the explicitly approved scope.
56. As a deployment maintainer, I want herdr, OMP, and Pi source trees left unchanged, so that suite support does not require maintaining forks of those products.

## Implementation Decisions

### 1. 领域范围与当前系统

- Suite 属于 Task 的 env config，不是新的 Task 类型、Chat 实例、原生 profile 或 trigger config。Task 至多引用一个 suite，同一 suite 可被多个 Task 使用。
- 当前系统已经分离触发设置与环境资源，并按宿主、固定/可选分类及 workflow 执行角色解析扩展。此能力将从“Task 直接选可选扩展”迁移到“Task 选择 suite”，不长期保留两套独立可编辑、隐式叠加的环境来源。
- 用户不选 suite 时不应用 suite 特有的资源与模式覆盖；现有固定扩展和角色规则照常成立。无来源 Task、来源 Task 已不存在等恢复情形继续遵守已有来源边界，不猜测 suite，也不将固定扩展概念扩大成所有外部原生启动的默认集合。
- 需要改变的产品能力是 Suite 图形化管理、Task 的可选单 suite 引用、声明配置读取与校验、资源装配、启动输入、Pi 固定适配扩展及一次性迁移。具体模块分割、路由、类型接口与测试接缝不在本规格决定。

### 2. 权威定义、路径与生成产物

- Suite 以自定义配置文件为权威数据源，图形界面读取和写入同一份定义；不建立独立数据库副本。
- MCP 连接定义由用户直接编辑集中配置文件。界面只负责选择和展示引用问题，不在本范围提供独立 MCP 增删改页面。
- Suite 声明资源位置、MCP 引用、宿主适用范围及场景能力设置；本机绝对路径集中在现有 TOML 路径根机制中，定义使用根引用。普通相对路径保持以声明配置为基准，不改成按 Chat 的工作目录解释。
- 程序按配置指定的位置生成 suite 物理目录、资源链接、必要的入口清单和原生配置。它们是装配产物，不是第二套定义，也不是历史内容快照。
- 技能链接整个技能目录；扩展优先保留包目录与其依赖布局。当前 Pi 显式包加载需要明确的扩展入口清单，不能把一个含若干扩展的裸目录当作已经可加载。
- 不为 skills/extensions 再建设通用注册平台，不把源目录中全部技能自动加入。首批技能来源已明确采用 wayfinder 优先、matt-pocock 其次；翻译技能复用其独立来源。
- 不选 suite 不等同于“选了一个空 suite”。后者仍可决定技能集合和明确模式，但 MCP 空列表只表示不追加服务器。
- 保存时校验声明与引用，发起前重新检查可读性、来源和适用范围。外部编辑删除集中 MCP 项只能在重新读取时被发现，产品不能承诺拦截外部编辑器的删除动作。
- 生成失败、资源缺失或权限不足必须明确反馈；不以空环境冒充成功，不自动复制资源替代所需链接，不提权或修改开发者模式。只清理程序拥有的产物，不删除链接目标。

### 3. 启动、恢复和固定扩展

- 资源使用原生 CLI、配置发现或公开扩展接口加载；不修改 herdr、OMP、Pi 源码，不引入内部 monkey patch。原生入口不足时优先评估公开 extension，仍不满足时不得静默缩减需求。
- herdr 继续承担原有终端与 agent 启动职责，不要求它理解 suite 定义；herdr-bot 负责将所选定义转换成其已有启动能力可承载的输入。
- Pi 保留原 agent 配置目录、认证、模型和会话存储；不复制或软链接认证状态，不重映射认证锁，不制作整套设置快照。OMP 同样不为本功能整体迁移 agent 目录。
- Suite 不改变 Task 的模型、思考级别、初始 Skill 调用、初始消息与 Chat 命名，也不强制创建新的原生会话或更改恢复身份。
- 固定扩展继续按照宿主与执行上下文加载。独立普通 Chat 不因为选择了某个 suite 获得节点角色；恢复 node Chat 沿用已有执行关联，不重发 trigger config。
- pi-suite 与现有 model-cost 一样纳入 Pi 固定扩展机制；无 suite 时仍加载但不追加 MCP、不覆盖 codemode。它不属于用户每个 suite 都要重复声明的可选扩展。
- 发起或新进程恢复读取来源 Task 当前的 suite 和资源定义，不维护历史 suite 版本，不主动热更新正在运行的 Chat。原生工具本身已有的重载行为不被包装成新的后台同步承诺。
- 同一 suite 的并发使用不得使一次启动读到半套生成内容；定义文件保存、产物发布和外部进程启动的具体协调方式留给后续契约设计，不在此承诺跨文件事务。

### 4. Skills、extensions 与 prompt templates

- 有 suite 时，skills 的明确选择不被常规用户、项目默认发现扩大；依赖技能按已批准清单显式包含。这个目标不是对任意第三方扩展的安全沙箱承诺。
- 可选扩展来自 suite，固定扩展独立保留。复用原生入口解析及合理的入口去重，不重写宿主模块加载器；不能因打包或显式根引入额外未选技能。
- 不能继续使用同时关闭 MCP 来源的粗粒度 provider 禁用方案来完成技能过滤。技能选择必须与已确认的 MCP 增量策略同时成立；现有原型并未证明这一组合已经全部实现。
- 本地 prompts 保持宿主原生模板语义：加载后可调用，不自动作为系统指令或发起消息执行。Pi 消费该部分，OMP 跳过 suite 的本地模板，保留其原有模板机制。
- OMP 跳过 suite 本地模板不意味着放弃 MCP 服务器提供的 prompts。两类资源来源与原生集成必须区分。

### 5. MCP 原生增量加载

- OMP 与 Pi 均把 suite MCP 作为额外能力，保留原用户、项目及原生允许的来源、禁用项和优先级。应用不重新比较两份 MCP 文件，不强制 suite 胜出，不自行实现字段合并。
- OMP 采用原生显式资源包提供额外 MCP 配置，由原生 manager 持有连接、工具、资源、prompts、通知、服务器 instructions 和管理行为。独立 manager 仅注册工具的替代原型不作为交付方案。
- 已验证 OMP 在宿主项目配置、suite 包配置与根目录回退配置之间有原生优先级；同名优先取胜，连接定义等价的异名项也可原生去重。高优先级禁用项可能抑制低优先级项。应用尊重这些行为，不把合法的原生覆盖当作 suite 冲突拒绝启动。
- Pi 的 pi-suite 使用公开 registerMcpServer 追加注册，保留 builtin MCP 实例，不调用自定义配置读取器替换其来源，不注册替代 MCP 管理命令，不为 suite 关闭原生 MCP。使用禁止默认扩展发现的启动方式时，必须显式保留所需内置 MCP 支持。
- Pi 文件中的同名配置按其原生规则优先于扩展注册。两个宿主不需要具有同样的底层优先级算法，用户共享的是 suite 定义及“额外能力”的语义。
- 不新增 MCP 常驻托管系统、跨 Chat 连接池或协议实现；实际服务进程、认证、资源和工具调用继续由宿主或服务器自身处理。

### 6. Pi codemode 与宿主适用范围

- Suite 编辑页的 Pi 配置区提供普通/only 切换。关闭 only 表示明确的普通 on，不表示禁用 codemode，也不继承原来的 only。
- 首批代码库探索 suite 在 Pi 下默认普通 on。该设置只作用于本次 suite 环境，不写回原全局设置。
- pi-suite 复用公开 createCodemodeExtension，传入明确 mode，并确保 codemode 被激活。不得重复注册内置和替代 codemode 实例；only 隐藏直接声明，但底层工具仍须可供脚本调用。
- Suite 可以声明支持的 agent 集合。选择界面、保存及启动均需尊重该集合；已保存的不兼容引用不能被静默替换成其他 suite。
- Pi 跳过 LSP，OMP 跳过 suite 本地 prompts；跳过不适用资源不应导致其余资源失败。仅 OMP 的 suite 不展示 Pi codemode 控件，也不推断 OMP 存在等价选项。
- 两个初始 suite 都声明 Rust、TypeScript、Java、Python LSP。OMP 采用原生增量配置、项目标记、可执行程序发现和优先级，不承诺排除其他原生语言服务，也不把程序缺失误报为已经启用。

### 7. 图形管理、引用与文件边界

- Suite 图形界面至少承载列表、创建、编辑、删除、资源声明、MCP 选择、宿主适用范围及 Pi 模式切换；Task 编辑界面提供兼容 suite 单选和不选选项。
- Suite 仍被 Task 引用时，后端拒绝删除并返回可展示的引用列表；用户先改选或解绑，不自动解除引用。
- MCP 文件被外部编辑导致引用失效时，界面标出问题，保存和启动重新检查；不静默删引用或回退另一份定义。
- 遵守既有统一文件持久化决策：业务校验与引用规则仍归业务，持久化和文件变更使用既有协调机制，不引入另一个通用 repository 或文件协议。
- 生成受控资源链接不等于放宽 zone 浏览、持久化路径或任意重解析点的现有拒绝规则。如何在已有边界内承载受控生成由后续契约设计明确。
- 保持既有同机会话写入信任边界，不因新增图形管理而变更会话认证、LAN 暴露或关机能力。

### 8. 首批 Suite

| Suite | 支持的 agent | 场景扩展 | 额外 MCP | LSP | Pi 模式 |
| --- | --- | --- | --- | --- | --- |
| 代码库探索（codebase-exploration） | OMP、Pi | codebase-tools、connect-bot | codebase-memory-mcp | Rust、TypeScript、Java、Python；Pi 跳过 | 普通 on，可切换 only |
| 代码实现（code-implementation） | 仅 OMP | codebase-tools、connect-bot、oh-my-wayfinder | codebase-memory-mcp | Rust、TypeScript、Java、Python | 不适用 |

探索 suite 的 18 个技能：grill-with-docs、teach、code-review、wayfinder、writing-for-agents、improve-codebase-architecture、expert-translate、grilling、domain-modeling、codebase-design、research、prototype、lighthouse、backtracer、setup-matt-pocock-skills、to-contract、to-spec、to-tickets。

实现 suite 的 11 个技能：tdd、to-tickets、retro、resolving-merge-conflicts、writing-for-agents、codebase-design、code-review、setup-matt-pocock-skills、to-contract、to-spec、domain-modeling。使用实际技能名称 to-tickets，不另造 to-ticket 别名。

两者当前都没有声明额外本地 prompt templates。初始 LSP 定义选用 rust-analyzer、typescript-language-server、jdtls、pyright 对应服务，遵循实际宿主支持；安装与环境条件在进一步说明中记录。

### 9. 一次性迁移

- 已确认的 22 个现有 Task 按稳定身份迁移；不按可变显示名猜测归属，不重新创建 Task，不改 trigger config、Bot 引用、历史来源 Task 或 Run/node 执行身份。
- 17 个映射到代码库探索：grill-with-docs、teacher-bot、wayfinder、能力创建者 · Bot＋Task、概要规划、概要方案转 Map、Task 创建者、通用提示词优化者、Bot 创建者、Scheduler Map 修改者、空白 Bot 创建者、Bot.when 编写者、Bot.plan_guide 编写者、Bot.node_guide 编写者、既有 Task.prompt 交互优化者、对话式架构重构、翻译科技类文章。
- hyper-auto-coding 映射到代码实现；补充 oh-my-wayfinder 是保留其已有初始命令能力所需的已批准变更。
- Tibo-monitor、C盘清理大师、manager、通用文本替换者不选 suite。Tibo-monitor 的 web-access 整合明确留到下一阶段，不创建第三个网页 suite，也不把该能力偷偷加入两个首批 suite。
- 映射由用户批准，迁移执行尚未发生。执行前检查稳定身份与当前数据，不能用过时盘点覆盖用户后续编辑。
- 记录完成状态，避免重复生成或重复迁移；失败不得静默丢失定义或标记为全部成功。沿用已有迁移与部分生效规则，不新增跨文件事务或通用回滚框架。
- 完成后移除旧 Task 可选扩展选择的运行时双轨逻辑；固定扩展与实际资源入口目录可以复用，不等于保留旧业务选择路径。

### 10. 后续契约设计范围

以上是已确认的产品行为、公开接口选择及既有架构约束。新模块接缝、内部接口形状、精确持久化 schema、生成发布协议、启动配置传递通道及每项承诺的测试覆盖由 /to-contract 决定。既有持久化和执行角色边界作为已接受事实保留，本规格不新增接缝方案。

## Testing Decisions

### 既有先例与约束

- 现有 Task 保存、动态选项目录、Chat 发起、来源 Task 边界、恢复环境、会话身份、并发恢复和运行数据迁移回归可提供既有行为与夹具先例；前端已有 Task 编辑和 Chat 发起测试。具体复用位置由后续契约设计决定，不在本规格指定新的测试接缝。
- 遵守项目测试指引：前端验证通过既有脚本入口；图形界面必须观察真实浏览器表面，不能只用组件或源码断言替代实际交互证明。后端复用既有网络、文件和进程隔离约束。
- 真实宿主验证使用测试拥有的配置、目录、命名服务与动态回环端口；不得读取真实凭据、操作正在运行的用户 Chat、访问已有 MCP daemon、执行真实关机或修改全局配置。
- 进程日志持续消费，清理只作用于所属进程，并确认退出；临时目录删除不能代替进程存活确认。失败、未执行、权限不足与通过分别记录。
- Windows 真符号链接验收必须显式启用项目要求的手动开关，不能自动提权或改变开发者模式。Junction 的通过不冒充文件 symlink、目录 symlink 或跨磁盘目标通过。
- 测试数据、路径根及迁移标记由各例独占；不要读取开发者的业务路径根。普通配置路径与扩展入口须覆盖各自声明位置的相对路径语义及含空格路径。
- 仅证明“参数包含某字符串”“没有抛错”“加载列表非空”不足以证明功能。资源验证应观察实际加载集合、模块相对依赖、连接次数、真实工具结果及模型实际收到的工具声明；不能只用回显工具冒充业务行为。
- 可使用受控回环模型生成确定性的工具调用来验证宿主机制，但不得宣称它证明真实模型的自主任务遵循。MCP 夹具应执行真实文件读取或独立可核对的计算。
- 以最终增量策略设计验收：保留原生 MCP 来源、验证宿主优先级和禁用行为；历史隔离原型的成功不能替代这一验收。只执行过工具层桥接也不能证明完整原生管理与资源集成。
- 核查 skills 默认发现过滤与原生 MCP 共存。粗粒度 provider 禁用的历史实验不能证明两项最终要求同时满足。
- 本规格仅记录已有验证先例与限制；测试接缝、承诺清单和覆盖映射留给 /to-contract，不在此指定。

### 已有技术验证可以证明什么

- OMP 已安装版本的 30 例原生合并矩阵和 7 例 source CLI 运行，证明项目/suite 同名、异名等价、不同参数、禁用和来源优先级行为；实际工具读取文件并计算独立可核对的 SHA256。原生 manager 资源列举、内容读取和 prompts 获取也已验证。
- 上述 OMP 成功入口是安装包的源码 CLI；不等于当前 checkout 构建、打包后的 TUI、全部管理交互、通知热刷新或完整 Herdr 发起链路已经验收。
- Pi 已安装版本的历史原型证明固定扩展可在无 suite 时不接管、普通/only 双向覆盖、仅 codemode 声明时底层 read/MCP 工具仍可调用。该原型排除了默认 MCP 文件，已被最终增量决策取代；追加注册、保留默认文件及同名优先级组合必须补充运行验证。
- 资源原型证明 junction、包入口清单、相对 TS import、相对资产读取和真实扩展工厂调用可工作；没有证明所有符号链接形态、真实业务扩展或完整并发发布。
- codebase-memory-mcp 的隔离动态端口试验证明同一 namespace 的两个客户端可共享 daemon/UI。独立 namespace 同端口仍可能产生 UI 端口竞争，不能把协议初始化成功当作 UI 已监听。
- 两个首批 suite 的 18/11 个技能路径、名称、源优先级、扩展绑定和 22 个迁移 ID 已完成配置级核验，但尚未执行产品迁移或全部技能工作流。

## Out of Scope

- 修改或 fork herdr、OMP、Pi 源码；使用内部 monkey patch 绕过其初始化或认证行为。
- 多 suite 叠加、suite 继承、跨 suite 配置合并与优先级规则。
- 隔离原有 MCP、强制 suite MCP 覆盖原项目/全局定义、绕过原生禁用项，或建立自有同名去重与字段合并算法。
- 用独立的仅工具 MCP manager 替代 OMP 完整原生集成，或重写 MCP 协议与客户端。
- 替换整个 agent 配置目录，复制/链接 OAuth 状态，映射认证锁，或创建新的凭据管理/broker 服务。
- 新的常驻 MCP 托管、跨 Chat 连接池、服务器安装与自动修复。
- 独立 MCP 增删改图形界面；本范围内由集中文件维护定义。
- 为 Pi 实现 LSP，为 OMP 模拟 suite 本地 prompt templates，或假定 OMP 有等价 Pi codemode 配置。
- 为完成本功能自动安装 LSP、修改系统 PATH、提权或开启 Windows 开发者模式。
- 历史 suite 内容版本、运行配置同步器、主动热重配已有 Chat，或复制真实资源作为长期快照。
- 通用资源注册平台、另一个数据库持久化副本、通用迁移回滚框架或跨文件事务保证。
- 变更 Task 的用途类型、workflow 执行角色、zone 身份、原生 Chat 身份规则、会话写入信任边界或认证策略。
- 本阶段整合 web-access、创建网页研究 suite，或自动扩充用户未批准的额外 suite。
- 把初始化资源选择承诺包装为对任意第三方扩展、动态 skill 指令或 shell 行为的安全沙箱。

## Further Notes

### 规格与关联材料

本规格发布在项目本地 Markdown issue tracker，标签为 ready-for-agent。最终行为以本规格为准；研究材料保留探索过程，不应从历史“仅 suite MCP”“替换 agentDir”“独立工具 manager”等原型恢复已撤销的要求。

- [首批 Suite 声明与集中资源引用](initial-suites.toml)：已记录 18/11 个技能、两组扩展、MCP、四种语言服务和探索 suite 的 Pi 普通模式。它是设计输入，不是当前产品已经支持的部署 schema。
- [已批准的稳定 Task ID 映射](migration-map.json)：22 项，状态仍为 approved-not-applied。
- [迁移讨论与用户确认](migration-proposal.md)。
- [当前领域词汇](../../GLOSSARY.md)。
- [路径样例](../../paths.example.toml)：本机实际配置使用已有 paths.local.toml；原目录的 expert-tranlate 拼写与技能名 expert-translate 不同，引用按真实路径处理。

### 相关 ADR

- [ADR 0016：统一文件持久化](../../docs/adr/0016-unified-file-persistence.md)：复用既有文件协调，不引入第二套文件协议，不扩大任意链接访问，不凭空承诺跨文件事务。
- [ADR 0017：可移植 zone 与提示词词典分离](../../docs/adr/0017-portable-zones-and-prompt-dictionary.md)：路径根、zone 绑定、提示词词典职责不同，不把 suite 路径解析扩展成通用文本执行或历史迁移。
- [ADR 0015：执行上下文决定节点角色](../../docs/adr/0015-node-role-from-execution-context.md)：固定角色扩展与恢复身份保持现有规则。
- [ADR 0014：尚未落盘的原生启动身份](../../docs/adr/0014-unpersisted-omp-startup-identity.md)：不预先发送任务、不伪造会话文件，不改变真实身份确认。
- [ADR 0004：同机会话写入边界](../../docs/adr/0004-session-write-boundary-same-host.md)：新增 suite 功能不改变现有会话写入信任模型。

### 验证材料与已知环境限制

- [OMP 原生配置矩阵](verification/omp-merge/report.md)、[原生 source CLI 验证](verification/omp-native-duplicate/report.md)：安装 OMP 18.16.0；矩阵相关六个实现文件与 checkout 的 SHA256 相同，不据此宣称整个构建相同。
- [Pi 历史隔离原型](verification/pi/report.md)：安装 Pi 1.5.0；最新增量策略需另行验证，不能复用隔离预期。
- [目录、MCP 并发、LSP 与技能依赖](verification/resources/report.md)。
- [整合检查摘要](verification/integration-summary.json)、[研究过程](findings.md)、[加载机制核查](loading-contracts.md)。
- [项目测试指引](../../docs/agents/testing.md)；既有先例包括 [Task 回归](../../backend/tests/tasks.rs)、[动态目录回归](../../backend/tests/dynamic_catalog.rs)、[恢复环境回归](../../backend/tests/chat_resume_environment.rs)、[Task 界面回归](../../frontend/tests/tasks.test.tsx)。这些链接是先例索引，不指定测试接缝。

当前源码 checkout 的完整运行分别被 Pi 缺失生成 provider 数据、OMP 缺失 Windows native addon 阻塞；已安装版本的成功必须与这些限制分开报告。Rust 语言服务器可执行；Java 语言服务器实体已找到但不在当前 PATH；TypeScript、Python 的指定服务器未在核查范围找到。未触碰用户 MCP 的实际 UI 端口，未执行真实符号链接验收，未执行产品迁移。

后续契约设计应特别核实：固定 pi-suite 在原生工厂期获得配置的可靠启动通道；保持 MCP 增量发现时的精确技能选择；宿主同名覆盖下的实际连接及完整原生功能；受控资源链接与既有文件安全边界的兼容；恢复与并发生成不污染其他 Chat。以上是实现证据与风险，不重新开放已确认的产品范围，也不在本规格预先选择接缝。

**下一步：运行 `/to-tickets .scratch/suite-cli-research/contract.md`，依据已批准契约生成实施票。**
