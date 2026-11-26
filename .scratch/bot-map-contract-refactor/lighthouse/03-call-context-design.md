# 调用上下文与系统代码改造契约：Lighthouse

> Question and discussion: [调用上下文与系统代码改造契约](../decision/03-call-context-design.md)

## Question

如何让已批准的新 Bot／Map／node 结构进入真实调用、公开成果交付、持久记录和现有界面，同时保留独立 Task、父子 Run 和 chief 各自的运行机制？本文是已确认的设计，不是已实施能力或运行验收结论。

## Discussion

### 1. 权威、范围与最小改动

静态结构以 [Bot 与 Map node 的完整数据结构和知识归属](02-definition-schema.md) 为权威：Bot 是能力索引，文案不注入执行；Task 自身完整；node 结构不变；Map 声明公开输入输出，实际路径属于本次 Run。本文只确定运行接口及消费者改造，不复制静态 schema。

保留现有执行引擎、身份归属、成果批准、显式交接、通知和恢复边界。chief 仍只直接委派 Task／Map，不新增 script 分支；Map 节点执行脚本照旧。早期基石要求 chief 覆盖三类实现，本次明确收窄为维持现状，不把未支持的能力列为交付要求。Q11 早期提到 chief 直接脚本交付的范围，已由 Q13 的最终选择取代。

### 2. 输入层级与消息装配

| 层级 | 字段与职责 |
| --- | --- |
| 静态 Map | map_input_rule.description／files：业务输入要求与命名文件声明 |
| 本次 Run | map_input.run_input／files：原始本次说明与实际文件绑定 |
| 静态 node | node.input：该节点本次业务委托；input_rule／output_rule：接线声明 |
| 本次 execution | inputs：派发时解析的实际文件绑定；保留现有冻结规则 |

运行 HTTP 和持久 JSON 继续 camelCase，因此为 mapInput.runInput、mapInput.files、inputsReady；Map 定义及模型工具的相应参数沿用其现有 snake_case 约定。规则不是实际值，map_input 不作为约束声明。files 以 `{id,path}` 表示实际绑定，Map 外部只面对公开输入 ID，内部节点通过 input_rule 引用。

旧 Run userInput 和 fileInputs 的新设计归整为 mapInput；inputsReady 仍单独保存准备状态，不因归整新增编辑原始业务要求的功能。prepare_inputs 的文件绑定与确认语义保持，适配新存储位置。必需文件齐备仍是确认输入就绪、派发业务节点前的条件，不阻止先启动管理者；chief 当前更早要求必需文件齐全的实现须对齐已批准的输入准备规则。已提供的非法或不可读文件仍拒绝。

用户在 scheduler 发起 Run 填写的说明就是现有 userInput。Chat 节点继续收到完整 Run 背景和自己的 node.input，分段装配，不把背景写回 node.input；结束节点保留现有精简上下文和无节点报告角色。独立 Task 仍以自己的固定设定、Skill 和用户输入发起，不强加流程协议。Task 固定设定与本次输入可能处于同一初始消息，公共角色由现有宿主扩展供给；不靠新增优先级规则解决冲突。

Q1 用户明确“指令没有互相矛盾，由用户自己保证，系统上不作任何限制”。因此不新增语义冲突检测、强制澄清、背景裁剪或授权推断机制；保留既有机械身份、路径、状态和授权校验。脚本只执行固定代码，沿用原有业务参数与命名材料，不增加 Run 背景或要求脚本解析自然语言。

### 3. 公开输出由 finish 登记并校验

不新增输出登记工具；在现有管理者 run 工具的 finish 动作中增加 map_output.files：

```json
{
  "action": "finish",
  "map_output": {
    "files": [{"id": "specification", "path": "E:/results/specification.md"}]
  }
}
```

对应 HTTP／Run 持久字段为 mapOutput.files。身份仍由公共 Session 和后端真实关联确定，不接受模型自行指定其他 Run。现有内部节点完成条件和结束节点例外继续适用。

管理者一次提交完整公开文件列表；系统检查声明 ID、重复 ID、全部必交项和文件合法可读。未声明公开文件时可省略文件列表，不为状态或交给用户继续的 Chat 制造文件。通过后保存公开成果并结束 Run；校验不通过不得结束，也不得返回完成成功。沿用既有持久化失败／部分效果反馈，不将这项设计宣传为新增跨文件事务。

