# Bot 指南的共同业务材料契约

创建、修订 Bot 指南及独立 Chat 共用本契约。先读“三种材料，直接交给消费者”，再按当前职责读取：基线生产者读“目标核实与基线生产”；编写者读“共同基线消费”和“三个编写者与表达优化者”；优化者读“共同基线消费”和“表达优化”；保存者读“一次合法保存与失败核实”。要求文件时另读“文件与独立 Chat”。各阶段只执行自身步骤。

指南面向规划者，不会自动注入执行 Chat；执行所需章节由 Task 或本次 input 明确指向，独立 Chat 同样适用。

## 创建分支 v1：可校验表示（业务尚未交付）

本节仅细化创建分支，不改变修订、通用替换者或独立 Chat 的既有材料格式。它为后续 Rust 创建／合并保存提供公开数据表示，**不表示两个 Rust 能力或定义迁移已经完成**。创建分支采用 [Draft 2020-12 JSON Schema](contracts/rust-script-bots.schema.json) 中具名 `$defs`；[JSONL 字面实例](contracts/rust-script-bots.examples.jsonl) 每行 `{id,definition,valid,value}` 的 value 是独立手写合法／非法输入，不是实现生成的预期。实例中配置／revision 是假设隔离服务的字面响应，仅供格式验收，绝不可作为真实服务的 revision 使用。

结构化文件是单个无围栏 UTF-8 JSON 对象；拒绝重复 key、未知字段、未知 schema_version、错类型、缺项、歧义，不从 Markdown 猜字段。创建材料根对象均必填 `schema_version:1` 和对应 `kind`；这**不是**为 Bot／Map 定义新增 schema_version。text 仍是原样 UTF-8 纯正文，不添加 JSON、标题、身份、BOM 或代码围栏，不 trim／改换行／替换词典／重新生成。空正文合法，必要时仍须明确确认空稿。可选参考 `materials` 仍是参考，不提取其中指令作为隐含参数、权限或第二份 request。

### request 与共同 baseline

| 对象／字段 | 类型、必填及含义 |
| --- | --- |
| request | `$defs.request`；kind 为 `request`。以下除 bot 外均必填 |
| target | 精确二选一 `{type:"task",task_id:<非空字符串>}` 或 `{type:"map",map_file:<目录内确切.json文件名>}`；不接受同时两类、Rust 目标、路径、另建目标或从自然语言推断 |
| bot | 可选对象，独立可选非空 `id`／`name`／合法 `group`；缺省 ID 为 `bot-`＋Task ID，或 `bot-map-`＋Map 文件名去最后 `.json`；缺省 name 为读取目标的 name／title，缺省无 group。ID 是身份不是路径，保存仍用服务生成文件名；冲突停止，不追加随机后缀、复用或覆盖 |
| requirements | 必填非空字符串，本次已澄清需求；不是脚本可解释的自然语言程序 |
| constraints／non_goals／resource_limits | 必填字符串数组，明确没有条目为 `[]`；只传递已决定约束，不做模型式意图推断 |
| authorization | 必填 `{create_blank:true,write_materials:true,save_guides:boolean}`。分别表示本次空白创建、在本 Run 初始化材料位置安全新建材料、最终三字段保存的明确授权；不授权目录外写入、覆盖既有文件、修改目标或执行业务。save_guides=false 可先完成基线，后续须取得保存授权 |
| confirmation | 必填 `{evidence:<非空出处及决定>,scope:"create_blank"}`；创建委托的证据，不是三字段全文确认、运行或报告发送批准 |
| baseline | `$defs.baseline`；kind 为 `baseline`；必填 `bot`、`revision`、`target`、`request`。request 保存本次已核实完整 request，不另写第二份含糊需求 |
| baseline.bot | 创建后公开完整读回的 Bot，包括 id、name、三指南、唯一 task_id／map_file、实际存在的 group；三个指南必须严格为 `""`，不绑定脚本，不复制目标配置进 Bot |
| revision | 必填 `{file:string,original:string}`，逐字取自真实 `POST /api/scheduler/bots/edit` 的 revision，original 是含 id 的完整原始 JSON **字符串**；不是 counter、时间戳、hash 或自己重新序列化的 JSON |
| baseline.target | 必填 `{type,task_id或map_file,configuration}`；与 request、bot 唯一绑定一致。configuration 是公开读取的完整 **对象**，不只给地址、名称、摘要或 `"见配置"` |

