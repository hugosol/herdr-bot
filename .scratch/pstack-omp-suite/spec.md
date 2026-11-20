# pstack 专用 OMP Suite：可持续更新的本地工程工作流迁移

Status: ready-for-agent
Triage: ready-for-agent
Contract: [已批准 — Rapid iteration](contract.md)

## Problem Statement

用户希望在 herdr-bot 中选择一个 pstack 专用 Suite，以 OMP 执行 pstack 的本地工程工作流，而不是继续依赖 Cursor，也不是把 pstack 叠加到现有探索或实现 Suite。

当前 pstack 将工程方法写在 skills、playbooks、worker 指引和 persona 中，并提供日志、PR 观察、worktree 审计与状态图验证脚本。其调用约定、角色模型、历史路径和部分外部能力依赖 Cursor。直接加载原版会带入无效路径、工具参数与模型名称；与现有技能集合混载还可能产生同名技能和行为指令冲突。

用户不希望重新实现角色引擎、调度器、权限系统或技能加载器。OMP 已有技能渐进加载、子代理和模型绑定能力，my-agent-toolkit 已有可开关、可恢复的持续提示模式。迁移应复用这些机制，同时保留 pstack 的方法与授权语义。

用户还需要持续跟进上游更新。直接修改复制后的技能，会使上游更新和本地适配混在一起，无法清楚知道哪些变化应保留、哪些适配已经失效。因此需要参考 oh-my-wayfinder，以固定上游快照和可审查的映射生成交付资源。

## Solution

在 my-agent-toolkit 中交付一个集中维护的 pstack-suite，包含原生 OMP 持续模式扩展、上游快照、文本适配映射、生成工具，以及生成的技能、persona 和配套资源。通过 herdr-bot 的独立 OMP-only Suite 选择这些资源，不另建项目，也不复制整个 Cursor 运行时。

该 Suite 独占 pstack 场景技能与行为模式，不混入 wayfinder 或其他场景集合；必要的 Task 固定扩展与项目指令继续生效。所谓“干净”是明确的场景资源选择，不是关闭一切原生能力或建立文件、凭据和进程沙箱。

新会话默认进入持续 poteto-mode，复用 toolkit 现有模式开关、会话状态恢复和提示注入机制。原生技能调用仍可使用，隐藏技能仍加载且按需读取；poteto-mode 通过保留的相对路径和触发规则路由到叶技能。无需额外技能加载器，不为每个技能注册 command。

swarm 与 arena 保留 pstack 的并行、竞争、评审和综合方法，使用 OMP task 执行，而不是转成 herdr Map。首次模型设置由适配后的 setup-pstack 检查可用模型、展示角色与面板配置，并经用户确认生成 OMP persona。角色调用使用原生 agent 名称，模型绑定集中可编辑，不写入全局 OMP 配置。

本次保留核心本地工程流程及必要外部技能依赖。Benny、云端执行环境、Slack/Grok 事件自动化及历史／学习增强作为独立专题暂缓，记录在包内维护文档中，不把失效入口伪装为已迁移能力。

## User Stories

