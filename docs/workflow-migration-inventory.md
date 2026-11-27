# Map 迁移清单

来源：磁盘 [`herdr-bot.toml`](../herdr-bot.toml) → 其 `paths_file = paths.local.toml` → `roots.project` → `[scheduler].map_directory = ${roots.project}/meta-data/workflow-maps`、`bot_directory = ${roots.project}/meta-data/bots`。[Task 存储](../meta-data/tasks.json)、[选项目录](../meta-data/task-options.toml)、[Suite](../meta-data/suites.json) 和 [script](../meta-data/scripts/) 沿同一配置链调查。原本机根指向项目主 checkout；本轮核对其发布 worktree 的配置与定义，未读取运行服务、生产 Run、用户 Chat 或认证，不推断其他部署也使用同一根。现场须用实际启动配置重新核实，并检查旧布局启动导入源。

状态只用 **未开始／迁移完成／废弃**。仅在必要定义完整转换、待补解决且取得用户明确确认后填写迁移完成，记录确认来源；用户明确不再迁移才填写废弃，废弃不授权删除。未选择、部分转换或未经用户确认仍为未开始。检查结果、待补和实际业务运行另记，不作状态，不作程序输入。以下关联和产物是已调查的静态定义，不是未选流程的验收要求。对其运行事实均未核验；从现场证据回填，不推断曾否运行。