Task configuration 为 GET task 的完整对象：id、name、chatName、agent、taskType（必须 normal）、model、thinking、skill（可 null）、prompt，以及实际存在的 group／suite；不使用创建请求扁平片段代替读回。Map configuration 为 GET Map 的完整定义：title、description、nodes，及实际存在的 group、input_requirements、input_files；所有 node 字段与规则完整保留，不删成空图或仅复制文件名。其字段类型见 Schema 的 task_configuration／map_configuration；引用对象调查、Map 环与预检仍用既有接口，不要求将全部传递闭包嵌进 baseline。

创建前目标必须真实存在且类型正确；合法空白创建只新增 Bot 一次，唯一绑定与三空指南不能由流程到达推断。创建后完整重读 Bot、真实 revision 与目标，核对后才交付 baseline。配置或材料失败时保存可得真实效果，不自动删除已创建对象。

### 三路 text、必要 handoff 与 saved

- 指南 handoff 使用 `$defs.guide_handoff`：必填 kind=`handoff`、stage=`guide`、bot_id、target（无 configuration 的唯一定位）、field（when／plan_guide／node_guide）、sources、confirmation、constraints、unverified。它不含正文、完整配置或 revision.original。
- `sources` 是非空 `{binding_id,direction,role}` 数组；direction 是本次生产者的 input／output，role 是 request／materials／baseline／text／handoff，binding_id 是该生产者已声明的本地命名绑定 ID。同方向 ID 不重复。它只记来源／用途，消费者**不沿索引寻址**；原材料及当前最终 text 必须已由 input_rule 直接送达，嵌套传递仍由调用接口显式映射。编写者至少记 baseline 输入和最终 text 输出；优化者另记收到的 text／handoff，记录新的 text 输出。
- `confirmation` 必填 evidence、text_sha256（64 位小写 hex，针对最终 text 的原样 UTF-8 bytes）、full_text=true、save_authorized（boolean）、semantics_preserved=true。evidence 记真实确认来源、范围和保义依据；hash 只绑定该次确认的**同一份最终正文**，不是业务效果证明或权限令牌。沿用有效确认时保义仍真实成立；语义变更必须先取得新的全文确认。仅全文已确认、未获保存权限的手稿可用 save_authorized=false 交付，但不能进入保存成功路径。
- 最终保存使用当前既有命名输入：baseline，when_text／when_handoff，plan_guide_text／plan_guide_handoff，node_guide_text／node_guide_handoff。分别对应 Bot.when／plan_guide／node_guide；每个 handoff.field、bot_id、target 及最终 text digest 必须与所属分支及共同 baseline 一致。不得交换、重复、猜缺失来源或用 node.input 生成正文。三个 handoff 必须各自有有效全文确认；保存授权由 request.authorization.save_guides=true 或覆盖三字段的后续明确授权（各 handoff.save_authorized=true）提供，不能由 canComplete 或报告发送批准补足。
- 保存时重读当前 Bot、revision 和完整目标，与同一 baseline 精确比较；保留所有非目标字段及唯一绑定。一次合法保存汇总三段原样 text，禁止分三次保存、自动合并或绕过运行禁写。写后完整读回 Bot、真实新 revision 与未变目标；三字段须逐字等于最终 text，非目标须逐项等于 baseline。
- `saved` 使用 `$defs.saved`：kind=`saved`，必填完整 bot、真实新 revision、完整 target（含 configuration），**不是**仅 Bot ID、响应摘要或候选。输出 ID 仍是 saved。
- 创建／最终保存另交 `$defs.result_handoff`：kind=`handoff`，stage=`created`／`saved`，bot_id、target、sources、confirmation_evidence（非空字符串数组）、checks（bot_read_back=true、target_unchanged=true）、unverified（字符串数组）均必填。创建 sources 记 request 及实际参考；保存 sources 记 baseline 和三组正文／handoff。输出 ID 仍是 handoff，不复制完整 Bot／Task／Map、baseline 或三段正文。失败和部分效果写入 execution.failure／报告，不伪造成功 checks 或成功材料。