1. As a Suite 使用者, I want 在 herdr-bot 中选择独立的 pstack Suite, so that 我无需组合现有探索或实现 Suite 就能使用 pstack。
2. As a Task 维护者, I want pstack Suite 只声明兼容 OMP, so that 不会把未经适配的资源交给 Pi。
3. As a Suite 使用者, I want 当前会话的场景技能来自选定的 pstack 资源, so that 同名技能不会被其他场景的实现替代。
4. As a Suite 使用者, I want 必要的 Task 固定扩展和项目指令继续生效, so that pstack 不会切断 herdr 接入或绕过项目约束。
5. As a Suite 使用者, I want 明确知道 Suite 隔离不等于安全沙箱, so that 不会把目录选择误当成文件或凭据权限隔离。
6. As a 维护者, I want extension 和全部相关资源集中在 toolkit 的 pstack-suite 中, so that 不需要建立另一个项目才能维护和交付。
7. As a 维护者, I want 保留上游版本、来源与许可证, so that 每次更新都有可追溯的基线。
8. As a 维护者, I want 上游快照不被本地适配直接改写, so that 可以区分原始内容与 OMP 差异。
9. As a 维护者, I want 每项适配由具名映射和修改原因表达, so that 可以独立评审某个宿主差异。
10. As a 维护者, I want 映射在锚点缺失、歧义或编辑冲突时明确失败, so that 上游变动不会被模糊替换掩盖。
11. As a 维护者, I want 从快照和映射重新生成相同交付资源, so that 生成产物不成为另一份人工维护的事实来源。
12. As a 维护者, I want 只读一致性检查和差异预览, so that 更新资源前后可以审查适配结果而不隐式写入。
13. As a 维护者, I want 分开审阅上游更新与本地行为变化, so that 不会在更新锚点时顺便改变工程策略。
14. As a 维护者, I want 资源清单覆盖技能正文、playbooks、worker 指引、persona、脚本和引用资料, so that 不会只迁移入口而遗漏运行依赖。
15. As a 维护者, I want 未修改资源被正确保留且非文本资源不被当作 Markdown 转换, so that 构建不会损坏辅助资产。
16. As a 维护者, I want 生成和清理只处理明确归属的产物, so that 用户文件、运行证据和非生成资源不会被误删。
17. As a Suite 使用者, I want 启动时加载已生成资源而不自动更新上游或构建, so that Chat 的启动不会成为不可预期的发布过程。
18. As a Suite 使用者, I want 新会话默认开启持续 poteto-mode, so that 每轮工作不必再次手动附加同一入口。
19. As a Suite 使用者, I want 用独立模式开关启停持续指引并看见状态, so that 可以明确控制当前会话行为。
20. As a Suite 使用者, I want 恢复 Chat 时尊重已保存的开关状态, so that 我关闭过的模式不会被默认开启策略覆盖。
21. As a Suite 使用者, I want 一次性原生技能调用与持续模式开关语义明确区分, so that 不会把读取一次技能误认为改变持久状态。
22. As a 主代理使用者, I want poteto-mode 提供任务类型与技能触发规则, so that 代理按需选用工程方法而非每轮读取全部正文。
23. As a 维护者, I want 保留技能之间的相对目录关系, so that 不必为可原样工作的链接增加适配映射。
24. As a 主代理使用者, I want 持续注入保留技能来源位置或明确读取入口, so that 相对路径不会因正文被作为字符串注入而失去基准。
25. As a Suite 使用者, I want 保留原版隐藏标记，同时使选定技能仍可由原生 URI、手工入口或显式路径读取, so that 默认目录曝光与实际可达性不会混淆。
26. As a 主代理使用者, I want 子代理得到实际可读的 worker 与技能入口指引, so that 它不依赖并不存在的父对话继承。
27. As a 工程使用者, I want 保留调查、机制解释、原因追踪、教学、请求澄清与进展报告能力, so that pstack 不被缩减为只会改代码的流程。
28. As a 工程使用者, I want 保留架构、功能实现、重构、缺陷修复、性能调查、原型、视觉对齐与运行取证方法, so that 不同任务仍使用适当的工程流程。
29. As a 工程使用者, I want 保留 pstack 工程原则及其按需引用方式, so that 迁移改变的是宿主接口而不是工程方法。
30. As a 并行任务使用者, I want swarm 支持分片、同题竞速与混合组织, so that 能根据任务选择覆盖或择优策略。
31. As a 并行任务使用者, I want worker 有清楚的完成条件、输出位置、验证目标与结果证据, so that 汇总不依赖模糊自述。
32. As a 并行任务使用者, I want 缺失、掉队或证据不全的结果明确记为缺口, so that 不会被统计成通过。
33. As a 方案设计者, I want arena 产生独立候选、按标准评分、选取基底并吸收有价值的差异, so that 不会被第一次方案锁定。
34. As a 方案设计者, I want arena 保留独立裁判、厂商差异与最终综合产物验证, so that 候选通过不能替代最终结果通过。
35. As a 工程使用者, I want 保留 interrogate 的对抗评审与复跑取证要求, so that 只有可证明的问题进入结论。
36. As a Suite 使用者, I want 这些内部并行由 OMP task 完成, so that 不必为临时切片建立 herdr Bot、Task 和 Map。
37. As a Suite 使用者, I want 首次设置列出实际可用模型、角色、面板席位和推理等级并等待我的确认, so that 没有未经确认的默认绑定。
38. As a Suite 使用者, I want 普通角色可以明确选择继承父会话模型, so that 不需要为每个任务角色重复指定模型。
39. As a Suite 使用者, I want 模型绑定集中生成原生 persona, so that 调整模型不必修改每份技能或每次委派指令。
40. As a Suite 使用者, I want pstack 模型设置只作用于 pstack Suite, so that 其他 Suite 和全局 OMP 默认值不被修改。
41. As a 面板使用者, I want 每个候选席位对应真正独立的子代理调用, so that 模型回退列表不会被误当成并行候选列表。
42. As a 面板使用者, I want 模型不可用、厂商检查不足或操作者覆盖造成的实际差异被明确报告, so that 不会把降级面板当作原定方案。
43. As a 维护者, I want 模型配置更新后重新生成并由新启动会话消费, so that 不依赖未承诺的运行时热更新。
44. As a 工程使用者, I want 保留 poteto 执行角色与 comment-sicko 注释评审角色的职责, so that 子代理仍遵循对应任务方法。
45. As a 工程使用者, I want 只读声明与实际工具边界被准确区分, so that 不会把 persona 文本当作已经成立的安全限制。
46. As a 工程使用者, I want deslop、control-cli、control-ui 作为必要技能依赖一并迁移, so that 提交前清理和真实 CLI/UI 验证不会变成失效引用。
47. As a 验证使用者, I want 保留行为验证与形式验证技能的创建、维护及真实执行要求, so that 项目能获得可复用而非只写在纸上的验证方法。
48. As a 验证使用者, I want 保留 TDD、blast-radius、性能数字核查及 statechart 能力, so that 不同验证问题仍有对应的方法和可执行资产。
49. As a Windows 使用者, I want 脚本、终端驱动、浏览器驱动及 Java/TLC 等依赖按真实环境适配, so that Linux 或 macOS 示例不会被错误宣称为本机可用。
50. As a 工程使用者, I want 保留日志、worktree 审计和 PR watcher 的真实行为与副作用说明, so that 报告工具不会被误当成零副作用或自动合并工具。
51. As a 交付使用者, I want 保留 PR 创建、监控、独立验证、shipping 与本地自主工作流程, so that 排除云 VM 不意味着删除核心交付方法。
52. As a 操作者, I want 沿用 pstack 原有授权范围与操作粒度, so that 迁移不会暗中增加统一逐次确认门禁或放宽原本需要暂停的操作。
53. As a 操作者, I want 开启模式与授权具体业务任务被区分, so that 持续提示注入不会被当作合并、部署或对外通信的空白授权。
54. As a 长任务使用者, I want 保留决策日志、安全暂停和交接说明, so that 暂缓历史增强不会让当前工作失去可交接的证据。
55. As a 汇报使用者, I want 保留文字及视频汇报的产物要求与验证, so that 没有渲染器时不会拿文字计划冒充完成的视频。
56. As a Suite 使用者, I want 暂缓能力在包内文档说明原作用、依赖及当前边界, so that 可以另开会话继续迁移，而不会忘记它们。
57. As a Suite 使用者, I want 未迁移的外部事件和历史读取入口不被展示成可用能力, so that 不会在运行时遇到留存的 Cursor 专属死链。
58. As a 维护者, I want 生成结果、真实宿主加载和实际工作流行为分别得到证据, so that 构建成功不能冒充整套迁移完成。

