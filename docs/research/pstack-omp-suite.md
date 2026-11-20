# pstack → OMP-only Suite：调查与待决边界

状态：事实调查完成，迁移设计待用户确认；未实现、未运行宿主验收。调查日期：2026-11-20。当前 herdr-bot 已在 codebase-memory-mcp 建立索引。

## 来源与术语

- pstack 源根：`E:/Developer/myhub/agent-skills/cursor/plugins/pstack`，manifest 版本 0.15.46。下文 P 指该根。
- OMP 源根：`E:/Developer/myhub/oh-my-pi`，下文 O 指该根；源码支持不等于当前安装版已实测。
- Toolkit 根：`E:/Developer/myhub/my-agent-toolkit`，下文 T 指该根。
- Suite 沿用本项目 GLOSSARY.md：场景工具集合，不是运行时、Task 或初始指示。Suite 的 `agents: ["omp"]` 表示兼容宿主，不是子代理 persona。
- Skill slash 入口把指令交给模型；extension command 执行注册的本地 handler；二者不能混称运行时能力。

## 总体结论

pstack 是宿主执行的工程工作流包，不是自带调度器、权限系统或记忆服务的运行时。发现 58 个公开 skills、2 个 agents、22 个 playbooks、4 组可执行入口，以及默认不启用的 Benny 自动化资料。Benny 另含 3 个内部 SKILL，不在公开 skills 集合中。

`P/.cursor-plugin/plugin.json:24-31` 只声明 skills 和 agents；根 plugin.json 只有元数据。调查未发现独立 commands/hooks/MCP 注册。`/poteto-mode`、`/swarm`、`/setup-pstack` 是 skill 入口；Cursor `/loop`、`/autopilot`、`/automate` 是宿主能力。

证据：P/README.md:41-53,93-107,258-286；P/automations/benny/FOR_AGENTS.md:79-89。

## 58 个公开 skills

名称对应 P/skills/<name>/SKILL.md。57 个声明 `disable-model-invocation: true`，仅 setup-pstack 未声明；隐藏自动调用不等于不能手工调用或被 mode 按路径读取。

| 分组 | 名称与用途 |
|---|---|
| 入口 | setup-pstack（模型/预算配置）、poteto-mode（主流程路由）、poteto-help（导航）、figure-it-out（设计可验证流程） |
| 理解与交接 | how、why、teach、recall、ketchup、ketchup-video、in-your-own-words、sharpen；分别覆盖代码机制、历史原因、教学、跨会话恢复、进展报告、视频、复述和请求改写 |
| 设计与并行 | architect、arena、swarm、interrogate；调用形状与架构、竞争方案及独立裁判、分片/竞速、多模型对抗评审 |
| 验证 | blast-radius、benchmark-checklist、tdd、create-verification-skill、maintain-verification-skill、create-formal-verification-skill、maintain-formal-verification-skill、statechart |
| 代码与表达 | no-comments、unslop、bro、technical-writing、typescript-best-practices |
| 学习与集成 | automate-me、reflect、correct、show-me-your-work、make-bot-ui |
| principle- 核心 | laziness-protocol、foundational-thinking、redesign-from-first-principles、attack-the-premise、subtract-before-you-add、minimize-reader-load、outcome-oriented-execution、experience-first、exhaust-the-design-space、build-the-lever |
| principle- 架构 | model-the-domain、boundary-discipline、type-system-discipline、make-operations-idempotent、migrate-callers-then-delete-legacy-apis、separate-before-serializing-shared-state |
| principle- 验证 | prove-it-works、fix-root-causes、sequence-verifiable-units、test-behavior-not-implementation、explain-the-number |
| principle- 委派与元规则 | guard-the-context-window、never-block-on-the-human、encode-lessons-in-structure |

后四行名称须加 `principle-` 前缀。证据：各 SKILL frontmatter；P/README.md:109-157,222-253。

## 流程与角色

poteto-mode 由主会话匹配 playbook、写 todo、委派和裁决。叶 skill、原则和 worker.md 按需读取，不能只复制入口 SKILL。