### 格式以外的校验与独立反例

JSON Schema 负责字段形状，不会访问服务、核实授权、验证 revision.original 或证明运行结果。生产／消费方还必须执行以下跨文件及真实服务判定；格式通过不等于业务有效。以下预期来自契约和字面值，不由实现算法生成：

| 独立输入／现场 | 必须结果 |
| --- | --- |
| target-unknown 的 `rust_script:"execute_code"`；request-script-forbidden | 分别拒绝未知能力／创建脚本目标，不写定义 |
| baseline-task 中 target.task_id=`normal-2`，其 configuration.id 与 bot.task_id 仍为 `normal-1` | target-conflict，无写入 |
| baseline-task 中 bot.id=`other-bot`，其真实 revision.original 仍写 `review-bot` | identity-conflict，无写入 |
| 真实服务 Bot revision.original 为 saved-task 的 original，而输入仍为 baseline-task 的 original | revision-conflict；即使三段 text 已与当前正文相同也不得成功 |
| 当前 normal-1.prompt 从 `只读，不修改。` 变为 `可修改。`，Bot revision 未变 | target-conflict，无保存 |
| when_text 原样为 `hello`，when_handoff 仍是 guide-confirmed（确认 abc 的 digest） | authorization-required，无保存；不能仅凭历史确认文字放行 |
| plan_guide_handoff.field=`when`，when_handoff 也为 when | ambiguous-request／target-conflict，无保存，不自行交换材料 |
| 合法三段正文 when=`abc`、plan_guide=`hello`、node_guide=`""` | 完整结果逐字见 saved-task；分支确认各自针对这些最终 bytes，不能以示例 itself 作为真实批准 |
| Bot 保存已成功并读回，saved 文件新建失败 | lifecycle failed，canComplete=false；write／readBack 仍为 succeeded，错误为 material-write-failed，保留已生成可读文件；原基线不刷新 |
| Bot 保存失败；或写后读回不可读／不一致 | 分别 write-failed／read-back-failed；后者不得声称未写入，不提交 true |

