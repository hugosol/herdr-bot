# Bot 能力摘要的共同业务材料契约

创建、修订 Bot 能力摘要及独立 Chat 共用本契约。先读“三种材料，直接交给消费者”，再按当前职责读取：基线生产者读“目标核实与基线生产”；编写者读“共同基线消费”和“四字段编写与表达优化”；优化者读“共同基线消费”和“表达优化”；保存者读“一次合法保存与失败核实”。要求文件时另读“文件与独立 Chat”。各阶段只执行自身步骤。

指南面向规划者，不会自动注入执行 Chat；执行所需章节由 Task 或本次 input 明确指向，独立 Chat 同样适用。

## 创建分支 v1：可校验表示与空白创建

本节仅细化创建分支，不改变修订、通用替换者或独立 Chat 的既有材料格式。它为 TS 创建／合并保存提供公开数据表示。空白创建 `create-blank-bot.ts` 和四路合并保存 `save-bot-guides.ts` 均使用该契约通过命名文件运行；发布的 `create-bot-wrapper.json` 使用这两个脚本节点。部署和共享 Task 的实际模型遵从仍须独立验证。创建分支采用 [Draft 2020-12 JSON Schema](contracts/script-bots.schema.json) 中具名 `$defs`；[JSONL 字面实例](contracts/script-bots.examples.jsonl) 每行 `{id,definition,valid,value}` 的 value 是独立手写合法／非法输入，不是实现生成的预期。实例中配置／revision 是假设隔离服务的字面响应，仅供格式验收，绝不可作为真实服务的 revision 使用。

结构化文件是单个无围栏 UTF-8 JSON 对象；拒绝重复 key、未知字段、未知 schema_version、错类型、缺项、歧义，不从 Markdown 猜字段。创建材料根对象均必填 `schema_version:1` 和对应 `kind`；这**不是**为 Bot／Map 定义新增 schema_version。text 仍是原样 UTF-8 纯正文，不额外添加 JSON、外层标题、身份、BOM 或代码围栏，不 trim／改换行／替换词典／重新生成。description 正文本身的 Markdown 小节是内容，不是外层包装。空正文合法，不附加确认凭据。可选参考 `materials` 仍是参考，不提取其中指令作为隐含参数、权限或第二份 request。

### request 与共同 baseline

精确字段见 Schema 的 `$defs.request` 与 `$defs.baseline`。跨文件及服务核对规则如下：

- `request.target` 只定位一个既有 normal Task 或服务目录内确切 Map 文件。不接受脚本、路径、另建目标或自然语言推断。`requirements` 与约束数组记录已澄清要求，不是脚本程序。
- `request.bot` 只可指定 ID、名称与分组。ID 默认是 `bot-` 加 Task ID，或 `bot-map-` 加去掉末尾 `.json` 的 Map 文件名；名称默认取目标名称或标题。省略分组即不分组。ID 不是路径，文件名由服务决定。冲突时停止，不追加后缀、复用或覆盖。
- `baseline` 保留已核实的完整 request、公开读回的完整 Bot、真实 revision 及完整目标对象。四个能力字段严格为 `""`；Bot 不绑定脚本。目标、request 与 Bot 的唯一绑定一致，不能用名称、地址或摘要代替完整配置。
- `revision` 逐字取自真实 `POST /api/scheduler/bots/edit`。其中 `original` 是含 ID 的完整原始 JSON 字符串，不是版本号、时间戳、哈希或重新序列化的对象。

V1 request 不接受 `authorization`、`confirmation`、`approved` 或其他批准凭据。成果批准属于本次 execution，不写入 request、baseline 或创建交接；下游保存权限不能倒灌为上游输入。不能由其他字段推断授权或把旧材料自动转换成 V1。[V2](contracts/create-bot-v2-materials.md) 的专用协议独立。

字段归属：调用方生产 request，target 只定位既有对象，bot 只提供可选 id／name／group，不能夹带四字段正文、目标绑定或 configuration。创建脚本经公开接口调查目标并唯一保存新 Bot；baseline.bot／revision／target.configuration 来自创建后的完整公开读回，baseline.request 原样保留已验证请求，不能用调用方配置代替服务事实。后续 V1 保存消费者读取同一 request／baseline／target，不恢复已删除的 request 凭据；材料生产者直接生成以下新协议，不先生成旧协议再转换：