| 实际 Map / 用途 | 状态 | 入口、节点/实现及资料 | 定义中的产物与仍待核实 |
|---|---|---|---|
| [`plan_workflow.json`](../meta-data/workflow-maps/plan_workflow.json) · 创建map | 未开始 | 入口 Bot `bot-map-plan-workflow`；`outline-planning` → 概要规划，`plan-to-map` → 概要方案转 Map，`bot-task-18e65aed740825f8-0` → prompt优化者——Map维度。用户另选根 manager 与 zone；节点固定 `hugosol/herdr-bot`。资料见下节。 | 内部 `outline` → `map` → 最终 `map` 与 `handoff`；追加公开 `created_map` 身份文件。最终业务定义留在 scheduler，不启动它。仓库资产已落地，尚无用户迁移完成确认；实际部署 zone、词典、安装、角色协议与真实业务验收待核。 |
| [`create-bot-task-workflow--bc-61ce48b80a4c.json`](../meta-data/workflow-maps/create-bot-task-workflow--bc-61ce48b80a4c.json) · 创建task | 未开始 | `bot-map-create-bot-task-workflow--bc-61ce48b80a4c`；`create-task` → Task 创建者、`optimize-prompt` → 通用提示词优化者、`create-bot-wrapper` → 创建bot 子 Map（`manager` Task）、`bot-task-18e66273ae718d14-1` → prompt优化者——Bot维度；[包装](create-bot-wrapper.md)、[业务材料](business-materials.md)。 | 声明内部 `handoff`、`saved`，核实子 Map 公开接口与外层交接；未选，不迁入现场。 |
| [`create-bot-wrapper.json`](../meta-data/workflow-maps/create-bot-wrapper.json) · 创建bot | 未开始 | `create-bot-wrapper` 同时是入口与创建task的子 Map；`create-blank-bot` → `create-blank-bot.ts`、`save-created-bot-guides` → `save-bot-guides.ts`；`write-bot-when`、`write-bot-plan-guide`、`write-bot-node-guide`、`write-bot-output-rule` 各指四字段编写 Task，`optimize-prompt` → 通用提示词优化者；[V1 业务材料](business-materials.md)、[包装](create-bot-wrapper.md)。 | 空白 `baseline`，四路 `text`、`handoff`，保存 `saved`、`handoff`；运行、子层材料与现场确认待核，未选。 |
| [`create-bot-v2.json`](../meta-data/workflow-maps/create-bot-v2.json) · 创建bot-V2 | 未开始 | `bot-map-create-bot-v2` 同时是入口与创建task-V2／创建task-script type的子 Map；`align-bot-requirements-v2` → Bot 需求对齐者、`bot-task-18e89651aa593290-1` → Bot 整体分层设计与创建者；[V2 材料](../meta-data/workflow-maps/create-bot-v2/materials.md)、[业务材料](business-materials.md)。 | 已选并迁入新定义：request 同时供给两节点，W1 交付 bot_requirements，W2 公开交付 created_bot。分层正文与材料契约已审定；尚待用户迁移完成确认和真实业务运行。 |
| [`create-task-v2.json`](../meta-data/workflow-maps/create-task-v2.json) · 创建task-V2 | 未开始 | `bot-map-create-task-v2`；`create-task` → Task 创建者，`bot-map-create-bot-v2` → 创建bot-V2（`manager` Task）；[本流程材料](../meta-data/workflow-maps/create-task-v2/materials.md)、[V2 材料](../meta-data/workflow-maps/create-bot-v2/materials.md)。 | 已选并迁入：W1 保存 Task 并交付 created_task 与定位 request，W2 调用子 Map 交付 created_bot；公开两个身份文件。通用嵌套输入逐层下传；待用户迁移完成确认和真实业务运行。 |
| [`create-task-script-type.json`](../meta-data/workflow-maps/create-task-script-type.json) · 创建task-script type | 未开始 | 无外层 Bot；`create-typescript-script` → 同名 Task（创建脚本并交接，不是此节点的 script 目标），`bot-map-create-bot-v2` → 创建bot-V2（`manager` Task）；[V2 材料](../meta-data/workflow-maps/create-bot-v2/materials.md)、[可信脚本规则](adr/0019-rust-script-bots.md)。 | 已选并迁入：脚本节点交付 created_script 与内部 request，子图交付 created_bot；公开两份身份文件。Task 仅提供产品脚本契约入口，保留 grill-with-docs；待用户迁移完成确认和真实业务运行。 |
| [`enhance-existing-bot--bc-301b25d9f8d6.json`](../meta-data/workflow-maps/enhance-existing-bot--bc-301b25d9f8d6.json) · 独立修订既有 Bot 四字段摘要 | 未开始 | `revise-bot-guides`；`prepare-bot-revision` → 既有 Bot 修订基线核实者；复用包装图的 `write-bot-when`、`write-bot-plan-guide`、`write-bot-node-guide`、`write-bot-output-rule`；`replace-text` → 通用文本替换者；[修订](revise-bot-guides.md)、[业务材料](business-materials.md)、[定义保存](scheduler.md)。 | `baseline`、四份 `text`、`saved`、`handoff`；未选，实际目标／revision／授权与运行待核。 |
| [`github-repository-exploration.json`](../meta-data/workflow-maps/github-repository-exploration.json) · GitHub 仓库拉取与探索 | 未开始 | 无外层 Bot；`github-clone` → `github-clone.ts`、`register-zone` → `register-zone.ts`、`github-repository-exploration` → 开源项目仓库探索。脚本在 `default/bot-playground`，探索节点使用动态 zone；Task 固定 prompt 为空。 | `cloned_repository`、`registered_zone`，W3 是无命名输出的结束节点。已选择并迁入新格式 Map／三个 Bot；探索要求统一使用 `mapInput.runInput`，Task 与 W3 提示词为空。代码与检查记录见下节；用户完成确认和真实运行仍待执行，旧 backup 保留。 |
| [`tibo-monitor-then-c-drive-inspection.json`](../meta-data/workflow-maps/tibo-monitor-then-c-drive-inspection.json) · Tibo监测后排查C盘 | 未开始 | 无外层 Bot；`tibo-monitor` → Tibo-monitor（Pi／`web-access-pi`）、`c-drive-temp-inspection` → C盘清理大师（Pi／`pi-codemode-pi`）；两个节点均绑定 `default/bot-playground`。 | 无命名文件输出；未选，监测、检查授权和真实运行待核，清单不授权文件改动。 |

## 创建map：已交付的转换与共享影响