## Implementation Decisions

以下仅记录讨论中已经确定的设计事实；不在本 spec 新选模块接口、测试 seam 或持久化协议。

### 范围与归属

- 交付为 pstack 专用 OMP-only Suite，不叠加现有探索／实现 Suite，不提供 Pi 适配。
- 不新建独立项目。extension、资源快照、映射、生成器及生成资源集中维护于 my-agent-toolkit 的 pstack-suite 包。
- herdr-bot 的 Suite 是场景资源集合，Task 是发起预设，Chat 是实际对话。Suite 不替代 Task 的模型、初始 Skill 或固定初始指示，也不等于运行时沙箱。
- 保留必要的 Task 固定扩展与项目指令。其他原生 persona、MCP、LSP 等来源需核实实际加载行为，不从技能白名单推导整个宿主已经隔离。
- 基础资源接入复用现有按宿主声明技能集合与扩展入口的能力，不预设修改 herdr-bot schema 或 OMP 源码。

### 内容生成与持续更新

- 采用 oh-my-wayfinder 式的 delta overlay：固定上游快照、明确资源清单、具名锚定操作、生成产物、一致性检查、差异审阅与上游比较。
- 上游授权文本和来源版本保留；生成资源不手工修改。上游更新与本地策略变更分开审阅。
- 映射只承担确有必要的宿主／平台差异。可通过保持目录关系继续工作的相对引用原样保留。
- 锚点缺失或不唯一、匹配歧义、冲突编辑等情况明确失败，不静默跳过或猜测替换。
- 映射覆盖实际修改的技能、playbook、worker/persona 及其他文本依赖；未修改资源和非文本资产按适当方式保留。
- 生成清理只限明确归属的产物，不能照搬参考生成器对目录内未知文件的破坏性清理；具体发布与所有权协议留给 contract。
- Suite 加载生成结果，不在 Chat 启动时下载上游、同步或构建。

