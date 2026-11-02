# herdr bot 交付物与上下文接口改造设计

保留 agent 判断与 manager 显式推进的最小方案

设计提案 2026 年 11 月 1 日

建议把交付物的名称、格式、路径绑定和交接依据从各节点提示词中提取出来，交给 Map、Run 记录和受控工具承载。manager 继续理解任务、判断结果、显式 complete 和 dispatch；后端只约束身份、引用、格式和合法操作，不代替 agent 决定业务流程。

本文用于确定下一步改造边界，并给出 dsh superagent 中可参考的代码位置。它不是实施补丁；数据字段、工具名称、绑定与版本策略均为方案示例，不能视为已获用户确认的最终 API。

## 1 保留分工 收紧交付协议

- **不改流程主导权** 保留 node → Bot → 普通 Task → 独立 Chat，以及 manager Task。上报不等于完成，节点何时完成和下游何时启动仍由 manager 调用工具决定。

- **不把提示词变成协议解释器** node.input 只写该节点的业务目标、业务限制和质量要求，不再重复路径推导、日志解析、execution 选择和上报字段约定。

| **问题**                   | **改造后的责任**                                            |
|----------------------------|-------------------------------------------------------------|
| 真实路径散落在节点指令中   | 用户在发起 Run 时给路径；manager 解释，工具校验并保存绑定。 |
| 交付格式靠自然语言反复约定 | Map 声明命名输入输出、基本格式和上下游关系。                |
| 下游需猜哪次报告与文件有效 | manager 验收具体交付物引用；下游接收已验收的固定引用。      |
| 通知和结果正文容易混在一起 | 短通知只提示状态与引用；真实内容由受控读取接口按需读取。    |

对照基线：herdr bot 80887d2f31249c737a3874b9041209d88914e155；my agent toolkit 3efcd77ec939cabeeeb8552f7958c17fd28aaa6a；dsh superagent 6337fc823ce20251c8de8085948165c81aaa701e。

## 2 最小模型与责任边界

不引入新的顶层任务系统。沿用现有 Run、node、execution、Chat 身份，在现有模型旁增加少量交付契约与运行事实。Map 描述“要什么”，Run 绑定“这次在哪里”，提交记录说明“哪次执行交了什么”，验收记录说明“manager 接受了哪个版本”。

| **承载位置**     | **保存什么**                                                                       | **不负责什么**                               |
|------------------|------------------------------------------------------------------------------------|----------------------------------------------|
| Map 契约         | 命名 input／output；text、Markdown 必需标题或 JSON 必需字段；生产者与消费者关系。  | 不写本次绝对路径，不携带本次报告。           |
| Run 绑定         | 用户发起输入、解释后的路径或输出根目录、归一化结果、允许的读写范围与覆盖授权。     | 不让每个节点重新解析用户的原始输入。         |
| 交付物提交记录   | 提交 ID、实际 Run／node／execution、逻辑名称、真实路径、格式检查结果和内容指纹等。 | 不把“已提交”直接等同于“已完成”。             |
| manager 验收记录 | 接受的提交 ID、语义判断和下游引用；需要时关联人类确认信息。                        | 不以节点自述、文件名或“latest”代替验收对象。 |

### 路径和身份来自不同来源

路径由用户输入决定，manager 可将“放到某目录”解释为具体输出绑定；信息不全或有歧义时询问用户。后端检查路径并落盘，一旦绑定成功，节点读取规范化结果。manager 的推断不是任意目录的写入或覆盖授权。

身份则不能由 agent 自报。工具调用沿用 toolkit 自动携带的实际 Session 身份，由后端解析其对应的 Run、node、execution。查询、提交、验收和读取都校验作用域；跨 Run 写入或过期 execution 的新提交应失败，授权范围内的历史记录仍可只读查询。工具级作用域检查不等于操作系统文件沙箱。

### 固定引用不是完整版本系统

建议提交记录追加保存，不覆盖历史记录。被接受的引用固定到一条提交及其 execution，而不是动态的“当前最新报告”。这解决交接对象漂移，但不意味着原路径中的字节不可变，也不等于 Map 或 Task 的完整版本管理。内容稳定性须另选快照或哈希校验策略，见第 7 节。

## 3 工具承担确定性协议 manager 保留判断

“注册工具”是让协议可以被发现、验证和执行，而不是把每一种操作拆成独立工具。可扩展现有 run、collect、report，再提供统一 read 资源入口；以下名称仅表示职责。