22 个 playbooks：investigation、bug-fix、perf-issue、hillclimb、runtime-forensics、trace-forensics、feature、refactoring、prototype、visual-parity、authoring-a-skill、eval、babysit、shipping、opening-a-pr、autonomous-run、autopilot-full、autopilot-stack、session-pickup、pause-safely、multi-phase-plan、worktree-cleanup。它们是 Markdown 流程，不是独立 command 或持久状态机。

- poteto-agent：委派执行角色；含 Cursor `is_background`、模型 slug 和插件目录回退，须改为 OMP task/persona。
- comment-sicko：审注释。原文宣称只读，但没有足以证明只读的工具权限配置；迁移应明确只报告、父代理编辑，并核实工具边界。

证据：P/skills/poteto-mode/SKILL.md:11-33,74-113；worker.md:1-11；P/agents/poteto-agent.md:1-10；P/agents/comment-sicko.md:1-29；P/skills/no-comments/SKILL.md:19-24。

## 真正需要适配的能力

| 能力 | 原实现 | OMP 迁移要求 |
|---|---|---|
| setup-pstack | 模型按 skill 指令选择角色/预算，写 ~/.cursor/rules/pstack-models.mdc；不是安装器 | 移除 Cursor 全局规则路径和硬编码 slug；使用已授权 OMP 模型与推理配置，确定配置作用域 |
| swarm / arena / interrogate | 模型通过宿主 Task 批量委派，依赖云/本地参数、隔离产物、SHA、独立裁判 | 改用 OMP task/wait、真实工作区隔离；本地 worktree 不等于云 VM；少厂商或少席须明示 |
| recall / automate-me | 读取当前项目 Cursor transcripts、Git、PR及外部记录 | 使用 OMP 项目范围会话入口；不复制 Cursor slug/JSONL 位置规则 |
| reflect / correct | 审批后改 skill，或把教训固化为类型、测试、结构约束 | 保持审批与资源所有权；不是自动记忆 hook |
| show-me-your-work | TSV 决策证据与事后 transcript 对账 | 保留证据工件，不误称自动审计或记忆数据库 |
| 粘滞 mode | Cursor Custom Mode 与一次 skill 附加不同 | 一次 /skill: 不保证每轮注入；如需强制持续模式，另设计会话级运行时 |
| autonomous / autopilot | 指令、watcher、宿主循环和人工授权组合 | OMP /loop 不等于定时调度或进程退出后继续；持久唤醒/恢复另定边界 |

证据：P/skills/setup-pstack/SKILL.md:8-83；swarm/SKILL.md:9-41；arena/SKILL.md:9-48；interrogate/SKILL.md:9-29；recall/SKILL.md:9-34；automate-me/SKILL.md:15-86；reflect/SKILL.md:9-17；correct/SKILL.md:13-30；show-me-your-work/SKILL.md:34-57。O/docs/tools/task.md:33-76、docs/task-agent-discovery.md:222-271、docs/session.md:624-637、docs/tools/recall.md:3-22。

OMP 自有 recall 的 Hindsight/Mnemopi 记忆查询不等价于 pstack 的项目活态恢复。省略 task model 也不一定继承父模型：persona 固定 model 和 override 仍可能生效。工具白名单不能未经核实地视作覆盖 MCP/extension 的沙箱。

## 可执行资产与外部系统