- **资产来源和非目标差异：** 使用[已批准正文](../.scratch/bot-map-contract-refactor/lighthouse/04-coherent-prompts.md)及其 Traverse 修订。保留[图文件名](../meta-data/workflow-maps/plan_workflow.json)、`meta-map`、三节点 ID、名称、Bot 绑定、zone、依赖、批准与自动关闭策略，以及 `outline`、`map`、`handoff` 接线。四个 Bot 的文件名、ID、名称、分组、目标不变；仅替换四字段正文并移除旧指南字段。原位更新三 Task 的 `prompt`，保留 ID、名称、分组、Chat 命名、OMP／模型／thinking、`codebase-exploration-omp`、初始 `grill-with-docs`／无／`writing-for-agents`。根 manager 不增加流程专项设置。输入继续仅使用本次要求；本轮追加公开 `created_map` 身份输出，不引入 `materials`。
- **唯一业务资料：** [规划交付契约](../meta-data/workflow-maps/plan_workflow/planning-deliverables.md) 供三个 Task 和阶段节点使用。定义、HTTP 约束和通用流转继续由 [scheduler](scheduler.md) 与 [workflow runtime](workflow-runtime.md) 维护；不是第二份阶段验收。实际词典应将 `${herdr-bot-path}` 指向包含该文件和系统文档的产品根。
- **真实共享消费者：** 仓库九张 Map 的 `bot_id` 与全部 Bot 的目标引用中，只有此图引用三个内部 Bot；无其他 Map 节点引用 `bot-map-plan-workflow`。四 Bot 经目录供规划者/chief 查找，三个 normal Task 仍可各自独立启动，不能据静态引用断言没有独立使用者。`codebase-exploration-omp` 还被创建task、创建bot、创建bot-V2、创建task-V2、创建task-script type、修订图和 GitHub 探索所用 Task 复用；`manager` 也是其他自举 Map 的子管理者。`create-task` 由两个创建task图复用，`optimize-prompt` 由创建task与创建bot复用，四字段 writer 由创建bot与修订图复用。Suite 通过配置引用外部 Skill 集、扩展及资源目录；Skill 来源和系统说明供多个流程使用，未复制或改写 Skill/Suite/script。后续剪裁提示词仍须先列出共享对象的调用方并取得修改范围确认。
- **待用户确认的真实业务证据：** 现场保存与读回身份、需求原文及参考路径、方案当前版本及确认范围、阻塞或能力缺口、转成的配置与后续优化差异、共享授权、最终内部 Map 与实际配置的比较、交接说明、执行/省略检查、未知项及不启动生成业务的效果证据。保留各阶段执行／批准／报告与显式完成事实，不用它们替代用户业务判断。

## GitHub 仓库拉取与探索：2026-11-27 选择性迁移

- **确认来源：** 本轮用户选定此 Map，并明确不把无助于任务完成的旧说明搬入 Task.prompt 或 node.input；接受只传启动时的 runInput，不需要同步管理者 Chat 后续补充。用户要求保留 GitHub 链接和 repo 父目录输入框，批准同步修改代码，而不只改配置与提示词。后续 review 中，用户确认只通过表单提供克隆参数、无需记录默认启动授权，并同意新增输入驱动动态 zone。上述为迁移方案授权，不是最终迁移完成或业务验收确认。
- **来源与落点：** 从 `backup/20261127-161156-bot-map-cutover/workflow-maps/github-repository-exploration.json` 及同批 `bots/` 三个旧定义迁入正式 `meta-data` 目录。保留 Map 文件名、W1/W2/W3、三个 Bot ID 和原文件名、目标引用、依赖、批准及关闭策略；W3 的动态 zone 后续按本轮 review 改为输入驱动。未修改或删除 backup。备份内 `evidence/verification.json` 记录字节/目录匹配与文件系统在线排除，但服务未重启、chief 未清理、外部效果和在线 HTTP 未核验；本轮不据此宣布现场切换完成。
- **输入与提示词：** GitHub 链接与父目录通过表单必填的两个字段生成唯一 `clone_request`，前后端拒绝缺项及已有文件替代，不再由管理者补齐；探索方向使用通用“本次要求”进入 `mapInput.runInput`。移除探索方向专用参数、`exploration_brief` 生成及其文件接线。Map 输入说明仅按链接、父目录、本次要求的业务含义组织；省略克隆请求文件说明，删除启动授权和通用步骤复述。W3 `input` 为空，原 Task `task-18e6bf4da8312bbc-0` 的空 prompt、模型、thinking、Suite 及其他设置不变。不把旧只读说明迁入 Task/node，也不把 Bot 摘要当执行约束；用户实际输入确定探索范围。
- **交付边界：** Map 新输入规则只声明 `clone_request`；公开输出只描述探索 Chat 交接，不声明文件。W3 保持结束节点，成功发起后按通用协议 finish，不等待探索完成。仓库核实与失败处理不变，真实仓库核验由脚本承担；W3 通过 `input_rule` 接收 W2 的 `registered_zone`，并用 `zone.input_id` 引用该本地输入；后端读取 `zone_id` 选址，管理者不再手填 zoneIds 或读取流程选址提示词，也不重复承担脚本业务验收。
- **登记服务地址：** 用户确认复用当前 listener 的实际绑定结果，由后端通过 `context.serviceUrl` 提供给 `register-zone.ts`，用于请求及输出 `service_url`；不新增 paths 配置、不依赖提示词词典或固定 18787。Bot 说明改为“当前 herdr-bot 服务”。
- **提示词 review 落盘：** Map.description 只描述用途，公开输出说明负责交付边界；三个 Bot 只保留选用所需的职责、输入输出摘要、关键限制和实际副作用，移除通用流转、实现校验细则、配置缓存及本 Map 专属说明。用户再次审核并确认的三个 Bot 正文已落盘：克隆与登记摘要进一步精简，探索明确以可访问的本地仓库代码为对象，不将本 Map 的两个前置步骤设为独立使用的必经流程。Task.prompt 与全部 node.input 继续为空，结构化文件接线和输入驱动 zone 不变。
- **共享影响：** 对正式定义及本批 backup 的静态引用调查中，仅此 Map 引用三个 Bot，仅探索 Bot 绑定该 Task；不据此排除用户独立调用。Task、克隆脚本和共享 Suite 未修改；通用 Run 文本传递机制不变；新增的输入驱动 zone 可用于 Task／脚本／子 Map 节点，人工动态和固定 zone 保持原语义。其他未选 Map 不迁入、不算废弃。
- **验证与现场边界：** 本轮定向检查结果由交付报告记录；未启动真实 GitHub 克隆、zone 登记或探索 Chat，未读取认证、清理 chief、归档其他 Run 或删除旧副本。待用户核对本次转换后再将状态改为“迁移完成”。