| **职责示例**   | **请求与返回要点**                                               | **必须守住的边界**                                   |
|----------------|------------------------------------------------------------------|------------------------------------------------------|
| 绑定本次路径   | manager 提交逻辑名称与用户授权路径；返回规范化绑定与问题。       | 缺少路径或越界时不默认猜测，不顺便覆盖已有文件。     |
| 读取节点上下文 | 节点查询命名输入、输出契约、已绑定路径、可读提交引用。           | 只暴露实际 Session 允许的上下文；正文按需分页。      |
| 提交交付物     | 节点给出逻辑输出名、已生成文件及短摘要；返回提交 ID 与格式结果。 | 后端自动附加身份，验证真实文件；提交不触发完成。     |
| 接受交付物     | manager 读取实际内容后，接受或拒绝指定提交，并记录理由。         | 不能接受其他 Run、无效执行或尚未通过规定检查的记录。 |
| 推进现有流程   | manager 显式 complete，再按需要 dispatch 下游。                  | 仅检查必要前置条件，不自动选择业务分支。             |

### 提交 格式通过 manager 接受是三件事

节点提交 → 后端记录并执行格式检查 → manager 读取真实内容并判断 → manager 接受具体引用 → manager 显式完成节点 → manager 为下游指定该引用并发起执行。任何一步失败都停留在相应事实，不伪装成后续状态已达成。

格式检查只回答“是否存在、是否能读取、是否满足基本结构”。语义验收回答“内容是否达成目标、关键决定是否正确、是否得到所需的人类确认”。节点说“已确认”或 DSH 的 human_action 问题字段，都不是人类批准的证据。

### 可靠通知是旁路 交付记录才是依据

提交事实应先可靠保存，通知携带短摘要、提交 ID 和读取引用。通知失败不抹掉交付物，也不能令 manager 误以为已经收到。manager 可通过 collect／read 找回待处理记录。后续再设计持久通知队列和重试；在 Herdr.prompt 的去重能力确认前，不应直接无限重放，更不能声称端到端 exactly once。

## 4 先跑通 plan workflow 的一条纵向链路

第一阶段只改 outline 与 generate_map：outline 按 manual 策略产出已确认大纲，generate_map 按 auto 策略消费选定大纲并生成 Map。业务流程不变，只把原来反复写在 node.input 里的路径和交接规则移入可查询的契约与运行记录。

### Map 只定义逻辑交付

| **节点**     | **命名输入与输出**                                                             | **基本格式**                                                                            |
|--------------|--------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------|
| outline      | 输出 confirmed_outline                                                         | Markdown；必需标题为“目标”“节点”“依赖”“待定问题”。                                      |
| generate_map | 输入 outline，来源为 outline.confirmed_outline 的已接受提交；输出 workflow_map | JSON；复用现有 Map 校验，顶层为 title、description、nodes；依赖保存在 node.blocked_by。 |

交付名称和 Markdown 标题仅为示例。Map 文件沿用现有结构，节点字段包括 bot_id、input、zone、blocked_by、report_policy，而不是另加 edges。逻辑关系属于 Map；本次接收哪个提交 ID 属于 Run。

### 启动时绑定真实路径

用户发起：“这次的交付物放到 /work/design/demo，先给我确认大纲，再生成 Map。”manager 解释输出根目录，并建议 confirmed_outline → outline.md、workflow_map → map.json。工具检查目录、名称冲突、访问范围和覆盖意图；必要时先向用户澄清，成功后把完整路径保存到本次 Run。

### 执行与交接

- **outline** 查询自己的输出契约和绑定路径，生成大纲。节点 input 只写业务目标，例如“明确流程边界和异常分支，讨论待定事项”。

- **人工确认** 保留现有 manual 语义：outline 在自己的节点 Chat 展示大纲并停轮等待用户确认，确认后再报告成果。人工交互不迁到 manager Chat。

- **manager 验收** manager 读取真实大纲并判断是否满足任务目标，接受具体提交，再显式 complete。首版不把新审批凭证机制设为前置条件，也不声称 manual 已有后端强制门禁。

- **generate map** manager dispatch 时绑定 outline 输入到被接受的具体引用。节点只读取该引用，不搜索 messages.jsonl，也不自行选择最新 report 或其他 execution 的同名文件。

