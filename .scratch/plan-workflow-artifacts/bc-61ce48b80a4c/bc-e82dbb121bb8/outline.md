# 概要方案：创建并优化 Task＋Bot 工作流

## 1. 方案身份、修订与确认状态

- 方案标识：create-bot-task-workflow--bc-61ce48b80a4c。
- 本文修订：r2；替代版本：r1。更新 W1–W4 的真实能力绑定、G1–G3 补齐状态、Skill 配置与证据；既有业务决策和权限边界不变。
- 规划 Run：bc-61ce48b80a4c；outline 执行：bc-e82dbb121bb8。
- 状态：已确认。用户在本 outline 节点 Chat 明确回复“确认”，确认本 r2 及下述确认范围。G1–G3 配置均已补齐并经本规划节点重新 GET 核实，无已知阻止翻译的能力配置缺项；方案可供 generate_map 翻译。业务运行仍未验证；报告发送尚待用户另行批准，本次方案确认不授权启动业务。
- 确认范围：可复用四阶段流程、创建阶段 grilling 与意图确认、自动优化并保存、优化 Skill 与通用能力边界、需求入口、业务 zone 和材料隔离、配置交付验收。不包含新业务启动授权或管理者配置选择。

## 2. 目标、验收、非目标与授权

目标：交付可复用工作流，每次接收一项能力需求，按“创建 Task → 优化 Task.prompt → 创建并绑定 Bot → 优化 Bot 三份指南”完成一组配置。

需求入口为每次新业务 Run 的 userInput（界面的本次要求），不使用本规划 Run 的 userInput 作为未来业务需求。两个创建阶段的 grilling 可以补充或澄清需求；补充结论必须写入材料。节点不隐式继承管理者输入或其他 Chat。

人工确认针对 Task.prompt 及 Bot.when/plan_guide/node_guide 是否清晰准确表达用户意图。创建阶段在意图确认后保存并读回，不另加机械写入审批。优化自动执行，无须再次审批；只改变表达，不自行改变已确认职责、权限、输入输出或验收语义。

验收：每次业务交付包含真实 Task/Bot ID、绑定、最终优化文本、配置读回证据及限制。完成只证明配置交付，不证明该新能力实际业务效果。Task 为 normal；Bot 绑定本次创建的 Task。

授权范围：本规划节点仅讨论、只读调查和规划文档写入；能力建设由用户在独立 Chat 人工发起。本方案未来业务允许创建本次 Task/Bot，并自动修改本次对象的指定文本字段。保持其他既有配置及四阶段执行者自身配置不变。

非目标：不建设调度器、插件或权限系统；不在本节点生成 Map JSON、启动 Task/Chat/Run 或派发；不直接改 .data；不自动运行新建能力、不安装工具、不写凭据、不执行外部业务、停服、关机或删除资源。共享选项目录变更或真实试验须另行授权。

## 3. 工作划分

W1–W4 是规划工作编号，不是真实 node ID。

| 工作 | 职责与输入 | 交付与完成条件 |
| --- | --- | --- |
| W1 创建 Task | 从派发给本业务节点的真实 run.json 读取 userInput；阅读用户给定材料，通过 grilling 澄清、起草并确认意图，确认执行配置 | 创建 normal Task；读回；交付 task-created.md，包含真实 ID、完整配置、原始需求及补充结论、已确认意图、资源权限和缺项 |
| W2 优化 Task.prompt | 从当前 W1 执行交付读取目标 ID、原文和已确认意图；独立新 Chat 使用 /writing-for-agents | 仅修改本次 Task.prompt，保留其余字段；通过服务保存并 GET 读回；交付 task-optimized.md，含优化文本、Task 身份、约束和验证结果 |
| W3 创建 Bot | 读取 W2 当前执行材料与真实 Task 最新配置；通过 grilling 澄清并确认三份指南的意图 | 创建 Bot，绑定 W2 的 Task；读回核对；交付 bot-created.md，含真实 Bot/Task ID、三份指南、确认意图及资源限制 |
| W4 优化 Bot 指南 | 从当前 W3 执行材料读取 Bot ID、三份原文、确认意图与 Task 能力边界；独立新 Chat 使用 /writing-for-agents | 仅修改 when、plan_guide、node_guide，保留 ID、名称、task_id；服务保存并读回；交付 bot-optimized.md，包含最终 Bot/Task 身份、绑定、文本及验收限制 |

W1/W3 的澄清或意图确认未完成时留在当前 Chat。W3 若发现用户补充要求实质改变 Task 职责，不擅自改 Task 或创建不相符 Bot：记录矛盾交回用户/管理者处理，不自动增加回边或重试。W2/W4 若不能在保留意图的前提下优化，报告阻塞，不自行设计新需求。

报告策略拟定为四项 auto：W1/W3 在业务步骤内明确要求意图确认，确认后保存、读回并自动报告；W2/W4 自动优化、保存、读回并报告。auto 不免除业务意图确认；manual 的报告确认不是意图确认机制。本规划节点自身仍严格遵守 manual。