业务执行与成果检查的职责按[结构权威的最小验收分工](02-definition-schema.md#9-最小验收与结束节点)由节点执行者承担；管理者办理通用交接，文件存在和显式 finish 都不证明其独立验收了业务质量。已经结束的 Run 不通过再次 finish 改写成果，保留首次完成事实；不增加分批登记、回滚、自动重试或自动发送报告。完成后外部控制流程原则上不变，仅调整使用公开文件结果所需的读取和交付。

### 4. 子 Map 对父节点的文件交接

父节点只声明需要的输出 ID，不预设路径。子 Map 生成成果后由管理者在 finish 登记其全部公开必交文件；父 execution 从该子 Run 的 mapOutput.files 取得自身冻结 outputRule 所需的同名子集。要求未声明 ID 时拒绝，不增加别名映射。

这些地址在父报告交付路径由后端取用，子管理者不再重复手填相同地址，也不能靠报告覆盖子 Run 的公开成果。下游仍通过现有父 node 输出引用取得文件。不要增加通用业务交接内容；确需额外说明，由 Map 声明对应交接文件。

子 finish、父 execution 的成果批准、报告通知和父管理者 complete 仍是不同事实。沿用现有子 Run 完成后才可批准父成果／提交可完成报告的边界；不因自动取得文件绑定而自动批准、报告、complete 或派发下游。普通 Task 节点报告方式保持。子 Run 尚未 finish 时仍可发送 can_complete=false 的中途报告，不要求附文件，不因此 complete 父节点或派发下游；进展／失败报告不被改造成成功文件交付。不为中途情况新增 Map 文件提前登记接口。

### 5. chief 的结构化交付，不统一生命周期

chief 保留原有能力发现、按需读取实现、对话解释及执行确认交互；不硬性规定每次向用户讲解哪些能力信息，不精简或重做现有确认流程。新字段必须接入现有预览、确认和变化核对，但不借机更改其余确认策略。

chief 的 Task／Map 委派增加可选 output_rule，形状与 node 相同：`[{id}]`；省略表示空列表，不要求文件。它是本次调用要求，不是 Bot 的文字 output_rule，也不是新增 Task 统一静态接口。运行请求和记录使用 outputRule。确认后的要求随原委派保存，不依赖 chief 对话记忆。

- Task：本次要求交给实际 Chat，报告由执行 Chat 提交 `{id,path}` 文件绑定。
- Map：所需 ID 必须属于 Map 公开输出，可选子集；成功文件交付取自已完成 Run 的 mapOutput，不重复输入地址。Map 即使没有被 chief 要求的文件，也必须交齐自身声明才能 finish。
- 成功结果的必交文件须齐全且合法可读。Task 的失败／受阻报告可保留实际已产生的部分文件，但不要求为发送中途情况而附文件。未 finish 的 Map 可以无结构化文件发送失败／受阻情况，不等待 mapOutput，也不为报告强行结束；正式 Map 文件交付仍从已完成 Run 取得，不增加提前登记或临时部分成果接口。

Traverse 后续 grilling 中，用户说明“可以允许report在未finish发送，这时can_complete保持为false”，进一步确认“未结束时自然不需要附加文件”，并在区分父节点 report 与 chief_report 后对 Q2 明确“同意”。据此收窄原“失败／受阻允许部分文件”的笼统表述：中途报告与正式成果交付分开；chief_report 沿用既有结果分类，不新增 can_complete 或进度枚举。最终发布授权为“可以，请更新”。

现有 chief 报告 background、execution、result、summary、deliverables、pendingDecisions 等说明保留；增加结构化 outputs 数组。文字 deliverables 不作为文件解析或机械校验来源。不要重复维护 Map 的实际地址。

不增加“所有 chief success 报告都必须先 finish”的通用门禁；该建议已撤回。需要交付 Map 公开文件时，使用已完成 Run 的成果是该文件交付的前提，而非让 chief 持续等待。finish 不自动发 chief_report。

报告仍先持久保存，之后向当前合法 chief 尝试通知；不绑定原 chief 窗口持续在线。原有报告确认、接收记录、通知失败反馈保持；通知失败不撤销结果或重跑业务。新 chief 可读取陌生报告所带的路径，读取文件并如实呈现，含义不明不按 ID 猜测；不要求先记得原委派、预先声明同名 ID 或先理解 ID 的业务含义。后端仍按原委派检查交付，不取消真实接收权限。路径持久保存不等于保存文件副本或保证文件永远存在。

### 6. 每个 Run 冻结自己的完整 Map

Q16 最初建议继续读取当前 Map，未获确认；Q17／Q18 最终批准只复制当前 Map，以有限范围获得基本冻结能力。

新 Run 在启动时保存本次校验通过的完整 Map 为 Run 目录内不可变 `map.json`，在管理者开始执行前落盘。原 Map 文件名保留为来源身份，不再作为此 Run 的执行定义入口。管理者取得副本绝对路径；派发、重试、依赖判断、输入准备、finish 校验和 workflow 图／定义展示统一读取同一副本。缺失或损坏明确报错，不静默回退到原 Map。

不新增副本编辑、版本编号、同步或更新机制。retry 使用原 Run 副本；新 Run 才读取原 Map 的最新定义。文件是普通 Map 格式，不另造一套 schema。

只冻结 Map 本身，不递归复制 Bot、Task、script、Skill、引用资料或文件内容；这些引用仍可能变化或失效。子 Run 启动时冻结自己的 Map，不在父 Run 启动时递归冻结子图。不承诺完整执行环境可复现，也不扩大到 Task 环境恢复快照。

### 7. 删除与既有界面

删除原 Map 不再提供连带删除 Run 历史的功能：移除该选项、deleteHistory 请求参数和对应调用分支。已有 Run 及其 map.json 保留，列表依赖 Run 已保存信息正常展示；进入详情或调用遇到失效引用时沿现有错误机制反馈。不新增依赖扫描、自动清理、修复或副本管理界面。不顺带取消其他独立 Run 删除能力；不替历史 Run 补造副本。

沿用现有页面：scheduler 编辑新的静态输入输出规则；发起 Run 表单保留本次说明和文件绑定的交互，适配新字段；workflow 展示冻结 Map、实际输入和公开输出，不新增页面端登记／finish 按钮；chief 通过原报告读取机制取得文件，不新增结果管理页面。

### 8. 消费者与实现接入范围

以下是本轮静态调查定位的接入面，不代表已改代码：

| 接入面 | 改造契约 |
| --- | --- |
| backend/src/scheduler/bots/mod.rs、scheduler/maps/mod.rs、scheduler/mod.rs | 消费新静态字段；运行解析使用 Run 副本，Bot 目标仍动态解析；Map 公开接口同名检查；删除 Map 不再连带清理历史 |
| backend/src/runs.rs、runs/preflight.rs、startup.rs、preparation.rs、retry.rs | 保存并统一读取 map.json；新实际输入字段贯通；管理者路径指向副本；保留输入准备、执行身份和故障事实 |
| backend/src/runs/completion.rs、reports.rs 及角色控制器 | finish 输出参数、校验与持久结果；父报告取公开成果；保留批准、显式交接与通知分离 |
| backend/src/chief_delegation.rs、chief_reports.rs | Task／Map 委派输出要求、原确认流程适配、结构化报告与文件校验；不新增 script 分支 |
| backend/src/chats/mod.rs、prompts.rs、tasks/catalog.rs | 保持 Task 独立启动、现有角色装配和词典边界；不新增 Bot 文案注入或语义冲突检测 |
| frontend/src/controllers、scheduler、workflow | 新字段类型和现有编辑表单、启动、冻结图、实际输入输出展示；删除联动历史选项 |
| my-agent-toolkit 的 extensions/workflow-chat | manager run/finish 参数、输入准备和公开结果；chief 委派／报告 schema；node／manager 指引及父层绑定；OMP／Pi 入口共同适配 |
| meta-data/scripts 及定义创建／修订消费者 | 不改变脚本宿主业务参数；现有读写旧 Bot 指南的脚本、材料契约必须随新定义改造，具体迁移与提示词由对应票提供 |
| docs/scheduler.md、workflow-runtime.md、usage.md、相关 schema | 产品实施时同步新结构、冻结与报告协议；本次不把设计当作现有产品能力 |

工具包位置由本仓 paths.local.toml 和 meta-data/task-options.toml 引用追踪，不凭目录邻接猜测。Herdr App/core 是会话宿主控制面，本轮未发现其直接消费 Bot／Map schema，不为此次改动引入宿主重构。

### 9. 后端 prompts 模块的集中维护

在[关联提示词的联合设计与确认](04-coherent-prompts.md)的同轮会话中，用户要求将按流程角色派发提示词的代码集中维护，并在 Q42 明确“可以，扩展原有prompts模块”。这是本节新增的已批准代码组织要求，不改变身份、状态或运行机制；本节为模块设计唯一权威，关联正文由该票维护。

扩展现有 `backend/src/prompts.rs` 为 `backend/src/prompts/`：`mod.rs` 提供装配入口，`dictionary.rs` 承接现有词典，`compose.rs` 组合 Skill、Task、角色上下文与业务输入，`context.rs` 格式化运行事实，`roles/` 集中管理者、节点及 chief 相关静态说明。静态正文可使用 `include_str!` 编译嵌入，不增加运行时部署文件依赖。具体内部文件拆分服务实际职责，不新增模板引擎、可插拔角色框架或重复入口。

调用方提供已经核实的角色与运行事实；模块负责基本流程说明、上下文格式及组合顺序。将 `runs/startup.rs`、`chief_delegation.rs`、`chats/mod.rs` 中对应拼接逻辑迁入，根管理者和子管理者复用同一权威正文；不要只建立一个转发函数而把字符串继续散放各处。后端负责基本流程说明，extension 负责工具调用提示；后续已确认的这一分工取代“后端只提供事实、所有说明都迁到 extension”的早期建议。

不搬入身份判定、状态准入、文件绑定解析或 Chat 启动；不搬入 toolkit 工具指引、Task 业务正文或 Map 专属文档。不修改角色装配资格、结束节点例外、恢复关联、原生确认或工具机制。只对明确类型的定义字段做词典替换；不替换用户输入、文件路径或报告，每条消息使用同一份词典快照，保留原生初始 Skill 的组合语义。

按[结构权威的职责修订](02-definition-schema.md#9-最小验收与结束节点)，后端只提供通用角色协议及本次运行事实，不为管理者注入某张 Map 专属的业务指引或业务文档入口，也不自动注入 Map.description。节点专属的条件式读取指引通过 node.input 供给；Chat 派发沿用同一 Snapshot::render(Field::NodeInput, …) 机制解析其中词典变量，再交给节点按条件读文档，不在后端展开文档正文。脚本仍接收原始业务参数，不扩大模板处理范围。无需为此新增 manager_rule、替换 Map 副本内容或给管理者增加查词典入口。

### 10. 验证来源、限制与确认来源

复用现有系统测试接入面，按 docs/agents/testing.md 选择实际定向命令：独立 Chat 发起、workflow_start、命名输入与派发、nested_completion、end_node、脚本执行、chief_delegation 的 Map／报告／接收记录，以及 scheduler 删除、前端编辑和 workflow 展示。后端现有文件包括 backend/tests/chats/chat_creation.rs、backend/tests/workflow/workflow_start.rs、workflow/lifecycle/nested_completion.rs、workflow/lifecycle/end_node.rs、workflow/scripts/typescript.rs、backend/tests/chief/chief_delegation/ 下的 map.rs、reports.rs、receipts.rs。真实接入验收设施由 herdr-bot-tester 维护；不新建通用自动跑流程平台，不将已有分层验收一律降级。最终命令和新增边界断言由后续实施按修改范围选择。

本轮仅静态读取。已定位的事实包括：Run 当前无 Map 快照及公开输出；finish 当前只检查节点完成；chief 当前仅支持 Task／Map；chief 报告先保存后通知，文件交付物目前为文字；节点上下文和脚本参数不同。Toolkit 现有公共说明存在遗漏 run 来源、输出门禁和结束节点例外的旧文案，联合提示词票须按已批准协议对齐，而非让消息顺序覆盖冲突。真实仓库探索 Task 的固定 prompt 为空，稳定规则分散在旧指南／委托中，属迁移调查线索，不授权直接删除。

确认来源：本票 2026-11-27 grilling 会话 Q1–Q21，用户最后明确“同意，请发布”。关键原话：Q1“系统上不作任何限制”；Q6“沿用原有规则即可，不要做不必要的修改”；Q7“保留原有的确认交互形式”；Q9“子map去登记地址似乎更好，可以按现有设计实现”；Q13“维持原状即可，不需要新增script分支”；Q15“尽量沿用当前设计”；Q17“范围是只复制map本身”；Q19“不保留原来的删除run历史功能”；Q20“尽量沿用原本逻辑”。Q8 批准 finish(map_output.files)，Q11／Q12 批准 chief 结构化输出及可选要求，Q14 明确新 chief 可如实报告陌生文件内容，Q18 批准 map.json 存储边界。

被撤回或取代的候选：单独成果登记工具、脚本增加 Run 背景、chief 直接执行脚本、固定／精简 chief 讲解与确认展示、对所有 chief 成功报告新增 finish 门禁、运行读取当前 Map、不分青红皂白冻结全部依赖。它们不是实施要求。

## Decision

以现有运行和交互机制承接新结构：finish 登记 Map 公开成果，父节点及 chief 按调用要求取得文件绑定；每个 Run 冻结自身 Map，chief 保持 Task／Map 异步委派，删除定义不再连带删除 Run 历史。扩展既有 prompts 模块集中后端角色说明与消息装配。本文完成调用及代码组织设计，不交付生产实现、提示词正文或历史数据迁移策略。

## User stories

1. 作为节点调用者，我希望保留完整 Run 背景与独立节点委托，以便沿用现有使用方式，不引入额外语义限制。
2. 作为 Map 管理者，我希望在 finish 一次登记并校验公开成果，以便不维护第二套登记工具或重复地址。
3. 作为父 Map 调用者，我希望按公开 ID 接收文件，以便不依赖子图内部节点和文件布局。
4. 作为重新启动的 chief，我希望从持久报告直接读取文件并如实呈现，以便不依赖旧会话记忆或持续在线等待。
5. 作为调用者，我希望未结束的执行可以不附文件报告中途情况，以便如实了解进展，而不强行完成流程或伪造正式成果。
6. 作为流程使用者，我希望 Run 使用自身 Map 副本，以便原 Map 后续修改不会改变本次流程定义，同时避免递归冻结整个实现环境。
7. 作为用户，我希望沿用现有界面、确认及脚本调用范围，以便重构不增加无关操作；删除原 Map 也不连带丢失 Run 历史。
8. 作为维护者，我希望扩展已有 prompts 模块集中后端角色说明与消息装配，以便梳理提示词时不必遍历启动、委派和 Chat 代码中的散落字符串。

## Preconditions

- 静态结构及公开 ID 规则由 [Bot 与 Map node 的完整数据结构和知识归属](02-definition-schema.md) 提供；迁移范围和共享维护规则由 [流程迁移清单与共享影响范围](01-flow-inventory.md) 提供。
- 现有真实执行关联、节点批准、报告保存与独立通知机制是复用基础，不得从窗口名称或提示词推断身份。
- 实现、公开输出登记、快照读取、chief 结构化交付尚需开发；本文确认不证明已可运行。
- 已确认提示词、转换规则、完整转换资产及历史切换条件仍需其提供方决策完成，不能假定它们已经满足。

## Postconditions

- [关联提示词的联合设计与确认](../decision/04-coherent-prompts.md)承接关联正文、创建修订指引和公共角色协议对齐，不在开发时临时生成。
- [可复用迁移方法与转换规则](../decision/05-reusable-migration.md)承接新定义及脚本／材料消费者的迁移方法；[单条流程全要素初步转换试验](../decision/06-conversion-trial.md)提供完整资产而非运行验收。
- [选择性迁移的系统切换与开发交接门槛](../decision/07-cutover-handoff.md)决定旧 Run 缺快照、旧字段、chief 历史报告和活动执行的切换边界；不从当前 Map 补造过去事实。
- 开发交接必须覆盖后端、现有 UI、工具包和定义读写消费者；不只修改顶层 schema 或 finish 参数。
- 后端角色正文与消息装配迁入扩展后的 prompts 模块；调用方提供已核实事实，保留现有词典、初始 Skill、角色装配及真实运行校验。最终正文及后端/extension 分工由关联提示词权威提供。

## Invariants

- ADR 0015：Task 独立完整，节点角色由真实执行上下文决定；结束节点不承担普通节点报告角色。
- ADR 0019：脚本是可信 TypeScript／Bun 执行，不伪造 Chat，不新增 chief 直接脚本执行，也不注入 Run 背景。
- runs/completion.rs 与 reports.rs 的成果批准、报告、显式 complete／finish、通知分离保持；未 finish 的中途报告可不附文件，can_complete=false 不推进父节点；文件存在不是业务验收。
- chief_reports.rs 的持久报告、当前 chief 路由与独立接收确认保持，不把 chief 改造成父节点管理者。
- Map 副本仅冻结图及其字段，不冻结引用实现；副本缺失不以当前定义冒充历史。
- 全部生产变更和真实业务运行验收仍在本地图的规划发布之外。
