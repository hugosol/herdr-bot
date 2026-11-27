# scheduler 使用与参考

scheduler 管理服务级 Map、node 与共享 Bot 的静态定义；定义保存和浏览不发起 Chat、不调度工作。安装见 [README](../README.md#构建与运行)，可复制样例与隔离操作见 [体验指引](example/README.md)。本页说明定义读写，不把尚未绑定的定义当成可以启动的 Run。

在配置组选 **scheduler** 管理 Map，选 **bot** 管理共享 Bot。Bot 页 **refresh** 重新读取 Bot；Map 页 **刷新全部** 重新读取目录、当前 Map 及 Bot。查看、筛选和刷新都不会执行工作。

Map 目录可按名称、文件名或组名搜索，并与分组筛选组合使用。筛选不改变当前选择或定义；修改分组见[定义分组](usage.md#taskbot-与-map-分组)。无定义、无匹配和读取失败是不同结果。

## TypeScript 脚本目标的表示契约

[ADR 0019](adr/0019-rust-script-bots.md) 的脚本目标字段为 `script`，例如 `create-blank-bot.ts`、`save-bot-guides.ts` 或用户自己的 `tools/export.ts`。路径相对配置 Bot 目录旁的 `scripts`（默认 `meta-data/scripts`），使用 `/` 分隔，不允许绝对路径、空段、`.`、`..`、反斜杠或冒号，扩展名为 `.ts`。与 `task_id`、`map_file` 按非 null 值两两互斥；均无值是不可运行草稿。定义保存检查形状，预检／派发检查实际脚本文件。

部署者须先准备 Bun 和脚本依赖。宿主动态加载可信 TypeScript，不自动安装依赖，不提供沙箱或脚本编辑器。脚本 node 不使用 `manager_task_id`。`node.input` 作为原文传给 `context.input`，可为空。脚本必须使用 `auto_approved: true` 与 `auto_close: false`，不能设为结束节点。脚本通过成功校验后由宿主记录批准并报告；失败仍可报告，但不批准。运行细节见[无 Chat 的执行](workflow-runtime.md#无-chat-的执行与报告表示契约)。

可机读表示见 [JSON Schema](contracts/script-bots.schema.json) 与 [JSONL 实例](contracts/script-bots.examples.jsonl)。用户手工添加脚本，通过现有 Bot JSON 的 `script` 字段绑定；不新增脚本管理 UI。发布创建图保留原 Bot 身份、输入输出接线与修订 replace-text。脚本最小返回状态和可选输出地址；完整编写示例见[使用说明](usage.md#动态-typescript-脚本)。预检不代表脚本业务已经执行成功。

## 定义职责与唯一格式

| 对象 | 字段与职责 |
| --- | --- |
| Map | 必需 `title`、`description` 字符串及 `nodes` 对象；key 是 Map 内 node ID。可选 `map_input_rule` 与 `map_output_rule` 是整图公开输入输出约定（见下文）。`description` 只描述用途，不作为 Map 专属执行指令。不含运行状态、管理者配置或重试策略。 |
| node | 必需 `title`、`bot_id`、`input` 字符串、`zone`（固定 ID 字符串或 `{"type":"dynamic"}`）及 `blocked_by` 字符串数组；可选 `input_rule`、`output_rule` 为文件输入／输出规则数组（见下文），`auto_approved` 为布尔值（缺省 `true`），`auto_close` 与 `end_node` 为布尔值（缺省 `false`）。`end_node: true` 标记唯一普通 Task 型结束节点（见下文）。批准模式与自动关闭只属于 node，不从 Bot 或 Task 继承。Map 型 Bot 的调用节点使用可选字符串 `manager_task_id` 指定 manager Task，执行前必需且必须为 manager 类型；普通 Task 型节点不得指定它。Bot 不接受此字段。 |
| Bot | 必需 `id`、`name`、`when`、`plan_guide`、`node_guide` 字符串；可选 `task_id`（normal Task）、`map_file`（目录内确切 Map JSON 文件名）或 `script`（scripts 目录内的相对 .ts 路径），三者互斥，也可都未绑定。跨 Map 共享，只保存目标引用，不复制定义。 |
| Task | 发起 Chat 的可复用预设，持有 agent、trigger config 与 env config；不嵌入 Map、node 或 Bot。 |
| Chat | 实际对话与原生会话身份，不写进静态定义。执行身份、报告、状态和历史不属于 Map 或 Bot。 |

Task、Bot 和 Map 顶层均可包含 `group` 字符串：同类型的同名归属形成分组，不需要独立定义或 ID。省略或 `null` 表示未分组；非空名称不得带首尾空白，区分大小写。分组仅用于组织列表与选择控件，不影响引用、运行或校验资格，node 不拥有分组字段。界面归属编辑和自举归类范围见[定义分组](usage.md#taskbot-与-map-分组)。

只接受这一种命名字段格式，没有 `schema_version` 或运行时旧格式兼容。旧 Map 的 `input_requirements`／`input_files`、旧 node 的 `parameters`、旧 Bot 的 `use_when`、`description`、`inputs`、`outputs`、`implementation` 都是未知字段，连同版本 1 也明确拒绝。上线前停止服务、备份旧文件，再由规划者改写为新格式：把业务参数整理成 `input` 原文，明确选择 `zone` ID；将旧能力说明整理成三份文档。不能从旧业务参数猜测 Task／zone 绑定。未转换的旧 Map 保留在配置定义目录之外；新版不会以旧字段或离线副本作为读取回退。不改写旧 Run 或 Chat 记录。

完整 Map 示例：

```json
{
  "title": "检查计划",
  "description": "汇总本次检查的记录。",
  "map_input_rule": {"description":"说明核对范围。","files":[{"id":"brief","required":false}]},
  "map_output_rule": {"description":"交付本次核对记录。","files":[{"id":"report"}]},
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
      "input": "结合可选的 brief 核对 beta 材料。",
      "input_rule": [{"id":"brief","type":"run","input_id":"brief"}],
      "output_rule": [{"id":"internal-review"}],
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
| 成果批准模式 | `auto_approved`；不控制报告发送。业务内容确认与保存授权仍由业务约定。 |

交接说明与可直接使用的纯正文分别声明输出，下游逐项引用；不只登记交接说明，再通过同目录、固定文件名或报告正文发现其他文件。内部交接使用初始化提供的中间资料位置，不在每个 input 中规定材料根／Run ID／工作编号／execution ID 的路径算法。用户明确指定的正式交付位置、格式或业务命名仍保留。输出 ID 不指定物理文件名，也不替节点分配地址或生成文件。

Bot 指南修订可直接引用同一 `baseline` 及三组 `*_text`、`*_handoff`。这不是 V1 创建保存的输入列表，后者不消费三份 handoff 或批准凭据。各分支的目标身份、基线、必要确认、安全保存与读回见[共同业务材料契约](business-materials.md)。

迁移旧定义时，先声明真实输出并补齐消费者的输入规则，再移除人工寻址、全图工作—文件映射、持续追踪上游当前 execution 和重复报告策略。通用保存与交接要求遵循[统一文件交接约定](workflow-runtime.md#统一文件交接约定)。规则仍为空或通用职责尚无承接方时，不得仅删文字造成信息丢失；本规范不表示存量 Map 或 Task 提示词已经迁移。

### Map 公开输入与输出规则

点击“编辑 Map 公开规则”，分别启用、编辑或移除输入和输出规则。`map_input_rule` 与 `map_output_rule` 均可整体省略；启用的对象内 `description` 缺省为空字符串，`files` 缺省为空数组。总体 Markdown 说明写跨文件或非文件要求，不是自动业务校验器。各文件的纯文本 `description` 可省略（等同空字符串），不解析 Markdown。禁止以 `null` 代替对象、数组、字符串或布尔值；未知字段拒绝。

输入文件项为 `{id,required,description?}`，必须显式填写布尔 `required`；输出文件项为 `{id,description?}`，不接受 `required`，声明即必交。ID 必须非空且各侧独立唯一；输入和输出可以同名。规则不填写实际文件地址。只需交接对话或传达完成状态时，可以只填写输出说明，不声明任何输出文件。

基于 Map 执行的父 node，其 `input_rule` 内的本地 `id` 对接子 Map 同名公开输入（不使用来源 ID），`output_rule` 只选子 Map 公开输出的所需子集。保存时拒绝子 Map 未声明的 ID；公开输出和内部节点输出无需同名，不配置静态导出映射，也不按子节点位置寻址。

### 根 Run 输入绑定

发起表单按 `map_input_rule.files` 展示输入项，以各文件说明或 ID 作为标签，并另标必需／可选。总体输入说明非空时在文件输入上方单独呈现。

“+ 发起 Run”中，“本次要求”只承载业务文字，文件输入控件分别填写服务器上已有文件的绝对地址。所有文件项（包括必需项）均可在启动时留空，交由管理者对话补齐；持久 `fileInputs` 不生成未提供项；不填占位地址、默认路径，也不从业务文字或目录自动找文件。启动表单更换文件只改变新 Run 的初始绑定，不需要重写 Map；已有 Run 可由管理者通过 `prepare_inputs` 整批替换绑定，但不改变已派发 execution 的输入地址。地址绑定不是文件内容快照，不复制或锁定源文件。

公开声明编辑入口为 `GET /api/scheduler/maps/{filename}/inputs`，返回已声明的 `map_input_rule`／`map_output_rule` 和 `{file,original}` revision；`POST` 同一路径提交 `{map_input_rule?,map_output_rule?,revision}`，一并替换两侧，省略任一侧即移除该规则，成功返回更新后 Map。非法规则不写文件；revision 已变化时拒绝覆盖。启动／预检接受可选 `fileInputs: [{id,path}]`，省略与 `[]` 均表示没有绑定。启动允许必需文件暂缺；未声明 ID、同次绑定 ID 重复或已提供文件缺失／不可读仍在 Run 分配与宿主启动前拒绝（文件错误为 `run-input-unreadable`），反馈 `target: "run-input"` 与 `inputId`，页面可定位对应文件输入。

新 Run 的 `inputsReady=false`。管理者须先阅读 Map 的 `map_input_rule.description`、每项 `map_input_rule.files[].description` 及已有材料，接收用户文字／文件、澄清语义与授权，必要时在授权范围内整理文件，再用 `prepare_inputs` 绑定并确认；全部业务节点在确认前均不得 dispatch／retry，不能先派发“准备节点”代替管理者完成输入准备。必需项在确认时强制齐全；直接与嵌套调用遵循同一规则，协议见[运行输入准备](workflow-runtime.md#管理者输入准备prepare_inputs)。

### 动态执行 zone

node 的 `zone` 为固定 zone ID 字符串，或严格对象 `{"type":"dynamic"}`。动态类型不包含预设 ID；Map 编辑中的固定／动态选择只改变定义，不指定某次运行的执行位置。动态节点允许在 Run 启动时尚无目标 zone，manager 根据上游 report 等运行信息在派发时选择已登记 zone。

manager 使用 `dispatch` 的 `zoneIds: {"nodeId":"zone ID"}` 逐节点指定，`retry` 使用 `zoneId` 且每次重新提供；固定节点不接受覆盖，动态节点缺值、未登记或目录无效均拒绝，不回退 manager zone。解析后的 ID 与目录保存在 execution 的 `zoneId`／`zone` 中，不回写 Map。Task、脚本、子 Map 共用选择机制；子 Map 的位置只作为子 manager zone，不覆盖子图内部位置。workflow 区分动态定义、尚未指定与已保存的实际执行位置。

此能力不改变文件输入与交接：下面的 `type:file` 仍使用固定来源 zone 或 manager zone，不能引用动态 node zone。

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

`type: file` 接受 `id`、`type`、字符串 `path` 及可选字符串 `zone_id`；省略 `zone_id` 时使用本次 Run 的 manager zone，绝不是 node 的执行 zone。`path` 是对应 zone 内以 `/` 分隔的相对文件路径，不接受绝对路径或越界访问，并沿用 zone 的链接安全规则。启动 Run 时确定所有引用的 zone 绑定，派发时检查文件与边界，实际输入保存本机绝对路径。`type: node` 只接受 `id`、`type`、字符串 `node_id`、`output_id`。`type: run` 只接受 `id`、`type`、字符串 `input_id`，引用本 Map 的 `map_input_rule.files` 声明，并按节点本地 `id` 重命名；不接受 `run_id`，不能选择其他 Run。未提供的可选输入不出现在执行 `inputs` 中；已提供但错误的地址不能当成缺席。三类实际文件在派发前检查存在且可打开；报告／Run 来源可在所有 zone 外，沿用公开文件的链接／重解析点／路径安全约定。其他必需字段、类型与未知字段校验不变；不自动同步文件或校验业务内容。

引用节点须存在于同图，`output_id` 须由提供方 `output_rule` 预先声明，否则拒绝保存整张 Map；删除提供方或移除被引用的输出 ID 同样拒绝，不级联清理。规则与 `blocked_by` 独立，不补依赖、不自动派发。

派发固定本次执行的 `inputRule`、`outputRule`，将解析后的地址以 `inputs: [{id,path}]` 保存在执行事实中；已有执行不随 Map 编辑热更新，新执行才使用新规则。来源当前执行尚未登记所需输出时明确拒绝派发，不猜路径、不回退旧执行，也不先创建 Chat。实际 report 的 `outputs: [{id,path}]` 只允许本次冻结声明中的 ID，地址更新不改变文件名。

读取、编辑及保存保持数组和来源字段；scheduler 显示静态规则，workflow 区分当前定义、冻结规则、实际输入和实际报告输出。

### 成果批准设置

`auto_approved` 是严格布尔值，不接受字符串或 `null`。`true` 选择自动批准模式，`false` 要求节点 Chat 在原生确认中获得明确 Yes。模式不等于已批准。报告发送不需要确认，批准不代替业务保存授权或管理者交接。派发时冻结模式，改定义只影响新执行。详见[节点报告与独立通知](workflow-runtime.md#节点报告与独立通知)。

### 结束节点

Map 可为一个 node 设置 `end_node: true`（省略为 `false`）。该节点必须引用普通 Task 型 Bot，不能引用脚本或 Map，也不能有后继节点；其他节点全部 completed 后才可派发，不能只满足部分 `blocked_by` 就提前交给用户。

结束节点成功发起后，manager 可显式 `finish`；不会自动结束 Run。发起失败或其他节点未完成时不能结束。它的 Chat 保留业务输入与已解析材料，但启动与恢复均不加载 node-chat，不要求 report／complete，不自动关闭。Run 的完成表示“已交给用户”，并非 Chat 已完成业务。包含结束节点的 Map 仍可嵌套使用，子 Run 结束后的上报与父层验收沿用原流程。

### node 策略编辑

在 scheduler 选中节点后点“修改 node”，或从“新增 node”开始：

`node_id` 是独立身份输入，编辑已有节点时只读。用独立控件设置 `zone`、`auto_approved`、`auto_close` 和 `end_node`。关闭 Tab 会终止 Tab 内终端及相关进程。结束节点不使用批准或自动关闭设置；取消结束节点后可继续编辑原值。脚本策略冲突会保留并显示原值，显式修正后再保存。

常用表单编辑标题、Bot ID、输入和前置依赖；后继依赖与关联的 Task.prompt 只读。用 **JSON 编辑** 编辑 `input_rule`、`output_rule` 等内容字段。内容 JSON 不得包含 `node_id`、`zone`、`auto_approved`、`auto_close` 或 `end_node`，避免与独立控件形成双重来源。保存合并内容对象和独立设置，不丢其他字段。无效 JSON 或非对象保留原文，不写文件；在草稿中切换模式不会静默覆盖内容。内容和独立设置均受离开确认保护，放弃不写定义。

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

内部结构合法即可保存：Bot 可没有执行目标，也可引用暂不存在的 Task／Map／脚本，但不能同时绑定多类目标；script 必须为合法相对 .ts 路径。node 可引用不存在的 Bot 或尚未绑定／失效的 zone ID。保存不证明启动前外部资源检查通过。

必需字段不可漏填，不用 null、对象、数组代替必填字符串；Map/node/Bot 必须为 JSON 命名对象，未知字段拒绝。`task_id`、`map_file`、`script`、node 的 `manager_task_id` 可缺省或为 null，有值必须是字符串。Task／Map 选择与原始 JSON 同步，选择它们会移除 script；脚本路径通过现有 JSON 编辑，原始冲突草稿仍拒绝，不按优先级运行。

`blocked_by` 只引用同图 node ID。未知前置、自依赖或环拒绝整个保存，不能通过删边／删节点生成成功图。依赖错误指出相关 node；结构错误提供可用字段路径，例如 `nodes.work.input`。**非法保存不覆盖原定义**，失败保留独立 ID 与原始草稿；保存失败和冲突都不是成功预览。

## 从选中 Map 校验启动配置

在 scheduler 选择 Map，点击“+ 发起 Run”。发起表单提供 **manager 类型的管理者 Task（omp 或 pi）**、已登记的管理者 zone、只读 Workspace、可空的 Markdown“本次要求”及按当前 Map 声明填写的独立文件绑定。Workspace 由管理者 zone 末级目录名推导，不能临时指定当前／默认 Workspace。没有运行名称，不允许临时覆盖 node 的 Bot、Task、zone 或 input。

“校验启动配置”调用真实后端整图预检，**校验通过不表示已经启动**，此检查不创建 Run、Tab 或 Chat。“启动 Run”再次执行同一预检，通过后初始化三份运行记录，并在配置选择的 TUI Workspace 或 App Space 启动真实管理者。后端先核实本次终端的原生身份和输入就绪，再保存 Run、必要的委派关联及 Chat 历史，最后提交初始指示。取得 Run ID 后进入可刷新、可重新进入的 workflow 基础只读详情；读取详情不会再次启动。部署前必须交付 manager-chat 外部角色扩展，配置入口存在不代表原生加载已验收。运行路径、工具协议、错误部分效果及配置见[Map Run 运行说明](workflow-runtime.md)。Workspace 在表单里只展示推导名称，实际目标由启动链路向 Herdr 核实。

预检每次重读当前 Map、共享 Bot、Task 与 zone 登记及真实目录，检查全部 node（包含尚有前置依赖的 node），而不是只检查当前可派发项。空 Map、非法图、缺失／无效 Bot、未绑定或不存在的 Task、错误用途／非法 agent、缺失登记或已失效的 zone 都拒绝。管理者必须使用 manager Task，node 经 Bot 的 `task_id` 必须引用普通（normal）Task，同一 Task 也可从 zone 独立发起；两种角色均支持 omp/pi；不制造默认绑定，也不把 Bot 三份文档、Task.prompt 或 node.input 的业务质量当成启动门禁。

脚本型 Bot 检查目标路径、实际脚本与节点设置，不要求 Task 或 `manager_task_id`。缺失脚本、混合目标或不适用设置明确定位 Bot 或 node，不创建 Run 或 Chat。预检不运行脚本，也不判断业务请求或脚本依赖能否完成操作；普通节点的实际材料在派发时另行检查。

Map 型 Bot 通过 `map_file` 继续检查被调用 Map，节点自己的 `manager_task_id` 必须引用 manager Task。合法 A→B→C 及不同节点重复调用 B 均接受；A→A、A→B→A 按**当前调用路径**拒绝，错误 `map-call-cycle` 的 `callChain` 给出确切文件名链。调用关系检查不展开父图、不保存定义快照、不启动任何宿主。嵌套调用的执行与父子 Run 事实另见运行说明；预检通过本身不是执行成功证据。

拒绝保留发起页并逐项指出错误及来源 node／Bot／Task。用“修正 node”回到原 node 编辑器，用“修正 Bot”回到共享 Bot 配置，用“修正 Task”直接打开原 Task 编辑器；Task 已删除时明确显示不存在，不转成新增或默认 Task。zone 错误可回到 zone 登记入口，或回到原 node 修改 zone ID。原 Map 文件损坏时按错误修复配置目录中的文件。修正后返回 scheduler，显式重新校验；刷新启动选项可重读 Task 和 zone，不自动重试启动。

zone 引用按稳定 ID 解析本机绑定。同一 ID 绑定一个目录，不同 ID 可绑定同一目录；推荐 `namespace/name`，界面提示规范而不强制该格式。启动 Run 时冻结本次所需 ID 与路径，后续改绑或取消登记只影响新 Run，不改变已有执行或后续派发的位置。旧目录消失、变成文件或被重定向时拒绝执行，不跟随新绑定或目录联接。注销不删除目录，目录内容没有版本或跨 server 一致性保证。

HTTP 对应 `POST /api/scheduler/maps/{filename}/preflight`，请求接受 `managerTaskId`、`zone`、`userInput` 三个字符串及可选 `fileInputs: [{id,path}]`；未知字段、查询参数、运行名称及 node 覆盖均拒绝。合法返回 `workspaceName`，无 Run ID。配置拒绝为 HTTP 400、`code: run-preflight-rejected`，`issues` 含错误码、说明、配置目标及可得的 `nodeId`／`botId`／`taskId`。此只读检查不缓存有效性；后续实际启动必须再次调用同一整图预检，不能信任页面曾经校验通过。

### 定位已关联的 Run Chat

发起后的观察、**定位** 与 **Resume** 见[运行说明](workflow-runtime.md#发起与观察)。定位不会启动新 Chat，Resume 不会接管旧 Run。

## 目录与定义管理

进入 scheduler 读取服务所在电脑的目录，与当前 zone 无关。Map 与共享 Bot 属于定义数据，允许纳入 Git，UI 编辑会修改对应文件；node 内保存稳定 zone ID，由运行所在 server 解析成本机目录。旧 `.data` 定义在启动时一次性迁入，冲突拒绝且保留旧源，详见 [定义布局升级](usage.md#从旧定义布局升级)。配置可独立省略：

```toml
[scheduler]
map_directory = 'meta-data/workflow-maps'
bot_directory = 'meta-data/bots'
```

默认值如上；普通相对路径按**服务配置文件所在目录**解析，绝对路径也可用；支持 `${roots.name}/relative` 前缀引用本机根，详见 [本机根目录](usage.md#本机根目录与解析规则)。首次读取创建缺失目录及父目录，保留已有文件，不安装示例。只扫描直接包含的 JSON 文件（后缀不分大小写），不递归。配置改变需重启，文件内容改变可“刷新全部”。目录创建／读取失败与正常空态分开报告。

Map 以文件名标识，列表优先显示定义中的 `title`；空白或不可读时显示文件名，同名标题不合并。列表按文件修改时间倒序，同时间按文件名升序，刷新重新读取标题与顺序。

选中 Map 后，常驻图显示真实的 `blocked_by` 依赖，包括无连线的节点。B 依赖 A 就画 A → B；边不代表材料流、运行状态、Chat 在线状态或实际调度顺序。静态图可平移并用键盘选择节点，不可拖动改定义或连线。node 明细分开显示本次 `input`、关联目标与固定 Task.prompt、成果批准模式、zone 和命名交接规则。脚本型目标显示脚本路径，Task.prompt 不适用。缺失或不可读的引用给出原因，不沿用旧提示词或捏造配置。空 Map 可查看但不能启动。

从外侧 **bot** 选择共享定义，不从 node 明细打开第二份编辑器。可按名称或 ID 筛选。切换 `when`、`plan_guide`、`node_guide` 阅读指南，`json` 为只读原始定义。Task 栏读取当前名称，未绑定、引用不存在或读取失败时给出相应提示。回到 **scheduler** 时重新读取来源，保留仍存在的 node 选择。窄屏可用 **返回 Map** 回到节点清单，未保存修改仍须确认。

Bot 列表（含空态）可新增，预览可修改；当前 Map 可新增 node，选中 node 可修改。ID 独立输入，新增初始为空，修改时只读，JSON 不含顶层 `id`。node 编辑器的 `node_id` 表示 `nodes` 的 key，不在 node 内容中新增字段；Bot ID 由服务端补回。ID 不是文件路径。新草稿使用打包的 [天气 Bot](example/bots/weather-capability.json)／[上海 A](example/maps/trip-readiness.json)，部署不依赖开发仓库；打开、修改草稿或放弃不写定义，也不自动安装依赖。

Bot 的 `new`／`edit` 页面使用 `save`／`cancel`，node 编辑页按钮不变。Task／Map 选择与 JSON 中的 `task_id`／`map_file` 同步；脚本通过 JSON 的 `script` 字段手工绑定，页面只显示当前路径，不提供固定能力下拉框或脚本管理器。保存前均是草稿，取消沿用未保存确认；非法路径、错类型或冲突不覆盖原定义。Task 列表仍只提供普通 Task，失效引用保留并提示。

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
| `GET /api/scheduler/maps/{filename}/inputs` | 已声明的 `map_input_rule`／`map_output_rule` 与 revision |
| `POST /api/scheduler/maps/{filename}/inputs` | `{map_input_rule?,map_output_rule?,revision}` → 更新后 Map |
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

复验入口与安全边界见[测试指引](agents/testing.md)。用 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch map_rules::` 检查真实 HTTP 保存、公开 ID 与独立文件读回；用 `npm --prefix frontend run test:browser:build -- schedulerNodeEditing.spec.ts --workers=1` 检查页面编辑和重载。定义检查须区分成功保存的读回与失败或取消后原始字节不变。定义、页面及预检证据不证明真实 Chat 已启动或消费材料，也不证明业务脚本执行成功。