### 技能路由与持续模式

- poteto-mode 原本以提示词给出技能位置、触发条件和读取要求，没有独立的技能查找／注入脚本。迁移不新增技能加载器。
- 保留原版隐藏标记及相对布局。隐藏仅控制默认目录曝光，选定技能仍被 OMP 加载，并可按需读取及手工调用。
- 主代理和子代理都必须获得正确的资源基准；正文作为提示内容使用时不得丢失相对路径的来源。
- 持续模式复用 toolkit 的 codebase-tools 声明式模式机制，包括开关、会话状态、默认开启入口、恢复、提示注入和可见状态，不另建模式框架。
- 新会话默认开启持续 poteto-mode；恢复尊重已有开关状态。持续模式开关与一次性原生 skill 调用是不同动作。
- 工程规则以生成的 pstack 内容为来源，不在 extension 维护另一套独立规则。注入的具体组织方式留给 contract，但不应每轮灌入所有叶技能正文。

### 子代理、角色与模型

- swarm、arena 保留上游工作方法，使用 OMP task；不迁成 herdr Map。临时子任务由主代理组织、验证和裁决，需要用户决定时交回主会话。
- 使用 OMP 原生 persona、任务执行和模型选择，不增加角色执行引擎、运行期转换器或替代 task 工具。
- 套件自有、可编辑的模型绑定经构建生成角色与面板席位 persona，以及供技能读取的角色清单。重复指引由共同来源生成，而不是人工维护多份。
- persona 使用原生模型和推理配置，skill 按 agent 名称委派，不依赖每次调用临时指定模型。
- 普通角色可明确选择继承父模型；实际存在更高优先级的操作者覆盖时，不能把省略模型字段等同于已经继承成功。
- 多模型面板每个席位对应独立 agent；一个 persona 的模型优先／回退数组不表示并行候选。
- arena 的候选生成、评分、不同厂商裁判选择与综合仍属于工作流策略，依据实际模型身份执行，不以代理名推断已满足厂商多样性。
- 适配后的 setup-pstack 检查可用模型，展示原有角色、席位与推理等级，经用户确认后保存配置并生成 persona；不预置未经确认的模型绑定，不写 Cursor 规则，也不修改全局 OMP 默认模型。
- 配置更新重新生成，新启动会话消费；不承诺运行中热更新。不可用模型、被覆盖的实际模型及其他面板偏离明确报告，不静默冒充预期配置。
- 契约讨论已确认：首次无模型绑定仍可进入 setup；确认并生成后，重新发起 Chat 使用角色，不增加同会话热加载。
- 更新生成资源或模型绑定前，操作者先停止使用该包的 pstack 会话，完成后重新启动。正常多会话并行仍保留；本期不承诺在线资源快照、热切换或并发发布。此为使用约定，不额外要求跨进程强制停用检测。具体可观察承诺见 contract。