## 4. 能力选用与限制

| 工作 | 所需能力 | Bot ID / 绑定 Task ID | 状态 |
| --- | --- | --- | --- |
| W1 | Task 创建者，含 grilling 和意图确认 | create-task / task-18e31d30fe69a51c-2 | G1 配置已补齐；业务执行待验证 |
| W2、W4 | 通用提示词优化者 | optimize-prompt / task-18e31d34797f1464-3；同一 Bot，不同独立 Chat | G2 配置已补齐；实际 Skill 执行待验证 |
| W3 | Bot 创建者，含 grilling 和意图确认 | create-bot / task-18e31d347ccbeb4c-4 | G3 配置已补齐；业务执行待验证 |

三组执行者均已核实为 normal / omp / gpt-6-astra / medium。G1/G3 的 skill=grill-with-docs，extensions=[omp-codebase-tools]；G2 的 skill=writing-for-agents，extensions=[]，使用通用固定 prompt，不包含本工作流的服务地址、对象字段或 API。无可选扩展不代表无宿主内置工具，节点角色由产品加载。新业务产出的 Task 配置由该次 W1 需求与确认决定，不强制同模型。

优化者 Bot/Task 保持通用：不写入 herdr-bot 专用 API、字段或流程。工作流通过 node.input 和上游交接材料提供原文、确认意图、真实 ID、允许字段、服务与接口文档、保存和读回要求。干净上下文指独立新 Chat、不加载完整 grilling 历史或无关方案，不排除必要的角色、工具和接口文档。

现有 create-bot-task → task-18e315be1cf0ef10-0（normal / omp / gpt-6-astra / medium / grill-with-docs）仅作为用户人工建设入口，不是四阶段业务选用结果，也不作为自动派发项。

## 5. 依赖与材料交接

严格依赖：W1 → W2 → W3 → W4。blocked_by 只约束前置工作，不传递数据。

业务材料根：E:/Developer/myhub/herdr-bot/.scratch/create-bot-task-artifacts/。每次业务 Run 使用其真实 id 作为单一路径段，每项工作再按工作编号和该节点真实 execution.id 隔离：根目录 / 业务RunID / W编号 / 执行ID / 交付文件名。文件名分别为 task-created.md、task-optimized.md、bot-created.md、bot-optimized.md；UTF-8 Markdown。这里的路径规则是执行时显式读取与构造约定，不是 input 模板插值。

Map 生成者须写明 W编号到真实 node ID 的映射。每个节点从派发提供的本业务 run.json 取得真实 Run ID、自己的当前 execution.id；下游按固定映射从同一记录取得当前上游 execution.id，再构造其材料路径。所有 ID 校验为安全单一路径段，拒绝分隔符、冒号、点路径及越界；不可按最新修改时间猜测身份。

每项报告附精确绝对产物路径与身份。下游核对材料头部 Run/工作/执行身份和真实目标 ID，并从配置服务重读目标。上游缺失、身份不符或当前执行未交接时阻塞，不读取旧执行结果。重试前先核实已保存的业务对象和报告，避免重复创建；不自动清理或回滚。

材料内容：
- W1 提供原始需求、grilling 补充结论、明确确认范围、完整 Task 及真实 ID、资源与约束；W2 只读取优化必要部分。
- W2 提供最新 Task 配置与优化结果，并携带后续创建 Bot 必需的需求及约束；W3 无需读取完整 W1 对话。
- W3 提供 Bot/Task 身份、三份指南及各自意图、Task 职责与权限摘要；W4 读取最小必要材料。
- W4 提供最终完整配置、绑定及读回结果，区分配置验收与业务未验证。

专用操作信息明确进入 W2/W4 input：服务 http://127.0.0.1:18787；仓库文档 docs/usage.md（Task 配置）、docs/scheduler.md（Bot 编辑 revision/save 契约）；Task 更新已从 backend/src/lib.rs 的路由和 update_task 核实为 PUT /api/tasks/{id}，使用完整 TaskInput（name、chatName、agent、taskType、model、thinking、skill、prompt、extensions），不提交 id；先从 GET 详情读取其余字段并保留，只修改 prompt。Task 写入响应为 tasks/options 列表，不能按单个 task 对象解析；仍需 GET /api/tasks/{id} 核对。先查询最新配置，仅变更允许字段，保存后重读比较；Bot 冲突先核实，不覆盖他人修改。精确 API 操作契约由 Map 生成者重读当前文档并落实为可读绝对指针，不固化到通用能力配置。

## 6. 执行条件

业务 W1–W4 zone：E:/Developer/myhub/herdr-bot，已通过 GET /api/zones 核实登记；产品文档读取也证明当前路径可读。启动前仍须检查真实目录有效性和宿主可用性。