```json
{"schema_version":1,"kind":"request","target":{"type":"task","task_id":"normal-1"},"bot":{"id":"review-bot","name":"审阅"},"requirements":"只读审阅","constraints":["不执行目标"],"non_goals":[],"resource_limits":[]}
```

`target.configuration` 来自公开 GET 的完整 Task 或 Map，保留实际存在的可选字段及所有 node 规则，不以创建请求、摘要或空图代替。共享 Map 材料格式使用 `map_input_rule`／`map_output_rule`；不以旧顶层字段或内部节点推断公开接口。精确形状见 Schema 的 `task_configuration` 和 `map_configuration`。引用调查与预检仍用公开接口，不把全部间接依赖复制进 baseline。

创建前目标必须真实存在且类型正确；合法空白创建只新增 Bot 一次，唯一绑定与四字段为空不能由流程到达推断。创建后完整重读 Bot、真实 revision 与目标，核对后才交付 baseline。配置或材料失败时保存可得真实效果，不自动删除已创建对象。

### 脚本空白创建的材料落点与隔离实例

`create-blank-bot.ts` 只接受命名 `request` 和可选参考 `materials` 输入；不沿参考内容寻找隐藏参数，也不从 node.input 补项。输出必须声明 `baseline` 和 `handoff`。材料实际新建在本 Run 的 `<manager zone>/.manager-zone/.workflow/<run ID>/<execution ID>/baseline.json` 与 `handoff.json`，沿用现有文件协调、祖先／junction 检查及 create-only 发布；不覆盖已有文件。仅完整读回成功的文件进入所属 execution 的报告 outputs，没有第二份输出登记表。