### 核心方法、工具与授权

- 保留 poteto-agent 执行方法、comment-sicko 注释审查，以及主代理对子代理结果负责的原则。提示词只读与实际工具权限分别表达。
- deslop、control-cli、control-ui 是必要的外部技能依赖，单独纳入迁移，不加载整个 cursor-team-kit。
- 保留真实 UI/CLI/API 驱动、验证、性能取证及形式验证要求；适配现有宿主与平台工具，不把不可用工具调用替换成无操作步骤。
- 保留日志、PR watcher、worktree 审计、statechart 等可执行资产。依赖、脚本写入位置与副作用需明确；例如 worktree 审计可能 fetch，PR watcher 本身不 merge。
- ketchup-video 的视频交付仍在核心范围；上游没有捆绑渲染器，实施需使用真实可用工具完成产物，不将文本方案当视频。
- 本地 PR 交付、shipping 与自主流程保留，云端执行引用按已批准的 OMP 本地任务策略适配；不增加持久调度或宿主退出后续跑承诺。
- 沿用 pstack 原有自主操作、任务授权及暂停规则，不增加统一逐次确认门禁，也不放宽强制审批。不同角色文本与实际宿主／项目约束冲突时须显式说明，不借工具转换静默改变授权。
- 保留当前会话、决策日志与安全交接；跨历史会话自动取证与云 agent 续接按暂缓范围处理。

## Testing Decisions

契约讨论中用户明确修正验收口径：skill 与提示词的具体模型执行效果不做测试，只保证代码相关功能准确。故事中涉及路由、评审、综合、授权遵循或验证方法的要求，表示迁移内容保留对应方法及代码机制可用，不构成模型自主行为或输出质量的可测试保证。这些效果也不转为人工必验。

- 生成工具参考 oh-my-wayfinder 已有的锚定映射、只读一致性检查、上游比较和差异预览。映射检查证明可重现，内容审查证明适配保留批准语义；两者均不证明模型一定遵从提示词。
- 持续模式参考 toolkit 已有 codebase-tools 的手动／自动入口、历史状态恢复、初始化／提醒及子代理机制。验证实际状态、提示贡献与原生读取，不将其称为模型理解或服从证据。
- Suite 接入参考现有原路径资源加载和启动验收。在实际 OMP 中核对资源目录与来源、扩展加载、明确发起的正文读取及子代理可达性，不能只检查声明或生成配置。
- 角色模型通过明确调用观察真实解析后的模型、推理信息、覆盖和错误；可使用受控 provider 验证机械分发，不要求真实模型自主作出正确角色或裁判选择。
- swarm/arena/interrogate 的文本流程与引用通过内容审查，task 的独立产物、席位和结果关联通过明确输入验证。不把不同 Chat/agent ID 当作文件隔离，也不要求模型自行分片、排名、综合或发现缺陷的效果测试。
- 现有可执行脚本和驱动仍在 Windows 的隔离目标上验证，检查退出、真实产物和已发生副作用。文本生成器或技能指令与代码实现分开，不因不测模型效果而省略已有代码检查。
- 使用自有临时配置、目录、端口、实例及可追踪进程，避免修改用户现有会话、凭据、全局模型配置或生产 Task/Suite。只清理本次拥有的资源，保留证据。
- 模式开关与恢复、无配置进入 setup、确认生成与新会话使用等机制仍须验证；缺依赖、凭据或原生能力时明确保留失败或未验证，不以跳过替代通过。
- 用户已选择快速迭代；具体自动检查与两项人工验收责任由 [已批准 contract](contract.md) 唯一维护。人工项不包含提示词执行效果。
- 当前只完成规划材料，没有运行产品测试或实际宿主验收。

