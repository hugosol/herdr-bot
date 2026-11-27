# Bot 与 Map node 的完整数据结构和知识归属：Lighthouse

> Question and discussion: [Bot 与 Map node 的完整数据结构和知识归属](../decision/02-definition-schema.md)

## Question

Bot、Map 和 node 的新静态格式是什么，各类知识由谁维护，独立使用与组合调用如何保持完整？本票决定结构、语义与提示词职责，不实现系统、不创作最终业务提示词。以下是新设计，不是当前产品已实现说明。

## Discussion

### 1. Bot 是能力索引，不是执行知识的补丁

Bot 是附加的能力描述／索引，Task、script、Map 是实际实现。没有 Bot 不影响 Task／Map 被手动调用并成功工作；三类实现的 Bot 能力说明同构。Bot 的描述可以精简抽象，但不能因省略关键能力边界而误导选用。

概要规划与 chief 使用 Bot 发现、理解和选择能力；概要规划据此形成经确认的业务方案。转换者依照该方案，解析 Bot 的目标引用，读取 Task 自身工作设定及引用资料、script 实际代码与接口、Map 自身定义与输入输出规则。转换者不依赖 Bot 说明补充执行知识，不将 Bot 文案作为实际接口权威；Bot 与实现不一致时指出问题，涉及业务方案变化须返回确认。

Bot 能力说明不注入执行 Chat，也不是执行者必须读取的依据。chief 同样只以 Bot 为能力索引，具体调用约束由下层实现提供。目标引用解析仍保留：不读取能力说明，不意味着运行机制不再读取 Bot 的实现引用。

Task 不新增 user.input guide，也不新增 Task／script 统一输入输出声明；独立 Task 的用户输入保持自由表达。Bot input_rule/output_rule 是规划型 Agent 阅读的摘要，而非运行校验规则。实际实现自身必须完整，缺失知识应修复实现，不以 Bot 文案或 node.input 暗中补建稳定能力。

**对早期基线的明确修订：**

- 设计基石第 3.2、6 节的“经 Bot 调用时附加 Bot 契约”不再采用。Q12 用户确认不注入，并称其为沿设计得出的推论。
- 早期“Bot 输入契约完整覆盖全部业务信息”收窄为“实现完整，Bot 提供足以正确选用与组合的摘要”。Q33 后用户明确 Bot 是“大概的要求”，Q34 同意具体粒度，Q45 再次确认仅供规划型 Agent 判断。
- 不把全部说明合成 contract_rule，不引入独立契约实体，不为 Task 增加 guide，不要求独立 Map 经 Bot 启动。
- 不新增 prerequisite_rule、effect_rule、failure_rule 或 capability_rule；这些曾是候选，并未批准。

### 2. DRY 与说明职责

接口／索引和实现是不同维度，必要的对应不构成应消除的冗余。允许 Bot 概述实现能力，但不在两处独立维护详细业务标准。确有重复标准时按实际需要共享来源，不强制每个 Bot 新建契约文件。执行者读取实现所需资料，而非必须读取 Bot。

| 内容 | 维护入口 |
| --- | --- |
| 能力发现与适用性 | Bot.when |
| 能做什么、职责边界、影响规划的特殊特征 | Bot.description |
| 规划需要知道的输入种类、含义与关键前提 | Bot.input_rule |
| 规划可期待的成果种类、含义与关键限制 | Bot.output_rule |
| 具体执行方法及完整稳定工作设定 | Task／script／Map 实现 |
| 本次业务目标、范围、特殊限制 | node.input 或用户实际输入 |
| 文件来源、依赖、位置和成果策略 | Map/node 结构化配置及本次绑定 |
| 身份、报告、交接、通知等公共协议 | 系统公共机制与公共角色指引 |
| 如何从方案生成 node.input、如何填写接线 | 转换 Task 使用的公共编写指引 |

Bot 的 plan_guide/node_guide 有效信息逐项归位后移除，不整段改名搬家。发现旧指南包含实现必需但实现未承接的知识，须先归还实现，不能删除造成缺失。

### 3. 硬性命名规则

本轮设计以内容性质区分命名：约束／契约／指引使用 *_rule，实际输入输出不带 _rule。规则可以声明逻辑 ID，实际值绑定该 ID 与本次路径；不是按“是否出现 ID”区分。

Map 静态接口使用 map_input_rule/map_output_rule；map_input/map_output 留给本次 Run 的实际输入输出，不放进静态 Map。本票只定这层命名和语义，Run 的具体存储、API、旧记录处理由后续调用设计与切换交接决定。