## 创建bot-V2：本轮选择性迁移

- **确认范围：** 三个 Bot 摘要、W1/W2 稳定职责、两个节点的条件式读取指针、材料契约和写入授权已逐项确认；[审阅记录](../.scratch/bot-map-contract-refactor/create-bot-v2-migration-draft.md)指向当前唯一正文，不维护第二套提示词。
- **来源与正式落点：** 从 `backup/20261127-161156-bot-map-cutover` 中的 create-bot-v2.json 和三个对应 Bot 迁入正式目录，保留文件名、ID、目标绑定、分组、节点身份、zone、依赖、批准与关闭策略。原位替换两个 Task.prompt，其他 Task 设置不变；backup 未修改或删除。
- **接口与交接：** 唯一必需 request 为 type/ref JSON，W1 和 W2 通过 Run 接线读取同一文件。W1 只交付 bot_requirements，W2 查询真实目标并交付 created_bot（Bot ID）。移除 materials、material_contract、target_materials 和旧 saved/handoff 接线；输入意图与参考路径使用本次要求。系统校验文件可读，不新增业务 JSON 或枚举校验。
- **唯一资料与授权：** V2 契约迁至 `meta-data/workflow-maps/create-bot-v2/materials.md`；节点写入前读取材料授权章节，允许在本 Run 内部材料位置新建必需文件，覆盖仍须明确授权。正式 Bot 保存、目标 prompt 修改和业务运行不由材料授权替代。
- **共享影响：** W2 和 prompt优化者——Map维度均读取 [分层与 DRY](prompt-information-ownership.md)。后者同步采用已确认的精简正文；创建map最终节点新增 created_map 文件，与原 map/handoff 一起交付，其外层 Bot、公开接口和阶段标准同步。未修改 Skill、Suite、脚本或模型设置。两个 V2 Task 的独立使用继续有效；静态引用清单不能排除独立调用者。
- **父流程后续迁移：** 创建bot-V2 迁移时，两张旧父图均留在 backup，嵌套输入下传尚未实现。后续创建task-V2 的迁入和输入机制修订见下节；创建task-script type 随后按本轮确认迁入，见其专节，不据此宣称 backup 中旧父图兼容。
- **完成边界：** 本轮不启动真实创建业务、不重启服务、不清理 chief、不删除旧副本。定义落盘和定向机械检查不替代用户迁移完成确认、真实模型效果及现场切换。

## 创建task-V2：本轮选择性迁移