若 outline 重试产生新 execution，旧提交仍可审计，但不能自动顶替新执行的产物。若需要复用旧产物，由 manager 发起显式复用并再次校验和验收；不要把“选择旧 report”隐藏在节点提示词中。

## 5 校验失败与边界情况必须可见

确定性检查集中在后端与工具边界，避免各节点各写一套约定。失败结果应包括稳定的原因类别、相关逻辑名、当前绑定和可恢复动作，但不要把恢复决定全部硬编码成自动调度。

| **场景**               | **建议行为**                                                                                               |
|------------------------|------------------------------------------------------------------------------------------------------------|
| 启动未给路径或含糊不清 | 保持未绑定状态；manager 询问必要信息。不得把工作目录、临时目录或模型猜测路径当成用户决定。                 |
| 两个输出落在同一路径   | 绑定阶段识别冲突；要求改名，或显式声明共享／覆盖规则。不得后写者静默覆盖前者。                             |
| 路径穿越与符号链接     | 规范化路径，检查真实解析位置与允许根目录；写入和读取时再次检查，避免绑定后链接变化绕过校验。               |
| 已有文件与覆盖         | 区分读输入和写输出；写入已有文件前检查针对该目标的覆盖授权。默认拒绝静默覆盖；有授权也保留原提交审计。     |
| 文件不存在或格式失败   | 记录提交失败或格式失败及原因；不允许把摘要当正文验收，不推进下游。格式修复形成新提交。                     |
| 被接受后文件改变或删除 | 快照模式读取验收时快照；哈希模式检测变化并阻止继续使用。不能只因引用 ID 未变就认为内容未变。               |
| 重试与旧 execution     | 新 execution 拥有自己的提交；旧报告不能靠“最新时间”混入。重启后的旧 Run 执行沿用现有只读边界。             |
| 人工贡献文件           | 用户／UI 可将文件显式登记到指定 Run 和逻辑交付名，记录贡献者及来源；走同样校验与验收，不伪造节点 Session。 |
| 通知失败或结果不确定   | 交付记录可查询，通知状态单列；确认去重和送达证据后才重试，manager 的处理也按提交 ID 防止重复验收。         |

尤其要防止“只检查路径字符串”。真实文件可能在校验后被替换，符号链接也可能改变。若实施层不能原子地保证某项读写约束，就把剩余风险写清楚并在使用时复核，而不是在接口描述里承诺不可实现的安全保证。

## 6 分阶段落地与验收场景

### 第一阶段 只验证交付闭环

扩展 Map 的命名输入输出与基本格式，新增 Run 路径绑定和提交／验收事实；让 plan_workflow 的两节点走通查询、提交、读取、接受和显式推进。保留现有存储与 Task／Chat 路由。第一阶段的价值不是工具数量，而是删除节点中的交付协议段落后流程仍能正确运行。

### 第二阶段 收敛上下文接口

为 run、node、execution、report、artifact 提供稳定且有作用域的读取资源。collect 继续提供运行快照和待处理报告索引，正文按引用读取；不要求 agent 解析 history.jsonl 或 messages.jsonl。旧日志继续服务审计和排障，不承担节点间正式交接协议。

### 第三阶段 再增强通知恢复能力

参考 DSH 将事实保存与待发送通知登记协调提交，再由持久队列重试。但先确认 Herdr 接收端的去重、返回语义、重启恢复和“不确定是否送达”行为。可使用现有存储，不把迁移 SQLite 设为前置要求。

| **验收操作**                              | **应观察到的结果**                                                         |
|-------------------------------------------|----------------------------------------------------------------------------|
| 删除两节点 input 中路径解析和报告选择段落 | 仅保留业务描述，仍能按绑定和契约完成交付。                                 |
| 另一个 Run 或旧 execution 调用提交／接受  | 工具拒绝；不改变当前 Run 的接受引用。                                      |
| 大纲缺必需标题，或文件根本不存在          | 格式失败；manager 可看到原因，下游不启动。                                 |
| 格式合法但大纲内容不满足业务目标          | manager 拒绝或要求修改，不能因格式通过自动完成。                           |
| 接受后修改或删除原文件                    | 快照仍可读取，或哈希检查明确阻止消费；不会静默读到新内容。                 |
| 重试、重复提交与重复通知                  | 可区分 execution 与提交；重复通知不会造成重复接受或完成。                  |
| manual 节点尚未获得用户确认               | outline 按既有提示词约束在自身 Chat 等待；不把格式通过当成确认或自动完成。 |
| 人工补充大纲后继续流程                    | 贡献来源明确，经过相同校验和 manager 验收，下游得到同一种引用。            |