description 是描述性内容，不是实际值或执行指令；保留 when、description 以及 node.input 等已确认名称，不进行无关全局改名。

### 4. Bot 完整字段及旧到新去向

| 字段 | 类型、缺省与职责 |
| --- | --- |
| id | 必需字符串；保留现有稳定身份及唯一性机制 |
| name | 必需字符串；保留显示名称 |
| group | 可选字符串或 null；缺省/null 为未分组，其他分组校验不变 |
| when | 必需字符串，可为空；发现与适用性说明 |
| description | 必需字符串，可为空；能力职责及影响规划的特征 |
| input_rule | 必需字符串，可为空；输入摘要，不是实际绑定或机器校验规则 |
| output_rule | 必需字符串，可为空；输出摘要，不是实际绑定或机器校验规则 |
| task_id | 可选字符串或 null；引用 normal Task |
| script | 可选字符串或 null；scripts 根下合法相对 .ts 路径 |
| map_file | 可选字符串或 null；确切 Map 文件名 |

三个非 null 目标互斥，也可均缺省，沿用“可保存但不可运行”的草稿机制。结构保存不等于目标存在、可运行或说明质量合格。未知字段拒绝、路径安全、现有目标检查沿用；不增加 type、嵌套实现对象、版本字段或旧字段兼容别名。

旧 id/name/group/目标引用不变；when 保留并按职责整理；plan_guide/node_guide 的有效摘要分配到 description/input_rule/output_rule 或 when，详细实现知识回到实现，公共规则回到公共指引；随后删除旧字段。

#### description 编写格式

固定顺序、按需出现的 Markdown：

1. 能力职责：完整 Bot 必写。
2. 能力边界：按需。
3. 用户参与：仅影响规划时填写。
4. 操作效果：仅调用者需要提前知道的外部效果。
5. 特殊前提：影响选用或安排且未由输入摘要覆盖的条件。

可选小节省略，不保留空标题，不写“无”凑格式。不重复 when 或输入输出，不描述内部步骤。模板由 Bot 创建／修订指引统一维护；后端不解析标题、不强制 Markdown schema。完整性是编写要求，不阻止字符串为空的中间草稿。

#### Bot 对象示例

以下是结构示例，不是获准落库的业务资产；引用 ID 仅作示意。

```json
{
  "id": "requirements-analysis",
  "name": "需求分析",
  "group": "analysis",
  "when": "需要澄清业务需求、范围与规则时选用。",
  "description": "## 能力职责\n分析业务需求并整理结论。\n\n## 能力边界\n不承担技术方案设计。\n\n## 用户参与\n业务取舍需要用户确认。",
  "input_rule": "业务目标、范围与必要参考材料。",
  "output_rule": "需求分析结论及明确记录的未决事项。",
  "task_id": "requirements-task"
}
```

script 型仅以 script 替代 task_id，Map 型仅以 map_file 替代 task_id；四个说明字段同构，不改变运行宿主。

### 5. Map 完整静态结构

| 原字段／新字段 | 决定 |
| --- | --- |
| title | 保留必需字符串 |
| description | 保留必需字符串，仅描述 Map 用途；不承载流程指令或专属业务验收入口，不自动注入执行上下文 |
| group | 保留现有可选分组与校验 |
| nodes | 保留必需对象；key 是图内 node ID |
| input_requirements | 移至 map_input_rule.description，删除旧顶层字段 |
| input_files | 移至 map_input_rule.files，删除旧顶层字段 |
| map_input_rule | 新输入规则对象，整体可省略 |
| map_output_rule | 新输出规则对象，整体可省略 |

