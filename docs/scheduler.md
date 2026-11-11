# scheduler 使用与参考

scheduler 管理服务级 Map、node 与共享 Bot 的静态定义；定义保存和浏览不发起 Chat、不调度工作。安装见 [README](../README.md#构建与运行)，可复制样例与隔离操作见 [体验指引](example/README.md)。本页说明定义读写，不把尚未绑定的定义当成可以启动的 Run。

主界面的配置组提供独立 `bot` 入口，直接进入同一套共享 Bot 管理功能；`scheduler` 入口进入 Map 视图。Map 内关联 Bot 会同步选中外侧 `bot`，返回时点击外侧 `scheduler`，不创建另一份编辑器或定义。两页只保留各自面板，Bot 页提供 `refresh`，Map 页提供“刷新全部”。紧凑／展开布局与刷新行为见[页面导航](usage.md#页面导航)。

Map 目录支持按名称、文件名或组名搜索，并可叠加“全部分组／未分组／指定分组”筛选；“清除筛选”同时重置两项条件。分组标题可折叠并显示当前筛选结果数量，行尾“分组”可直接调整归属。详情页的 group 按钮与新增 node、发起 Run、编辑文件输入声明及删除 Map 集中在顶部操作区；手机端分组按钮独占一行，其余操作按两列排列，避免挤压标题与说明。分组保存仍即时生效，不改变 Map 或 node 的编辑与删除确认规则。

## 定义职责与唯一格式

| 对象 | 字段与职责 |
| --- | --- |
| Map | 必需 `title`、`description` 字符串及 `nodes` 对象；key 是 Map 内 node ID。可选 `input_requirements` 为 Markdown 字符串（缺省为空），给用户与管理者共同说明非文件的总体输入要求；可选 `input_files: [{id,required,description?}]` 声明命名文件材料（见下文）。不含运行状态、管理者配置或重试策略。 |
| node | 必需 `title`、`bot_id`、`input`、`zone` 字符串及 `blocked_by` 字符串数组；可选 `input_rule`、`output_rule` 为文件输入／输出规则数组（见下文），`report_policy` 为 `auto`（默认）或 `manual`，`auto_close` 为布尔值（省略按 `false`）。引用共享 Bot，提供本项工作的原文输入、报告策略、完成后自动关闭设置、zone ID 与同图前置依赖；两项策略只属于 node，不从 Bot／Task 继承。Map 型 Bot 的调用节点使用可选字符串 `manager_task_id` 指定 manager Task，执行前必需且必须为 manager 类型；普通 Task 型节点不得指定它。Bot 不接受此字段。 |
| Bot | 必需 `id`、`name`、`when`、`plan_guide`、`node_guide` 字符串；可选 `task_id`（normal Task）或 `map_file`（目录内确切 Map JSON 文件名），二者互斥，也可都未绑定。跨 Map 共享，只保存目标引用，不复制定义。 |
| Task | 发起 Chat 的可复用预设，持有 agent、trigger config 与 env config；不嵌入 Map、node 或 Bot。 |
| Chat | 实际对话与原生会话身份，不写进静态定义。执行身份、报告、状态和历史不属于 Map 或 Bot。 |

Task、Bot 和 Map 顶层均可包含 `group` 字符串：同类型的同名归属形成分组，不需要独立定义或 ID。省略或 `null` 表示未分组；非空名称不得带首尾空白，区分大小写。分组仅用于组织列表与选择控件，不影响引用、运行或校验资格，node 不拥有分组字段。界面归属编辑和自举归类范围见[定义分组](usage.md#taskbot-与-map-分组)。

只接受这一种命名字段格式，没有 `schema_version` 或运行时旧格式兼容。旧 node 的 `parameters`、旧 Bot 的 `use_when`、`description`、`inputs`、`outputs`、`implementation` 都是未知字段，连同版本 1 也明确拒绝。上线前停止服务、备份旧文件，再由规划者改写为新格式：把业务参数整理成 `input` 原文，明确选择 `zone` ID；将旧能力说明整理成三份文档。不能从旧业务参数猜测 Task／zone 绑定。历史更名工具只处理其原来的命名迁移，不完成这次格式切换。

完整 Map 示例：

```json
{
  "title": "检查计划",
  "description": "先准备，再核对。",
  "nodes": {
    "A": {
      "title": "准备",
      "bot_id": "review-work",
      "input": "准备 alpha 材料；在工作中说明材料位置。",
      "zone": "default/review",
      "blocked_by": []
    },
    "B": {
      "title": "核对",
      "bot_id": "review-work",
      "input": "对照要求核对 beta 材料。",
      "zone": "default/review",
      "blocked_by": ["A"]
    }
  }
}
```

共享 Bot 是独立文件，按 `id` 关联而非文件名；A、B 或另一张 Map 可引用同一份 Bot，不产生副本：

```json
{
  "id": "review-work",
  "name": "核对工作材料",
  "when": "需要对照要求核对材料时。",
  "plan_guide": "安排核对范围、材料位置和交接条件，不修改原始材料。",
  "node_guide": "阅读本项 input，核对差异并说明依据与未确认事项。",
  "task_id": "review-node-task"
}
```

`when` 用于决定是否选用能力，`plan_guide` 指引规划者安排工作，`node_guide` 指引规划者填写 node.input。三份都是独立文档，可按任意顺序读取，可各自为空字符串；不要求先读其他文档，也不会自动注入节点执行提示词。节点执行使用关联 Task 的固定 `prompt` 与本次 `node.input`，不以 `node_guide` 替代。Markdown 只是文档内容，不是必填规则、类型检查或业务质量门禁。文档不会自动补全 node 输入、校验业务参数或读取文中路径。定义原文保留 `${key}` 占位符。Task 固定 prompt、Bot 三份指南、Map description 与 node.input 适用本机提示词词典；系统发送配置字段前单次替换，agent 直接读文件时按统一追加的词典路径说明自行解析，不做 JSON 解释。用户输入与 agent 产物不替换。

### node.input 的业务职责

`node.input` 只承载本项工作的业务指示：目标与非目标、命名材料的用途、允许修改的对象和字段、交付内容与格式、验收条件及业务冲突处理。必要的业务操作指引若尚未由关联 Task 提供，仍须保留；文件流转规则不能替代操作知识。Bot 指南不会自动注入，因此不能以“指南中已有”为由删除执行所需的业务要求。

| 内容 | 承载位置 |
| --- | --- |
| 文件来源与上游交付物引用 | `input_rule`；正文解释命名材料用途，不重复地址或寻址算法 |
| 可交接文件的逻辑标识 | `output_rule`；正文说明内容、格式与业务完成标准 |
| 本次 `userInput`、解析后的文件地址、中间资料位置 | 系统初始化，不为取得这些信息读取运行记录或推导运行身份 |
| 前置依赖与流程推进 | `blocked_by` 与管理者编排，不由消费者筛报告重做调度 |
| 报告发送时机 | `report_policy` 与系统控制段；业务内容确认、保存授权仍属于业务约束 |

交接说明与可直接使用的纯正文分别声明输出，下游逐项引用；不只登记交接说明，再通过同目录、固定文件名或报告正文发现其他文件。内部交接使用初始化提供的中间资料位置，不在每个 input 中规定材料根／Run ID／工作编号／execution ID 的路径算法。用户明确指定的正式交付位置、格式或业务命名仍保留。输出 ID 不指定物理文件名，也不替节点分配地址或生成文件。

例如汇总 Bot 三份指南时，命名输入可为 `when_text`、`when_handoff`、`plan_guide_text`、`plan_guide_handoff`、`node_guide_text`、`node_guide_handoff` 和 `baseline`。各编写者、表达优化者与保存者直接取得同一份共同基线；text 是所属字段唯一完整正文，handoff 仅记确认依据、来源、语义限制和未验证项，不复制完整正文／配置或承担隐式寻址。input 说明原样汇总三个字段、核对同一 Bot 及其 normal Task／Map 目标、确认范围和真实 revision，一次合法保存并完整读回，保持全部非目标和执行目标配置不变。完整最终对象另交付 saved。详细职责与独立 Chat 分支统一见[共同业务材料契约](business-materials.md)，不复制协议或查询运行记录。材料身份、共同基线、用户确认及保存冲突检查不能随寻址去重删除。

迁移旧定义时，先声明真实输出并补齐消费者的输入规则，再移除人工寻址、全图工作—文件映射、持续追踪上游当前 execution 和重复报告策略。通用保存与交接要求遵循[统一文件交接约定](workflow-runtime.md#统一文件交接约定)。规则仍为空或通用职责尚无承接方时，不得仅删文字造成信息丢失；本规范不表示存量 Map 或 Task 提示词已经迁移。

### Map 命名文件输入与根 Run 绑定

`input_requirements` 只承载非文件的总体要求；非空时在发起 Run 表单的“本次文件输入”上方以 Markdown 展示，缺省或空字符串时不显示独立要求块。每份文件的要求放在对应 `input_files[].description`，与总体要求共同供用户准备和管理者核实输入，不是自动业务校验器。点击“编辑文件输入声明”，一并编辑输入要求与声明数组并保存，例如：

```json
[{"id":"brief","required":true,"description":"业务委托：包含目标与约束。"},{"id":"appendix","required":false,"description":"参考材料：注明用途。"}]
```

`input_files` 声明材料 ID、布尔 `required` 及可选纯文本字符串 `description`，不保存每次实际文件地址。`description` 可省略或为空字符串，显式 `null` 及其他非字符串均拒绝；它作为标签显示在对应文件输入框上方，不解析 Markdown，缺省或为空时回退显示该项 ID。必需／可选状态由 `required` 单独渲染为标签后缀，不写入描述。ID 必须非空且在本接口内唯一；其他 Map 与其他 Run 可重复使用同名 ID，各自解析。缺省或 `[]` 表示无文件接口。重复 ID 保存被拒绝且不改变原 Map；编辑带原文 revision，文件已被其他操作修改时拒绝覆盖。

“+ 发起 Run”中，“本次要求”只承载业务文字，文件输入控件分别填写服务器上已有文件的绝对地址。所有文件项（包括必需项）均可在启动时留空，交由管理者对话补齐；持久 `fileInputs` 不生成未提供项；不填占位地址、默认路径，也不从业务文字或目录自动找文件。启动表单更换文件只改变新 Run 的初始绑定，不需要重写 Map；已有 Run 可由管理者通过 `prepare_inputs` 整批替换绑定，但不改变已派发 execution 的输入地址。地址绑定不是文件内容快照，不复制或锁定源文件。

公开声明编辑入口为 `GET /api/scheduler/maps/{filename}/inputs`，返回 `input_requirements`、`input_files` 和 `{file,original}` revision；`POST` 同一路径提交 `{input_requirements,input_files,revision}`，成功返回更新后 Map。启动／预检接受可选 `fileInputs: [{id,path}]`，省略与 `[]` 均表示没有绑定。启动允许必需文件暂缺；未声明 ID、同次绑定 ID 重复或已提供文件缺失／不可读仍在 Run 分配与宿主启动前拒绝（文件错误为 `run-input-unreadable`），反馈 `target: "run-input"` 与 `inputId`，页面可定位对应文件输入。

新 Run 的 `inputsReady=false`。管理者须先阅读 Map 的 `input_requirements`、每项 `input_files[].description` 及已有材料，接收用户文字／文件、澄清语义与授权，必要时在授权范围内整理文件，再用 `prepare_inputs` 绑定并确认；全部业务节点在确认前均不得 dispatch／retry，不能先派发“准备节点”代替管理者完成输入准备。必需项在确认时强制齐全；直接与嵌套调用遵循同一规则，协议见[运行输入准备](workflow-runtime.md#管理者输入准备prepare_inputs)。

根输入可读性与普通节点解析使用 `workflow_dispatch file_consumption::` 专项核对真实文件和 Windows 实际访问拒绝；原生 Chat 真正消费材料仍由原生专项负责，不能从页面成功或记录存在推断原生消费已经通过。

### 文件输入与输出规则

node 的 `input` 是业务指示文本；`input_rule`、`output_rule` 是静态交接约定，均为数组，可省略或为空数组。规则不是本次执行的真实输入输出，不预设文件名，也不强制生成文件。

```json
"input_rule": [
  { "id": "brief", "type": "file", "zone_id": "default/business", "path": "brief.md" },
  { "id": "review", "type": "node", "node_id": "A", "output_id": "summary" },
  { "id": "local_brief", "type": "run", "input_id": "brief" }
],
"output_rule": [
  { "id": "decision" }
]
```

`id` 是当前节点内用户自定的逻辑标识，不是固定字段名称或磁盘文件名。两个规则数组各自的 ID 必须非空且唯一；数组顺序不表示派发顺序。一个输出 ID 在一次执行中绑定一个实际地址。

`type: file` 接受 `id`、`type`、字符串 `path` 及可选字符串 `zone_id`；省略 `zone_id` 时使用本次 Run 的 manager zone，绝不是 node 的执行 zone。`path` 是对应 zone 内以 `/` 分隔的相对文件路径，不接受绝对路径或越界访问，并沿用 zone 的链接安全规则。启动 Run 时确定所有引用的 zone 绑定，派发时检查文件与边界，实际输入保存本机绝对路径。`type: node` 只接受 `id`、`type`、字符串 `node_id`、`output_id`。`type: run` 只接受 `id`、`type`、字符串 `input_id`，引用本 Map 的 `input_files` 声明，并按节点本地 `id` 重命名；不接受 `run_id`，不能选择其他 Run。未提供的可选输入不出现在执行 `inputs` 中；已提供但错误的地址不能当成缺席。三类实际文件在派发前检查存在且可打开；报告／Run 来源可在所有 zone 外，沿用公开文件的链接／重解析点／路径安全约定。其他必需字段、类型与未知字段校验不变；不自动同步文件或校验业务内容。

引用节点须存在于同图，`output_id` 须由提供方 `output_rule` 预先声明，否则拒绝保存整张 Map；删除提供方或移除被引用的输出 ID 同样拒绝，不级联清理。规则与 `blocked_by` 独立，不补依赖、不自动派发。

派发固定本次执行的 `inputRule`、`outputRule`，将解析后的地址以 `inputs: [{id,path}]` 保存在执行事实中；已有执行不随 Map 编辑热更新，新执行才使用新规则。来源当前执行尚未登记所需输出时明确拒绝派发，不猜路径、不回退旧执行，也不先创建 Chat。实际 report 的 `outputs: [{id,path}]` 只允许本次冻结声明中的 ID，地址更新不改变文件名。

读取、编辑及保存保持数组和来源字段；scheduler 显示静态规则，workflow 区分当前定义、冻结规则、实际输入和实际报告输出。

定向自动验收：`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch named_inputs:: -- --nocapture`。测试通过真实 HTTP 保存／读取／创建 Run／显式派发，使用隔离存储、动态数值回环端口和记录型 Herdr 进程，核对实际提交的初始化与无隐式启动；它不替代真实 Chat 的角色注入验收。

`report_policy` 独立于业务 `input`：`auto` 在结果可交接时自行发送 report；`manual` 调用 report 后由工具展示完整待发送 payload，只有用户在宿主原生确认界面明确批准该次 payload 才发送，自然语言同意不构成发送授权。修改、补充和更正均须重新确认；拒绝、确认超时（toolkit 的 reportConfirmationTimeoutMs 统一配置，默认 20000 毫秒）或无可用确认 UI 均不发送，停止并等待用户重新提出请求，不自行重试。它不新增后端审批状态、授权令牌或 report/complete 门禁；业务批准要求独立保留，管理者仍负责 complete。Map 读取／保存及新增 node 模板显式返回默认 `report_policy: "auto"`；只允许这两个小写字符串，`null`、其他类型和非法值均拒绝。派发时固定本次执行策略；修改 Map 不热更新已有 Chat，恢复沿用原执行，独立 retry 读取当前 Map。旧执行缺字段视为 auto，无需迁移。

### node 策略编辑

在 scheduler 选中节点后点“修改 node”，或从“新增 node”开始：

- `auto_close` 默认关闭；关闭 Tab 会终止 Tab 内终端及相关进程，请按节点分别选择。`auto_close` 只接受 JSON 布尔值，不能填字符串或 `null`。
- `report_policy` 用 `auto`／`manual` 互斥单选圆点选择，省略按 `auto`。
- `node_id` 继续使用独立身份输入（已有节点只读）；`zone` ID、`report_policy`、`auto_close` 分别由独立控件读取和编辑。只读、新增与修改共用同一明细布局：身份与标题之后先显示 report_policy、auto_close 和 zone，再显示 Bot ID、左右对称的前置／后继依赖，最后显示输入内容、Task.prompt、input_rule、output_rule。策略和自动关闭状态用带文字及原始值的状态标识展示，不仅依赖颜色。默认表单可编辑节点标题、Bot ID、输入内容和前置依赖；在前置栏展开“选择前置节点”勾选依赖，后继栏由其他节点的 blocked_by 计算，只读展示。Bot ID 支持已有目录建议，也保留尚未登记的引用；Task.prompt 随当前草稿关联的 Bot 更新，但不在 node 中编辑。通过“JSON 编辑”切换高级模式，完整编辑 `input_rule`／`output_rule` 等内容字段；“表单编辑”切回常用字段。两种模式共享同一草稿，表单只改目标字段，不丢弃其他属性；原始 JSON 无效或不是对象时保留全文并停留在 JSON 模式。“node 内容 JSON”排除顶层 `node_id`、`zone`、`report_policy`、`auto_close`，其余字段保持既有 JSON 数据形状。字段格式与保存约束可展开查看。
- 保存时合并最新内容对象与独立设置，再提交既有 node 保存接口，不丢弃内容字段；JSON 中手动加入独立字段会明确拒绝，避免双重来源。无效 JSON／非对象草稿保存失败且原文保留；独立控件仍可编辑，不会格式化或覆盖 JSON。内容或独立设置的未保存修改均受既有离开确认保护；取消／放弃不会写入定义。Task 的 `prompt` 编辑与展示职责不变。
- 保存后可重新打开核对；Map 读取、编辑及保存按既有默认值约定显式返回 `auto_close: false` 与 `report_policy: "auto"`。不改变报告接口、审批权限或 manager 的完成权限。策略指示是发给 agent 的约束，不保证模型必然遵从。

## 删除 Map 与历史 Run

选中 Map 后点击详情操作区的“删除 Map”，弹窗会显示文件名，并提供三个选项，不再追加确认：

- **删除历史**：不可撤销地删除 Map 定义（含全部 node），同时删除关联 Run 的状态、执行历史和报告记录。
- **保留历史**：只删除 Map 定义，关联 Run 仍可在 workflow 中查看；当前 Map 与依赖图会显示不可用，不补造历史定义。
- **取消**：不删除任何内容。

两种删除都先检查整个 Bot 定义目录。只要任一 Bot 的 `map_file` 引用本 Map，就拒绝并显示 Bot ID 与文件位置；不自动解绑。损坏、不可读或链接／重解析点 JSON 定义，以及无法读取的目录均使引用检查失败，不能按无引用处理；拒绝发生在历史清理前。取消不发送删除请求。

“删除历史”还会完整检查 Run 的双向执行引用：所选 Run 有 `parent`，或其当前／旧 execution 有 `childRunId`，以及其他 Run 单向引用所选记录时，均拒绝并定位父 Run、node、execution 与 child Run。即使父子同属所选 Map，也不级联删除、不自动解绑；修改现有 Map／Bot 定义不清除已保存的执行关联。“保留历史”不移除这些记录，仍只受 Bot 定义引用检查约束。任何无法可靠读取／解析的 Run 关联或身份不一致都会拒绝历史清理；损坏的历史／报告 JSONL 正文不阻止安全判定无关联的旧记录被清理，原有链接边界保持不变。

无父子引用时，删除不以 Run 的持久状态或 Chat 是否在线／工作中为门槛；仍标为 `running` 的记录也可清理。两种删除都不会停止 Chat，也不会删除 Chat、原生对话历史、共享 Bot、Task 或 zone 内工作文件。删除历史后，相关 Chat 后续针对该 Run 的上报或操作可能失败；正在进行的后端写入不会重新创建已删除的 Run 历史。

成功后刷新 Map 列表并清空该 Map 的详情和节点选择。格式损坏但仍列在目录中的 Map 也可删除。失败会显示具体错误；若历史删除只完成了一部分，或删除完成后目录读取失败，提示会明确说明，不宣称全部成功。响应丢失时应取消弹窗并刷新核实，不盲目重复提交。

## 保存与校验边界

内部结构合法即可保存：Bot 可没有执行目标，也可引用不存在或用途已失效的 Task／Map，但不能同时绑定两类目标；node 可引用不存在的 Bot 或尚未绑定／失效的 zone ID。不会制造默认绑定或复制默认 Task。`zone` 必须显式存在且为字符串，但可为空表示尚未填写；它是跨 server 的稳定 zone ID，不是目录路径，也不是 Bot 文档里的业务键。保存不证明启动前外部资源检查通过。

必需字段不可漏填，不用 `null`、对象、数组代替字符串；Map/node/Bot 都必须是 JSON 命名对象，不能用位置数组代替。未知字段明确拒绝，不静默丢弃。显式 `nodes: {}`、`blocked_by: []`、文本 `""` 合法。`task_id`、`map_file`、node 的 `manager_task_id` 可缺省或为 null（读取／保存结果省略无绑定字段）；有值必须是字符串。Bot 编辑页的 Task／Map 控件与原始 JSON 同步；主动选择一类目标会移除另一类引用，原始双绑定草稿则明确拒绝，不能按隐含优先级运行。不额外限制文档长度。

`blocked_by` 只引用同图 node ID。未知前置、自依赖或环拒绝整个保存，不能通过删边／删节点生成成功图。依赖错误指出相关 node；结构错误提供可用字段路径，例如 `nodes.work.input`。**非法保存不覆盖原定义**，失败保留独立 ID 与原始草稿；保存失败和冲突都不是成功预览。

## 从选中 Map 校验启动配置

在 scheduler 选择 Map，点击“+ 发起 Run”。发起表单提供 **manager 类型的管理者 Task（omp 或 pi）**、已登记的管理者 zone、只读 Workspace、可空的 Markdown“本次要求”及按当前 Map 声明填写的独立文件绑定。Workspace 由管理者 zone 末级目录名推导，不能临时指定当前／默认 Workspace。没有运行名称，不允许临时覆盖 node 的 Bot、Task、zone 或 input。

“校验启动配置”调用真实后端整图预检，**校验通过不表示已经启动**，此检查不创建 Run、Tab 或 Chat。“启动 Run”再次执行同一预检，通过后初始化三份运行记录、启动真实管理者并在原生身份关联保存成功后提交初始指示。取得 Run ID 后进入可刷新、可重新进入的 workflow 基础只读详情；读取详情不会再次启动。部署前必须交付 manager-chat 外部角色扩展，配置入口存在不代表原生加载已验收。运行路径、工具协议、错误部分效果及配置见[Map Run 运行说明](workflow-runtime.md)。Workspace 在表单里只展示推导名称，实际目标由启动链路向 Herdr 核实。

预检每次重读当前 Map、共享 Bot、Task 与 zone 登记及真实目录，检查全部 node（包含尚有前置依赖的 node），而不是只检查当前可派发项。空 Map、非法图、缺失／无效 Bot、未绑定或不存在的 Task、错误用途／非法 agent、缺失登记或已失效的 zone 都拒绝。管理者必须使用 manager Task，node 经 Bot 的 `task_id` 必须引用普通（normal）Task，同一 Task 也可从 zone 独立发起；两种角色均支持 omp/pi；不制造默认绑定，也不把 Bot 三份文档、Task.prompt 或 node.input 的业务质量当成启动门禁。

Map 型 Bot 通过 `map_file` 继续检查被调用 Map，节点自己的 `manager_task_id` 必须引用 manager Task。合法 A→B→C 及不同节点重复调用 B 均接受；A→A、A→B→A 按**当前调用路径**拒绝，错误 `map-call-cycle` 的 `callChain` 给出确切文件名链。调用关系检查不展开父图、不保存定义快照、不启动任何宿主。嵌套调用的执行与父子 Run 事实另见运行说明；预检通过本身不是执行成功证据。

拒绝保留发起页并逐项指出错误及来源 node／Bot／Task。用“修正 node”回到原 node 编辑器，用“修正 Bot”回到共享 Bot 配置，用“修正 Task”直接打开原 Task 编辑器；Task 已删除时明确显示不存在，不转成新增或默认 Task。zone 错误可回到 zone 登记入口，或回到原 node 修改 zone ID。原 Map 文件损坏时按错误修复配置目录中的文件。修正后返回 scheduler，显式重新校验；刷新启动选项可重读 Task 和 zone，不自动重试启动。

zone 引用按稳定 ID 解析本机绑定。同一 ID 绑定一个目录，不同 ID 可绑定同一目录；推荐 `namespace/name`，界面提示规范而不强制该格式。启动 Run 时冻结本次所需 ID 与路径，后续改绑或取消登记只影响新 Run，不改变已有执行或后续派发的位置。旧目录消失、变成文件或被重定向时拒绝执行，不跟随新绑定或目录联接。注销不删除目录，目录内容没有版本或跨 server 一致性保证。

HTTP 对应 `POST /api/scheduler/maps/{filename}/preflight`，请求接受 `managerTaskId`、`zone`、`userInput` 三个字符串及可选 `fileInputs: [{id,path}]`；未知字段、查询参数、运行名称及 node 覆盖均拒绝。合法返回 `workspaceName`，无 Run ID。配置拒绝为 HTTP 400、`code: run-preflight-rejected`，`issues` 含错误码、说明、配置目标及可得的 `nodeId`／`botId`／`taskId`。此只读检查不缓存有效性；后续实际启动必须再次调用同一整图预检，不能信任页面曾经校验通过。

两类目标、节点 manager 配置、循环和引用保护的可重复入口：`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch bot_targets`、`npm --prefix frontend run test:browser:build -- workflowBotTargets.spec.ts --workers=1`。后端用真实临时定义、Windows 独占读锁与目录 junction 核对无启动／无删除；页面检查真实表单、HTTP 持久读回、重载、取消和引用拒绝。多层正例只证明公开预检接受，不替代真实嵌套执行与原生角色消费验收。运行前使用本 worktree 独立 `CARGO_TARGET_DIR` 并先构建静态页面；完整回归入口见[测试指引](agents/testing.md)。

可重复契约检查：`npm --prefix frontend run test:browser:build -- runPreflight.spec.ts --workers=1`。真实隔离页面及 HTTP 检查空图、缺失绑定、角色／agent、真实目录注销／重登记／搬迁／替换及原配置修正；结果目录保留页面截图与响应、文件核对、外部 Herdr 记录。外部调用缺席只承诺每个测试从服务启动至最终完成请求的有界窗口（90 秒用例上限），不声称无限期不存在；记录型 Herdr 只证明拒绝路径无启动调用，不证明原生宿主或模型执行质量。P8 的有效节点实际 cwd、P9 的有效 Run/Chat 关联由实际运行链路另行验收。

## 目录与定义管理

进入 scheduler 读取服务所在电脑的目录，与当前 zone 无关。Map 与共享 Bot 属于定义数据，允许纳入 Git，UI 编辑会修改对应文件；node 内保存稳定 zone ID，由运行所在 server 解析成本机目录。旧 `.data` 定义在启动时一次性迁入，冲突拒绝且保留旧源，详见 [定义布局升级](../README.md#从旧定义布局升级)。配置可独立省略：

```toml
[scheduler]
map_directory = 'meta-data/workflow-maps'
bot_directory = 'meta-data/bots'
```

默认值如上；普通相对路径按**服务配置文件所在目录**解析，绝对路径也可用；支持 `${roots.name}/relative` 前缀引用本机根，详见 [本机根目录](../README.md#本机根目录与解析规则)。首次读取创建缺失目录及父目录，保留已有文件，不安装示例。只扫描直接包含的 JSON 文件（后缀不分大小写），不递归。配置改变需重启，文件内容改变可“刷新全部”。目录创建／读取失败与正常空态分开报告。

Map 列表显示定义中的 `title`，不以 JSON 文件名作为正常名称；标题为空、全为空白或文件无法读取／校验时回退显示文件名，仍可选中查看问题或删除。可按名称或文件名筛选。Map 仍以原文件名标识，同名标题不会合并，选择、加载和删除不重命名文件。列表按文件最后修改时间倒序排列，时间相同按文件名升序；刷新全部重新读取名称和顺序。列表选择 Map 后，依赖图在节点清单上方常驻展示，无折叠入口、图外说明或缩放工具栏。节点清单与 workflow 共用依赖层级排序：前置层在前，同层沿服务返回的当前定义顺序稳定排列；图中对应为从左到右的层级、同列从上到下的稳定顺序，不改写 Map 文件，也不改变实际执行顺序。选择 node 以 `Task.prompt` 显示关联 Bot 绑定 Task 的固定提示词，以 `input` 显示本次输入原文，并显示 `zone`、`bot_id`、`report_policy`、`auto_close`、`input_rule`、`output_rule` 和 `blocked_by`，不展示 `node_guide`。`report_policy`、`auto_close`、`zone` 集中在详情顶部的配置概览；策略及布尔值并列，`manual` 与 `auto_close=true` 用暖色强调，zone ID 独占一行并可换行。字段名保留 JSON 原名，节点输入在 Task 提示词之前展示。`input_rule` 按显式 `id` 列出 `type` 及 `path`，或 `node_id`、`output_id` 字段，`output_rule` 列出输出 `id`，避免仅用箭头表示来源。Bot 缺失或无效、未绑定 Task、Task 不存在或读取失败时显示对应提示，不沿用旧 prompt；Task 的 `prompt` 为空时明确标识为空。前置／后继可点击定位，关闭详情后焦点回节点清单。依赖图自动布局全部 node，包括独立项；自动适应视口，保留平移与键盘选择。箭头来自 `blocked_by`，如 B 依赖 A 就显示 A → B，不表示执行、材料传递或 Chat 在线状态。无节点的 Map 可阅读，不等于可以启动。

外侧 `bot` 入口打开左右分栏列表与详情，标题为 `bot`；列表按文件最后修改时间倒序排列，同时间按文件名升序，可按名称／ID 筛选。操作按钮为 `new`、`refresh`、`edit`、`delete`。node 明细不再提供“Bot 明细”跳转；需要管理共享 Bot 时从外侧 `bot` 入口选择对应条目。分别切换 `when`、`plan_guide`、`node_guide`，或通过 `json` 看只读原始定义。只读预览以独立 Task 栏显示绑定 Task 的当前名称，替代直接显示 `task_id` 的引用行；未绑定、引用不存在或名称读取失败时显示对应提示，不嵌入 Task 配置。原始 JSON 仍保留 `task_id`；刷新 Bot 后重新读取 Task 名称。原始 JSON 不是编辑器。通过外侧 `scheduler` 返回 Map 时重查来源并保留仍存在的 node 选择；来源已移除时显示原因。PC 上 Map 目录保留在左侧；右侧工作区顶部的依赖图横跨整个工作区宽度，下方左侧为节点列表、右侧为节点检查器，查看和修改 node 在同一检查器中切换。宽度不超过 1100px 时一次聚焦一个工作区域：选中节点进入详情，“返回 Map”回到图与列表并恢复该节点按钮焦点；“Map 目录”可随时展开文件筛选与刷新入口。手机导航收为横栏，不挤占节点正文宽度。编辑中的“放弃编辑”返回当前节点详情，“返回 Map”关闭检查器；有未保存修改时两者仍需确认。

Bot 列表（含空态）可新增，预览可修改；当前 Map 可新增 node，选中 node 可修改。ID 独立输入，新增初始为空，修改时只读，JSON 不含顶层 `id`。node 编辑器的 `node_id` 表示 `nodes` 的 key，不在 node 内容中新增字段；Bot ID 由服务端补回。ID 不是文件路径。新草稿使用打包的 [天气 Bot](example/bots/weather-capability.json)／[上海 A](example/maps/trip-readiness.json)，部署不依赖开发仓库；打开、修改草稿或放弃不写定义，也不自动安装依赖。

Bot 的 `new`／`edit` 页面使用 `save`／`cancel` 按钮，node 编辑页按钮不变。Task 下拉框显示普通（normal）Task 的名称与 ID，与 JSON 中的 `task_id` 双向同步；选择“未绑定”会移除该字段，仍允许保存。已有不存在或非 normal 的 Task 引用会明确标注并保留，不静默清除或替换；不提供 manager 作为新绑定选项。Task 列表读取失败或 JSON 尚未合法时停用下拉框，保留原始草稿；读取失败不妨碍按原定义保存。选择只修改草稿，点击 `save` 才提交，`cancel` 沿用未保存修改确认。

显式保存 Bot 使用服务端安全文件名，修改保留原文件名与全局唯一身份；保存 node 只改目标 node，保留 Map 标题、说明、其他 node 和其他文件（当前 Map 排版可变化）。确认保存成功后结束草稿；node 直接使用保存响应中的 Map 更新内容与依赖图，不再额外读取确认同一次保存。Bot 保存不改 Map，所有引用者仍读取同一共享能力。Bot 已保存但后续目录刷新失败时，保留已确认保存的 Bot 内容，明确提示“已保存，但刷新失败”，由用户显式刷新，不重新提交保存。未收到有效保存响应时不能确定是否已写入：保留原始草稿并提示先核实，不宣称未保存，也不自动重试；显式刷新仍须确认放弃未保存改动。

node 冲突版本以整个 Map 为单位，即使另一页面只改 B，旧 A 草稿也拒绝覆盖；Bot 按原文件与原始全文检查。冲突提示重新加载时，先复制要保留的草稿，再放弃并重开；不自动合并或重试。切换对象、tab／server、刷新、返回或浏览器重载前，有未保存改动须确认放弃；取消保留草稿。筛选只改变可见列表，不重新请求或丢弃草稿。没有 Map JSON 编辑、ID 重命名、node 跨 Map 搬移或独立 node 库。

## 安全删除与并发

删除 node 先完整检查所属 Map；有其他 node 依赖它时列出标题与 ID，不能强制、级联或清除依赖。Bot 删除先检查整个配置 Map 目录的全部 Map，存在引用则列出文件与 node。损坏／不可读 Map 或目录错误也是阻塞，不能当成零引用。只有完整检查且无引用才最终确认；取消不提交删除。确认后再次检查身份、原始版本、结构和引用，外部变化则拒绝。成功只删除目标，清除其详情并更新当前列表／图；最后一项删除后显示空态。

同一应用实例协调 Bot 和 node 写入、删除，持锁到最终检查与提交。Bot 删除先提交后，仍可保存引用该缺失 Bot 的 node；这不是允许删除已被引用的 Bot。外部编辑器不参与协调，**避免在页面提交期间编辑相关定义**；不承诺与任意外部写入跨文件原子提交。文件写到一半可报错，写完后显式刷新。不提供撤销、引用迁移或快照。

## 共享 Bot 问题排查

合法 Map 的 node、输入和依赖边不因 Bot 问题而消失；Map 页和 Bot 管理页汇总“Bot 问题”，关联问题显示在 node。单个 Bot 出错不隐藏其他健康定义，也不捏造说明或默认实现。

- **缺失 ID**：检查配置目录、JSON 扩展名以及 `id` 与 `bot_id` 是否一致，文件名不决定身份。
- **读取失败**：错误列出文件名；检查服务进程账户的文件读取权限。不能解析时可能无法推断 ID，node 同时显示缺失 ID。
- **结构无效**：按文件名、字段路径核对 JSON、必需字段、类型、未知旧字段；尤其区分 `when` 与旧 `use_when`，空字符串与漏填／null。
- **重复身份**：多个文件声明同一 ID 时列出全部文件，不任选赢家；即使一个文件无效也保留冲突。修正重复声明。

Map 文件加载、结构或依赖失败时清除当前图和详情，不把上次成功内容冒充本次；目录仍可选择其他有效 Map。修复后“刷新全部”同时重读列表、当前 Map 和 Bot，保留仍存在的 node 选择并更新其详情；确认 node 已移除时才清除选择并说明原因。切换另一张 Map 不沿用原 node 选择。普通加载、从 Bot 返回和显式刷新统一校验请求归属；较早请求的迟到结果不能覆盖当前 Map、选择或错误。没有自动监听、修复、运行时迁移或跨文件快照。

## HTTP interface 与路径边界

| 方法及路径 | 输入／结果 |
| --- | --- |
| `GET /api/scheduler/maps` | 顶层 Map 文件列表 `files`，按文件名索引的可用标题 `titles` 和分组名 `groups`；未分组文件不进入 `groups`；单个 Map 读取／校验失败不阻断目录 |
| `PATCH /api/scheduler/maps/{filename}/group` | `{group: 字符串或null}` → 更新后的 Map 字段与 `revision` |
| `PATCH /api/scheduler/bots/{id}/group` | `{group: 字符串或null}` → 更新后的 Bot 字段与 `revision` |
| `GET /api/scheduler/maps/{filename}` | 完整校验后的 Map |
| `GET /api/scheduler/maps/{filename}/inputs` | `input_requirements`、`input_files` 与 revision |
| `POST /api/scheduler/maps/{filename}/inputs` | `{input_requirements,input_files,revision}` → 更新后 Map |
| `GET /api/scheduler/bots` | Bot 目录和局部问题 |
| `GET /api/scheduler/bots/new` | 打包草稿，不含 id |
| `POST /api/scheduler/bots/edit` | `{id}` → content、revision |
| `POST /api/scheduler/bots/save` | `{id, content, revision}`；content 是 JSON 字符串；新增 revision 为 null |
| `POST /api/scheduler/bots/check-delete`、`.../delete` | 检查 `{id}`，最终提交 `{id, revision}` |
| `GET /api/scheduler/maps/{filename}/nodes/new` | 打包 A 草稿、所属 Map revision |
| `POST .../nodes/edit` | `{id}` → 真实 node、Map revision |
| `POST .../nodes/save` | `{id, content, revision, create}`；create 是布尔值 |
| `POST .../nodes/check-delete`、`.../nodes/delete` | 检查 `{id}`，最终提交 `{id, revision}` |

revision 为 `{file, original}`，绑定服务端目录内文件名与原始全文，不接受客户端任选路径。失败包含具体 message；不自动覆盖较新定义。读取不接受 zone、查询参数或请求体。Map filename 只能是顶层 JSON 文件名，拒绝绝对路径、`../`、分隔符和 Windows 文件流名。

目录及各级父路径不跟随符号链接、目录联接或重解析点，目录项也不能越界读取。全部标题、文档、input、zone 仅作文本显示，不按 HTML、CSS、脚本或 Agent 指令执行；文中路径不触发文件读取或 Chat。浏览、筛选、预览、恢复、刷新不改写现存定义；只有显式保存／确认删除才修改目标。

可重复契约检查在 `frontend/browser-tests/schedulerDefinitionCutover.spec.ts`：真实隔离 scheduler 保存未绑定定义、分别读三份文档、提交非法依赖并核对原文件。node 读取／保存会显式补全省略的 `report_policy` 与 `auto_close`；CRUD 检查保护节点 ID 集合、业务输入、依赖与非目标内容，不要求返回对象逐字等于省略默认字段的输入。失败与取消仍按原始文件字节检查未写入。`frontend/browser-tests/nodeSettings.spec.ts` 经交付页面、真实 HTTP 和独占 Herdr 记录夹具验证节点独立设置、新旧草稿默认值、独立控件与原格式内容 JSON 合并保存、非法草稿保留与独立设置放弃、关闭后果说明，以及实际派发的报告策略指示与既有 report／manager complete 权限；不把提示词送达当作模型遵从证据。执行期间的报告策略保持另由 `backend/tests/workflow_dispatch/report_policy.rs` 专门验证。

完整浏览器回归使用 `npm --prefix frontend run test:browser:build`，同时覆盖既有 scheduler 编辑／删除与 workflow Resume；前端单元测试或图形专项冒烟不能替代此入口。完整入口、安全约束和人工网络／链接规则见 [测试指引](agents/testing.md)。这些定义检查不证明真实 Herdr／omp 启动或业务执行质量。