## Out of Scope

- Benny 的 Slack 问题分流、tracker 接入、真实复现修复与消息回传自动化。
- Cursor 云 VM、云端 worker 供应、云 agent URL 接入、远端环境与持久远程执行平台。
- Slack/Grok webhook、外部事件接收、常驻桥接、定时触发、事件认证与投递恢复。
- recall、automate-me、reflect、correct 的历史／学习增强专题；其中 correct 不等于记忆服务，但仍按用户同意的独立专题暂缓。
- session-pickup 中依赖 Cursor transcript 或云 agent 的自动历史读取适配。不得据此删除普通当前上下文、决策日志与安全暂停交接。
- Pi 适配、其他 Suite 改造、全局 OMP 配置迁移或把 pstack 合并进现有探索／实现 Suite。
- 新建独立代码仓库，或把整个 cursor-team-kit 作为默认插件加载。
- 为所有 skills 注册命令、重写技能加载器、角色执行引擎或模型运行时。
- 将 swarm/arena 转成 herdr Map，或为临时子任务建立新的持久业务定义。
- 新的统一权限引擎、额外逐次审批政策、OS/VM 沙箱及凭据隔离平台。
- 宿主或后端重启后自动接管旧 Run、无人值守持久队列、运行中模型配置热更新。
- 未经授权更新用户现有生产部署、认证信息、Task 预设或正在运行的 Chat。

## Further Notes

### 依据与当前状态

本 spec 综合本次完整讨论，包括已确认的 Q7–Q18 及更早的专用 Suite、暂缓专题与 OMP task 选择。后续决定覆盖早期提案：不建立独立 oh-my-pstack 项目，不把技能曝光误称为正文全量加载，不为权限或角色重新建设运行时。

调查基线为本地 pstack manifest 版本 0.15.46；公开集合最初有 58 个 skills、22 个 playbooks、2 个 persona 和 4 组脚本入口，Benny 另有内部资源。这些数字用于来源说明，不是最终交付数量承诺：暂缓功能、必要依赖和生成的角色席位会改变交付清单。实际清单须明确资源归属和引用闭包。

已存在的调查与待办材料：

- [调查与逐项设计记录](../../docs/research/pstack-omp-suite.md)
- [包内待迁移事项](E:/Developer/myhub/my-agent-toolkit/pstack-suite/MIGRATION-BACKLOG.md)
- [项目领域词汇](../../GLOSSARY.md)
- [Suite 按宿主声明技能源路径的 ADR](../../docs/adr/0018-host-specific-native-skill-paths.md)
- [Suite 使用与装配说明](../../docs/usage.md)
- [oh-my-wayfinder 生成维护方式](E:/Developer/myhub/oh-my-wayfinder/docs/maintenance.md)
- [toolkit 持续模式先例](E:/Developer/myhub/my-agent-toolkit/extensions/codebase-tools/README.md)
- [OMP 原生 persona 与模型策略](E:/Developer/myhub/oh-my-pi/docs/task-agent-discovery.md)

当前已写调查文档及包内待迁移文档，尚未实现生成器、适配技能、角色生成或持续模式，也未发布新的 Suite 定义。本 spec 的 ready-for-agent 是规划输入状态，不表示功能已实现、生产部署已获授权或验收已通过。

### 下一步

[Contract 已批准](contract.md)，其承诺、seam 选择、生命周期及验收责任由该文档维护。用户选择 Rapid iteration，模型自主执行提示词的效果不纳入验收；具体代码检查与人工项见 contract，本文不重复维护清单。

下一步执行 `/to-tickets .scratch/pstack-omp-suite/contract.md`。若实施发现已批准承诺或 seam 不成立，先修订 contract，再调整受影响票据；本次规划发布不构成生产部署或外部写操作授权。