Map 仍以文件名引用，不增加 Map ID、schema_version 或默认 Bot 绑定。其余现有图校验、引用、删除保护与保存边界不变；删除时关联 Run 历史的处理由后续已批准修订取代，见 [调用上下文与系统代码改造契约](03-call-context-design.md#7-删除与既有界面)。修订来源为该票 Q19：用户明确“不保留原来的删除run历史功能”，Q21 后确认发布；不得将此前“边界不变”解释为继续保留该联动功能。

两侧规则对象均支持 description 字符串和 files 数组；description 省略视为空字符串，files 省略视为空数组。总体说明只写跨文件或非文件要求，逐文件要求写在文件项，不重复维护。整个规则对象省略表示未声明相应约束。

文件项 id 必填、非空，在各自接口内唯一；输入与输出接口独立，不要求全局唯一。逐项 description 可省略，按现有输入声明惯例视为空字符串。输入项必须显式包含布尔 required；输出项不设 required，已声明输出均须交付。不得以 null 替代对象、字符串或数组；未知字段拒绝。

总体输出说明可以非空而 files 为空，例如交给用户一个继续工作的 Chat。说明供人或 Agent 阅读，不自动转换为强制业务校验；不是字段不可编辑。首版结构化公开输出仍是文件，不新增 Chat、任意 JSON 值等输出类型。

### 6. node 完整字段

| 字段 | 保留的格式／缺省 |
| --- | --- |
| title、bot_id、input | 必需字符串；input 允许为空 |
| zone | 固定 zone ID 字符串或动态对象；人工选址为 `{"type":"dynamic"}`，输入驱动为 `{"type":"dynamic","input_id":"本地输入 ID"}` |
| blocked_by | 必需字符串数组，同图前置依赖 |
| manager_task_id | 可选字符串或 null；Map 型调用的 manager 配置沿用现有要求 |
| input_rule | 可省略，缺省空数组；保留 file/node/run 三类来源 |
| output_rule | 可省略，缺省空数组；项为 {"id":"…"} |
| auto_approved | 布尔，缺省 true |
| auto_close、end_node | 布尔，缺省 false |

file 来源保留 id/type/path 和可选 zone_id；node 来源保留 id/type/node_id/output_id；run 来源保留 id/type/input_id。既有路径边界、ID 唯一性、引用有效性、依赖图与执行目标适配检查不变；不自动以材料引用补 blocked_by。

**2026-11-27 GitHub 迁移 review 的批准修订：** 用户同意通过现有 output→input 文件接线提供执行位置；动态 zone 可用 `input_id` 引用自身输入。后端从该 JSON 文件顶层 `zone_id` 取得已登记位置，拒绝人工覆盖；dispatch 与 retry 共用解析，历史执行位置不变。人工动态 zone 继续保留。具体格式和失败边界由[产品动态 zone 契约](../../../docs/scheduler.md#动态执行-zone)维护；选址关系从提示词迁到结构化配置，不新增 manager_rule。

node 不增加说明指南、Bot 契约副本或新的业务字段。Task、script、Map 的目标及策略限制沿用现状；Map 型调用补入本票明确的公开接口对接检查。

#### 完整 Map 对象示例

以下示例同时展示可选字段，不表示必须显式写出所有默认值或已经实际运行。

```json
{
  "title": "分析需求",
  "description": "组织需求分析并向调用者交付结论。",
  "group": "analysis",
  "map_input_rule": {
    "description": "说明本次业务目标、范围及操作边界。",
    "files": [
      {"id": "brief", "required": true, "description": "需求参考材料。"}
    ]
  },
  "map_output_rule": {
    "description": "交付本次需求分析结论。",
    "files": [
      {"id": "specification", "description": "需求说明及未决事项。"}
    ]
  },
  "nodes": {
    "analyze": {
      "title": "分析退款需求",
      "bot_id": "requirements-analysis",
      "manager_task_id": null,
      "input": "分析会员退款需求，重点明确已使用权益的退款边界，暂不展开技术方案。以 brief 为业务依据。",
      "zone": {"type": "dynamic"},
      "blocked_by": [],
      "input_rule": [
        {"id": "brief", "type": "run", "input_id": "brief"}
      ],
      "output_rule": [{"id": "analysis"}],
      "auto_approved": true,
      "auto_close": false,
      "end_node": false
    }
  }
}
```

示例中内部 analysis 与公开 specification 不必同名；管理者登记公开 ID 与本次实际文件路径，不在定义中配置 analysis→specification 映射。文件路径不在静态输出规则内。

### 7. node.input 的生成责任

保留现有“概要规划与用户确认→概要方案转换为业务 Map→整图提示词优化”分工，不重建规划体系。

转换者读取已确认方案与真实实现，按三类归位：实现已有稳定职责不重复；材料接线、依赖、执行位置与成果策略进入各自结构化字段；本次目标、范围、材料用途与特殊限制进入 node.input。

允许整理表达、消除重复、明确指代，不得新增业务目标、授权、交付承诺或改变范围。没有额外业务差异时 input 可以为空，尤其脚本只消费结构化材料时无需凑文字。检查“实现＋本次委托＋实际材料及适用运行上下文”是否完整，不要求 input 单独复制全部上下文；概要方案已有但执行者实际收不到的必要信息仍须落实。

缺少业务决定返回确认；缺少稳定实现能力报告并修复实现，不能通过 input 偷偷补建。Bot 能力摘要不是执行补漏通道。公共生成规则由转换 Task 的公共指引承接，最终提示词由关联提示词票联合确认。

本流程特有、由节点执行者使用的条件式业务指引及文档入口属于 node.input，例如交付本阶段成果前读取对应业务标准；Task 独立使用也必需的稳定知识仍由 Task／实现资料承接。指针只要求节点在适用条件满足时读取，不要求启动时展开全部文档。Map 管理者的职责见下节，不将专属业务指引放进 Map.description 或新增 manager_rule。

### 8. 输入准备与公开输出交接

输入继续允许先启动管理者、再澄清和准备。输入文件 required 表示确认就绪并派发业务节点前必须齐全，不是启动管理者 Chat 前必须齐全。独立／嵌套沿用同一准备机制与操作授权边界。

Map 可以不声明公开文件输出。调用方需要按稳定 ID 取得成果时，Map 必须声明相应输出；适用于父 node、chief 或用户独立调用。单纯嵌套不强制制造文件；只需要完成状态、执行操作或交给用户一个 Chat，不因此增加文件输出。

- Map 自身交齐 map_output_rule.files 中全部已声明成果。
- 父 node 的 output_rule 可只要求子 Map 公开输出的子集；要求未声明 ID 时拒绝，不靠 input 临时描述内部寻址补齐。
- 父输入 ID 与子 Map 输入 ID 同名对接；父接收的输出 ID 与子 Map 公开输出 ID 同名。下游仍可通过现有输入规则绑定为自己的本地 ID，不加跨层别名配置。
- 子 Run 保存全部公开成果；父 node 仅登记自身要求的部分，不自动扩大父输出集合。不检查子 Map 是否每项输出都有上层消费者。
- 子管理者在本次交付中登记公开 ID→实际路径；不配置内部 node ID 静态映射，不在 Map 定义写死文件路径或寻址算法。

Q20 最终选择动态登记，取代 Q16/Q20 早期静态导出映射候选。路径本身不作为稳定成果身份，以免固定地址引入重复运行覆盖、相对根和旧文件辨识问题。

### 9. 最小验收与结束节点

节点执行者按 Task／实现资料及本次 node.input 承担业务执行、成果检查和如实报告。Map 管理者只承担通用输入准备、编排和交接协议，依据执行状态、批准、报告和声明成果办理交接，不加载每张 Map 专属业务逻辑，不重复承担节点业务验收。系统检查必交输出登记、路径合法可读等机械条件；管理者仍须显式完成 Map，但这不是其独立验证全部业务质量的证明。Chat 停止、进程退出或暂时没有工作不等于成功；不新增自动重试、回滚或复杂验收状态机。

**Traverse 后续 grilling 的批准修订：** 原“LLM 管理者判断业务质量”及由管理者读取专属交接标准的安排被上述分工取代，不删除业务标准，而是归还节点执行侧。确认来源为本轮用户明确“这里很明显是约束节点执行者，map管理者不需要了解每个map特定的业务逻辑，更不应该从后端注入某个map特定的提示词”，并在归纳上述修订后回复“可以，请更新”。讨论中的 manager_rule、自动注入 Map description 和给管理者额外供给业务文档入口均已撤回，不是新字段或接入要求。

调用者可以继续或按业务需要检查，不强制新增独立验收流程；不取消现有父节点交接与显式完成边界。结果交齐不等于业务质量由代码证明。

结束节点成功发起最后一个 Chat 即完成“交给用户继续”的职责，Map 仍由管理者显式结束，不自动结束。已声明文件须在 Map 结束前登记齐全；后续用户对话不再保证向上层交付新成果。需要等待成果的工作应在普通节点完成，不依赖结束节点未来可能产生的文件。

### 10. 确认来源、证据与限制

确认来源：2026-11-26 至 2026-11-27 本票 grilling 会话。Q12/Q13/Q14/Q15 确认索引、实现独立与 DRY；Q18–Q27 确认输出可选声明、必交、动态路径、子集及同名对接；命名讨论中用户要求“作为命名的一个硬性规则”；Q33 后用户进一步明确 Bot 仅供概要设计，Q34/Q45 确认摘要粒度；Q36–Q46 确认字段、草稿、输入准备、实际委托生成、Map 描述与结束节点边界。Q47 后用户确认“同意，我们的认知已经达成共识”，授权整理发布。

静态证据：backend/src/scheduler/bots/mod.rs、scheduler/maps/mod.rs、scheduler/mod.rs、runs/startup.rs；docs/scheduler.md、docs/planning-deliverables.md、docs/adr/0015-node-role-from-execution-context.md、docs/adr/0019-rust-script-bots.md；meta-data/workflow-maps/plan_workflow.json、create-bot-task-workflow--bc-61ce48b80a4c.json 及 Bot 指南。未运行服务或流程，未证明动态执行效果；示例不是迁移资产。

现有“开源项目仓库探索”的 node_guide 声称 Task.prompt 为空、要求调用者填入稳定只读边界，这是迁移调查线索而非本票已核实 Task 的结论。后续必须核实真实实现，修复归属，不能只删指南。

当前 GLOSSARY.md、产品说明、提示词和 schema 描述旧实现；本票不把设计冒充已实施产品、不原地改写外部设计基石。后续按本 authority 统一更新消费者；尤其规划交付文档当前要求概要阶段核实具体目标配置，与新阶段分工需重新对齐。

## Decision

Bot 收敛为面向规划／chief 的四字段能力索引，实际实现独立完整；Map 归整静态输入输出规则并由管理者登记公开成果，node 结构不变、业务委托承接本次差异及节点专属读取条件；管理者只承担通用编排交接，不加载 Map 专属业务逻辑。以上结构和职责已经确认，生产实现、运行接口、最终提示词及数据迁移仍由后续票据决定和交付。

## User stories

1. 作为概要规划者，我希望从 Bot 获得精简且不失真的能力摘要，以便选择与组合能力而不提前理解内部实现。
2. 作为 Map 转换者，我希望依据已确认方案和真实实现生成具体委托与接线，以便不依赖 Bot 重复维护详细执行约束。
3. 作为用户，我希望 Task／Map 独立使用完整，以便不经过 Bot 也能完成工作。
4. 作为嵌套流程调用者，我希望按公开成果 ID 接收所需子集，以便不依赖子图内部节点或固定文件位置。
5. 作为节点执行者，我希望在自身工作设定和本次委托中取得适用业务标准，以便完成成果检查；管理者按通用协议交接，不重复实现每张 Map 的业务验收。
6. 作为维护者，我希望区分规则声明与实际值、接口摘要与实现，以便减少重复维护且不丢失必要执行知识。

## Preconditions

- 实现自身必须承接完整稳定职责，概要方案必须承载本次业务决定；不能假设所有存量定义已经满足。
- 保留现有 normal Task 独立使用、执行角色与调用关联分离、script 可信宿主及文件路径安全机制。
- [流程迁移清单与共享影响范围](01-flow-inventory.md)的共享原件、用户选择和实际验证规则继续适用。
- 调用设计需覆盖新字段的存储、API、UI、工具、实际结果登记与完成检查；设计已确认不等于代码已具备。

## Postconditions

- [调用上下文与系统代码改造契约](../decision/03-call-context-design.md)以本票为静态结构与分层权威，决定具体 Run/API/跨仓消费者设计；不得再默认注入 Bot 契约。
- [关联提示词的联合设计与确认](../decision/04-coherent-prompts.md)共同完成创建、修订、概要规划、转换与执行资料的正文和必要领域文档对齐，不由本票示例代替。
- [可复用迁移方法与转换规则](../decision/05-reusable-migration.md)承接逐项信息归位和字段迁移；[单条流程全要素初步转换试验](../decision/06-conversion-trial.md)验证完整定义样例与引用。
- [选择性迁移的系统切换与开发交接门槛](../decision/07-cutover-handoff.md)决定旧 Run/Chat、活动执行和新版切换的兼容性边界，不据静态定义推造历史。

## Invariants

- Task 是独立完整的会话预设；节点角色由执行上下文决定，遵循 ADR 0015；不新增 node Task 类型。
- Task、script、Map 三类实现保持各自执行方式，script 继续使用 ADR 0019 的可信 TypeScript／Bun 模式，不为统一索引重建宿主。
- Bot 描述不注入执行 Chat；实现引用仍保留；Bot 说明不是运行权限或业务验收强制规则。
- node.input 是实际业务委托，可含本流程特有的条件式业务指引，不复制 Task 稳定规则；input_rule/output_rule 是接线声明，延续 scheduler 的来源与依赖分离。
- Map.description 只描述用途；专属业务标准由节点执行侧读取并落实，不新增 manager_rule，不通过后端为管理者注入特定 Map 的业务提示词。
- Map 结束、父层接受交接与用户业务验收不混为一体；结束节点后续对话不承担本次上层必交成果。
- 不执行生产重构、数据迁移或 /to-tickets；不声称已通过运行验证。