- `P/skills/show-me-your-work/scripts/log.sh:1-39`：Bash 追加和清洗 TSV，不是工具调用 hook。
- `P/skills/poteto-mode/scripts/worktree-audit.sh:1-134`：Bash/git/gh/jq，报告清理分类；不直接删除，但会 fetch 和写临时文件，不能称完全无副作用。Cursor/macOS 分支须替换。
- `P/skills/poteto-mode/scripts/watch-pr/`：Bun + commander + 已认证 gh；bootstrap 自动安装依赖到脚本目录；读取状态、评估 gate、轮询，不 merge。多会话共享依赖写入须考虑。
- `P/skills/statechart/scripts/`：真实 TLC/Java 子进程和 TypeScript 管线，生成 Mermaid/报告；支持 Windows 清理。mise 可供应 TLC/Java，已有 TLA2TOOLS/JAVA 可跳过安装。用户 view 是可执行模块。证据：statechart.ts:86-150、run.ts:67-70,187-204,274-329,350-492、worker.ts:10-30、kill.ts:3-13。
- ketchup-video 规定视频/字幕/抽帧验收，但未捆绑渲染器。
- deslop、control-cli、control-ui 来自 cursor-team-kit，不属于包内能力。
- make-bot-ui 是 Cursor Automation/Grok webhook 设计指令，不附带服务。

### Benny

默认休眠资料包，包含 setup-benny、triage-issue-reports、reproduce-and-fix-issues 三份内部技能、两个 prompt 及配置样例。两条实际流程：Slack 线程问题分类/去重/建票，以及可信 triage 后真实 UI 复现、有界修复、证据验证、draft PR；不 merge/deploy。依赖 Cursor Automations、Slack、tracker、forge、应用控制适配器与凭据。只协调者能写 Slack，worker 不得获得密钥；缺必要输入 fail closed。

这是额外的事件接入与外部写能力，不能靠重命名 skill 宣称迁移完成。证据：P/automations/benny/FOR_AGENTS.md:7-89、skills/setup-benny/SKILL.md:66-95,179-213、templates/configuration.example.yaml:15-54。

## Suite 与 extension 的最小接入

现有 Suite 即可表达 OMP-only：`agents: ["omp"]`，`skills.omp` 放改写后技能集合目录，`extensions.omp` 放显式资源包或可执行入口。无需为了该声明修改 herdr-bot 代码。