配置服务：http://127.0.0.1:18787，已实际读取 Bot/Task/zone。节点 cwd 不决定配置位置。现有 Skill 位置：C:/Users/hugos/.omp/agent/skills/writing-for-agents/SKILL.md，本规划 Chat 已读取；未来优化 Chat 的实际调用尚未验证。

新业务 managerTaskId、管理者 zone：用户未指定，预检待配置，不从本规划 Run 借用。缺这两项不阻止 Map 文件交付，启动预检仍待配置；G1–G3 真实绑定已核实。新业务 userInput 由使用者每次启动填写，为唯一初始需求入口。

## 7. 证据、未决项与阻塞

事实来源：
- 显式读取 E:/Developer/myhub/herdr-bot/.data/workflow-runs/bc-61ce48b80a4c/run.json 的 userInput、id 及 nodes.outline.execution.id；规划产物路径按安全单一路径段核实，不修改记录。
- 本节点 Chat 的 Q1–Q10 答复，尤其 Q6 的语义确认、Q9/Q10 的通用单 Skill 与输入隔离。
- 初次目录调查未见三项独立能力；用户人工建设后，本规划节点重新 GET /api/scheduler/bots、GET /api/tasks、三个新 Task 的 /api/tasks/{id} 及 /api/zones。三组 Task/Bot 与 capability-build-handoff.md 的 JSON 逐字段相等，绑定、normal 类型、执行配置、固定 prompt 与全部指南一致；目录响应无局部错误，业务 zone 仍登记。
- docs/scheduler.md、docs/usage.md、docs/workflow-contract.md、docs/workflow-runtime.md、GLOSSARY.md：固定 prompt 与 node.input 的执行职责、指南不自动注入、依赖不传材料、节点角色与报告策略。

G1、G2、G3：配置已补齐，业务执行待验证。当前 GET /api/tasks 的 options.skills 已包含 writing-for-agents，G2 Task.skill 也为该值，原配置选项缺口已解除。建设交接记录用户授权新增共享选项，并记录 omp skill list 发现 Skill、资源可读及静态 /skill:writing-for-agents 调用链；这些属于建设者提供的发现观察与静态证据，本规划节点未重跑宿主命令，也未观察新 Task 实际执行。

证据边界：本规划节点直接核实的是当前服务配置和绑定，不是历史 POST 过程。建设交接记录 G1 首次创建后的客户端解析错误已通过查询现场解决，无重复创建、无未处理部分失败；此历史事实来自交接文件。三组新执行者的实际 Skill 调用、固定/可选扩展加载、模型质量、grilling 确认及授权保存行为仍未作业务验证。配置就绪不等于业务效果已验证。

本规划节点仅进行只读服务核实和文档修订；未进行配置写入试验、业务试运行、Map 生成、结构校验或启动预检。当前没有已知能力配置阻塞；用户已确认 r2，待单独批准报告交接。缺少新业务管理者启动配置只影响预检，不影响静态 Map 翻译。

## 8. 下游要求

G1–G3 已补齐并核实，W1–W4 均可按本修订落实；用户已明确确认 r2，本文可交给 generate_map。节点交接报告仍须单独取得发送授权，由管理者核实后决定完成本节点和派发下游。下游不应要求把未授权业务试运行作为静态 Map 生成前提，但必须保留业务未验证边界。

Map 生成者重读所选 Bot 的 when/plan_guide/node_guide 及绑定 Task；显式写入每项 node.input 的目标、材料协议、必要资源、授权、字段限制、交付与完成条件，不依赖隐式上下文。node.input 为原文字符串，不做模板插值。落实 W编号映射、业务 zone、严格串行依赖与 auto 策略；意图确认作为 W1/W3 的业务要求保留。

只新增业务 Map，保持本次 plan_workflow 与现有执行配置不变。按当前契约完成 JSON、服务结构和绑定检查；没有管理者启动配置时标预检未执行，不代选；有明确配置时方可只读预检。保存及预检均不代表业务验证或已启动，不自动发起 Run。

最终本规划节点另行展示文件绝对路径、修订、确认范围和限制，请用户单独批准发送 report；方案确认不等于报告发送确认，报告不等于 node completed。

## 9. 能力建设需求索引

《能力建设需求》r1，G1/G2/G3：
E:/Developer/myhub/herdr-bot/.scratch/plan-workflow-artifacts/bc-61ce48b80a4c/bc-e82dbb121bb8/capability-needs.md

建设返回：《能力建设交接》r1，G1/G2/G3：
E:/Developer/myhub/herdr-bot/.scratch/plan-workflow-artifacts/bc-61ce48b80a4c/bc-e82dbb121bb8/capability-build-handoff.md

能力建设需求 r1 保留建设时的要求与历史缺口，当前状态以本文 r2 和服务核实为准，不要求重复建设。此索引仅用于追踪；本概要自包含，详细建设指令留在独立文档。