实施落点可先集中在 backend/src/runs 下的 startup、reports、collect、completion，并同步 toolkit 的 manager-chat、node-chat 工具入口与 shared-logic。files.rs 适配文件相关约束；scheduler 处理 Map／Bot 静态定义与预检，流程推进仍由 manager 与 runs 承担。

## 7 实施前需要定下的少数选择

### 交付物内容如何保持一致

优先讨论两种做法。快照方案在提交或接受时复制到受控位置，下游读取固定快照，复现性好但需存储、清理和访问控制。哈希方案保留用户路径并记录内容指纹，每次消费前核对，成本较小但原文件改变或消失会阻断流程。两者都需要定义检查时点以及读写竞争行为。当前尚未选定，不应声称“固定引用”已经使字节不可变。

### 绑定何时冻结 改绑如何记录

建议首次 dispatch 前完成必要绑定；执行中改绑必须是 manager 的显式动作，保留旧绑定与受影响提交，不能重解释已有引用。需要决定只允许未执行节点改绑，还是允许生成新的绑定修订。该选择与完整的 Map／Task 版本管理不同，不应借机扩大到全系统配置版本化。

### 是否需要结构化的人类确认依据

首版保留 manual report_policy 的提示词语义：节点在自己的 Chat 等待确认，manager 另做交付物语义验收。结构化记录确认依据是可选增强，并非本次必做。若未来要求后端强制用户确认，须另定可信消息或 UI 来源、授权对象与内容版本的关联；不能把 agent 自述当批准，也不应要求每次人工改文件都机械重走审批。

### 复用什么接口 如何兼容旧 Map

优先复用现有工具和对象，新增职责可以是 run 子操作、read 资源或 report 的交付物字段。旧 Map 没有契约时，是否维持旧行为并显示“无结构化交付保证”，或在指定新流程中强制契约，需要明确迁移规则。示例 API 名、JSON 字段和状态名均须在实施时评审。

### 本次明确不做什么

- **不更换任务模型** 不以 Ticket 替换 Run／node／execution，不引入自动推进调度器，不把 DSH 的 ready 自动结束照搬到 herdr bot。

- **不扩张基础设施** 不强制迁移 SQLite，不开展 IRC 接入或全渠道 rollout，不重建现有 Task 和 Chat 生命周期。

- **不重新打开无关契约** 当前 workflow contract 不覆盖 Delivery／artifact binding，需要为本次交付物能力做窄范围契约补充；不因此全面重开 ADR 0015 的配置版本化议题。

建议下一步先定内容稳定策略和绑定改动规则，再为 plan_workflow 写一个可实施的窄变更说明。完成第 6 节场景后，再判断是否推广到其他 Map。

## 8 代码参考索引 输入与交付接口

所有链接固定到 dsh superagent 提交 6337fc823ce20251c8de8085948165c81aaa701e，便于后续对照。该项目没有实现本文的 Map／Run 交付物格式与绑定注册表；可参考的是抽象方法和边界，而不是复制一个现成功能。

**01 结构化输入而非拼接提示词**