- **确认来源：** 用户同意逐层将父 Run 本次要求与调用节点委托合成子 Run 本次要求；明确 Task 的稳定信息由 prompt、Skill 和引用资料承接，独立调用与嵌套调用同构，不新增需求交接。用户批准 created_task/created_bot 身份交付，并在逐项审阅 Task 提示词后再次确认“配置已保存并读回、真实 ID 已交付”的完成门槛。
- **来源与落点：** 从 `backup/20261127-161156-bot-map-cutover` 迁入同名 Map、入口 Bot 和共享 `create-task` Bot。保留文件名、ID、目标绑定、分组、节点、zone、管理者、依赖、成果批准与关闭策略；backup 保留原样。
- **接口与职责：** 不再声明 materials；参考资料路径与用途使用本次要求。W1 通过公开接口保存 Task、完整读回并交付 created_task；request 仅按创建bot-V2 契约定位同一 Task。W2 input 为空，只传 request 文件并接收子图公开 created_bot。移除旧 handoff/saved/material_contract 接线，不增加需求副本。最终配置按身份从服务读取；子阶段获准的 prompt 优化不改变 Task ID。
- **正文与共享影响：** 原位更新 `task-18e31d30fe69a51c-2` 的 prompt，其他字段保持；共享 create-task Bot 摘要原样迁入。该 Task/Bot 还供未迁移的旧创建task使用，未为它维护第二套定义。子图及两个内部 Task 不变。Task 公开创建接口集中在 usage，本流程材料规则位于 Map 同名目录，由 W1 条件式读取。
- **通用机制：** 所有嵌套 Map 使用父 Run 输入加调用节点委托，逐层保留来源；不读取 Chat 后续对话，不替换用户文本，不扩大命名文件输入。该机制不代替实现对象自包含，也不新增额外业务授权。
- **完成边界：** 定义保存及定向检查不代表模型遵从、用户迁移完成确认或现场切换通过。本轮不创建真实业务 Task/Bot、不启动业务、不重启部署、不清理 chief、不删除 backup。验证结果见[交付报告](bot-map-contract-refactor-delivery.md)。

## 创建task-script type：2026-11-28 选择性迁移

- **确认来源：** 用户逐项批准精简 Task、Bot 四字段、父 Map 与材料契约；明确保留初始 grill-with-docs，Map.description 只描述创建脚本及绑定 Bot 的用途，不复述内部实现方式。正文批准不等于迁移完成签收。
- **来源与落点：** 从 backup/20261127-161156-bot-map-cutover 迁入同名 Map 与 bot-1795065462879497700-23132-0.json，保留 Map 文件名、节点 ID、Bot ID、目标绑定、zone、manager、依赖、批准与关闭策略；backup 不变。
- **分层与接口：** Task task-18e95895bb36d00c-0 仅保留产品脚本契约入口，创建方式由用户目标与初始 Skill 决定，其他 Task 字段保持。父图不声明 materials；节点按[本流程材料契约](../meta-data/workflow-maps/create-task-script-type/materials.md)交付 created_script 与内部 request，request 的 type=script、ref 指向同一正式脚本。子节点 input 为空，接收 request、交付 created_bot；父图公开 created_script/created_bot，不再要求 handoff/saved 或需求副本。
- **审阅与共享影响：** 已联合核对父子 Map、四个 Bot、三个业务 Task、manager、实际初始 Skill、材料与公共协议。脚本 Task 的独立使用受本次精简影响；静态引用中仅本图使用该脚本创建 Bot。后续共享审阅经用户批准三处调整：创建bot-V2 的 description 只描述用途，创建者 Task 按本阶段报告检查与未验证项，request 的脚本路径引用 scheduler 唯一契约；其余正文、三个 Bot、接口和执行设置保持。调整同时影响创建task-V2 与独立调用；不修改共享 Skill、Suite 或公共运行机制。Chat 后续对话不自动下传，子图仍按真实目标及本次要求处理缺项。
- **完成边界：** 本轮只保存定义与材料规则，不生成真实业务脚本或 Bot、不发起 Run、不重启服务、不清理 chief、不删除 backup。定向静态检查与真实模型/现场验收分开，检查范围见交付报告；状态保持未开始，待用户对落盘结果确认。

下一条流程从[单流程迁移指引](agents/workflow-migration.md)开始，仅复用调查和 grilling 方法，不复用创建map的业务答案；共享原件原位维护、无需等待所有消费者一起迁移。停机、归档和清理见[交付与现场交接](bot-map-contract-refactor-delivery.md)。