- skills.omp 只发现集合的直接子 SKILL；保留引用树。不同实体同名拒绝启动。不要同时加载原版和迁移版，或未经处理叠加同名 wayfinder skills。
- Suite 不代替 Task 初始 Skill、模型或固定初始提示；OMP 跳过 Suite 本地 prompts 字段。
- Suite 的 agents 字段不是 persona 清单，没有单独 persona 字段。
- 可在 T/extensions/pstack/omp-package/agents/ 放改写 persona，Suite 指向 omp-package 目录。OMP 从显式 extension root 的 agents/*.md 发现角色；只有资源、没有 TS handler 也可加载。
- Persona 推荐 pstack-poteto-agent、pstack-comment-sicko，避免覆盖常用名称。项目/用户 persona 仍可能优先，Suite 不是 agent 沙箱。

证据：[Suite schema](../../backend/src/suites.rs)、[入口校验](../../backend/src/suites/extensions.rs)、[原生资源包先例](../../backend/src/suites/mcp.rs)、[宿主技能决策](../adr/0018-host-specific-native-skill-paths.md)、[使用合同](../usage.md)。O/packages/coding-agent/src/task/discovery.ts:53-118；extensibility/extensions/directory-resolution.ts:102-145。

### 是否注册 command

**基础迁移不需要。** OMP 已提供 `/skill:poteto-mode`、`/skill:swarm` 等入口，加载正文和参数作为用户归属消息交给模型。不要为 58 个 skills 重建 handler 或 Cursor 别名。

`registerCommand` 是本地代码 handler，不会自动提交模型指令。只有选择了确定性配置写入、明确的会话模式开关、状态查询、强制生命周期/审批等需求后，才值得增加可执行 extension。文件存在或命令注册不等于后台服务、外部权限或持久恢复已实现。

证据：O/packages/coding-agent/src/modes/skill-command.ts:62-124；extensibility/extensions/loader.ts:264-273、runner.ts:1726-1759。Toolkit 约定：T/entrypoints/omp/connect-bot.ts:1-14，extensions/connect-bot/harnesses/omp-assembly.ts:199-270，adapters/omp/native/extensibility/extensions/commands.ts:27-53。若增加代码，沿薄 entrypoint + OMP assembly + 自有逻辑模式，不创建 Pi 入口；有状态逻辑按 owner/session 隔离，不用模块级全局开关。

## 补充：Benny、云执行和外部触发的替代边界

这三项不是并列产品：Benny 是业务流程，Cursor 云 VM 是执行环境，Slack/Grok webhook 是事件入口及外部回传通道。

- **Benny**：Slack 新根帖触发分类/归属分析、tracker 去重建票、原线程 verdict；另一个流程等可信 marker 后核实修复归属、真实 UI 两次复现、一次有界修复、证据验证和 draft PR。herdr 的 Map/Bot/Task、manager 显式派发、报告、文件交接和批准可承接流程结构，但尚不是现成 Benny：须新增业务定义、应用控制适配器和 Slack/tracker 工具。依据：P/automations/benny/FOR_AGENTS.md:7-35；docs/workflow-contract.md:11-48。
- **云执行**：pstack swarm 默认请求 cloud/background，非默认已推送分支通过 cloud_base_branch 指定，依赖用户电脑资源时才 local。OMP task 已有批量异步和可选 isolated checkout，足以替代并行分片、竞争方案与代码目录隔离，不等于云 VM、OS 权限隔离或本机退出后继续。当前会话是否提供 isolated 参数取决于配置，不能默认已启用。Herdr 托管原生 Chat/终端；zone 只绑定目录，不自动创建 VM 或安全沙箱。依据：P/skills/swarm/SKILL.md:23-31；O/docs/tools/task.md 的 Inputs/Flow/Modes/Side Effects；本项目 GLOSSARY.md。
- **Slack/Grok**：make-bot-ui 的具体链路是浏览器按钮→本地服务保管 sender key→Cursor webhook routine→agent 收到不可信 JSON。它不是一个随包运行的 Grok 服务。herdr 页面已能手动发起 Task/Map、查看运行，可替代“自己的网页操作 agent”这个需求；不能直接当作 Slack webhook 接收端。OMP RPC 提供 stdio JSONL 的外部控制协议，不是现成 HTTP webhook/Slack 服务。依据：P/skills/make-bot-ui/SKILL.md；O/docs/rpc.md:1-35。
- **未发现现成外部事件入口**：herdr-bot backend/src 的描述搜索、索引符号查询及 slack/webhook/cron/grok 字面搜索未发现实现；不据此断言用户未部署外部服务。若保留事件自动发起，需可信桥接服务处理签名/凭据、事件去重、线程身份、授权与结果回传，再使用已有启动接口。不要把外部 payload 当系统指令，也不要把同机来源检查当公网认证。
- **持久性边界**：herdr scheduler 是工作图定义与发起入口，不是 cron；管理者显式推进，依赖满足不自动派发。后端重启后旧 Run 保留但只读，普通 Chat Resume 不接管；不是无人值守持久队列。OMP 进程内后台/loop 也不代表主机关闭后继续。依据：docs/workflow-contract.md:11-48；docs/workflow-runtime.md:28-34,83-90。

建议按业务价值迁移：先保留 Benny 的分流/验证/修复规范，可从 herdr 手动发起；是否接 Slack、是否远程持续执行分别决策。若后续需要停机后仍接收事件，常驻事件服务应独立于单个 OMP extension 会话。用户随后确认将这三个专题留作独立迁移或参考事项，见下节。

## 已确认范围与待迁移参考事项

用户确认将以下三项移出当前迁移讨论，另开会话展开；保留为待迁移或参考事项，不表示已迁移或永久删除：

1. Benny：Slack 来源 issue 的自动分流、复现、验证与有界修复方案。
2. Cursor 云执行：云端并行 worker 与远端执行环境。
3. Slack/Grok 事件自动化：外部事件/webhook 唤醒、授权和结果回传。

本轮聚焦 pstack 的本地核心工程能力。排除这些外部集成不等于其余全是 SKILL：仍包含两个 agent persona、playbooks/worker 指令、可执行脚本及资料模板。模型路由、会话历史、后台与隔离调用仍须按 OMP 语义适配。

用户确认 Q6：目标从一开始就是 pstack 专用的 OMP-only Suite，不叠加探索/实现 Suite。pstack 独占场景技能与行为模式，不加载 wayfinder 或其他场景技能集合；保留必要的 Task 固定扩展与项目指令。其余原生 persona、MCP/LSP 等来源须逐项核实取舍，不把技能白名单当作完整运行时沙箱。跨 Suite 同名技能不要求改名，同一会话最终技能来源须验收。

用户已确认 swarm/arena 使用 OMP task 子代理，不迁成 herdr Map；其默认流程不要求人类逐阶段介入，由主代理委派、收集、裁决及验证。需要用户决定时交回主会话。持续模式拟复用 toolkit 的 codebase-tools 声明式框架；具体默认启用、提示资源、恢复与子代理指引尚待确认。

## 补充：swarm/arena 与既有持续模式

swarm、arena 是 pstack 自己编写的可复用 skill 工作流，不是 Cursor 内建调度能力，也不是所有工作必经阶段。poteto-mode 的触发规则明确将 fan-out 路由到 swarm、bakeoff 路由到 arena；亦可独立调用。swarm 支持分片、同题竞速或混合，围绕 done predicate、覆盖与结果证据汇总。arena 则让同题候选独立产出，按 rubric 和跨厂商裁判选基底，再吸收其他候选的少量优点，最终验证综合产物。执行并行仍依赖宿主 Task。依据：P/skills/swarm/SKILL.md、arena/SKILL.md、poteto-mode/SKILL.md:15-30。

持续 poteto-mode 的原始目标是让主路由与工程纪律每轮都在上下文中，而非新增后台执行器。P/README.md:95-105 明确 Cursor Custom Mode 保持 skill 每轮可见，直至退出。叶技能、原则和 playbooks 仍按触发条件读取，不必每轮灌入整个资源树。

用户指出 toolkit 的 /codebase-tools 已有同类机制，源码核实成立：T/extensions/codebase-tools/definition.ts 声明 toggle、session-entry 激活状态、inheritance follow、first-enabled-run init、every-enabled-run reminder 和 marker；harnesses/omp-assembly.ts 通过 ompAdapter 装配。README.md:12-65 规定手动/自动入口、历史值优先、会话恢复及初始化进度语义。因此 pstack 持续指引应复用既有框架声明与装配模式，而非新建状态/注入框架。它不必与 Cursor 每轮附加完整 skill 的字节级行为相同：建议首轮提供路由、后续提醒，具体内容与启停策略待确认。尚未实施或运行验收。

## 已确认维护方式：上游快照加映射生成

用户要求参考 E:/Developer/myhub/oh-my-wayfinder，为技能文本适配建立映射脚本，以支持持续更新。已核实其 docs/maintenance.md、deltas/manifest.json、build.mjs 和 ops.mjs：upstream 快照 + manifest 文件白名单 + 按行为命名的 anchor/find/content/insert 映射 → 生成 skills；anchor 与 find 要唯一，缺失/歧义/重叠直接失败；build --check 只读检查产物一致性，check-upstream 与 preview 分别负责版本比较和映射审阅。

pstack 迁移采用同一维护原则：不手改生成产物；每项适配独立命名并说明原因；上游版本更新与适配行为变更分开评审。适配应覆盖实际修改的 SKILL、playbook、worker/persona 及其他文本依赖，而不只入口文件；无需改动的资源按清单保留，脚本/二进制资产不得误当 Markdown 统一处理。Suite 加载生成产物，不在会话启动时同步或构建。toolkit 的原生模式扩展独立维护，不混入上游生成目录。

实现时须注意参考实现会删除生成技能目录内不在白名单的所有文件，且逐文件写入并非整包事务；不能未经所有权划分直接对含脚本、运行产物或用户文件的 pstack 目录照搬清理逻辑。映射通过只证明文本可重现，不能证明上游新增行为已适配或真实 OMP 使用正确。当前已确认生成维护方式，尚未创建生成器或最终映射；输出仓库与具体适配语义仍待完成设计。

## 打包位置与相对路径

Q10 用户明确要求不新建项目，在 E:/Developer/myhub/my-agent-toolkit/pstack-suite 中集中打包 extension 及相关资源（上游快照、映射、生成器、生成的技能/persona/脚本）。沿用 toolkit 既有扩展框架，不再采用独立 oh-my-pstack 仓库方案。

Q9 用户提出尽量保持原相对路径：若生成的 skills/poteto-mode 与其他 skills/<name> 同级，../<name>/SKILL.md 无需替换。这仅免除路径映射，不代表整个 poteto-mode 正文无需适配；其 Cursor Task 参数、模型规则路径及外部插件引用仍需逐项处理。持续模式必须保留源文件目录信息或给出真实入口路径，不能把正文导入成字符串后假设相对路径仍有基准。Q9 已获用户确认：保留原版隐藏标记及相对路径布局，由持续 poteto-mode 路由读取；其他真正的宿主差异才进入映射。

## 模型、学习、外部技能与权限补充

- Q11：setup-pstack 原本是提示词层角色配置，写 Cursor alwaysApply 规则，主代理读取后为 Task 指定模型；不切换主会话模型，也没有独立模型执行器。OMP 已支持 task.agentModelOverrides、persona frontmatter model、modelRoles 与受授权限制的 per-call model；应优先复用，arena 的候选列表和跨厂商裁判选择仍属于工作流。来源：P/skills/setup-pstack/SKILL.md；O/docs/task-agent-discovery.md:239-271。
- Q12：用户表示若历史/学习可独立拆分则列入待办。核实 recall、automate-me、reflect、correct 不是 poteto-mode 与其 playbooks 的显式必经调用；可作为独立专题暂缓，基本调查/实现/修复/验证流程不依赖它们。correct 实际主要读 Git/review 并改架构、类型、lint/tests，不是历史服务。保留主流程需要的普通上下文、show-me-your-work 决策日志、pause-safely 交接说明；session-pickup 本身有 Cursor transcript 耦合，不能以这四技能暂缓为由宣称它已兼容。若暂缓通用历史读取，其 transcript 分支也须明确纳入待迁移范围，保留基于分支/交接材料的续接另行确定。
- Q13：实读 cursor-team-kit 三份 SKILL 后确认，它们也是技能，不是 Cursor 专属 API。deslop 针对 main diff 去除多余注释、防御代码、any 绕过、深层嵌套等；control-cli 指导复用/搭建 CLI/TUI harness，涉及输入、终端截图/转录、profiling；control-ui 指导复用 Playwright/Cypress/CDP 控制真实页面，取截图、日志、profile。可单独选择三份依赖纳入 pstack-suite 的快照/映射，不必加载整个 cursor-team-kit；CLI 的 POSIX pty/tmux 示例须按 Windows 实际能力适配。是否纳入待用户选择。
- Q14：此前建议不是逐字继承原版权限。poteto-mode Autonomy 默认允许可逆工作与团队聊天/ticket/eval 等外部动作，不逐次询问；部署、删数据、共享分支强推、客户消息需暂停。shipping 明确执行 merge；autopilot-full 以操作者对队列的授权为边界，允许 owner 合并及对自有分支受限 force-with-lease，禁止绕过 forge 强制批准；worker.md 对外消息更保守，原文层次并不完全统一。应呈现原规则与差异，不能未经用户确认把迁移改成所有外部动作逐次确认。开启持续模式本身与对具体任务的执行授权仍须区分。

## Q11 / Q13 / Q14 后续确认

- Q13 用户确认 deslop、control-cli、control-ui 既然是 skills，则直接迁移，作为 pstack-suite 必要依赖纳入快照与映射；仍须处理真实平台差异，不加载整个 cursor-team-kit。
- Q14 用户确认参考 pstack 原本权限，不新增统一确认门禁。原样保留授权相关文本可保留提示词层策略；宿主实际工具许可、项目约束、外部凭据与 forge 门禁仍独立存在，不能承诺跨宿主实际权限完全相同。适配中与授权相关的差异须显式说明，不能借转换静默加严或放宽。
- Q11 用户询问是否还需角色模块并将模型写死在 OMP task。澄清方向：无需独立运行时角色模块；角色可由 OMP persona/原生配置表达，主 skill 选择角色或面板成员。模型是可配置绑定而非写死到每次调用；构建期映射可生成适配资源，无必要先引入运行期转换脚本。角色粒度、配置作用域以及多模型面板的确切映射尚待确定。

## Q15 / Q16 与模型落地提案

Q15 用户确认角色模型只作用于 pstack Suite 会话，不修改全局 OMP 默认或其他 Suite。Q16 用户要求待迁移文档放在 pstack-suite，已创建 `E:/Developer/myhub/my-agent-toolkit/pstack-suite/MIGRATION-BACKLOG.md`，记录四类暂缓专题及核心交接边界。

为使模型实现具体且避免未核实的 Suite --config 通道，Q17 用户已同意构建期生成原生 persona 的最小方案：套件自有可编辑模型绑定文件 → 生成各角色/面板席位的 agents/*.md，frontmatter 使用 OMP 的 model 和 thinking-level；同一角色正文复用 poteto-agent 或明确的评审模板，不手维护重复正文。skill 按角色名调用 task，不写 per-call model；OMP 原生执行和模型选择保持不变。inherit-parent 席位省略 model，且核实不存在该 agentName 的 operator override。arena 候选列表生成独立 agent 名称目录供 skill 读取，绝不能把多个候选写成一个 persona 的 model 数组（OMP 的数组是优先/回退列表，不是并行席位）。跨厂商裁判仍由 arena 流程按已解析的模型厂商选择；不可用模型/覆盖冲突需明确报告，不能冒充预期面板。

此方案不需要运行期角色模块、动态注册 task 或全局 settings 写入。模型配置变更重新生成并在新启动的 Suite 会话验收，不承诺运行中热更新。

Q18 用户已确认：首次由适配后的 setup-pstack 检查可用模型，展示原有角色、候选席位和推理等级，经用户确认保存套件模型配置并生成 persona，不再写 Cursor mdc，也不预置未经确认的模型绑定。普通角色可明确选择继承父模型；arena/interrogate 席位逐项确认，不静默退化成同模型重复执行。后续修改重新生成，新启动会话生效；模型不可用或操作者覆盖改变实际绑定时明确报告。模型设计分支已收敛，文件格式与生成器细节属于实现工作。

## 已收敛的入口与资源决策

1. 核心资源集中在 toolkit/pstack-suite；外部自动化、云执行及历史/学习专题按 MIGRATION-BACKLOG.md 暂缓。
2. Q7 已确认：独立维护 OMP 专用资源树，保留上游版本、许可证及必要资源，不直接加载或修改 Cursor 原版。
3. Q8 已确认：pstack Suite 新会话默认开启持续 poteto-mode，提供 /poteto-mode 开关；恢复尊重保存的开关状态，复用 toolkit 现有机制。/skill:poteto-mode 保留一次性技能调用语义。
4. Q9 已确认：保留原隐藏标记与相对布局，由 mode/playbook 显式路由读取。隐藏不等于资源禁用：OMP 仍加载技能，并保留 skill:// 和 /skill: 可达性。验收主代理和子代理的实际引用与消费，不以元数据发现代替正文读取证明。

截至 Q18，各项设计选择已按上文逐项确认，等待用户对整体共同理解与进入实施的最终确认。已写调查文档及 toolkit/pstack-suite/MIGRATION-BACKLOG.md；尚未实现生成器、技能适配、persona 或扩展，也未修改 Suite/Task 定义或源插件。实施验收须覆盖技能发现与引用、原生 persona 调用、模型路由、并行产物隔离及选定脚本在 Windows 的真实消费；暂缓历史功能不作为已交付项。不能以 manifest 正确代替宿主验收。