创建拒绝时核对原目录、配置与文件没有被改写，不生成成功材料。写后目标读回失败仍保留真实写入事实；已有同名材料或材料不可写时不覆盖，也不回滚已创建 Bot。复现入口见 [TypeScript 空白创建回归](agents/testing.md#typescript-空白创建公开-http文件回归)。

### 四字段正文、可选业务说明与 saved

- 字段 handoff 使用 `$defs.guide_handoff`：必填 kind=`handoff`、stage=`guide`、bot_id、target（无 configuration 的唯一定位）、field（when／description／input_rule／output_rule）、sources、constraints、unverified。它只供编写与表达优化说明来源及语义限制，不含批准凭据、正文、完整配置或 revision.original，不是保存的必需输入。
- `sources` 是非空 `{binding_id,direction,role}` 数组；direction 是本次生产者的 input／output，role 是 request／materials／baseline／text／handoff，binding_id 是已声明的本地命名绑定 ID。同方向 ID 不重复。它只记来源／用途，消费者**不沿索引寻址**。编写者记 baseline 输入和最终 text 输出；优化者另记收到的 text／handoff，记录新的 text 输出。
- V1 不生产或消费 `authorization`、`confirmation`、`approved`、`text_sha256`、`save_authorized` 或 `confirmation_evidence` 等人类批准凭据；未知字段直接拒绝，无旧输入兼容。成果批准属于本次 execution，与合法保存的数据一致性检查独立。修订和 V2 保留各自确认与交接协议，同步使用四字段 Bot 格式。
- 最终保存只接受五个唯一命名输入：`baseline`、`when_text`、`description_text`、`input_rule_text`、`output_rule_text`。四份正文分别原样写入 Bot 的 when、description、input_rule、output_rule。命名绑定就是字段归属，不从文件名、正文语义、邻居文件、handoff 或 node.input 猜归属／生成正文；缺失、重复、未知绑定均拒绝，不接受旧式字段或额外交接。纯正文允许任意合法 UTF-8（含空串），保存端不猜测其业务语义。canComplete 或成果批准不替代业务数据正确性核对。
- 保存时重读当前 Bot、revision 和完整目标，与同一 baseline 精确比较；保留所有非目标字段及唯一绑定。一次合法保存汇总四段原样 text，禁止分次保存、自动合并或绕过运行禁写。写后完整读回 Bot、真实新 revision 与未变目标；四字段须逐字等于最终 text，非目标须逐项等于 baseline。
- `saved` 使用 `$defs.saved`：kind=`saved`，必填完整 bot、真实新 revision、完整 target（含 configuration），**不是**仅 Bot ID、响应摘要或候选。输出 ID 仍是 saved。
- 创建／最终保存另交 `$defs.result_handoff`：kind=`handoff`，stage=`created`／`saved`，bot_id、target、sources、checks（bot_read_back=true、target_unchanged=true）、unverified（字符串数组）均必填。created 和 saved 阶段均不含 confirmation_evidence 或其他凭据。创建 sources 记 request 及实际参考；保存 sources 只记 baseline 和四份正文。输出 ID 仍是 handoff，不复制完整 Bot／Task／Map、baseline 或四段正文。失败和部分效果写入 execution.failure／报告，不伪造成功 checks 或成功材料。

### 合并保存的运行与部分生效边界

`save-bot-guides.ts` 是用户可维护的 TS 业务脚本，不创建 Chat，也不调用通用替换者。材料在本 Run 的 `<manager zone>/.manager-zone/.workflow/<run ID>/<execution ID>/saved.json` 和 `handoff.json` 安全新建，使用与空白创建相同的文件协调、祖先／链接检查、create-only 发布和完整读回；仅实际可读文件登记到当前 execution 的 outputs。保存不消费字段 handoff，不要求批准凭据或正文批准 digest；其严格五项命名输入和真实 baseline／revision／目标检查不能由批准事实代替。

配置合法保存成功后，如果 saved 或 handoff 的新建／读回失败，报告 `canComplete=false`，保留 write／readBack 的实际结果、原错误和已成功的命名输出；不删除已保存配置、不覆盖碰撞材料，也不更新原 baseline。重跑仍使用原 baseline 时会因 revision 已变化而报 `revision-conflict`，即使四段正文相同。要继续需核实新的有效基线，并遵守运行时 fail／retry 的独立门禁。成功报告也只表示可交接，仍由 manager 经原工具 complete，再从显式 saved／handoff 输出 ID 安排下游。

### 格式以外的校验与独立反例

JSON Schema 负责字段形状，不会访问服务、验证 revision.original 或证明运行结果。生产／消费方还必须执行以下跨文件及真实服务判定；格式通过不等于业务有效。以下预期来自契约和字面值，不由实现算法生成：

| 独立输入／现场 | 必须结果 |
| --- | --- |
| V1 请求定位脚本目标，或混入旧批准字段、重复键或错误归属字段 | 拒绝输入，不写定义 |
| baseline 的目标定位、完整配置或唯一绑定不一致，或目标真实配置已有变化 | `target-conflict`，不写入 |
| baseline Bot 身份与真实 revision 中的身份不一致 | `identity-conflict`，不写入 |
| 服务 revision 已变化，但四段正文恰好相同 | `revision-conflict`，不认定成功 |
| 缺 `when_text`，只带旧 `when_handoff`，或绑定 ID 重复 | 拒绝，不猜正文归属 |
| 合法正文依次为 `abc`、`hello`、空串和 `report` | 四字段逐字节等于输入；不要求批准 digest |
| Bot 已保存且读回，材料文件新建失败 | `material-write-failed`，报告 `canComplete:false`，保留成功写入和完整读回事实及已有可读输出 |
| Bot 保存失败，或写完后读回失败 | 分别为 `write-failed`、`read-back-failed`；后者不能声称未写入 |

错误结构、stage／code、unknown 与部分效果见[运行失败表示](workflow-runtime.md#结构化失败与实际效果)。JSON 重复 key 即使普通 parser 会覆盖也必须拒绝；目标／revision／材料错误不能靠再次解析、自动重试、回滚或内容相同规则恢复。仅登记真实已写且完整读回的输出，写后材料失败不把已有配置删掉。

`python scripts/test-script-contracts.py` 使用 `scripts/requirements-contracts.txt` 的依赖离线核对 Schema 和独立字面实例，不证明 HTTP、写入或原生行为。修订继续使用下文约定的 Markdown 或指定格式，不补 V1 的 `schema_version`、digest 或创建授权。

## 三种材料，直接交给消费者

以下章节保留修订及独立非 v1 调用的原有业务确认与保存授权职责；创建 v1 的字段形状和保存必需输入以上节为准：字段 handoff 不含批准凭据，最终保存只接收 baseline 与四份明确命名的 text，不适用下文四组 handoff 验权要求。V2 保留专用确认与交付协议，同样使用新 Bot 格式。

| 材料角色 | 唯一内容 | 消费者 |
| --- | --- | --- |
| `baseline`（共同基线） | 本次核实得到的完整 Bot 定义及真实 revision、唯一执行目标的完整配置、需求、允许修改范围与授权、非目标、资源限制及确认依据 | 四个编写者、各自优化者、最终保存者直接接收同一份 |
| `text` | 对应字段的唯一完整 UTF-8 纯正文；不加代码围栏、身份、外层标题或包装说明 | 优化者／保存者直接读取，不从 handoff 提取 |
| `handoff` | 确认依据与范围、材料来源及用途、必须保留的语义限制、未验证项／阻塞；必要时记录实际保存与核对结论 | 与相应 text 一起直接提供 |

`baseline`、`text`、`handoff` 是材料角色，具体 input/output ID 由调用接口明确声明；多字段可用 `when_text` 等本地 ID 区分。handoff 不复制完整正文、Bot／Task／Map 配置或共同基线，不承担隐式寻址。其来源索引仅供追踪：消费者已取得所需材料，不沿索引寻找未声明文件。最终保存的完整对象另交付 `saved`，不塞入 handoff。

workflow 的每个必要消费者都在 `input_rule` 中直接引用共同基线和所属 text／handoff，在 `node.input` 中说明字段、材料用途、授权及额外验收；`blocked_by` 只表达前置依赖。嵌套 Map 通过命名输入显式传入，不能借父 Chat 或依赖边继承材料。独立 Chat 直接提供这些材料的正文或可读绝对路径和用途；同一共同基线只提供一次，无须用户抄写完整配置。

编写者消费 baseline，交付 text＋handoff；创建案例的表达优化者消费同一 baseline、所属 text＋handoff，交付新 text＋handoff；修订案例由编写者在同一 Chat 内完成表达整理和确认，不额外安排表达优化。最终保存者消费同一 baseline 和四组最终 text＋handoff，原样汇总一次保存。正文调整使既有确认失效时，回到受影响语义的确认，不自行扩大意图。

共同基线是业务核实依据，不是系统提供的内容快照、配置锁或交付后锁定；系统只保存实际文件地址绑定。消费者完整读取当前材料，比较真实配置；读不到、缺少必要材料、确认失效或内容冲突就明确阻塞，不回退聊天历史、另一执行或猜测路径。文件地址固定也不保证内容不变或持续可读。

## 目标核实与基线生产

目标定位只需提供真实 Bot ID 或明确要绑定的目标（normal Task ID／Map 文件名）、本次业务需求和可选参考材料。核实者自行经公开接口读取完整配置，不要求用户代抄。Task 型与 Map 型目标二选一；不改绑、不改目标配置、不以能力描述代替已实现能力。

1. 读取 `docs/scheduler.md` 的 Bot 查询、编辑、revision 与保存契约，Task 型另读 `docs/usage.md` 的 Task 规则；使用本次明确的服务位置。
2. 读取完整 Bot 目录，处理影响目标判断的目录问题；已有 Bot 通过 `POST /api/scheduler/bots/edit` 获取完整 content 与真实 revision。Task 型读取 `GET /api/tasks/{URL编码ID}` 的 task，核实 normal、职责、输入输出及环境；Map 型读取 `GET /api/scheduler/maps/{URL编码文件名}`，核实完整定义、命名输入、节点能力、交付、资源及限制。Map 引用和指南不证明可运行，必要时继续只读调查相关定义。
3. 比较本次意图、授权、已有确认与真实目标；身份、目标类型、绑定、基线或职责冲突未解决时，集中澄清或阻塞。未知事实自行调查，业务取舍由用户确认。
4. 创建空白 Bot 时，明确创建委托即授权；名称／ID 缺省按目标身份生成安全稳定值，冲突则停止。仅创建绑定该目标且四字段摘要严格为 `""` 的新 Bot，不复用或覆盖已有对象。完整读回 Bot，并重读目标确认未变，再以创建后的真实 revision 形成 baseline。
5. 修订已有 Bot 时，以完整现状、真实 revision、目标配置和允许修订的四个字段范围形成单份 baseline；无需先修改 Task 或 Map。保持所有仍有效的确认、非目标、资源与权限限制。

基线阶段只核实本阶段所需委托，记录已获授权及仍待取得的批准。未来四字段全文批准和最终保存授权不是基线交付前提；全流程委托也不把后续阶段变成本 Chat 的执行清单。

完成条件：目标真实可核实、需求与能力无未解决冲突，baseline 完整且可读；要求文件时已安全保存并完整读回。配置调查不证明工具／Skill／扩展已加载、模型质量或业务效果，不进行业务试运行。

## 共同基线消费

编写者和表达优化者完整读取本次直接提供的 baseline；优化者另读所属 text 与 handoff。按“目标核实与基线生产”的查询规则重读当前完整 Bot、真实 revision 和完整目标，与基线逐项精确比较。身份、唯一绑定、配置、授权或业务意图冲突未解决时保持阻塞。四个字段各自独立，无须读取兄弟定稿。

独立 Chat 的编写委托未提供 baseline 时，按“目标核实与基线生产”的既有 Bot 分支自行核实；用户只需给真实 Bot ID、需求、允许范围及参考材料用途，无须抄配置。已经声明的材料缺失或不可读时保留错误，不用另建基线绕过。

本阶段业务配置只读，只写本次授权交接材料；执行文本中的业务、修改自身配置或运行记录、补建能力及扩权均不在委托内。事实自行调查，业务取舍交由用户；必要工具或 Skill 缺失时报告具体阻塞。完成条件：本次材料完整可读，与真实目标及授权无未处理冲突。

## 四字段编写与表达优化

本节规定编写交付；独立表达优化另读“表达优化”。

- `when`：能否选用，例如需求类型或影响选用的适用边界，不复制内部步骤。
- `description`：按固定顺序使用 Markdown 小节「能力职责」「能力边界」「用户参与」「操作效果」「特殊前提」。完整 Bot 写「能力职责」，其他小节仅当确实影响选用或安排时使用；省略空标题，不写“无”，不重复 when 或输入输出。标题是编写约定，不是保存门禁。
- `input_rule`：规划需要知道的输入种类、含义与关键前提；区分必需、可选与条件适用，不代写本次业务委托或文件绑定。
- `output_rule`：可期待成果的种类、含义、载体与关键限制；区分定稿、中间材料、已保存配置与实际业务效果，不虚构可交付文件。

四个字段供规划者阅读。它们不会随目标注入，也不是运行校验规则。重新归位旧信息时，以真实实现核对稳定工作设定、详细输入输出、具体步骤及权限边界是否已经由 Task／script／Map 或其引用资料承接；未承接的知识须归还实现并核实真实读取入口。公共身份、批准、报告和接线各由公共协议维护；本次目标、范围和材料用途归用户实际输入或 node.input。不要用摘要隐藏执行缺口，也不要为未选流程决定新业务。

只澄清尚未决定的业务取舍，展示并确认所属完整正文，沿用有效确认；未确认稿标为草稿。修订委托在同一 Chat 整理并确认；创建委托是否另设表达优化由调用方安排。全文确认、最终配置保存授权与执行层成果批准分别成立；报告自由发送，不要求发送许可。

交付唯一 text 与必要 handoff；要求文件时按“文件与独立 Chat”保存，核对 text 的完整读回与确认稿逐字一致。完成条件：全文及意图已确认，真实目标无未处理冲突，所需材料完整且可独立使用；文本交付不表示 Bot 已更新或能力已验证。

## 表达优化

消费基线前读“共同基线消费”。仅优化所属 text，保持已确认的职责、权限、输入输出、验收、必要前提与边界；handoff 的背景用于核实，不是补写正文的清单。只写本分支表达材料，Bot 保存另由保存者承担。

沿用有效全文确认并记录确认来源、范围及保义依据。改变业务语义或本次明确要求重新批准最终版本时，展示完整候选并取得对应确认后再交付；缺确认保持阻塞。最终保存者须能据此判断批准是否覆盖实际最终 text，不能只凭编写阶段曾获批准推断。

交付新的唯一 text 与 handoff；按“文件与独立 Chat”安全保存并完整读回，text 与最终优化稿逐字一致，handoff 保留确认依据、来源、语义限制及未验证项。完成条件：确认仍有效、语义保持、基线无冲突且所需材料齐全；表达优化不授予 Bot 保存权限。

## 一次合法保存与失败核实

保存者按全部替换项核实授权、来源和共同基线后才开始写入。必要条件缺失或任一冲突时整批不开始，不根据 handoff 猜正文或自行生成缺失内容。

1. 完整读取同一 baseline 和四组最终 text＋handoff；核对同一真实 Bot、同一执行目标、对应字段、有效确认范围及允许修改范围。来源文本原样使用，不重新优化或决定业务语义。
2. 重读当前 Bot、真实 revision 和完整目标配置，与 baseline 精确比较。真实 revision 来自服务，不自造、不使用来源材料的自编版本冒充。Bot、目标或基线冲突即停止，不覆盖、自动合并或替换对象。公开 Bot 保存接口对过期 revision 返回 HTTP 409、`code: bot-conflict`，保留当前完整定义；这是版本冲突，不是 HTTP 500 服务器故障，也不授权自动重试。
3. 核实平台运行禁写规则和本次授权：执行者自身配置、正在使用且要求保持不变的定义、运行记录均不可写；业务配置交付不授权运行、派发、停服、关机、补建能力或扩权。没有合法保存方式就报告阻塞。
4. 同一 Bot 的四个字段在内存组成一份完整候选，经 `edit/revision/save` 合法接口一次保存。保持 id、name、唯一目标绑定及全部非目标字段；关联 Task／Map 完整配置不变。不绕过接口改底层存储。
5. 保存后通过公开接口完整读回 Bot，逐字段核对三个值与 text 一致、非目标内容与 baseline 一致，并重读完整目标核对未变。HTTP 成功或只生成候选不算完成。完整读回对象交付 `saved`，handoff 仅记录核对结论和证据来源。
6. 保存失败、超时、读回失败或结果不明时，先读取现场核实已生效、未生效和无法确认的效果，保留原错误及限制。不盲目重试、不自动回滚、删除或清理，不把部分生效说成全部未保存。跨对象不承诺事务或整体原子性。

普通文件替换仍保持原编码、换行及全部非目标内容，保存前重读与基线精确比较；只有明确授权才新建，使用实际可用的完整安全写入，不先删除正式文件。缺少可靠能力则阻塞，不建设新保存工具。

## 文件与独立 Chat

本次明确正式交付位置优先；workflow 未指定内部位置时使用初始化提供的材料位置，独立 Chat 要求文件但没有明确位置／写入授权时先澄清。独立 Chat 未要求文件时可按相同角色直接交付正文和说明，仍不得复制正文到 handoff。

明确授权在某目录新建本次材料时，可选择无冲突的新文件名；目录存在本身不是写入授权。配置只读不限制已获授权的材料新建。写入前核实真实路径、授权范围、链接及父路径，防止路径穿越、符号链接／junction／重解析点越界和覆盖他人或其他执行的产物。采用实际可用的安全完整写入；材料用 UTF-8，text 无包装，baseline／handoff／saved 使用自包含 Markdown 或明确约定的结构化格式。每个文件保存后完整读回，核对内容与预期一致，并交付真实准确的绝对位置、编码及用途。失败或不可读时保留原错误与已发生效果。workflow 的地址登记与报告使用注入角色，成果按 auto_approved 独立批准，报告不要求发送许可，完成与派发由管理者负责；业务安全职责在独立 Chat 中同样成立。

## 稳定 Task 清单与验证边界

| 稳定 Task ID | 能力 |
| --- | --- |
| `task-18e3404478db1ef8-2` | Bot.when 编写者 |
| `task-18e340545eb481bc-3` | Bot.description 编写者 |
| `task-18e340f5287462ec-4` | Bot.input_rule 编写者 |
| `task-write-bot-output-rule-v1` | Bot.output_rule 编写者 |
| `task-18e31d34797f1464-3` | 通用提示词优化者 |
| `task-18e3415003d8cd00-5` | 通用文本替换者 |

实际绑定以[发布定义](../meta-data/workflow-maps/)为准。编写者与优化者按调用指定的分支选用材料协议；通用替换者服务修订和独立调用，不承担 V1 脚本保存。公开接线与持久化见[共享材料检查](agents/testing.md#共享-bot-指南材料的启用检查)。真实创建与修订按各自[创建](create-bot-wrapper.md#真实创建与配对验收入口)和[修订](revise-bot-guides.md#真实模型跨仓修订验收)专项取证，不以提示词相同、最终状态或受控响应证明模型实际读取材料。