[src/core/model.ts L235–L257 TicketInput](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/core/model.ts#L235-L257)

借鉴输入事件有明确种类、来源和读取信息的表达方法。这里的 Input 是触发器、文本等上下文，不是 Map 的命名文件输入，也不是交付格式注册表。

**02 业务对象和执行对象分开**

[src/core/model.ts L277–L315 Ticket 与 Execution](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/core/model.ts#L277-L315)

借鉴任务、执行和输入各有明确字段的组织方式。herdr bot 已有 Run、node、execution、Chat 分层，无需用 Ticket 替换；此处未提供本文建议的路径绑定方案。

**03 可发现的上下文资源目录**

[src/app/resources.ts L23–L55 RESOURCES](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/resources.ts#L23-L55)

借鉴资源名称、所含信息和 worker 可读范围的统一目录。可用于 run／node／artifact 查询；不是要求沿用 DSH 的全部资源类型。

**04 分页与作用域校验**

[src/app/resources.ts L82–L110 main 及 list 及 readable](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/resources.ts#L82-L110)

[src/app/resources.ts L235–L241 ticket 路由作用域检查](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/resources.ts#L235-L241)

借鉴分页、来源授权及 worker 只能读取自身 ticket 的检查。迁移时以真实 Session 映射到现有身份；这些是应用层权限，不是操作系统沙箱。

**05 统一读取工具的参数契约**

[src/app/tools/tool.ts L19–L34 readTool](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/tools/tool.ts#L19-L34)

借鉴严格 schema、resource／offset／limit 和可发现资源语法。可让节点按需取正文，不必把全量日志、全部交付物都塞入启动提示词。

**06 result 目前只是报告摘要**

[src/app/resources.ts L276–L282 report 与 result 路由](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/resources.ts#L276-L282)

这里按 assignment 读取 report.summary，result 则取 latestReport 的 summary。不能将它描述为版本化文件交付系统；herdr 的接受引用需自己建模，避免误用 latest。

**07 结构化报告与保存后通知**

[src/app/tools/executor.ts L75–L101 orchestrator_report](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/tools/executor.ts#L75-L101)

借鉴 state、summary 等字段及 report 与 relay 的原子登记思路。human_action 表达待询问的问题，不证明批准；不要连带复制报告后自动结束的生命周期。

## 9 代码参考索引 通知与职责边界

**08 短通知指向可读正文**

[src/app/notices.ts L107–L128 resultRelay 与 reportRelay](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/notices.ts#L107-L128)

超过 600 字符的摘要被截短并附 read；普通 progress 无回答时保持内部。借鉴减少上下文噪声，但下游交付应指向已接受提交，而非可变的 latest result。

**09 通知进入持久副作用队列**

[src/app/sessions.ts L99–L105 Sessions relay handler](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/sessions.ts#L99-L105)

[src/app/sessions.ts L156–L173 prompt 与 relay](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/sessions.ts#L156-L173)

借鉴稳定 requestId 与有序 relay 的边界。可靠性依赖 Effects 和 AgentDelivery 配合；不能只给 Herdr.prompt 加循环就宣称同等保证。

**10 副作用可恢复与退避重试**

[src/app/effects.ts L60–L71 enqueue 与 handle](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/effects.ts#L60-L71)

[src/app/effects.ts L130–L145 attempt](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/effects.ts#L130-L145)

借鉴队列记录、失败状态、退避以及 seq／generation 防止覆盖新记录。实现细节基于其存储，可移植语义，不要求 herdr 为此迁移数据库。

**11 接收侧去重是重试前提**

[src/app/delivery.ts L14–L29 messageId 与 AgentDelivery](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/delivery.ts#L14-L29)

[src/app/delivery.ts L40–L84 history 与 send](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/app/delivery.ts#L40-L84)

DSH 用稳定消息 ID、持久历史和 pending inbox 判断重复，并在确认前 flush。先验证 Herdr 对应能力；不据此承诺跨系统 exactly once。

**12 明确不要照搬自动结束**

[src/core/assignments.ts L117–L133 Assignments report](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/core/assignments.ts#L117-L133)

ready 在无 followed Threads 时可形成 completed 并结束 ticket。herdr bot 要保留 manager 的显式 complete，这段是差异边界而不是待移植功能。

**13 领域变更集中在清晰入口**

[src/core/engine.ts L19–L38 Engine](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/core/engine.ts#L19-L38)

借鉴领域写入在事务入口组织、调用方接收角色限定接口的方式。这里主要提供边界思路，不表示 herdr 缺少已有的运行状态结构。

**14 语义判断与外部执行隔离**

[src/core/ports.ts L48–L83 Assessor 与 Executor 与 Outbox](https://github.com/deepseek-ai/dsh-superagent-prototype/blob/6337fc823ce20251c8de8085948165c81aaa701e/src/core/ports.ts#L48-L83)

借鉴判断请求、执行排队和消息发送作为独立端口。保留 agent 判断，后端守住可验证的状态与副作用边界，不变成大量业务 if else。

herdr 侧的对照落点：backend/src/runs/startup.rs 229–253 将 Task prompt、node.input、report_policy 与三类记录路径组合为节点上下文；reports、collect、completion 分别承载报告、快照与完成职责。toolkit 的 extensions/workflow-chat 下 manager-chat、node-chat 的 shared-logic/tools/index.ts、implementation.ts，以及 common/requests.ts 是工具注册、执行与自动 Session 身份传递的对应层。

保留原有分层，先把交付物协议从提示词中提取出来，再逐步增强查询与通知。这是本方案与 DSH 对照后的核心取舍。