错误结构、stage／code、unknown 与部分效果见[运行失败表示](workflow-runtime.md#结构化失败与实际效果)。JSON 重复 key 即使普通 parser 会覆盖也必须拒绝；目标／revision／材料错误不能靠再次解析、自动重试、回滚或内容相同规则恢复。仅登记真实已写且完整读回的输出，写后材料失败不把已有配置删掉。

表示验证入口：安装 `python -m pip install -r scripts/requirements-contracts.txt` 后运行 `python scripts/test-script-contracts.py`，离线验证 Schema 与字面实例；这不是 HTTP 业务验收。定义与材料运行交付仍须按所属票据验证 S1/S2/S3。修订分支继续下文原有 Markdown／已约定格式，不能因共享 Task 看到本节而要求修订材料补 schema_version、digest 或创建授权。

## 三种材料，直接交给消费者

| 材料角色 | 唯一内容 | 消费者 |
| --- | --- | --- |
| `baseline`（共同基线） | 本次核实得到的完整 Bot 定义及真实 revision、唯一执行目标的完整配置、需求、允许修改范围与授权、非目标、资源限制及确认依据 | 三个编写者、各自优化者、最终保存者直接接收同一份 |
| `text` | 对应字段的唯一完整 UTF-8 纯正文；不包代码围栏、身份、标题或说明 | 优化者／保存者直接读取，不从 handoff 提取 |
| `handoff` | 确认依据与范围、材料来源及用途、必须保留的语义限制、未验证项／阻塞；必要时记录实际保存与核对结论 | 与相应 text 一起直接提供 |

`baseline`、`text`、`handoff` 是材料角色，具体 input/output ID 由调用接口明确声明；多字段可用 `when_text` 等本地 ID 区分。handoff 不复制完整正文、Bot／Task／Map 配置或共同基线，不承担隐式寻址。其来源索引仅供追踪：消费者已取得所需材料，不沿索引寻找未声明文件。最终保存的完整对象另交付 `saved`，不塞入 handoff。

workflow 的每个必要消费者都在 `input_rule` 中直接引用共同基线和所属 text／handoff，在 `node.input` 中说明字段、材料用途、授权及额外验收；`blocked_by` 只表达前置依赖。嵌套 Map 通过命名输入显式传入，不能借父 Chat 或依赖边继承材料。独立 Chat 直接提供这些材料的正文或可读绝对路径和用途；同一共同基线只提供一次，无须用户抄写完整配置。

编写者消费 baseline，交付 text＋handoff；创建案例的表达优化者消费同一 baseline、所属 text＋handoff，交付新 text＋handoff；修订案例由编写者在同一 Chat 内完成表达整理和确认，不额外安排表达优化。最终保存者消费同一 baseline 和三组最终 text＋handoff，原样汇总一次保存。正文调整使既有确认失效时，回到受影响语义的确认，不自行扩大意图。

共同基线是业务核实依据，不是系统提供的内容快照、配置锁或交付后锁定；系统只保存实际文件地址绑定。消费者完整读取当前材料，比较真实配置；读不到、缺少必要材料、确认失效或内容冲突就明确阻塞，不回退聊天历史、另一执行或猜测路径。文件地址固定也不保证内容不变或持续可读。

## 目标核实与基线生产

目标定位只需提供真实 Bot ID 或明确要绑定的目标（normal Task ID／Map 文件名）、本次业务需求和可选参考材料。核实者自行经公开接口读取完整配置，不要求用户代抄。Task 型与 Map 型目标二选一；不改绑、不改目标配置、不以能力描述代替已实现能力。

1. 读取 `docs/scheduler.md` 的 Bot 查询、编辑、revision 与保存契约，Task 型另读 `docs/usage.md` 的 Task 规则；使用本次明确的服务位置。
2. 读取完整 Bot 目录，处理影响目标判断的目录问题；已有 Bot 通过 `POST /api/scheduler/bots/edit` 获取完整 content 与真实 revision。Task 型读取 `GET /api/tasks/{URL编码ID}` 的 task，核实 normal、职责、输入输出及环境；Map 型读取 `GET /api/scheduler/maps/{URL编码文件名}`，核实完整定义、命名输入、节点能力、交付、资源及限制。Map 引用和指南不证明可运行，必要时继续只读调查相关定义。
3. 比较本次意图、授权、已有确认与真实目标；身份、目标类型、绑定、基线或职责冲突未解决时，集中澄清或阻塞。未知事实自行调查，业务取舍由用户确认。
4. 创建空白 Bot 时，明确创建委托即授权；名称／ID 缺省按目标身份生成安全稳定值，冲突则停止。仅创建绑定该目标且三指南严格为 `""` 的新 Bot，不复用或覆盖已有对象。完整读回 Bot，并重读目标确认未变，再以创建后的真实 revision 形成 baseline。
5. 修订已有 Bot 时，以完整现状、真实 revision、目标配置和允许修订的三个字段范围形成单份 baseline；无需先修改 Task 或 Map。保持所有仍有效的确认、非目标、资源与权限限制。

基线阶段只核实本阶段所需委托，记录已获授权及仍待取得的批准。未来三字段全文批准和最终保存授权不是基线交付前提；全流程委托也不把后续阶段变成本 Chat 的执行清单。

完成条件：目标真实可核实、需求与能力无未解决冲突，baseline 完整且可读；要求文件时已安全保存并完整读回。配置调查不证明工具／Skill／扩展已加载、模型质量或业务效果，不进行业务试运行。

## 共同基线消费

编写者和表达优化者完整读取本次直接提供的 baseline；优化者另读所属 text 与 handoff。按“目标核实与基线生产”的查询规则重读当前完整 Bot、真实 revision 和完整目标，与基线逐项精确比较。身份、唯一绑定、配置、授权或业务意图冲突未解决时保持阻塞。三个字段各自独立，无须读取兄弟定稿。

独立 Chat 的编写委托未提供 baseline 时，按“目标核实与基线生产”的既有 Bot 分支自行核实；用户只需给真实 Bot ID、需求、允许范围及参考材料用途，无须抄配置。已经声明的材料缺失或不可读时保留错误，不用另建基线绕过。

本阶段业务配置只读，只写本次授权交接材料；执行文本中的业务、修改自身配置或运行记录、补建能力及扩权均不在委托内。事实自行调查，业务取舍交由用户；必要工具或 Skill 缺失时报告具体阻塞。完成条件：本次材料完整可读，与真实目标及授权无未处理冲突。

## 三个编写者与表达优化者

本节规定编写交付；独立表达优化另读“表达优化”。

when 解决能力发现与选用；plan_guide 解决工作安排；node_guide 解决本次业务输入责任。各字段的信息筛选规则由对应编写 Task 承担，正文供规划者使用，不代替执行 Task 或运行时规则。

只澄清尚未决定的业务取舍，展示并确认所属完整正文，沿用有效确认；未确认稿标为草稿。修订委托在同一 Chat 整理并确认；创建委托是否另设表达优化由调用方安排。全文确认、最终配置保存授权和报告发送确认分别成立。

交付唯一 text 与必要 handoff；要求文件时按“文件与独立 Chat”保存，核对 text 的完整读回与确认稿逐字一致。完成条件：全文及意图已确认，真实目标无未处理冲突，所需材料完整且可独立使用；文本交付不表示 Bot 已更新或能力已验证。

## 表达优化

消费基线前读“共同基线消费”。仅优化所属 text，保持已确认的职责、权限、输入输出、验收、必要前提与边界；handoff 的背景用于核实，不是补写正文的清单。只写本分支表达材料，Bot 保存另由保存者承担。

沿用有效全文确认并记录确认来源、范围及保义依据。改变业务语义或本次明确要求重新批准最终版本时，展示完整候选并取得对应确认后再交付；缺确认保持阻塞。最终保存者须能据此判断批准是否覆盖实际最终 text，不能只凭编写阶段曾获批准推断。

交付新的唯一 text 与 handoff；按“文件与独立 Chat”安全保存并完整读回，text 与最终优化稿逐字一致，handoff 保留确认依据、来源、语义限制及未验证项。完成条件：确认仍有效、语义保持、基线无冲突且所需材料齐全；表达优化不授予 Bot 保存权限。

## 一次合法保存与失败核实

保存者按全部替换项核实授权、来源和共同基线后才开始写入。必要条件缺失或任一冲突时整批不开始，不根据 handoff 猜正文或自行生成缺失内容。

1. 完整读取同一 baseline 和三组最终 text＋handoff；核对同一真实 Bot、同一执行目标、对应字段、有效确认范围及允许修改范围。来源文本原样使用，不重新优化或决定业务语义。
2. 重读当前 Bot、真实 revision 和完整目标配置，与 baseline 精确比较。真实 revision 来自服务，不自造、不使用来源材料的自编版本冒充。Bot、目标或基线冲突即停止，不覆盖、自动合并或替换对象。公开 Bot 保存接口对过期 revision 返回 HTTP 409、`code: bot-conflict`，保留当前完整定义；这是版本冲突，不是 HTTP 500 服务器故障，也不授权自动重试。
3. 核实平台运行禁写规则和本次授权：执行者自身配置、正在使用且要求保持不变的定义、运行记录均不可写；业务配置交付不授权运行、派发、停服、关机、补建能力或扩权。没有合法保存方式就报告阻塞。
4. 同一 Bot 的三个字段在内存组成一份完整候选，经 `edit/revision/save` 合法接口一次保存。保持 id、name、唯一目标绑定及全部非目标字段；关联 Task／Map 完整配置不变。不绕过接口改底层存储。
5. 保存后通过公开接口完整读回 Bot，逐字段核对三个值与 text 一致、非目标内容与 baseline 一致，并重读完整目标核对未变。HTTP 成功或只生成候选不算完成。完整读回对象交付 `saved`，handoff 仅记录核对结论和证据来源。
6. 保存失败、超时、读回失败或结果不明时，先读取现场核实已生效、未生效和无法确认的效果，保留原错误及限制。不盲目重试、不自动回滚、删除或清理，不把部分生效说成全部未保存。跨对象不承诺事务或整体原子性。

普通文件替换仍保持原编码、换行及全部非目标内容，保存前重读与基线精确比较；只有明确授权才新建，使用实际可用的完整安全写入，不先删除正式文件。缺少可靠能力则阻塞，不建设新保存工具。

## 文件与独立 Chat

本次明确正式交付位置优先；workflow 未指定内部位置时使用初始化提供的材料位置，独立 Chat 要求文件但没有明确位置／写入授权时先澄清。独立 Chat 未要求文件时可按相同角色直接交付正文和说明，仍不得复制正文到 handoff。

明确授权在某目录新建本次材料时，可选择无冲突的新文件名；目录存在本身不是写入授权。配置只读不限制已获授权的材料新建。写入前核实真实路径、授权范围、链接及父路径，防止路径穿越、符号链接／junction／重解析点越界和覆盖他人或其他执行的产物。采用实际可用的安全完整写入；材料用 UTF-8，text 无包装，baseline／handoff／saved 使用自包含 Markdown 或明确约定的结构化格式。每个文件保存后完整读回，核对内容与预期一致，并交付真实准确的绝对位置、编码及用途。失败或不可读时保留原错误与已发生效果。workflow 的地址登记与报告使用注入角色及 report_policy，完成与派发由管理者负责；业务安全职责在独立 Chat 中同样成立。

## 稳定 Task 清单与验证边界

| 稳定 Task ID | 能力 | 独立 Chat／workflow 配对测试唯一 owner |
| --- | --- | --- |
| `task-18e33fdc55177d1c-1` | 空白 Bot 创建者 | 21 |
| `task-18e3404478db1ef8-2` | Bot.when 编写者 | 21 |
| `task-18e340545eb481bc-3` | Bot.plan_guide 编写者 | 21 |
| `task-18e340f5287462ec-4` | Bot.node_guide 编写者 | 21 |
| `task-18e31d34797f1464-3` | 通用提示词优化者 | 21 |
| `task-18e3415003d8cd00-5` | 通用文本替换者 | 21 |

本表只列本契约统一维护的既有共享 Task；创建专属新增项归 21、修订专属新增项归 22、外层 Task 修改项归 24，各自交付时在所属长期清单记录实际稳定 ID。旧独立 create-bot／create-bot-task 不改职责，也不自动迁移用户部署。

公开定义读写、预检、真实临时文件材料绑定及真实 revision 冲突的启用检查入口：`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch shared_business`。全后端回归入口为 `node scripts/test-backend.mjs`，先生成当前 worktree 静态页面并使用独立 CARGO_TARGET_DIR。该检查只证明公开接线与持久边界，不以提示词全文快照、配置非空或受控宿主响应证明模型使用材料。

P40–P45 的真实创建、修订与六个 Task 配对业务结果分别由 21／22 的隔离原生 Chat 验收负责；外层修改归 24。实际执行命令、原生历史、模型请求、授权、真实文件与完整配置读回才是业务证据。缺依赖、业务失败、冲突和未验证项如实保留，普通回归通过不能代替真实模型验收。
