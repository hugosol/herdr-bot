# Map Run 运行说明

本页是 Map Run 执行、批准、报告、交接、故障处理与接入的参考。管理者和节点会收到本页与三份运行记录的绝对路径，按职责读取，不从 cwd 推导。静态定义见 [scheduler](scheduler.md)，长期需求与验收职责见[运行契约](workflow-contract.md)，验收入口与隔离要求见[测试指引](agents/testing.md)。

## 无 Chat 的执行与报告表示契约

本节细化 [ADR 0019](adr/0019-rust-script-bots.md)。用户维护的 TypeScript 脚本由已有 Bun 执行；两个发布实现为 `create-blank-bot.ts`、`save-bot-guides.ts`，业务规则不再编入 Rust。manager dispatch 保存独立 execution 后返回，后台执行并自动保存报告、尝试一次通知。V1 创建消费 request／可选 materials；V1 保存仅消费 baseline 与 when_text／plan_guide_text／node_guide_text，不消费三份指南 handoff 或人类批准凭据；各自输出 baseline 或 saved 及事实 handoff。完整协议见[共同业务材料](business-materials.md#创建分支-v1可校验表示与空白创建)。通用脚本只需返回状态和可选 outputs。编写与宿主操作见[动态脚本使用说明](usage.md#动态-typescript-脚本)。不创建 Chat、自动 complete 或派发下游；Run 持久化和显式管理流程保持不变。

### execution 与节点交接分别表达

- 脚本 execution 在原 `nodes[nodeId].execution` 位置使用 `kind: "typescript"` 和 `script` 相对路径。`id` 仍是真实独立执行 ID，归属由所在 Run／node 确定；保留冻结的 `zone`（真实本机绝对目录）、`inputRule`、`outputRule`、`inputs: [{id,path}]`、`autoApproved: true`、`approved`、`approvalSource`、`autoClose: false`。**不出现** `taskId`、`agent`、`chat`、`childRunId`，不填空白或假身份。没有 Session、Tab 或 Chat 可供恢复、定位或关闭。
- `lifecycle` 只描述本次脚本工作：开始为 `{status:"running",startedAt:<Unix毫秒>}`；成功结束为 `{status:"succeeded",startedAt,endedAt}`；失败结束为 `{status:"failed",startedAt,endedAt,failure}`。时间为非负整数，结束时间不得早于开始。成功要求脚本明确返回 success、正常退出，且全部声明输出真实可读。宿主在成功校验后记录 `approved=true`、`approvalSource=auto`，并提交 `canComplete=true` 报告；脚本失败不批准且报告 false。脚本无 `approve` 工具，不伪造 Chat。报告保存与通知反馈独立于业务写入，不能把通知失败当作业务未写入。
- 外层 `NodeState.status` 仍只有 pending／running／failed／completed，`failure`、`completion`、`previousExecutions` 保持原语义和归属。`lifecycle.status` 结束不会更改 node 状态。典型可观察组合：`node.status=running + lifecycle=succeeded + 当前canComplete=true` 表示等待 manager complete；`running + failed + 当前canComplete=false` 表示失败已报告、等待 manager 决定 fail；manager fail 后为 `failed`，显式 retry 才创建新 execution。不得把 succeeded 画成节点已交接，或从失败直接自动 retry。
- 脚本结束后 recover 拒绝；没有 Chat 不制造恢复入口。retry 保留旧 execution 和报告，新 execution 不继承旧输出或完成资格。暂停不取消已开始工作，断连不撤销已接受工作；重启旧 Run 保持既有只读语义，不自动修复 `running`、补结束或重跑。

### 脚本日志尾部读取

`POST /api/workflow/manager/script-output` 接受且仅接受 `{"sessionId":"公共 Session 提供的当前管理者身份"}`，复用 manager → 当前 Run 的归属校验，不接受调用者指定 Run／node／execution。返回 `{runId,observedAt,nodes}`；时间为 Unix 毫秒，每项为 `{nodeId,nodeName,executionId,status,lines,lastOutputAt}`，status 为脚本的 running／succeeded／failed，尚无输出时 lastOutputAt 为 null。只列当前 Run 同层的当前 TypeScript execution，不递归子 Map，不包含旧 execution。

后台持续读取普通 stdout 与 stderr，过滤 stdout 中的宿主协议，按接收顺序合并；独立管道不能保证原始跨管道写入顺序。尾部最多五个逻辑行，每行最多 512 字节，未换行片段也可见；处理跨块 UTF-8、CRLF 和终端控制序列。该预览不是完整日志、交付 outputs、报告或交接完成依据。静默不表示脚本停止，持续日志也不保证业务有进展。

读取不消费 collect 通知、不改状态，断开请求不取消脚本。尾部在后端内存中按 execution 隔离，retry、Run 结束或删除后不再保留原有尾部，重启不恢复。GitHub 克隆进度可在预览中看到，原有失败诊断不由预览替代。

管理者应直接调用原生 `collect`，不要通过 eval 或 codemode 嵌套调用。接入配套 Toolkit 时，等待中的工具结果区按节点显示状态、最后输出时间和尾部；无输出明确标示。Pi 在 Suite only 下仍可直接调用。结束时，工具保留最后快照和原有业务结果；日志只进入界面 details，不进入模型可见正文。取消等待或读取失败时停止观察，保留并标记最后快照，不自动重连或取消脚本。再次 collect 时读取当前 execution 的尾部。

### 脚本在途工作、断连与暂停

派发被后端接受并保存 execution 后，业务由后端持有，不依赖派发 HTTP 连接继续存活。调用方断连不能撤销它；不要为取得结果再次 dispatch。通过原 Run 详情独立读取原 execution 的 lifecycle、报告与真实材料。脚本的文件读写不长期占用 Run 操作资格：业务仍在途时，同 Run 的合法 status 与 pause 可以完成；这不改变完整派发批次本身的串行规则。

pause 只拒绝新的 dispatch／retry，不取消已开始的脚本工作；原 execution 仍能真实写入、完整读回、登记输出并自动报告。resume 只恢复派发资格，不自动派发 pending node、不重跑已有 execution、不重新通知已报告结果。报告仍须由 manager 显式 complete，不能把暂停或恢复当作交接。

脚本独立进程不提供业务回滚或重启恢复；后端退出后须核实原执行和可能仍在运行的外部效果，不能假定所有派生进程均已结束。重启后旧 Run 为 `managed=false`，保留已保存的 execution／lifecycle、报告与文件；在途时保存的 `running` 不补写成失败或成功。不自动恢复、补报告、重发通知或重跑业务。旧 manager 的运行管理动作仍按既有 `403 manager-unassociated` 拒绝，不能通过 Run resume 或普通 Chat Resume 取得写入资格；历史名称 PATCH 是原有独立例外，不是接管接口。需另起工作时先核实原文件效果，再显式发起独立新 Run。

### 失败、显式重跑与旧执行隔离

脚本失败报告不是 manager 的 fail 动作：先读取 `lifecycle.failure`、报告正文及实际文件，区分已保存的定义、已读回的配置、已生成的部分材料与尚未完成的步骤。业务结束后 node 仍为 running；直接 retry 返回 `node-not-retryable`，false 报告也不能首次 complete。manager 核实效果后以非空 reason 显式 fail，才记录节点执行阶段失败；fail 不更改脚本 lifecycle、不取消业务，也不回滚已发生效果。重复 fail 保留首次原因。

随后须明确 `confirmed=true` 和非空 reason，原 retry 门禁（Run running、输入已准备、非派发失败）继续适用。缺确认／空依据返回 `retry-confirmation-required`；缺必填参数拒绝解析，不新增 execution 或业务效果。合法 retry 重新解析当前定义与真实输入，创建独立 execution 和独占材料位置，`previousExecutions` 保留原 execution、manager failure、脚本结构化失败；旧文件与追加报告不删除。已创建的 Bot 不因材料失败变成“未保存”，不能靠重跑撤销它；修改请求须有对应授权，不能为求成功删除旧 Bot 或刷新旧 baseline。

脚本没有原 Chat：recover 返回 `node-not-recoverable`，不会启动新业务或转为待继续会话；Task／Map 有真实原 Chat 时仍按原 recover 规则处理。新执行从无报告、无 outputs 开始；旧 true 迟到仍归原 execution，不能成为当前 pendingReports 或 collect 待办。首次 complete 只看当前执行最后追加的报告，较早 true 后有 false 时仍拒绝；当前输出缺项不会从旧执行、其他 node 或其他 Run 补齐。同名输出 ID 在不同 execution 中可以对应不同真实文件，已派发消费者的地址不会随重跑改绑。

### 脚本报告已保存但通知失败

脚本没有等待通知回执的 Chat。管理者从详情或 status 读取已保存的报告，并按登记地址核对材料，再显式 complete。业务成功和通知失败可以同时成立；不为此重跑脚本、重新保存、重报或回滚。用 `workflow-report-delivery` 服务诊断按 Run、报告和目标 Chat 定位失败，解释见[节点报告与独立通知](#节点报告与独立通知)。

### 报告来源与兼容读取

脚本报告仍使用 `type:"report"`、`id`、`at`、`runId`、`nodeId`、`executionId`、`body`、`canComplete`、`outputs` 及真实 manager `to`。`from` 为 **null**，同时必需 `source:{kind:"typescript",script:"相对脚本路径.ts"}`；来源由后端凭真实 execution 生成，不能从 Session 查询、伪造或由外部调用者提交。`body` 为人类可读结果／错误／已发生效果说明，结构化失败保留在该 execution 的 `lifecycle.failure`。失败必须报告 false；成功 true 仍不自动 complete。输出只登记实际生成且完整读回成功的文件，ID 属于该 execution 冻结声明且单份唯一。

旧 Rust execution／报告按原 kind 与 rustScript 只读展示，不反写记录或保留执行分派。原 Task／Map execution 的既有无 `kind`、必需 `taskId`／`agent`、可空真实 `chat` 与可选 `childRunId` 继续读取，**不要求迁移或反写判别字段**；其报告继续读取既有真实 Chat `from`、没有 `source` 的格式，历史缺 canComplete 不补 true。新增脚本分支不能将所有旧记录误判为脚本，不能通过把身份字段普遍变可选来接受残缺 Task／Map 历史。脚本报告来源与 execution 类型／能力／归属不一致须拒绝，不授予当前执行完成资格。旧 Chat 报告仍按已保存的 `from` 读取，不因历史 execution／Chat 关联缺失或 Tab 字段不完整而使整个 Run 不可读；报告适用性仍严格按 node／execution ID 判断，新提交继续验证真实发送者。不得增加历史清理或第二份运行权威；本阶段不需要改动现有内部身份边界。

### 结构化失败与实际效果

通用脚本可返回 `failure` 诊断，不要求采用业务专用结构。发布的创建与保存脚本返回 `{stage,code,message,effects}`，字段和枚举见 [JSON Schema](contracts/script-bots.schema.json)，业务判据见[共同业务材料](business-materials.md)。`effects` 区分写入、完整读回和材料各自的成功、失败、未尝试或未知；已分配材料地址不等于已登记输出。unknown 不能解释成未写入，后续读回或材料失败不会撤销已有写入。旧 baseline 即便正文相同也不能代替当前 revision；不要自动刷新基线、重跑或回滚。

业务失败在已派发 execution 的 `lifecycle.failure` 和 false 报告中表达，不等于预检或 dispatch 拒绝。报告保存失败和通知失败另按报告协议解释。

## 流程标题

新 Run 在发起时冻结当前 Map 名称、从根到直接父级的名称路径和当前 Map 的 node 名称。根 Run 默认名称与管理者 Chat 名为 `[M]A`；嵌套为 `[S]B-A`、`[S]C-A/B`。普通及结束节点 Chat 使用 `节点名称-A/B/C`，脚本节点不创建 Chat。chief 委派的 Map 使用相同规则，独立 Chat 仍使用 Task 命名规则。

每个 Chat 的 Herdr Tab 与历史会话列表保存相同完整名称，不按固定字数裁短；显示空间不足时由各界面处理截断。名称仅供展示，不通过 `-` 或 `/` 解析身份。Map/node 后续改名不追溯改变该 Run 的命名来源；手动 Run 重命名仍可用，但不联动已有或后续 Chat 名称。旧 Run 缺少命名快照时保持原命名行为，不批量改写历史记录。

## 公开接口与持久字段参考

以下是公开 HTTP/文件 interface，不要求用户手填请求；日常操作从 scheduler 发起、workflow 查看，再在已关联管理者 Chat 表达意图。静态 Map/Bot 的 snake_case 格式见 [scheduler](scheduler.md#定义职责与唯一格式)；运行 HTTP 与记录使用 **camelCase**，不要混用。

workflow 区分当前定义与已保存执行。有执行时读 `autoApproved`、`approved`、`approvalSource`；未派发时只显示当前 `auto_approved` 模式，不显示已批准事实。`inputs` 是已解析的实际地址，`zone` 是本次保存的目录，cwd 不另存。旧执行缺字段时不要从当前定义补造历史值。

| 方法及路径 | 请求与结果 |
| --- | --- |
| `POST /api/scheduler/maps/{filename}/preflight` | `managerTaskId`、`zone`、`userInput` 三个字符串及可选 `fileInputs: [{id,path}]`；文件 ID 只属于当前 Map 的 `input_files` 接口，必需项可暂缺；未声明／重复 ID 或已提供文件不可读仍拒绝。请求的 `zone` 是稳定 ID，空要求传 `""`。整图检查成功返回 `workspaceName`，不创建 Run/Chat。 |
| `POST /api/scheduler/maps/{filename}/runs` | 与 preflight 相同的严格请求，重新预检。完成启动返回 HTTP 201 详情包；启动失败可返回 HTTP 500 详情包，含已分配 `id`、可得事实及 `startupError`，不是“没创建过”的保证。预检/角色入口失败在分配前拒绝；不能忽略非 2xx 响应正文或盲目再次发起。 |
| `GET /api/workflow/runs` | 返回 `runs` 数组及有效 `pollIntervalSeconds`；每项 `id`、可空 `run`、`managed`、`readError`。按创建时间倒序，同时间以 ID 排序；不可读项明确错误，不假造空 Run。 |
| `GET /api/workflow/runs/{id}` | 详情包 `id`、可空 `run`、`managed`、`startupError`、`readError`；可读 Run 另含完整 `reports` 与 `pendingReports`。必要记录不可读明确失败；仅有启动错误但当前记录不可读时仍可返回错误现场。 |
| `PATCH /api/workflow/runs/{id}` | 严格 JSON 正文 `{"name":"显示名称"}`；去除首尾空白后必须非空，允许重名。HTTP 200 返回与 GET 相同的更新后详情包。当前管理与历史未接管的可读 Run 都可重命名；不改变运行状态、执行身份或管理归属。非法正文/空名返回 HTTP 400（空名 `run-name-required`），读取/保存失败返回 HTTP 500、`run-read-failed` / `run-save-failed`。 |
| `POST /api/workflow/runs/{id}/locate` | `{}` 选择 manager；`{"executionId":"..."}` 选择指定执行，包括已保存的旧执行。只聚焦与保存坐标及原生身份同时匹配的目标。目标缺失、身份改变或结果未确认时如实反馈，不启动新 Chat、不切换同名终端，也不保证 Windows 窗口置前。 |
| `POST /api/workflow/manager/{action}` | action 为 status、prepare_inputs、dispatch、complete、fail、reconcile_dispatch、recover、retry、pause、resume、finish、collect；各自的严格业务参数及返回字段见下文，不是任意动作/状态修改入口。 |
| `POST /api/workflow/node/context` | 严格 JSON 对象只含 `sessionId`；同机来源及真实执行归属与 report 使用同一解析器，返回 `{runId,nodeId,executionId,autoApproved,approved,approvalSource}`。按匹配 executionId 读取冻结策略，包括 retry 前旧执行及合法子管理者／父节点双角色；不读当前 Map、不保存报告或触发通知。无关联／歧义／非法角色拒绝，不可读返回错误，调用方不得默认 auto。 |
| `POST /api/workflow/node/approve` | 严格 JSON 对象 `{sessionId,executionId,source}`；source 为 `auto` 或 `manual`。sessionId 来自公共 Session，executionId 由工具先调用 context 后捕获，source 由冻结 autoApproved 与原生确认结果选择，三者均不是模型可选目标。返回 `{runId,nodeId,executionId,autoApproved,approved:true,approvalSource,changed}`。不需要说明、报告 ID、成果版本、输出或文件；不产生报告、主动通知、complete、dispatch、finish 或重投。人工 HTTP 场景仅证明后端提交，不证明原生 Yes。 |
| `POST /api/workflow/node/report` | `{sessionId,body,canComplete,outputs?}`。`canComplete` 必须是布尔值；分开返回保存、通知与诊断结果。报告无需发送确认，不改变批准事实。详见[节点报告与独立通知](#节点报告与独立通知)。 |

仅 `github-repository-exploration.json` 的直接启动表单使用可选的 `githubExploration: {githubUrl, parentDirectory, explorationDirection}`。链接和克隆父目录须成对填写或成对留空；探索方向独立可空，可直接填一段文字。预检不写文件。启动时由服务端在选定的管理者 zone 下分配 `.manager-zone/.workflow/<run_id>/`，按需生成 UTF-8 `clone_request.json` 和 `exploration_brief.txt`，将实际绝对路径加入既有 `fileInputs`；不修改 `userInput`，不允许同一个 ID 同时绑定已有文件和生成材料。材料写入失败返回 `run-material-write-failed`，不启动管理者 Chat；可能留下已写入的部分材料。本 Map 的执行授权由其 `input_requirements` 约定：用户发起 Run 即授权本次已声明的工作，管理者核实材料后沿用授权，无需再次询问是否执行。缺项、用户明确限制、与既有要求冲突或范围外操作仍须澄清。此为提示词约定，不新增授权参数、持久化字段或后端门禁；`inputsReady` 仍由管理者完成输入审阅后通过 `prepare_inputs` 声明，不自动派发。Run 状态和历史仍保存在 `scheduler.run_directory`。

approve 与 context 均只接受同机来源及真实执行关联；结束节点无节点权限。无关联、歧义、非法管理者角色沿用 `node-unassociated`／`node-ambiguous`／`node-role-invalid`／`end-node-no-authority`（403）。非法正文、未知字段、数组、重复 key、查询参数为 400。批准 executionId 不匹配为 `approval-execution-mismatch`（400），已被 retry 替代为 `approval-execution-stale`（400）；终态为 `run-terminal`，不可批准阶段为 `node-not-approvable`，来源不符为 `approval-source-mismatch`（均 400）。持久化失败 `approval-save-failed`（500）保留原错误，不发布成功事实；读取失败为 `run-read-failed`（500）。同次重复批准返回 changed=false，不重复业务效果。manager complete 的未批准拒绝为 `node-not-approved`（409），约定交付未登记为 `output-missing`（409）；仍检查当前最新报告，先前 true 不覆盖最新 false。

管理者角色公开 `run`（按 action 选择下文状态动作）与 `collect`，节点角色公开 `approve` 与 `report`；HTTP 将 run 的各 action 分成上表路由，不表示另建一组可绕过角色约束的工具。运行角色传输请求中的 `sessionId` 由公共 Session 自动取得，模型仅给动作矩阵列出的业务参数。后端检查同机来源和已保存真实关联；不接受额外 Run/角色/node/接收者归属；仅 approve 的 executionId 是 context 捕获的执行绑定，不是模型选择。页面创建 Run 的配置请求与角色请求不是同一种身份协议。Map filename 和 Run ID 沿各自路径校验，不接受任意文件路径。

### 三份记录的字段

`run.json` 是一个完整对象；各 JSONL 文件每行一个完整对象，以换行结束。时间字段使用 Unix epoch **毫秒整数**，ID 是不透明字符串，不从名称/cwd/时间推导关联。以下可空项以 null 表示；初始化的节点执行、失败和完成均为空，旧执行数组为空。

| 对象 | 序列化字段及意义 |
| --- | --- |
| Run | `id`；可选 `name`（仅供人辨识的名称，未命名时省略，旧记录缺少此字段仍可读取）；可选 `naming`（新 Run 的展示名称快照：`mapPath` 为根到当前 Map 的名称数组，`nodeTitles` 为 node ID → 发起时名称；旧记录缺席不补造）；`map`（Map 文件引用）；`createdAt`；`managerTaskId`；管理者实际目录 `zone`、`workspaceName`；新 Run 的 `managerZoneId` 与 `zoneBindings`（ID → 本次本机路径）；原文 `userInput`；`inputsReady` 表示管理者是否已确认输入就绪（新 Run 为 false；历史记录缺字段也按 false 读取，不视为已验证）；独立 `fileInputs: [{id,path}]` 保存本次调用方或管理者准备的文件地址（省略可选项不生成条目，无输入为 `[]`，旧记录缺字段按空数组读取而不重写文件）；可空 `manager` Chat 引用；`status`；可空 `finished`；`nodes`（node ID → 节点事实）。不是整张 Map 定义快照；旧记录可以没有 managerZoneId 和 zoneBindings，保留本机历史，不从当前登记补造。 |
| Chat 引用 | `sessionId`、`workspaceId`、`tabId`、`paneId`、`agentName`；原生身份与实际宿主关联，不是新 Tab 即独立身份的证明。 |
| 节点事实 | `status`；可空 `execution`、`failure`、`completion`；`previousExecutions` 各项保留被替代的 `execution` 与 `failure`。可选 `autoCloseFailure: {code,message}` 保存该完成关联的关闭反馈，不是节点 `failure`。 |
| execution 共用字段 | `id`、执行目录 `zone`、可选 `zoneId`、`autoApproved`、`approved`、`approvalSource`（null、`auto` 或 `manual`）、`autoClose`、`endNode`（缺省 false）。`inputRule`、`outputRule` 是冻结规则，`inputs` 是解析后的 `{id,path}` 数组；新执行保存数组，未知历史值不从当前定义补造。派发和 retry 分别冻结批准模式、关闭与结束节点设置。 |
| Task 或 Map execution | `taskId`、`agent`（omp 或 pi）、可空 `chat`。Map execution 可含 `childRunId`，子 Run 可含 `parent: {runId,nodeId,executionId}`。无关联不证明没有外部启动效果。 |
| TypeScript execution | `kind: "typescript"`、相对 `script` 路径及 `lifecycle`，没有 `taskId`、`agent` 或 Chat。见[无 Chat 的执行](#无-chat-的执行与报告表示契约)。旧 Rust 分支仅作历史读取。 |
| failure | `stage`、`message`；启动失败可含 `effects`，执行阶段失败不制造启动效果。详细阶段以实际响应为准，不能只看 failed。 |
| effects | `recordsInitialized`、可选 `childRunId`、`childRecordsInitialized`、可空 `workspaceId/tabId/paneId/agentName/sessionId`、`agentStarted`、`associationSaved`、`inputAttempted`、`inputSubmitted`；尝试提交时另有 `inputOutcome`：`submitted` 为收到成功确认，`rejected` 为宿主明确拒绝，`unknown` 为超时、通信或响应不可理解，不能断言未发送。未尝试或旧记录缺此字段，不补造结果。只表示已观察阶段，不证明未确认阶段绝无效果。 |
| completion | `id`、`at`、真实管理者 `actor` Chat 引用、原文 `reason`；保留首次交接事实。 |
| finished | `id`、`at`、真实管理者 `actor` Chat 引用；结束节点交接时另有 `reason: "handed-to-user"`，普通完成省略；重复 finish 不覆盖。 |
| report | `id`、`type=report`、`at`、`runId`、`nodeId`、`executionId`、真实 `from`/`to` Chat 引用、`body`、`canComplete` boolean 及可选 `outputs: [{id,path}]` 实际地址数组；历史缺新增字段可读但不补 true。只在 messages.jsonl 追加，不包含已读或投递成功事实。 |

管理者、node 与 retry 的共同启动阶段统一为 `workspace`、`tab`、`start`、`identity`、`association`、`history`、`submit`；管理者另有记录初始化阶段 `initialize`，node 另有定义解析阶段 `resolve` 与提交运行状态阶段 `state`。其中 `history` 指提交首次指示前写入 **Chat 历史会话列表**，不是追加 Run 的 `history.jsonl`。管理者在此失败也返回 `stage=history`（不再归入 `association`）；已保存的真实关联和 `associationSaved=true` 保留，`inputAttempted/inputSubmitted=false`，不会提交首次指示。node 与 retry 同样保留关联和失败事实；retry 不覆盖旧执行。Run 执行历史追加失败仍使用既有 `historyError`／部分生效反馈，不等同于这个启动阶段。

`history.jsonl` 保存按动作追加的执行/状态事实，不是重放协议：公共说明为操作 `id/at/actor/action`、目标 `nodeId`（节点动作）、`before/after`、说明 `reason` 及相关执行/失败引用。不同动作按需带 `execution`、`failure`、`previousExecution/previousFailure`、核实 `finding` 等；不要求所有事件有全部字段，不用历史字段缺席反推当前状态。具体 complete/fail/recover/retry/reconcile_dispatch/pause/resume/finish 的内容见对应章节。通知代次、collect 检查位置、managed、readError 和 manager startupError 都不是第四份持久业务记录。

当前事实读取以 `run.json` 为准，`reports` 为全部追加报告，`pendingReports` 为 node ID → 当前适用报告 ID。status/动作使用 `snapshot` 而非详情的 `run` 字段；collect 不带报告正文。HTTP 成功码不能掩盖 `historyError` 或逐项失败，错误码、已生效字段和已发生效果必须一起解释，见[部分提交](#独立读取与部分提交)。

## 动态 node zone 派发

Map node 的 `zone: {"type":"dynamic"}` 声明每次执行由 manager 选址，不包含预设 ID。固定 `zone` 字符串维持 Run 启动时的绑定快照；动态位置不要求在启动 Run 时已存在。

`dispatch` 保留 `nodes` 数组，并可携带 `zoneIds: {"nodeId":"已登记 zone ID"}`。每个选中的动态节点必须有明确值；固定节点或未选节点不得提供覆盖。`retry` 对动态节点必须另传 `zoneId`，不能继承前次值。manager 从上游 report 等工作信息作出选择；系统不提取报告文本、不自动创建或登记 zone，也不自动派发。

公共派发入口按当前 zone 登记校验 ID 和目录，冻结到本次 execution 的 `zoneId` 与 `zone`，再进入 Task、脚本或子 Map 分支。缺值、未登记或目录无效时拒绝，不启动对应业务执行；既有执行不跟随改绑，重试可以显式选择另一个位置。动态选择不写回 Map 或 Run 的固定 `zoneBindings`。历史 execution 缺少 `zoneId` 时保持未知，不从现有定义猜测。

`recover` 仍只恢复原执行跟踪；Run `pause`／`resume` 仍只控制新派发资格，不重选已有执行位置。子 Map 的选址只决定子 manager zone，内部 node 独立遵循自身定义。`input_rule` 文件来源保持原语义，`type:file` 不引用动态 node zone，上游输出依然按报告登记的实际绝对文件地址交接。

## Map 型节点与子 Run

Map 型 Bot 只保存 `map_file`，不固化管理者。具体 node 的 `manager_task_id` 必须引用 manager Task；派发使用该 node 的 `zone` 和原始 `input` 创建独立子 Run。内部节点仍使用子 Map 各自的 zone，不展开或复制进父图。同一 Map 的每次调用、每次明确 retry 都有独立 Run 和 execution，不能按 Map 名或 node 名猜测归属。

父 execution 的可选 `childRunId` 与子 Run 的可选 `parent: {runId,nodeId,executionId}` 保存双向执行关联；普通执行／根 Run 省略它们，旧非嵌套记录仍可读取。两侧真实 Chat 引用相同、来源 Task／agent 一致且关联唯一时，该子 manager 才同时拥有子 Run 的管理者与父 execution 的节点角色。OMP／Pi 启动装配各自的 manager-chat 和 node-chat；没有新增 Task 类型或普遍放宽身份唯一性。`run`／`collect` 只定位子 Run，`context`／`report` 只定位关联的父 execution，包括保留的旧 execution；`approve` 也只批准真实父 execution，但旧子 manager 不能批准 retry 后的新 execution，旧执行的批准请求按陈旧执行拒绝。批准请求的 executionId 只校验已取得上下文，不能选择其他归属。请求不能指定其他 Run／execution／接收者来选择身份；同 Run 混用、缺少／错误／多重关联和复用原生 Session 仍拒绝。

父节点本次已经解析的 `inputs: [{id,path}]` 按**本地 id 同名**作为子 Run 的 `fileInputs`。例如上游输出 `upstream-result` 被父 node 规则绑定为 `child-material` 时，子接口必须声明 `child-material`，而不是上游输出名。每层只解析所属 Run 的输入；同名跨 Map／Run 不串材料。空接口和可选缺席合法，不生成占位路径；即使子接口必需项暂缺，也可启动子管理者；子 Run 同样从 `inputsReady=false` 开始，须由子管理者澄清、补齐并确认后，才能派发任何内部业务节点。父 execution 已冻结的 inputs 不因子 Run 后续绑定而改变。

### 从 workflow 查看子 Run

workflow 的全部 Run 列表将根 Run 与子 Run 并列列出；Run 名称、Map 标题和 node 标题都不是导航身份。打开父 Run 后，当前节点的“当前执行事实”内以该 execution 保存的 `childRunId` 提供“查看子 Run”；展开“旧执行历史”后，每条旧 execution 使用自身保存的 `childRunId`，retry 后不会改指最新 child。目标仍是独立的 `#workflow/{id}` Run 详情，可手动“刷新 Run”并返回全部 Run。整页刷新沿用主界面默认进入 chat 的规则，再进入 workflow 并从列表选择相同 ID 即可重读原现场，不创建新 Run；这里不提供嵌套树或额外双向导航。

执行未保存 `childRunId` 时明确显示未保存关联，不从当前 Bot 的 `map_file`、名称或 Chat 猜测子 Run。目标记录缺失或不可读时，详情查询显示公开读取错误，不跳转到同名 Run、不修复记录，也不重新发起 Run／dispatch。查看、刷新和浏览器导航只读取保存事实，不恢复 Run 管理资格。

### 局部暂停、失败与并发推进

父子 Run 是独立控制边界。父层 `pause` 仅拒绝本层新 dispatch／retry；已启动的子 manager 和子内部工作仍可显式派发、报告、验收及 finish。`resume` 只恢复本层资格，不自动重派失败工作或派发 pending node。子内部派发／执行失败保留在所属 Run，不自动失败、暂停或终止祖先、兄弟分支或独立 Run；现场处理仍须由所属 manager 显式决定。

慢宿主输入与报告通知不应阻止其他 Run 推进，也不应使父子交接互等。报告先保存，再尝试通知；通知尚未返回时，管理者仍可依据已保存的有效报告显式 complete／finish。父 dispatch 尚在启动子 manager 时，子内部工作可推进；向父层的 report 可能等待该父层动作提交，但不能因此堵住子层 finish 或独立 Run。这里不承诺自动恢复、全局吞吐或固定锁结构。

### 子流程交付与父层验收

子 manager 的 `report` 登记的是**父 execution 冻结的 `outputRule`** 中的输出 ID 与实际绝对文件地址，不从内部 node 的同名输出自动取值，也不新增 Map 输出声明。未知／同次重复 ID 拒绝整次报告；增量报告按同 execution 的追加顺序合并，同名更新，省略保留既有登记，未登记不补造路径。普通及 Map 型下游都通过父 node 的 `type=node` 输入规则解析这些地址；只用当前 execution，旧尝试即使曾交付也不能补齐新尝试的缺项。上游 completed 只满足依赖，所需输出未登记或文件不可读时派发仍在 `resolve` 阶段失败，不创建下游 Chat 或子 Run。

子 manager 尚未显式 `finish` 时不能外报 `canComplete=true`（HTTP 400，`child-run-not-finished`）；`false` 进展报告仍可保存。读取关联子 Run 失败不当作已结束。`finish` 要求内部普通工作节点交接完成；指定结束节点只需成功派发，不等待其业务完成。它不要求父输出全部登记。子 manager 也必须等到子 Run 显式 `finish` 后，才可通过 `node/approve` 独立批准父节点整体成果；提前批准返回 HTTP 400 `child-run-not-finished`。父 execution 的 `autoApproved` 在本次派发时冻结，不随当前 Map 改动：自动模式由子 manager 主动提交 `source=auto`，人工模式须经原生 Yes 后提交 `source=manual`（HTTP 本身不证明原生确认）。内部节点的批准、交接与子 Run 完成都不会批准父 execution。

成果批准、报告、已登记材料及父层业务验收相互独立：子 `finish`、批准和任何报告都不完成父节点，也不自动放行或派发下游；父 manager 须核对当前 execution 已批准、**最新有效报告严格 true** 及全部冻结输出 ID 已登记后显式 `complete`，后来的 false 不能回退较早 true。报告发送不再要求独立发送审批，也不产生批准事实。

报告保存与宿主通知分阶段反馈。即使真实通知失败，`saved=true` 的报告、完成的子 Run 与已有文件仍可由公开详情读回；不要为通知失败重报、重派、回滚或重新 finish。父节点首次合法 complete 后，才按该 execution 冻结的 `autoClose` 尝试一次精确关闭其关联子 manager Tab。子 finish 不关闭父执行的 Tab；重复 complete 不重复关闭，也不扫描或级联关闭内部 Tab。关闭失败不撤销已保存的父交接。

子启动在既有阶段外公开 `child-preflight`、`child-register`、`child-initialize`、`child-association`。`effects.childRunId` 表示已经保存的父执行指向，`childRecordsInitialized` 表示三份子记录完成初始化；随后仍逐项报告真实 workspace／Tab／agent／身份／关联／提交效果。已登记的子 Run、已启动的 Chat 和已保存的另一侧关联可能在后续阶段失败后保留；这不是跨文件事务，不会自动回滚、重发或重派。应读取逐项结果、父 execution、子 Run 详情与 `startupError` 核实现场，不能把 failed 解释为未创建。子 Run 的结束、对外报告与父 manager 的交接仍是不同动作。

### 独立 retry 与迟到旧报告

执行阶段 failed 后，管理者必须先核对旧 Chat、子 Run、部分材料与外部副作用，再以 `confirmed=true` 和非空 `reason` 明确授权 retry。未确认／无说明返回 `retry-confirmation-required`，不创建新身份或调用宿主。合法 retry 新建 execution、child Run 和原生 Chat，旧 execution／failure 留在 `previousExecutions`，旧子 Run 的三份记录及外部文件不覆盖、不撤销。`recover` 只恢复原 execution 的跟踪；普通 Chat Resume 只定位／恢复原生对话，两者都不创建替代 child，也不重放初始输入。

派发阶段失败不能套用执行 retry：已有 child 登记即使没有 Chat 也不是未创建；`reconcile_dispatch=not_created` 拒绝清掉已知 child（`dispatch-child-already-known`）。输入实际接受但响应损坏属于 `inputOutcome=unknown`；核实和确认参数均不赋予盲目重试资格，retry 仍返回 `dispatch-reconciliation-required`。先按下一节核实真实部分效果。

旧子 manager 继续管理自己的旧 child，仍可向原父 execution 保存报告并通知原父 manager；这保留通知语义，不使报告适用于新 execution。`pendingReports`、`collect`、首次 complete 的严格 true 门禁以及下游文件解析均按当前 execution 隔离。新 execution 尚无报告或最新为 false 时，晚到的旧 true 不放行；新 execution 缺少输出时，不从旧报告、内部同名 node 或其他 Run 补齐，即使旧文件仍真实存在。

### 子启动失败的核实与验证

先读取 dispatch 的逐项 `failure.stage/effects`，再读取父 `execution.childRunId`、子 Run HTTP 详情／列表与独立磁盘记录，最后核对实际宿主 Tab、agent 和提交效果。`child-initialize` 失败可能只有持久父执行指向，子三份记录尚不可读；当前进程列表仍保留这个 child 的 `id` 和 `readError`，详情保留 `startupError`。没有 Chat 不授权删除该指向、自动重建、重派或回滚；核实只读宿主身份，必要时保存原两侧关联。`inputAttempted=true/inputSubmitted=false` 不等于未发送：按 `inputOutcome` 区分明确拒绝与结果未知，不能凭已有 Chat 抹掉该区别。失败的宿主动作和初始输入不因核实重放。

## 发起与观察

根 Run 的 Map 文件输入声明与本次实际文件绑定分开维护，操作见 [Map 命名文件输入](scheduler.md#map-命名文件输入与根-run-绑定)。本次要求只传递业务文字，不编码 Run／execution 身份或文件发现协议；文件绑定在运行背景的 `fileInputs` 独立显示，重新进入详情及后端重启后仍读持久事实。未提供即缺席，包括必需文件尚缺时也可启动管理者；不会生成占位地址或默认路径。已保存地址不承诺源文件正文不再变化。

在 scheduler 选中 Map，打开「+ 发起 Run」，选择 manager 类型的 omp 或 pi Task 与已登记 zone。Workspace 名称来自管理者 zone 的目录名，不退回默认或当前 Workspace。可先校验配置；「启动 Run」会重新执行同一整图预检，然后创建独立 Run。三份 Bot 文档允许空文本，不是业务质量门禁。

启动前请确认角色 Task 的固定扩展入口与所选宿主匹配，已在部署的 my-agent-toolkit 中交付并可加载；各宿主不自动续接其他会话，且管理者能按绝对路径读取 Map、本说明及运行记录。运行期间保持定义与 Task 配置不变。本服务对已提供的实际文件输入在根启动、输入准备与节点派发前检查存在且可打开；管理者另行负责输入语义核实，服务端不自动探测业务内容正确性或持续可用性，也不修改原生会话目录。缺失扩展入口会明确拒绝启动，不用空扩展代替。

当前交付目录也将 auto `connect-bot` 作为无角色限定的固定扩展，manager 与 node 会一并加载并尝试面板登记；它不替代角色扩展的 Run 关联和协议，角色通信仍不依赖 connect-bot 是否连接。固定入口与适用角色统一在 Suite 页面只读查看，未选择 Suite 也加载。下文关闭 connect-bot 的专项验收用于证明角色通信的独立性，不代表当前默认禁止加载它。

新启动的原生会话身份及其独立性由宿主保证。首次节点派发、合法 retry 和嵌套 Map 节点启动均不扫描历史 Run 排重；保留官方身份来源、类型、路径／文件头及 cwd 校验，保存当前执行关联后才提交 Task.prompt 与 node.input。无关历史 Run 不可解析不阻断该启动路径；不因此迁移旧记录、补批准字段或自动恢复失败执行。已有 Chat 恢复、派发核实及报告／批准等操作的身份归属校验保持不变。

Herdr 确认 `agent start` 不代表已经发布原生身份。对刚启动的管理者或 node，后端在同一次 `identity` 阶段的 **30 秒总期限**内，仅当 `agent_session` 缺失或为 null 时，每隔 100 毫秒只读查询同一个 agent；身份出现后才校验并保存关联、提交初始化输入。命令失败、响应结构错误、身份不匹配或原生文件错误不会被当成“尚未就绪”反复查询。期限耗尽则报告 identity 失败并保留已发生的外部效果，不再次启动、不补发输入，也不自动接管旧失败 Run。

新启动的 omp 可能已分配原生身份、但尚未创建会话文件。herdr-bot 仅在官方 `herdr:omp` 上报绝对原生路径、实际文件打开为 NotFound、文件名符合 omp 的时间戳与 UUID 约定且有实际进程 cwd 时，接受路径中的身份；文件存在时仍读取文件头，权限、损坏及非标准路径错误不降级。管理者、节点和节点 retry 均先保存身份关联与历史，再提交初始指示；不会为取得身份提前发送任务或创建原生文件。身份及历史中的预期文件路径不代表已经落盘或当前可恢复，信任边界见 [ADR 0014](adr/0014-unpersisted-omp-startup-identity.md)。没有后台落盘轮询、自动修复或对旧失败 Run 的补关联。

管理者 Task 的固定 prompt 定义职能与限制；初始 Skill、model/thinking 和固定 prompt 按 Task 原配置生效。本次输入提供 Run ID、Map 路径、本说明、三份记录的路径及管理者 zone 下 `.manager-zone/.workflow/<Run ID>` 的中间资料位置，最后以独立段落附上非空本次要求的原文。空要求不附段落。Bot 文档、完整 Map 正文与 node.input 不隐式注入管理者。

取得 Run ID 后进入 workflow 详情 `#workflow/<Run ID>`。列表跨 Map 列出 Run，按创建时间倒序并以本地日期分组。每行显示时间、名称或 Map 标识、Run ID、状态及交接数 `n/m`。搜索匹配 Run 名称、ID 或 Map 标识，可与 Map、状态和管理归属筛选叠加。尚无 Run、无匹配、记录读取失败与时间未知分别表达。管理归属在详情中与生命周期状态分开。

点 Run 只打开保存的现场，不进入 Map 编辑或触发工作。进入详情时隐藏列表，返回全部 Run 或浏览器后退恢复列表条件和滚动。切换功能页再返回时仍显示选中的 Run。**刷新 Run** 只重读列表和所选现场，不创建 Run 或 Chat，也不重试任何运行操作。

workflow 仅在该入口可见且浏览器文档可见时轮询，默认在上一轮查询结束后等待 **5 秒**；离开 workflow、打开 server 面板或隐藏浏览器文档时暂停，返回可见时立即查询。一次刷新按顺序读取列表、所选 Run 和当前定义/名称目录；慢响应期间不启动下一轮，重复手动刷新合并到在途查询，不取消后重新发起重叠请求。离开前已发出的查询可以结束，但旧入口或旧 Run 的迟到结果不会覆盖新选择。

列表或 Run 查询失败时显示「更新失败」及最后一次完整成功查询时间，保留上次成功的列表、详情和图，不将 Run 改成 failed，也不显示假空列表。没有成功查询时明确显示尚无成功查询。可见时后续轮询或手动刷新成功后更新现场并清除更新错误；这是只读查询，不是重试派发、report、collect 或任何运行操作。当前 Map 本身缺失、非法或无法对应时仍遵守下文的不伪造历史图规则。

用节点列表或只读图查看详情，窄屏用节点选择框。刷新不自动切换仍存在的节点。当前报告、当前定义、已保存执行、完成说明与旧执行分别显示；在「运行背景」查看本次要求、管理者配置与真实 Chat。图只画定义中的真实依赖，不替独立节点造边。

只读图和 scheduler 共用依赖层级排序：前置层在前，同层沿当前服务返回的定义顺序稳定排列；图从左到右表示依赖，箭头从前置 node 指向依赖它的 node。此顺序不是执行调度顺序，不改写 Map 文件，manager 不作为 node。图、节点列表和详情展示 Task 名称：有当前执行时以保存的 execution.taskId 为准，否则使用当前 Bot 绑定；名称来自当前 Task 目录，名称不可读时显示标识，不冒充历史名称快照。

图中同时使用状态文字、独立符号与对应文字颜色，不依赖卡片整块填充表示状态；运行节点的独立箭头只在 Run 由当前后端管理且处于 running 时播放纯 CSS 动画，未接管或暂停时静止，减少动态效果设置下停用动画。选中态另用静态外框，不按 Chat 在线或工作状态推断完成。pending 节点仅按当前 Run 状态与当前定义的依赖显示 ready/blocked；这只是读取时的说明，不自动派发。节点详情继续区分「当前定义」的 Bot/Task、zone/input、前置依赖与阻塞原因，以及「已保存执行」的 Task/zone/真实 Chat 标识、失败阶段及效果、交接完成说明/时间和当前执行适用报告正文/时间。没有执行或 Chat 关联时明确提示，不隐式补建。

节点详情的交接字段均只读：当前/旧 execution 显示保存的 `autoApproved`、`autoClose`，并分别读取 `approved` 和 `approvalSource`，不取当前 Map 覆盖执行设置。保存的执行 `zone` 单独显示；实际 cwd 未单独持久化，页面仅显示「未单独记录」，不把当前 Map zone 当成执行事实。新执行保存派发时的 `inputRule`／`outputRule` 和解析后的 `inputs: [{id,path}]`，页面分别展示冻结规则与实际输入，不从当前 Map 或后续报告重建。历史迁移不能恢复的规则或实际输入明确标为未记录；当前 Map 的 `input_rule`／`output_rule` 仍属于「当前定义」，不是历史事实。

当前执行累计 `outputs` 仅合并同 node、同当前 execution 的报告，遵循 `messages.jsonl` 追加顺序（不是 `at` 时间排序）：同名后报覆盖，省略名字或整个 outputs 保留已有登记，不继承旧 execution 或其他 node 的地址。最新 `canComplete` 只读最后一条适用报告，区分 `true`、`false`、字段缺失及无报告，不回退到较早的 true。每份当前/旧报告单独显示其 `canComplete` 和本次提交的 outputs，不把累计值冒充该报告的提交值。这里只展示登记地址，不读取或验证业务文件；可交接声明不等于管理者 complete。

图始终注明来自**当前 Map，非历史定义快照**。当前 Map 丢失、不可解析或 node 标识集合无法与 Run 一一对应时，不画图也不按新名称误配旧 node；保留本次要求、保存的 Run/node 状态、失败、完成、当前/旧执行及报告与 Chat 关联。名称不可读时显示保存标识，不伪造过去的 Bot/Task/输入定义。运行期间仍应保持定义不变：同标识的定义修改不代表已保存历史定义，页面只将其标为当前定义。

首次打开 Run 链接、切换入口后返回、手动刷新只读取事实。浏览器整页刷新遵守[页面导航](usage.md#页面导航)规则回到 chat，保留 URL 指定的导航布局；重新进入 workflow 查看记录不会重新启动 Run。本次要求不可编辑；详情中的 Run/node 状态不是 Chat 在线或轮次状态。没有启动进度页、状态操作按钮或日志浏览器。需要核实、修复、恢复跟踪或交接时，请向已关联管理者 Chat 表达意图；报告本身不完成 node，普通 Chat Resume 不接管旧 Run。管理者、当前执行与默认折叠的「旧执行历史」保留真实 Chat 关联并提供 Resume；旧报告单独显示执行 ID、来源 Chat、正文与时间，不混入当前执行适用报告。旧报告的来源 Chat 也可独立 Resume。无执行或 Chat 关联时只说明缺失，不创建执行。

Resume 复用 chat 历史入口的实际定位／恢复：按所选原生 sessionId 查找，不按同名 Task、旧 Tab/pane、当前选中项或 cwd 猜测。已运行的 Chat 导航到 Herdr 内部焦点；需要新进程时加载指定原生对话并读取**来源 Task 最新 env config**，不重放初始 Skill、model/thinking、固定 prompt 或本次 input。恢复不是 recover、retry 或 Run 的 resume 状态动作，不改变节点状态、报告适用性或「历史现场 / 未接管」标记，也不恢复管理者写入资格。当前 Map 丢失、损坏或 node 无法可靠对应时不补造历史图，但这些已保存关联与折叠历史仍可使用。

定位／恢复失败与结果未知使用既有错误反馈：已发生 Tab/启动效果不假报回滚，已提交启动不等于已确认加载同一 Chat；未知时先核实 Herdr，不自动重试、再次启动或补发输入。页面成功仅表示响应中确认的 Herdr 内部导航，不保证 Windows 窗口置前或任务已继续。用户需要交接或修复时，在已确认进入的管理者 Chat 表达意图；角色指引不得把普通 Chat Resume 当成自动派发或接管许可。

## 记录及配置

沿用服务 TOML 的 `[scheduler]`：

```toml
[scheduler]
map_directory = '${roots.project}/meta-data/workflow-maps'
bot_directory = '${roots.project}/meta-data/bots'
run_directory = '${roots.data}/workflow-runs'
run_instructions = '${roots.project}/docs/workflow-runtime.md'
workflow_poll_interval_seconds = 5
```

`workflow_poll_interval_seconds` 是服务启动配置，默认 5 秒，允许 1..=4294967295 的整数秒；零、负数、小数或字符串会让启动失败。修改后重启服务，页面从 `GET /api/workflow/runs` 的 `pollIntervalSeconds` 读取有效值；没有页面设置、推送或独立配置平台。间隔从上一轮完整查询结束起算，慢响应不追赶漏掉的 tick。

普通相对目录在服务启动时按服务配置文件所在目录解析一次；上例的命名根来自顶层 `paths_file` 指定的本机配置，详见 [本机根目录](usage.md#本机根目录与解析规则)。向 Chat 提供绝对路径，不按 Chat 的 zone/cwd 重新推导。默认 Run 根为 `.data/workflow-runs`，默认 `run_instructions` 为 `docs/workflow-runtime.md`，均相对服务配置目录，不依赖编译机器目录。修改根后须重启。

每个 Run 目录只有三份业务记录：

- `run.json`：当前已提交事实，包含 Map 引用、时间、管理者 Task/zone/Workspace、本次要求、管理者真实 Chat 关联及节点状态。
- `history.jsonl`：追加执行/状态历史；新 Run 初始为空。
- `messages.jsonl`：追加报告；新 Run 初始为空。

### 执行宿主字段

当前与旧的 Task、Map execution 都要求 `agent`，不根据管理者宿主或后来的 Task 补造；缺失时报告读取错误。脚本没有 `agent`，不伪造该字段。retry 按当前定义解析新执行，保留旧执行原值。

### 独立读取与部分提交

三类记录的读取入口就是初始指示给出的三个绝对路径：`run.json` 直接解析为完整当前事实，`history.jsonl` 和 `messages.jsonl` 逐行读取为追加历史和报告；不要通过重放历史或取最后一份报告来重建当前状态。`GET /api/workflow/runs/<Run ID>` 的 `run`/`reports` 提供当前事实与完整报告，管理者 `status` 另给派生准入及 `pendingReports`。历史从已有 `history.jsonl` 路径读取，不另建日志浏览 API。当前失败原因、首次完成说明与 `previousExecutions` 中的旧执行/失败仍保存在当前事实中；历史保留失败、recover、retry 和完成时的原因与说明。旧执行报告可晚到，但不能替代当前执行的交接依据。通知代次及待办索引是内存/查询派生值，不落成第四份业务记录，也不是已读或完成凭据。

同 Run 状态动作重读当前事实，先提交事实，再追加历史。`reconcile_dispatch=uncertain` 只追加核实历史。启动、派发、retry 的 Chat 关联与初始指示是不同阶段；报告保存与通知也是不同阶段。

当前状态写入失败时，动作报错且不发布候选状态、不追加成功历史；应先查询已提交现场并排除文件问题。dispatch/retry 如果已经创建 Chat，其 `effects` 与 `persistenceError` 仍如实报告外部效果，不表示 Chat 已撤销。

当前状态提交后，仅历史追加失败属于**部分生效**：非批次状态动作的响应、dispatch 的每项 `results[]`、retry 的 `result` 都保留 `historyError` 原因，并明确返回 `code=history-append-failed`、`stateCommitted=true` 和“当前状态已提交，但执行历史追加失败；状态未回滚”的说明。HTTP 200 在这里表示动作结果可解析，不表示所有记录都写入成功；调用者必须同时检查这些字段。批次仍处理其他合法目标。不得因为这一次错误自动封锁 Run、回滚事实或自动重试；当前磁盘状态及后续查询均保持已生效结果，排除文件问题后可显式执行下一次合法动作。重复已生效动作不会补写缺失的首次历史。若历史尾部残缺，仍拒绝向残缺记录追加，不能自动截断“修复”。

用户应从 workflow 进入已关联的管理者 Chat，表达“检查当前事实与历史写入错误，核实后继续”，不要手填身份参数、修改运行文件或盲目重派。manager-chat 的公共 Prompt 必须解释上述已提交/未提交区别、读取三份路径与检查每项结果的责任；node-chat 应继续按真实执行身份报告，不把历史错误当作回滚或重做指令。本文描述的接口位于后端 HTTP 和三份持久记录；外部角色扩展的实际部署/Prompt 注入仍须独立 toolkit/Herdr 证据，不能以 HTTP 验收冒充。

正常顺序为整图预检 → 三份记录全部初始化 → Workspace/Tab/agent 启动 → 获取原生 session_id → 持久保存 Run 的真实关联 → 提交初始指示。单文件更新使用同目录临时文件后替换，不先删除正式文件。不承诺三文件事务、断电恢复、多进程写入协议或外部 Chat 回滚。

### 删除历史与父子引用完整性

Map 的“删除历史”在移除任何记录前扫描完整 Run 关联，保护 `parent` 与当前／旧 execution 的 `childRunId` 两个方向；仅一侧存在也足以拒绝，即使两侧都属于同一张 Map。错误定位父 Run／node／execution／child Run，不按标题、Task 名或目录名猜测关系。不能通过修改当前定义、清空运行引用、级联删除或回滚外部 Chat 绕过保护。无法可靠读取的关联拒绝清理；无关联旧路径仍可清理损坏的 JSONL 详情，保留既有路径与链接边界。

child 的父关联发布／初始化与删除扫描在同一进程内协调；协调不跨外部 Herdr 调用。删除先完成时，后续 child 创建和迟到启动保存必须拒绝，不重建已删除记录；关联先存在时，删除须拒绝。该规则不新增多进程协议、跨文件事务或自动修复。

### 损坏记录与 Windows 文件协调

三份记录都是必要文件。缺失不会被当作空文件重新创建；当前记录不可解析、报告结构损坏、历史不是 JSON 对象或 JSONL 最后一条没有换行时，相关读取/追加明确报错，错误包含文件路径。详情读取同时检查历史记录是否可读，但不重放历史；管理者 status 的当前事实仍以 `run.json` 为准，不依赖历史推导。运行身份扫描无法读取记录时返回 `run-read-failed`，而不是把文件故障伪装成身份未关联；另一个可正常识别的独立 Run 仍可工作。旧 Run 未接管规则不变。

历史的 dispatch、retry 和状态转换追加，以及报告追加，均在同一次文件集合协调内先检查已有完整记录，再执行追加；不向残尾或损坏 log 继续写，不自动截断、补换行、重建空数据或重试。若损坏是在外部启动等待期间出现，已发生的 Chat/已提交状态不能回滚；历史失败仍按上面的 `stateCommitted`/`historyError` 部分生效反馈处理。文件拒绝访问、只读或 Windows 不允许替换时，正式旧文件保持原字节；排除访问问题后须由调用者显式发起下一次合法操作。

协调仅在当前进程内，统一持久化 module 一次取得整组路径的协调资格，目录范围与后代文件范围相交时互斥；读取、初始化、整体替换和追加均通过受限访问对象进行，不向业务暴露文件锁。不相交的 Run 文件集合不会互相排队。文件访问锁不跨外部 Herdr 启动、初始输入、报告投递或 collect 等待；同 Run 的业务串行边界仍覆盖整个派发批次，不能把文件可读误解为状态操作可越过批次。没有新增跨文件原子、断电安全或多进程写入保证。

用户遇到损坏提示，应保留原文件及错误路径，从 workflow 进入原管理者 Chat 表达“先核实保存事实、文件故障和已有外部效果，不要重派或修改记录”。不要手填身份、编辑运行 JSON、删除坏行或用空文件覆盖；部署维护者应先备份原字节，再依据可信记录处理存储故障。manager-chat 指引应明确：读取失败不是空 Run，历史追加失败不是状态回滚，先查已提交事实并说明未知项；node-chat 遇到报告保存失败须如实说明未保存，不宣称通知成功，也不因投递失败重复报告。角色 Prompt 的真实注入仍需外部 toolkit/宿主证据，本段不是替代验证。

## 基础管理者状态协议

`POST /api/workflow/manager/status` 的传输正文仅为 `{"sessionId":"<当前真实原生身份>"}`，返回 `snapshot`，内容来自当前已提交的 `run.json`，不保存第二套 ready 状态。角色扩展必须通过公共 Session 能力自动取得身份；这不是模型业务参数，模型不提供 Run ID、角色或归属。

后端先校验同机来源，再通过已保存的管理者原生身份解析唯一的当前后端 Run 归属；没有关联、重复关联、非管理者均拒绝，不按名称、Task、Tab 或 cwd 猜测。session_id 不是秘密认证凭据：接受同机进程冒用风险，不构成强认证或沙箱。重启后旧 Run 只读，普通 Chat Resume 不代表接管 Run。

### 后端重启：历史现场只读

嵌套管理者 Chat 的普通恢复依据持久唯一关联：子 Run 的 manager 与父 node 的同一次 execution 必须相互对应；它同时加载 manager-chat 与 node-chat，而非根据 Chat 名称、cwd 或当前选择推测角色。retry 后的旧 Chat 仍对应保留的旧 execution 与旧 child，不改绑到新执行。恢复使用明确来源 Task 的当前会话环境，不重新应用 trigger config，不新建 Run、execution 或提交业务输入；非法／不唯一的关联明确拒绝，不能以普通 manager 角色替代双角色校验。旧非嵌套记录缺少 parent／childRunId 字段仍按非嵌套历史读取，不要求迁移或增加旧业务别名。角色查找扫描 Run 记录，但只反序列化身份与角色关联字段；旧记录缺少成果批准字段不阻断任何 Chat 的恢复。JSON 无法解析或关联本身不可判定时仍拒绝恢复，不跳过记录、不推测角色。此读取方式仅用于 Chat 恢复，不迁移 Run，也不改变 workflow 操作的完整记录校验。

历史 Run 仅允许通过上述 PATCH 修改人用名称；名称写入 `run.json`，重启后保留，不向执行历史或报告追加事件。名称不参与调度、身份解析、Chat 名称、Map 引用或管理权限；同 Run 的名称更新与工作流写入互斥，外部身份等待期间保存的名称也不会被迟到的启动关联覆盖。

管理归属仅存在于创建 Run 的当前后端进程；`managed=false` 是服务上下文事实，不是持久 Run 状态。真正重启服务后，旧 Run 仍可通过详情链接及 `GET /api/workflow/runs/<Run ID>` 读取，原 running/paused/completed 等状态、节点失败、执行与已知管理者/节点 Chat 关联、报告和三份记录保持原样。服务不扫描记录恢复控制权，不把 running 改写为 failed/completed，不发起执行，也不提供自动或手动接管入口。

重启前的管理者身份调用 status、prepare_inputs、dispatch、complete、finish、pause、resume、fail、recover、retry、reconcile_dispatch、collect 均拒绝（HTTP 403、`manager-unassociated`）；旧节点身份 report 同样拒绝（HTTP 403、`node-unassociated`）。已有报告或 pendingReports 不恢复写入资格，collect 不进入等待、不返回正常快照；普通 Chat Resume 也不恢复 Run 管理。用户可继续读取保存事实、核实原 Chat 与产物，但不能通过改文件、重放初始指示或将 Resume 当作接管来继续旧 Run。角色工具应将拒绝反馈给调用者，不自动重试或另起执行。

管理者启动失败的 `startupError` 仅保存在原后端内存，浏览器刷新可以查询；真正重启后为 null，不从业务历史重建到页面。已提交 Chat 关联仍保留，节点的持久 failure 则继续可见。需要跨进程查询启动诊断时，应由部署环境保留服务 stdout/stderr（含 Run ID、stage 和已发生效果）；三份业务记录不是诊断日志，也没有页面日志浏览入口。

status 除 `snapshot`（包含 `inputsReady` 与当前 `fileInputs`）外返回派生的 `ready`（可派发 node ID）、`blocked`（每个 node 的当前阻塞原因数组）及 `canFinish`。输入尚未确认（`inputsReady=false`）时所有业务节点均阻塞；依赖只在所有前置 node 为 completed 时满足；running、failed 等非 pending 节点不可首次派发。查询不保存第二套 ready 真相，不自动选择路线或创建 Chat。

### 管理者输入准备：prepare_inputs

Map 的 `input_requirements` 只承载非文件的总体要求，非空时才在发起 Run 表单的“本次文件输入”上方以 Markdown 展示；逐文件要求由各项 `input_files[].description` 承载。文件描述是可选纯文本字符串，可省略或为空，显式 `null` 及其他非字符串均拒绝；它在对应文件输入框上方作为标签显示，缺省或为空时回退为 ID，必需／可选状态由 `required` 单独渲染，不解析描述中的 Markdown。管理者先读取总体要求、每项文件描述、本次要求与已有材料，接收用户继续提供的自然语言或文件，核实目标、业务语义、材料用途及授权，缺项或歧义先澄清。可在用户授权范围内将口头需求整理成所需文件，不能凭文件存在或字段非空就认定满足要求。即使没有命名文件声明，也必须先完成语义核实再确认，不以派发业务节点代替此阶段。

`POST /api/workflow/manager/prepare_inputs` 接收公共 Session 取得的 `sessionId`，以及模型业务参数 `fileInputs: [{id,path}]`、`confirmed: boolean`、`reason: string`。fileInputs 是本 Run 全量绑定的替换，不是增量 patch；未列出的绑定移除，空数组表示清空。后端校验当前 Map 的声明、唯一 ID 及所供绝对路径的真实可读性；未知／重复 ID、无效或不可读文件拒绝，不能降级成缺席。`confirmed=false` 可保存尚缺必需项的准备状态，保持 `inputsReady=false`；`confirmed=true` 仅在管理者完成全部语义核实后使用，后端此时强制必需文件齐全，并保存 `inputsReady=true`。reason 记录本次准备／确认依据，不是业务内容校验的替代。返回后的已提交事实与部分生效错误应按既有动作协议处理。

输入未就绪时 dispatch／retry 均拒绝，不创建业务 Chat 或子 Run；确认不会自动派发，须另行显式 dispatch。直接发起和嵌套子 Run 使用同一输入准备流程；已派发 execution 的规则和实际 inputs 保持冻结，不因 Run 绑定调整而热更新。此阶段只解决输入准备，不增加输出要求、输出处理或交付后禁止改写规则。

### 管理者显式批量派发

`POST /api/workflow/manager/dispatch` 的传输正文为：

```json
{"sessionId":"<公共 Session 能力取得的当前真实身份>","nodes":["A","B"]}
```

模型业务参数为目标 `nodes` 与可选 `zoneIds`，所选动态节点必须指定位置。`sessionId` 由工具取得，模型不填 Run ID、角色或归属。后端解析当前管理 Run，重查 `inputsReady`、状态与依赖，返回 `results` 和 status 查询字段。目标为 Task 时启动独立 Chat，为 Map 时启动[子 Run](#map-型节点与子-run)，为脚本时交由 Bun [后台执行](#无-chat-的执行与报告表示契约)。每项包含 `nodeId`、`status`（`rejected`、`failed` 或 `running`）。拒绝项有 `reasons`，启动失败项有 `failure.stage/message/effects`；已发生的 Workspace、Tab、pane、agent、原生身份及指示尝试/确认均如实保留。`persistenceError` 表示失败事实未能落盘，`historyError` 表示当前事实已提交但追加历史失败；调用者不能把它们当作全量成功或自动重试理由。

普通 Task 型目标沿 node、Bot、normal Task 解析完整 trigger config 和 env config，在本次解析的 zone 创建独立原生 Chat。固定 zone 沿用 Run 快照，动态 zone 按本次明确选择；位置必须仍安全、可用。目标 Workspace 只取已保存的管理者实际 Workspace ID；目标不存在或查询失败即报错，不按目录名重选，不退回当前/默认 Workspace。原生身份必须不同于本 Run 的管理者及已有 node Chat，宿主返回的 cwd 必须与本次保存的 zone 实际路径一致。真实关联保存前不提交初始指示；运行文件锁不跨宿主调用。

节点初始输入按原有 Skill → Task 固定 prompt → 本次 input 组合规则提交。本次 input 包含自身 node 标识/名称、运行说明及三份记录的绝对路径，并在业务原文外提供独立策略控制段，明确本次 `auto_approved: true|false` 及行为定义，另以独立背景段给出本次 `userInput`，命名文件输入段只给地址，最后提供经本机词典单次替换的 node.input；Task 固定 prompt 同样单次替换，不解释 userInput、规则 JSON、文件地址、报告或产物；不附 Bot 文档、完整 Map、上游聊天或文件正文。只有替换前的 Task 固定 prompt 含未转义且闭合的 `${key}` 时，系统才追加字典路径，供 agent 直接读取原始定义时使用。`$${key}`、未闭合写法、用户或其他消息段与替换值均不触发。管理者与节点均获知管理者 zone 下 `.manager-zone/.workflow/<Run ID>` 的中间资料位置；该位置按 Run 区分、与 node 执行 zone 解耦，不建立背景副本、每 node 子目录或输出预分配协议，不限制业务产物的地址，也不是输出登记白名单。框架不替节点创建业务文件或读取其正文。node 应主动按需读取说明与记录，在自身 zone 工作；记录路径可以位于 zone 外，不按 cwd 推导替代路径，也不增加可读性门禁。node 不得通过自行退出、停轮或报告假定完成交接，流程推进由管理者显式决定。

派发读取 Map 的 `input_rule`／`output_rule` 数组，并固定为本次执行的 `inputRule`／`outputRule`；`type: file` 使用本次 Run 已确定的 zone 绑定解析相对 `path`，可选 `zone_id` 缺省时使用 manager zone，并检查文件存在、可读及 zone 安全边界；`type: run` 只按本 Run 的 `fileInputs` 解析本 Map 已声明的 `input_id`，用本地 `id` 重命名，未提供可选项明确缺席，不接受其他 Run 选择；`type: node` 按同 Run 的 `node_id` 当前 execution 查找 `output_id`。输出 ID 是逻辑交接标识，不是文件名。解析只取 `messages.jsonl` 中适用执行的报告 `outputs: [{id,path}]`，按追加顺序累积覆盖；省略保留，新 execution 不继承，旧执行迟到报告不污染当前执行。不解析报告正文、扫描目录或聊天，也不另存输出 registry。保存报告成功即可解析，不等待通知，通知失败不使地址失效；所有解析出的实际地址在本次派发前仍须存在且可打开，失败不启动对应消费者。

来源节点尚无执行或当前执行未登记所需输出时明确拒绝派发，不创建消费者 Chat、不回退旧执行、不猜路径。规则引用仍不隐式添加 `blocked_by`，也不自动派发。成功解析的实际输入保存为执行的 `inputs: [{id,path}]`，初始化提供这些地址而非正文；后续上游更新登记或 Map 编辑不热更新已派发输入。多个消费者可得到同一地址，业务汇总由明确安排的业务 node 完成，不由框架读取或合并。上游报告产物的地址可以位于所有 zone 外；直接文件规则必须位于其声明或缺省的 zone 内。不复制、移动或重定向业务文件。

同批拒绝或启动失败不取消其他合法目标，不关闭或回滚已创建的 Chat。成功派发后 node 为 running；启动失败为 failed，保留执行 ID、来源 Task、zone、可得 Chat 关联及失败阶段/原因，并追加执行历史。正常已记录的 running/failed 再次 dispatch 均拒绝，不增加外部启动、不覆盖首次事实。崩溃或关联保存失败窗口不承诺完整幂等，先核实已发生效果，勿盲目重发。同 Run 派发批次互斥；不同 Run 的身份与事实独立。

manager-chat 需要在独立 toolkit 中通过公共 `PromptDeclaration` 的 `first-enabled-run` 提供 status/prepare_inputs/dispatch 及下文 complete/finish 指引，并将工具接到对应协议。node-chat 同样通过公共 Prompt 能力给出节点职责与稳定路径读取指引。后端协议和页面不是该外部扩展已交付或真实注入已验证的证据。部署缺少 metadata 指定的任一角色扩展入口时先补齐实际扩展，不能用空函数、手填 sessionId 或 Tab 代替真实角色工具验收。

### 统一文件交接约定

以下区分后端在启动／派发前的文件检查与节点持续承担的业务职责。业务 input 的编写边界见 [node.input 的业务职责](scheduler.md#nodeinput-的业务职责)。

- 消费者以派发时保存并注入的实际文件输入为准，直接读取命名地址；不读取运行记录筛选材料，不持续追踪上游是否产生新 execution，不因上游重试或更新输出登记自行切换地址。文件缺失、不可读或业务内容冲突时报告具体阻塞，不猜路径、不回退其他版本。
- 登记固定的是地址，不是内容。生产者可以在业务授权内原地修改自己负责的文件；安全写入、避免覆盖其他节点／执行的产物及完整读回职责不变。需要保留不同版本时可写新文件并更新同一输出 ID 的地址；后续派发按届时适用登记解析，更新不重定向已有消费者。成果批准不等同于业务内容保存授权，也不冻结材料或正文。
- 交接说明与可直接使用的纯正文是独立文件时，分别声明输出 ID 并登记地址；消费者逐项声明输入，不从同目录文件名或交接说明中推导未声明材料。输出规则不代替交付内容、格式、确认范围和验收要求。
- 内部交接材料使用初始化提供的中间资料位置。生产者先核实写入位置、权限与路径边界；不要沿符号链接、目录联接或重解析点越出授权位置。新文件选不冲突的真实路径，不能覆盖其他节点或执行的产物；按业务约定安全写入后，从正式路径完整读回并核对，再登记。业务明确要求的正式位置仍有效，规则本身不预分配路径。
- 已提供的根输入在预检、启动与输入准备入口，普通节点的直接文件／报告输出／Run 输入在本次派发前检查实际文件存在且可打开。检查只证明当时可读，固定输入地址不等于文件正文快照；框架不读取业务正文，不复制、锁定、校验或阻止后续改写。中间资料位置仅按 Run 区分，不提供自动的每 execution 隔离，上述不覆盖约定依赖生产者遵守。派发后删改不会触发系统重试、修复、补文件或搬运，不承诺持续可用性、不可变存储或服务端强制隔离保证。
- 文件来源解析不证明业务正确。消费者仍核验目标 Bot 的 normal Task／Map 目标、共同基线、确认范围和实际配置冲突；保存时继续遵守业务授权、真实 revision 及完整读回要求。Bot 指南的共同基线、唯一正文与必要说明统一按[共同业务材料契约](business-materials.md)生产、直接传入与核对。管理者仍负责完成与派发，`blocked_by` 与材料引用保持独立。

例如消费者已获得 `text → A.md` 后，生产者将修订正文写入 `B.md` 并更新 `text` 登记；该消费者继续使用 `A.md`，后续消费者可取得 `B.md`。若在授权范围内原地修改 `A.md`，原消费者之后读取的就是新内容；删掉它后读取会失败，系统不提供旧内容副本或自动修复。

本节记录已确认的职责与交接约定，不代表旧 Map、Task 提示词或外部角色扩展已同步迁移。精简旧业务 input 前须核对结构化规则与统一协议的实际承接，不能将文档约定当作运行强制能力。

### Bot 创建与修订的业务文件交接

[共同业务材料](business-materials.md)定义基线、唯一正文与交接说明的职责。[创建 V1 包装](create-bot-wrapper.md)、[指南修订](revise-bot-guides.md)与[创建 V2](contracts/create-bot-v2-materials.md)有不同材料与授权接口，不混用。共享 Task 即使在独立 Chat 也须核实真实目标、基线、revision、允许修改范围及完整读回。成果批准不代替业务确认、保存授权或管理者完成。

包装子 Run 的 saved 与 handoff 由父层验收；若外层还有 `optimize_bot`，父管理者须验收最后的优化成果才 finish。定义保存或文本优化不证明运行行为，不为核实定义而启动用户业务。

### plan_workflow 的业务文件交接

[plan_workflow 定义](../meta-data/workflow-maps/plan_workflow.json) 依次交接概要方案、已保存的 Map 和整图优化成果。规划者确认方案，转换者读取命名概要方案并新增 Map，优化者读取完整已保存定义，核对并交付最后的 Map；最终配置以优化成果和服务完整读回为准。管理者在各步读实际材料，验收后显式完成和派发，不能用交互中的草案或前一阶段的文件替代最终交付。

此 Run 只交付业务定义，不启动该业务。根业务启动配置可待补齐，但方案内 Map 调用节点的 manager、zone、业务输入和材料映射缺失会阻止翻译。业务确认、成果批准与报告独立；按[规划交付契约](planning-deliverables.md)核对范围、授权和可翻译条件，不用验收器代替真实角色批准或消费材料。

## 角色归属与来源集成检查

运行工具的归属不能由 Run ID、Task 类型或工具调用者自报决定。后端只按保存的真实原生 session_id 查找关联：管理者必须在当前后端内有唯一 Run 归属，且不能同时关联任何 Run 的当前或保留节点执行。出现跨 Run 的 manager/node 身份冲突时，所有管理者动作统一返回 `403 manager-ambiguous`，不按先找到的角色授权；应核实实际 Chat 关联，不能用同机来源或重复请求绕过。无关联节点/管理者身份也不能因来源在回环地址而取得权限。旧节点执行仍可提交归属于原执行的历史报告，但不能冒充当前执行或管理者，迟到时间不改变报告适用性。

同机 TCP 来源与真实 Session 关联独立校验，不构成认证或沙箱。`sessionId` 不是秘密；同机进程仍有冒用风险。双角色的唯一例外是[同一子管理者的持久父子关联](#map-型节点与子-run)，不能将此例外用于其他 Run 或 execution。核对实际角色、独立节点 cwd、公共能力、配置基准及 connect-bot 关闭时的行为；入口文件存在、手填 HTTP 身份或受控宿主不证明原生注入和工具行为。

## 派发验证入口与证据边界

入口与隔离要求见[测试指引](agents/testing.md)。派发验收以独立预期核对实际原生身份、cwd、Workspace 和接收输入：Task 固定配置、本次要求、业务 `input` 与命名地址应按规则传递；上游聊天、Bot 或 Map 正文与输入文件内容不得隐式注入。确认真实 node 能主动读取 zone 外说明与记录。受控 Herdr 可以证明接线与阶段错误，不能证明原生角色加载、Prompt 注入或模型实际读到材料。

## 显式交接完成与结束 Run

管理者检查结果确实可交接后才使用 `complete`，不必等执行 Chat 停轮或退出。管理者工具的业务参数只有 `nodeId` 与 `reason`；`reason` 必须为非空说明，应记录接受的结果、地址及限制，原文保留。传输身份仍由公共 Session 能力自动取得，不让模型或用户手填归属：

```http
POST /api/workflow/manager/complete
Content-Type: application/json

{"sessionId":"<当前管理者真实身份>","nodeId":"A","reason":"结果已可交接；清理工作仍在原 Chat 继续"}
```

Run running 或 paused 时，running node 可以完成；人工已处理并确认可交接的执行阶段 failed 也可完成，但 pending 或派发阶段 failed 不能借 complete 跳过工作或核实。首次 `complete` 有三项独立门禁：当前 execution 已有 `approved=true`，按 `messages.jsonl` 追加顺序的当前 execution 最新报告严格声明 `canComplete=true`，且本次冻结 `outputRule` 的每个 ID 已在当前 execution 的报告中登记。无报告、缺字段或最新 false 均拒绝，不回退较早或旧 execution 的 true。没有输出约定时无需登记文件；有输出约定时可以跨本次执行的多份报告登记。后端检查登记，不读取文件或判断业务质量。管理者核实实际结果、材料与限制后再交接。完成不产生报告、不补发材料、不自主派发下游；原 Chat 默认继续存在，只有本次执行明确启用下述自动关闭时才尝试关闭其 Tab。所需结果地址与限制仍应通过原 Chat 的 report 补齐，后端不额外判断结果质量。A/B → C 中只完成 A 不够：C 仍由 B 阻塞；两者均 completed 后，管理者须另行显式 dispatch C。

成功后返回 `changed=true`、`snapshot`、`ready`、`blocked`、`canFinish`、`reports` 和 `pendingReports`。说明与操作 ID、时间、真实管理者 `actor` 保存在 `snapshot.nodes.<nodeId>.completion`，状态与说明在同次当前事实写入中提交；对应 history 的 `action=complete` 含前后状态、说明与执行引用。同 Run 动作与整个派发批次串行，动作快照在该边界内生成，后续动作仍检查最新事实。completed node 不再列入可操作报告待办；补充 report 不重新打开节点或 Run。

报告保存与首次完成共用既有同 Run 操作边界：false 先保存则首次 complete 拒绝；完成先生效，后续 false 不追溯撤销。门禁判断在后端操作边界内进行，不由 toolkit 预检替代，也不新增跨文件事务。Run 尚未结束时重复 complete 返回 `changed=false`，不再重审完成声明，不覆盖首次说明、时间、操作 ID、身份或成功历史，不重复自动关闭；完成后补 false 仍保持此幂等结果。拒绝返回 HTTP 409，并附 `code`、原因、`changed=false` 和当前查询字段；例如 `node-not-completable`、`completion-reason-required`、`node-unknown` 或 `run-terminal`。声明门禁拒绝使用 `report-not-completable`，原因明确给出当前适用最新报告 ID 与 false／缺失声明；没有当前报告时明确说明不存在，不伪造引用。拒绝不写当前事实、历史或报告，不发新通知或宿主命令。无关联身份仍按管理者权限拒绝，不接受额外 Run/归属字段。

### 节点完成后自动关闭关联 Tab

Map node 的 `auto_close` 缺省为 `false`；dispatch/retry 读取派发时的当前 Map，并固定为本次 `execution.autoClose`。Run 创建时不冻结此选择，之后修改 Map 不改变已有执行；recover、核实或恢复原 Chat 沿用原执行设置。旧执行缺字段按 `false` 使用，不改写历史或补偿关闭。

只有 manager 合法 complete 使节点首次进入 completed，且当前完成事实保存成功、本次执行启用自动关闭时，才异步尝试关闭一次。TUI 先读取当前执行保存的 Workspace 范围，若已知关闭最后一个 Tab 会连带关闭 worktree group，则不发送关闭命令；其余情形使用保存的 `chat.tabId` 执行 `herdr tab close <tab_id>`。保留 TUI 默认 `ui.confirm_close = true`，让原生拒绝也能保护查询到关闭之间新出现的 worktree group；App 核对该执行保存的 Space／Tab／Pane 与当前 App/core 原生会话关联，再执行带 `confirmed: true` 的 `tab.close`。这只授权目标 Tab 和由此变空的容器；不发单独的容器关闭命令，TUI 要求关闭额外 worktree group 时直接报告失败。没有保存的 Tab 不猜测，App 关联无法核实也不会改用其他目标。当前事实保存失败不关闭；事实已提交但历史追加失败仍可关闭，保留原有部分生效反馈。重复 complete 不产生新资格。

关闭不会等待 agent 停轮、空闲、退出或收尾文字，也不先发送 stop。complete 的响应与同 Run 后续合法操作均不等待关闭命令返回；外部等待不持有 Run 操作锁或文件协调范围。关闭用于终止该 Tab 内终端及相关进程，不删除 Chat 历史；宿主关闭回执不证明布局实际移除或进程实际退出，也不用 park／settle 代替关闭。受控拒绝、命令不可用或不能确认命令结果时，不撤销完成、不重试。Run 的节点明细可独立读回 `autoCloseFailure`，界面在已完成事实旁显示原因；不把关闭反馈写成节点工作失败或新的完成事件。report、停轮、等待输入、退出登记、fail、pause/resume、整体 finish 和读取均不独立触发关闭。

`run.json` 的节点事实可附带 `autoCloseFailure: {code,message}`，对应当时的 completion。跳过关闭不代表失败；失败反馈自身无法保存时查服务诊断。读取、刷新、后端重启和恢复已完成的 Chat 都不重放关闭。若服务在外部关闭期间退出，完成事实仍有效，不承诺补送或跨崩溃恰好执行一次。受控宿主回执不能证明实际布局或进程效果。

### 结束 Run

全部普通工作节点 completed 后，管理者显式结束本次 Run。Map 若标记 `end_node: true`，该唯一无后继的普通 Task 节点只需成功派发，不需 report／complete；它必须在其余节点全部 completed 后才能派发。脚本型或 Map 型节点不能作为结束节点。结束节点执行保存 `endNode: true`，保留真实 Chat 与材料关联，启动和恢复均不加载 node-chat，也不自动关闭。其节点状态保留 running，不伪造业务完成；Run 的 `finished.reason: "handed-to-user"` 表示已交给用户。包含结束节点的 Map 仍可嵌套，显式 finish 后沿用原有向父层报告及验收逻辑。

管理者调用：

```http
POST /api/workflow/manager/finish
Content-Type: application/json

{"sessionId":"<当前管理者真实身份>"}
```

finish 没有模型业务参数。running 或 paused 的 Run 须全部普通工作节点 completed，且指定结束节点（如有）成功派发，才能成功；条件未满足返回 HTTP 409、`code=nodes-incomplete`，不自动完成节点。成功返回 `changed=true` 及同样的查询字段，Run 变为 completed，`snapshot.finished` 保留首次操作 ID、时间与真实管理者，history 追加 `action=finish`。`canFinish` 表示**当前仍可执行 finish**，所以终态为 false；重复 finish 仍返回已有成功事实、`changed=false`，不追加历史。终态可查询、可接收普通工作节点的补充报告（结束节点 Chat 无节点报告资格），但拒绝 complete 和新 dispatch；dispatch 拒绝项不会调用宿主。活动中的 Chat 不阻止完成或 finish；finish 不关闭 Chat，complete 仅按本次执行的自动关闭选择尝试关闭关联 Tab。

当前事实保存失败返回 HTTP 500、`complete-failed` 或 `finish-failed`，不采用候选状态或追加成功历史。当前事实已保存但历史追加失败时，仍返回已生效快照与非空 `historyError`，不要当作全部未生效或盲目重试。三份记录不承诺跨文件事务；重复调用也不会修复或重复追加首次缺失的历史。

manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 应将 status → 读取输入要求与材料、澄清并 prepare_inputs 确认 → 读取报告/核实交接 → complete → 显式 dispatch 下游 → 全部普通工作节点完成且结束节点（如有）成功派发后 finish 的职责与上述拒绝/部分生效语义注入真实管理者 Chat。node-chat 指引应明确 report、停轮、退出都不替代管理者交接。本仓库没有外部 toolkit 的角色工具实现；HTTP 路由与此说明不是工具已装配或 Prompt 已注入的证明，不通过手填 HTTP 身份冒充真实角色工具验收。

## 故障处理速查：先看现场，再向管理者表达意图

本节面向用户与角色指引维护者。下面的自然语言是向**已关联管理者 Chat** 表达意图的例子，不是要求用户填写工具参数；后文 HTTP 示例仅供接入维护者核对协议。用户不需要复制 sessionId、Run/执行归属，不直接编辑 `run.json`、`history.jsonl` 或 `messages.jsonl`。

### 所有故障共用的入口

1. 打开 **workflow → 对应 Run → 失败 node**，先读「失败阶段与原因」、已发生效果、已保存执行、当前适用报告和「旧执行历史」。记下节点标识和可得的原 Chat，不能只凭 Chat 在线、停轮或 Tab 是否存在判断执行成败。图来自当前 Map，不是历史定义快照。
2. 先检查是否显示「历史现场 / 未接管」。若后端已重启，旧 Run 只能读取；普通 Resume 可帮助查看原 Chat 与产物，但不能恢复管理者写入资格。不要编辑文件强行接管。需要另起工作时，应先核实旧副作用，再从 scheduler 显式发起独立新 Run；这不是修复或继续原 Run。
3. 在 node 的已保存 Chat 入口点击 **Resume**，核实原生对话、错误、产物地址和限制。也可展开旧执行历史进入指定旧 Chat；旧执行报告不能代替当前执行结果。没有关联时页面不会补建 Chat。Resume 失败或结果未知时先核实 Herdr 实际现场，不连续点击试图重启；已提交启动不等于已确认加载指定 Chat。
4. 完成实际修复或核实后，在 Run 的**管理者 Chat → Resume** 进入管理者，说明节点、观察依据、修复内容、产物位置、限制与想要的处理方式。不要把「继续」这种含糊表达当作另起执行的许可。管理者须先查最新 status，再落实准入允许的明确动作。
5. 阅读管理者工具结果，再回 workflow **刷新 Run**，对照状态、执行/Chat 身份、首次说明、当前适用报告和保留的旧历史。状态动作成功不代表原 Chat 已继续工作、材料已经传给下游或下游已经启动。若结果含已发生效果、保存错误或 `historyError`，先核对已提交事实，不把报错解释成全部回滚，也不盲目重试。

### 四种常见处置

| 现场与向管理者表达的意图 | 管理者应做什么 | 用户回页面核对什么；不能推断什么 |
| --- | --- | --- |
| **原执行可继续**：执行阶段 failed，已在原 node Chat 核实并修复。「A 缺少的源文件已补齐，原 Chat 可以继续；请恢复原执行的跟踪，不要另开执行。」 | 记录人工确认与修复依据，使用 recover。Run paused 也允许；必要的原 Chat 进程恢复另用普通 Resume。 | A 回 running，原执行 ID/Chat 不变，失败历史保留；没有新 Chat、输入重放或自动修复。paused 仍为 paused。需要继续对话时向原 Chat 明确补充指示，recover 不代发。 |
| **确需另起执行**：执行阶段 failed，已检查旧 Chat、部分产物和重复副作用。「A 原执行不能继续，旧结果只作参考；我已核实副作用，同意另起独立执行，保留旧现场。」 | 人工确认后 retry；paused 时先取得恢复派发资格的明确意图并 resume，再 retry。不确定的派发失败必须先核实，不能直接 retry。 | 新执行 ID 与新原生 Chat，旧执行/失败/产物仍保留；新执行可能再次失败，应读其实际效果。旧 Chat 不被终止，副作用不被撤销，旧报告不成为新执行交接依据。 |
| **人工结果已可交接**：执行阶段 failed，人工已实际完成并核对结果。「A 的结果已人工补齐在绝对地址……，限制是……，已确认可交接；请记录人工完成，不再执行。」 | 确认当前 execution 已批准、约定交付已登记且最新报告严格声明 canComplete=true 后，用 complete 的说明记录人工依据；尚无可用声明时由已有关联 node Chat 如实 report，不伪造报告，也不能由管理者单方面或 force 绕过。 | A completed、完成说明可读、依赖满足才出现下游资格；不产生假 report，不自动传材料或派发下游；仅按本次执行的自动关闭选择尝试关闭关联 Tab。pending/派发阶段 failed 不可这样跳过工作。 |
| **仅通知失败**：node report 返回保存成功而通知失败。「报告已保存，报告标识是……；请读取该报告并核对交接，不要因为通知失败重报、重派或回滚。」 | 先 status/collect，再按报告标识读取正文与实际材料；确认后才 complete。node 保留原报告标识与投递错误，不仅因通知/诊断失败重发 report。 | 已保存报告与待办仍在，node 状态不被通知错误改变。collect 只给标识与快照，不含正文；读取/通知/collect 不清除未完成待办。输入已提交也不保证管理者已读或被打断。 |

**派发阶段 failed 先核实，不套用上表的执行恢复。** 有 Tab 不等于已有正确原生 Chat。向管理者分别说明「已确认原 Chat 存在且可跟踪」「已确认没有创建原生 Chat」或「目前无法确认」，并给出证据，由其 reconcile_dispatch：已创建只读核实原目标身份与已有 child；仅 Chat 已知但输入未确认仍保持 failed，不能靠核实放行 retry；未创建且没有已登记 child 才恢复 pending，之后仍需显式 dispatch；不确定保持 failed。不要为取得 pending 而谎报未创建，不把确认参数当成回滚授权。核实不会重发可能遗漏的初始指示或补发材料。

### 角色责任与批准动作矩阵

以下是实际角色 Prompt 应覆盖的内容与工具业务参数审查基准，**不是已注入的声明**。身份与归属必须由公共 Session 能力和后端真实关联取得；模型不能提供 sessionId、Run/角色/执行归属或接收者。工具 description、Task 固定 prompt、本文路径存在或模型碰巧做对，都不能单独证明公共 Prompt 已注入。

| 管理者动作／业务参数 | 准入与状态责任 | 实际执行／交接边界 |
| --- | --- | --- |
| status；无 | 唯一可信管理者；终态仍可查当前快照、ready、blocked、canFinish。 | 读取不是派发授权，后续动作重新检查现场；不修改状态。 |
| prepare_inputs；fileInputs、confirmed、reason | 真实管理者全量替换本 Run 文件绑定；确认时必需项齐全。 | 先读要求与材料、澄清并语义核实；confirmed=true 才表示输入就绪，不自动派发。 |
| dispatch；nodes、可选 zoneIds（所选动态节点必填） | Run running 且 inputsReady=true，逐项 pending，依赖及结束节点约束均满足。 | 创建独立 execution，按目标启动 Chat、子 Run 或脚本。逐项结果；失败不取消其他项或回滚。 |
| complete；nodeId、reason | Run 未结束，running node 或人工已确认可交接的执行阶段 failed；说明非空，当前 execution 必须 approved=true，最新报告严格 canComplete=true，且约定交付已登记。 | completed 是交接完成并满足依赖，不要求 Chat 退出；不制造报告、传材料或自动派发。 |
| fail；nodeId、reason | Run 未结束、node running；保存执行失败与首次原因。 | 保留原 Chat；不暂停、重试或终止实际执行。 |
| reconcile_dispatch；nodeId、finding、reason | 仅派发阶段 failed；已创建／未创建／不确定按核实事实处理，不接受任意目标状态。 | 原派发目标与身份由系统核实，不由模型自报替换；不创建 Chat 或重放输入。 |
| recover；nodeId、confirmed、reason | 执行阶段 failed、有关联原 Chat、人工确认可继续；paused 也可用。 | 只恢复原执行跟踪，不修复现场、恢复进程、补发指示或撤销失败历史。 |
| retry；nodeId、confirmed、reason、可选 zoneId（动态节点必填） | running Run 且 inputsReady=true 的执行阶段 failed；人工核实同意，派发不确定先核实。 | 独立新执行、新 Chat，完整 trigger config 生效；保留旧事实与副作用，不转用旧报告。 |
| pause；无 | running → paused；等待同 Run 在途完整批次结束后生效。 | 只阻止新执行，已有 node 继续工作/report，管理者仍可核实、recover、complete。 |
| resume；无 | paused → running，只恢复派发资格。 | 不自动 dispatch/retry，也不是普通 Chat Resume。 |
| finish；无 | running/paused 且全部普通工作节点 completed，结束节点（如有）成功派发，显式结束；重复返回首次事实。 | 不等待或终止 Chat。终态拒绝后续状态变更，不重开 Run。 |
| collect；无 | 当前管理者收取通知，返回 reason、快照与报告标识；不修改 node 状态。 | 按标识读取正文，核实交接后才 complete；不把收取当成已读/完成。 |

管理者须逐项读批次结果、已发生效果及部分生效错误；保留原执行、材料地址与限制，显式选择后续动作。**后端**落实真实角色、当前执行、依赖、状态与持久化约束，不能以模型表现替代这些约束。普通 node Chat 负责实际工作、独立 `approve`（无业务参数）与 `report`（`body`、必填布尔 `can_complete`、可选 `outputs`）：交接可用时写清地址/用法/限制，可以随后继续不影响结果的收尾；report、停轮、退出均不等于 completed。旧执行补充报告必须归原执行，不代表当前新执行完成。pause 不撤回 node 的已有工作。

collect 的正常原因分别是 notification、paused、completed、no-running、timeout；有 running 且无新通知时可等待，不保证有 ready 就即时返回。正常到期不改状态；取消、断线、预算耗尽或读取失败是错误，不是空快照或正常 timeout，不自主重试。管理者若要再检查须明确重新调用；工具中断/Chat 结束须经公共 HTTP 取消等待，整个请求预算大于后端期限（默认后端 90 分钟、调用方建议 91 分钟）。node 对 report 要区分保存失败、保存成功/通知失败和诊断失败，不能仅因通知/诊断失败重报。

### 可重复演练与 H2 证据要求

执行 Agent 先固定四项独立预期：recover 保持原身份；retry 创建新身份并保留旧事实；人工 complete 不造报告或传材料；仅通知失败不丢报告、不改状态或重报。最终 completed 不能排除这些违规。

按[测试指引](agents/testing.md)隔离真实页面、原生 Herdr 和角色工具。每种处置用独立现场，只在本轮拥有的材料或通知条件制造故障，不能修改运行记录或操作用户会话。执行阶段失败由真实 manager 显式 fail 后再按适用处置执行；仅通知失败不得 fail 节点。入口加载或隔离不具备时记录未验证。

保留页面失败及阶段、实际 Resume 结果、核实与修复、人工补充输入、管理者收到的意图、原生工具参数和结果、刷新后的状态与三份原始记录、实际文件及外部调用。通知失败须保存 `saved`、报告 ID、`notification` 和管理者读到的同一报告。负向观察注明起止，不外推为无限期无动作。

记录两种角色实际收到的 `first-enabled-run` 正文及启用时机。分别报告指南内容、后端约束与真实角色实演的证据。P64/P65 的操作和取证归执行 Agent；H2 只由用户最终判断指南是否清楚可行。

## 失败核实、原执行恢复与人工交接

失败不等于 Chat 已退出，更不等于运行应暂停。后端不会因派发或执行失败自动 retry、pause、终止 Chat、补发初始指示或传递材料。先读取 status、原 Chat 与三份记录，再请用户核实或修复现场。Run running/paused 均允许以下非启动动作；completed 终态全部拒绝（HTTP 409、`run-terminal`），保留首次 finish 事实。

### 记录执行失败：fail

`POST /api/workflow/manager/fail`：

```json
{"sessionId":"<公共 Session 能力取得的当前管理者身份>","nodeId":"A","reason":"已核实：缺少上游源文件，原 Chat 暂不能继续"}
```

只有 running node 可首次 fail；`reason` 必须非空。成功保存 `status=failed`、`failure.stage=execution`、`failure.message` 并保留原 execution，不改变 Run 派发资格。Task／Map 保留原 Task、zone 与真实 Chat；脚本保留固定能力、zone、lifecycle 及真实输入，不要求或制造 Chat。执行失败不凭空生成启动效果字段；派发失败的 `failure.effects` 仍保留启动时观察到的实际效果。history 的 `action=fail` 包含首次原因、前后状态、执行引用、管理者 actor、操作 ID 与时间。重复 failed 的 fail 返回 `changed=false`，不覆盖首次失败、当前文件或历史，不调用宿主；它也不能把派发失败改成执行失败。pending/completed 首次 fail 返回 `node-not-failable`，空原因返回 `failure-reason-required`。

### 只核实派发失败：reconcile_dispatch

`POST /api/workflow/manager/reconcile_dispatch`：

```json
{"sessionId":"<当前管理者身份>","nodeId":"A","finding":"uncertain","reason":"用户尚不能确认原派发是否创建了 Chat"}
```

模型业务参数仅 `nodeId`、`finding`、`reason`；没有任意目标状态或调用者自报 Chat 归属。只有派发阶段 failed 可核实；执行阶段 failed 必须 recover 或确认可交接后 complete，返回 `node-not-dispatch-failed` 防止绕过。非空原因必须说明人工核实依据。

- `finding=uncertain`：当前事实完全保持 failed，保留原失败原因与执行。`changed=false`，history 追加这次不确定核实及依据，但不触发宿主调用或派发。
- `finding=not_created`：人工确认未创建原生 Chat，且没有已登记子 Run，才恢复 pending 并清除当前失败/候选执行；旧执行、原失败与依据仍在 history。已有 `childRunId` 时拒绝 `dispatch-child-already-known`，包括记录初始化失败或 Chat 未启动的子 Run，不能把 Chat 缺席当作 child 不存在。已记录真实 session_id 或 Chat 关联时拒绝 `dispatch-chat-already-known`。pending 只表示再次有资格，不自动 dispatch；已有 Tab 不会被关闭。
- `finding=created`：后端只对**原派发效果中记录的 agent** 调用只读原生身份查询，按原 execution 的宿主类型核实真实会话与原 zone，不替换已知身份或复用管理者/其他节点身份。嵌套执行核对 child 的父 execution、Task、zone 与已有关联，冲突返回 `dispatch-child-mismatch`；只在原 child 上补存缺失 manager 关联，再保存原父执行的同一 Chat，不新建 child。跨文件写入不是事务，失败不回滚已保存的另一侧，应重读两侧事实。原执行 ID、Task、zone、Workspace/Tab/pane/agent 引用不变，不创建 Chat、不提交初始指示、不补发遗漏材料。缺少原派发现实目标或宿主身份不可确认时返回 `dispatch-chat-unverified`，身份/zone 冲突返回 `dispatch-chat-mismatch`。没有尝试提交或已经收到成功提交确认时可恢复 running；**已尝试但没有成功确认时，单独看到 Chat 不证明提交效果，继续保留 failed、原 failure 和 retry 拒绝**，`inputOutcome=rejected/unknown` 也不因核实而改变。无已知原目标不能猜测或编辑记录强行关联。

每次成功核实的 history `action=reconcile_dispatch` 保留 `finding`、依据、原 failure/执行和当前执行引用；仅确实恢复 running 时清空当前 failure，失败历史不删除。created 查询失败不伪造成功，也不自动再查或启动。

### 恢复原执行跟踪：recover

`POST /api/workflow/manager/recover`：

```json
{"sessionId":"<当前管理者身份>","nodeId":"A","confirmed":true,"reason":"用户已补齐源文件，确认原 Chat 可继续"}
```

业务参数仅 `nodeId`、`confirmed`、`reason`。只有执行阶段 failed 且有关联原 Chat 才能 recover；派发失败返回 `node-not-recoverable`。人工确认 `confirmed=true` 与非空修复依据缺一不可，否则返回 `recovery-confirmation-required`。成功将原执行跟踪恢复 running、清空当前 failure；原执行 ID/Task/zone/Chat 全部不变，history 保留原 failure、修复说明和 `action=recover`。这不是 Chat Resume，不调用宿主、不重新发起或恢复进程、不补发消息；即使 Run paused 也可执行，不恢复派发资格。已恢复 running 后再次 recover 拒绝，不重复追加成功记录。

### 独立新执行：retry

用户核实原 Chat、失败原因和已发生副作用，明确需要另起工作而非继续原执行时，管理者使用：

```http
POST /api/workflow/manager/retry
Content-Type: application/json

{"sessionId":"<公共 Session 能力取得的当前管理者身份>","nodeId":"A","confirmed":true,"reason":"用户已检查原 Chat 与部分产物；保留现场，另起独立执行"}
```

模型业务参数为 `nodeId`、`confirmed` 和非空 `reason`；动态 zone 节点另须显式提供 `zoneId`，固定节点不得提供覆盖。身份仍由 Session 自动取得。仅 **running Run 中的执行阶段 failed** 可以 retry。`confirmed=true` 是人工核实/同意的记录，不是自动安全判断或回滚授权。缺确认或空依据返回 HTTP 409、`retry-confirmation-required`；pending/running/completed node 返回 `node-not-retryable`。paused 明确返回 `run-paused`，必须先显式 resume；resume 本身不创建 Chat。终态优先返回 `run-terminal`。旧 node Chat、无关联身份和额外 Run/执行/角色字段不能调用管理者动作。

派发阶段 failed 返回 `dispatch-reconciliation-required`，包括未知原生身份的异常窗口：先 reconcile_dispatch，不以 retry 的 confirmed 代替核实。确认未创建且无已登记 child 才恢复 pending，之后另行 dispatch；确认原 Chat 已创建并满足提交效果条件才恢复原执行 running，若核实该执行无法继续，再 fail 记录执行失败后选择 retry。只有 Chat 已知但输入结果未确认时仍保持派发 failed，不获得 blind retry 资格。不确定保持 failed，不盲目启动另一 Chat。若原执行可继续，使用 recover，不是 retry。

retry 复用首次 dispatch 的完整启动路径：当前 node → Bot → 普通 Task 的 trigger config/env config，并按执行上下文选择角色（普通工作节点加载对应宿主的 node-chat，结束节点不加载）、定义 zone、已保存管理者的实际 Workspace ID，创建独立新原生 Chat。model/thinking、初始 Skill、Task 固定 prompt 与 node.input 按原有组合规则全部生效，不只重放 node.input。实际目标不可用就失败，不回退目录或 Workspace。新身份不得复用管理者、当前执行或任何保留旧执行的身份；真实新关联保存成功前绝不提交初始指示。不会结束旧 Chat、撤销产物、清理现场、转发旧报告或自动派发下游。

当前 `snapshot.nodes.<nodeId>.execution/failure` 只属于新尝试；`previousExecutions` 依次保留每次被替代执行及当时的 failure，当前事实读取不依赖 history 重放。首次节点的该数组为空。解析目标失败的新尝试仍有独立执行 ID，并保留原来源 Task/zone 引用用于解释失败；它不是可用目标或自动回退。`history.jsonl` 追加 `action=retry`、管理者 actor、依据、`previousExecution/previousFailure` 和新 `execution/failure`，不覆盖既有 dispatch/fail/报告历史。

正常返回 HTTP 200，包含 `result` 与 status 相同的 `snapshot/ready/blocked/canFinish/reports/pendingReports`。`result.status` 为 running 或 failed；失败有 `failure.stage/message/effects`，实际新 `execution`、可得新 Chat、`persistenceError`、`historyError`。即使外部新 Chat 已启动而身份查询、关联保存或指示提交失败，也不把错误归到旧执行、不声称外部回滚。关联保存失败且失败事实也未能落盘时，snapshot 保持最后已提交的旧事实，result 与诊断保留新尝试和外部效果；不要因旧快照仍 failed 就盲目 retry。确认新 Chat 与磁盘后，按实际事实处理。普通读取/内部失败返回 `retry-failed` 或 `retry-internal`，不是自动重试许可。状态已提交而 history 追加失败仍是部分生效，不回滚新执行。

旧执行 Chat 可以继续 report；后端按持久保留的原执行身份归属，记录原 executionId/from，不把它伪装成新执行报告。旧/新/旧/新的交错报告全部追加到 messages.jsonl，status 的 reports 保留完整正文历史；pendingReports 只按每个 node 的**当前适用执行**汇总最新报告 ID，不按全局最新时间选择。retry 后旧报告不能成为新执行待办，也不能完成新执行；新执行仍须管理者核实其结果并显式 complete。completed 后旧/新补充报告均不重开待办或 Run。

用户处理路径：进入已关联管理者 Chat → 说明已核实的旧 Chat、产物位置、限制和另起执行意图 → 观察管理者 retry 结果的新执行/新 Chat 与保留的旧事实。不要手填 sessionId、编辑运行记录、为了获得 pending 状态而谎报原 Chat 未创建。manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 必须注入上述 recover/retry 区别、人工确认、暂停/终态拒绝及部分生效语义；node-chat 必须说明旧执行 report 只补历史，不能完成当前新执行。本仓库交付 HTTP 和运行说明；metadata 指定的外部角色入口未部署时，真实角色工具/Prompt/Herdr/omp 验收仍未完成。

### 人工已处理且确实可交接

用户已人工补齐结果且确认可交接、当前 execution 已批准且约定交付已登记，并且最新报告严格声明 `canComplete=true` 时，管理者可对执行阶段 failed 显式调用上文 `complete`，用非空 reason 写清结果地址、限制与人工处理依据。pending/派发 failed 不能这样跳过核实。该动作只完成状态交接、解除依赖，不产生 report、不补发材料、不启动下游；仅按本次执行的自动关闭选择尝试关闭关联 Tab；失败和原执行历史仍可读。缺少当前 true 声明时必须先在**已有关联的 node Chat** 使用现有 report 如实声明完成情况、地址与限制，管理者单方面判断不能绕过门禁；completed 后补充报告不重新打开待办。管理者须在后续显式 dispatch 下游时自行提供所需材料，不依赖新增消息工具或自动传播。

### 错误、调用者与用户处理路径

上述 HTTP 只接受同机、唯一当前管理者真实 session_id；角色扩展必须通过公共 Session 能力自动取得它，模型和用户不填写身份/Run/角色/归属。未知字段、无效 finding 或缺少必要参数明确拒绝，不存在任意状态后门。成功返回与 complete 相同的查询字段、`changed`、`historyError`。状态写入失败返回 HTTP 500、`fail-failed` / `recover-failed` / `reconcile-dispatch-failed`，不采用候选状态或追加成功历史；状态已提交但历史追加失败时保留生效快照并返回非空 `historyError`，不假称全部未生效或自动重试。

用户从 workflow 查看 Run/node，进入已关联管理者 Chat 表达意图，而不是手填工具参数或编辑运行文件：

1. 派发失败：查看已发生效果与原 Chat → 必要时普通 Resume 进入已存在 Chat → 核实「已创建 / 未创建 / 不确定」→ 将结论和依据告诉管理者，由其 reconcile_dispatch。不确定继续 failed，不盲目重复 dispatch。
2. 执行失败：确认原 Chat 和当前原因 → 人工修复现场 → 告诉管理者「已修复，继续跟踪原执行」，由其 recover；需要恢复原生 Chat 进程时另用普通 Resume，它不代替 recover 或接管 Run。
3. 人工处理后已可交接：告诉管理者结果地址、限制与确认依据，原 Chat 先提交如实的当前 true 报告，再由管理者显式 complete；缺少当前 true 时拒绝，不制造假报告。下游只获得资格，管理者必须再显式决定派发。

manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 必须包含上述动作矩阵、人工确认责任、禁止自动行为与错误/部分生效语义，node-chat 必须明确 report 不改变状态、失败后补充材料仍使用已有 Chat。**本仓库没有外部 toolkit 角色工具源码**，当前 HTTP 接线与说明不代表实际角色工具或 Prompt 已交付。metadata 指向的 `manager-chat.ts` 入口未部署时，真实工具/Herdr/omp 演练仍受阻；不能让用户手填 sessionId 或用受控原生身份宣称已验收。

## 暂停新执行与恢复派发资格

管理者的 `pause` / `resume` 都没有模型业务参数。传输正文仅由公共 Session 能力提供真实身份，后端仍按唯一管理者关联与同机来源检查；node、未关联身份及额外的 Run/node/角色字段不能借此修改状态：

```http
POST /api/workflow/manager/pause
Content-Type: application/json

{"sessionId":"<公共 Session 能力取得的当前管理者身份>"}
```

`resume` 使用相同正文，路径为 `POST /api/workflow/manager/resume`。不要把普通 Chat 的 **Resume**（定位或恢复原生对话）当作此状态动作；普通 Resume 不接管 Run，也不恢复派发资格。

- `pause` 将 running Run 转为 paused，排在同 Run 已接受的完整派发批次之后生效。它不关闭或终止任何 Chat、不等待它们停轮或退出。生效后 `ready=[]`，新 dispatch 的逐项结果为 `rejected`，原因含 `Run 状态为 paused`，不产生宿主启动调用。
- 已有执行仍可 report，管理者仍可在确认可交接后 complete。报告仍只通知管理者，不自动完成。paused Run 满足完成条件（普通工作节点全部 completed，结束节点如有则已成功派发）后可以直接 finish，无需先 resume，活动 Chat 不阻止成功。
- `resume` 只将 paused Run 转回 running，重新给出由当前事实与依赖派生的 `ready` / `blocked`。它不自动 dispatch、retry、补发输入或终止已有 Chat；管理者必须另外显式选择目标并 dispatch。
- 已在目标状态时重复 pause/resume 返回 `changed=false` 与当前查询字段，不重写当前记录、不覆盖首次操作 ID/时间/actor、不重复追加成功历史，也不产生外部调用。一次真正的反向转换后再次调用是新的显式状态动作，不是重复请求。
- completed 终态的 pause/resume 返回 HTTP 409、`code=run-terminal`、`changed=false` 和当前查询字段，不改变首次 finish 事实。重复 finish 仍返回首次结果。

成功动作返回与 complete/finish 相同的 `snapshot`、`ready`、`blocked`、`canFinish`、`reports`、`pendingReports`、`changed` 与 `historyError`。当前状态在 `run.json`，首次生效转换在 `history.jsonl` 中保留 `id`、`at`、真实管理者 `actor`、`action=pause/resume`、`before`、`after`。所有检查、写入和返回快照沿用同 Run 动作串行边界；文件锁不跨宿主调用。当前事实保存失败返回 HTTP 500、`pause-failed` / `resume-failed`，不采用未提交状态、不追加成功历史；历史追加失败则如实返回已生效状态和非空 `historyError`，不回滚、不自动修复或重试。

### 完整批次与并发调用

同 Run 的业务串行边界覆盖**整个显式派发数组**（包括逐项拒绝、启动失败、其余目标的实际启动及最终结果/快照），不是只覆盖某个 node 或一次文件写入。在途批次 `[A,B]` 尚未返回时提交 pause，pause 不能在 A 与 B 之间生效，也不能撤回 B 或终止 A；它在完整批次之后检查最新事实并保存 paused。批次响应仍描述该批次完成时的 running Run，后续 pause 响应描述 paused Run，两个响应都保留已启动 A/B 的事实，不互相覆盖。等待中的后续 dispatch 同样重新检查最新状态与依赖；先前 status 的 ready 或等待前的 pending 不是启动授权。

文件协调与业务排队不同：宿主启动/输入受阻时，同 Run 的只读 status 仍可读取**已提交**记录（例如初始输入尚未确认，节点仍 pending，但真实 Chat 关联已保存），不保证只读查询会等到整批完成。不要把这个中间快照当作可以绕过排队的理由。独立 Run 的动作不排在本 Run 的批次或 pause 后面；已启动 Chat 的活动上报也不受这条业务队列控制。节点 report/管理者 complete 等需要修改本 Run 业务事实的动作仍在同 Run 边界排队，不能把“Chat 继续工作”理解为业务写入可以穿插进批次。

遇到慢派发时，用户应保留现有 Chat 与现场，等待批次结果和 pause 的明确回执，不关闭 Tab、编辑记录或根据旧 ready 重发。manager-chat 应说明这些顺序及中间查询的含义；node-chat 应继续自己的工作/报告职责，不把管理者提交 pause 当作撤回已有任务。宿主报错、当前事实保存失败与历史部分失败继续按原有明确反馈处理，不通过自动重试或假快照隐藏问题。

### 用户与管理者操作指引

人类需要处理现场时，先从 workflow 查看 Run 与节点事实并进入已关联的管理者 Chat，表达「暂停新派发」；由管理者调用 pause，而不是退出所有节点 Chat、关闭 Tab 或直接编辑运行文件。已有 node 可以继续提交含结果地址与限制的 report，管理者可以确认 complete；暂停不表示工作已经完成。处理后表达「恢复派发资格」，由管理者 resume、重新读取状态并决定下一次显式 dispatch。不要将 resume 工具成功当作下游已启动。若全部工作已交接，直接 finish，不必为结束而恢复派发。

manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 必须覆盖上述边界及重复/终态/部分生效反馈；node-chat 应明确 pause 不撤回其已有工作或报告职责。这是角色指引的交付要求，不是本仓库已交付外部工具或已验证真实 Prompt 注入的声明。当前 worktree 只交付 HTTP 能力和本文；metadata 指向的外部角色入口未部署时，真实角色工具与 Herdr/omp 链路仍未验证，不能让用户手填 sessionId 代替。

## 单节点真实宿主闭环验收

多节点后续入口见 [真实 A／B → C 验收与通知取证](workflow-fan-in-acceptance.md)：固定双材料/第三份产物预期、预先写入 C 的材料地址、三个真实通知现场与报告后收尾的顺序证据。该指南不是已通过声明，单节点或角色部署前置条件缺失时不得用多节点准备结果代替。

本节是进入多节点验收前的独立入口，只验证一个无前置 node 的 manager → dispatch → 实际文件/report → complete → finish。即使本节全部通过，也不代表 P62/P63 的 A/B → C、并行、汇合或多轮推进已经通过。HTTP TestServer、记录型 Herdr 副本和手填身份请求都不能代替本节。

### 独立预期与观察 seam

开始前先保存本轮预期，不能读取实际产物后再改预期。固定 node ID 为 `ONE`，`blocked_by=[]`，输出为 node zone 根目录的 `single-node-result.txt`。文件必须是无 BOM 的 UTF-8，行尾仅 LF，末尾有一个 LF，完整内容固定为：

```text
workflow-single-node
answer=42
```

启动前保存这个字面预期，不从模型报告或服务返回值生成。将 `$nodeZone` 设为本轮实际节点目录，用下列 PowerShell 独立读取原始字节；缺文件、BOM、CRLF 或正文不同均失败，不生成替代文件。

```powershell
$expected = [Text.Encoding]::UTF8.GetBytes("workflow-single-node`nanswer=42`n")
$artifact = Join-Path $nodeZone 'single-node-result.txt'
$actual = [IO.File]::ReadAllBytes($artifact)
if ([Convert]::ToBase64String($actual) -cne [Convert]::ToBase64String($expected)) {
    throw "实际产物字节与独立预期不符：$artifact"
}
Write-Output "产物字节符合独立预期：$artifact"
```

声明一个输出 ID `result`。`node.input` 要求写入上述文件、完整读回、通过实际 node 工具批准成果，然后在真实 `report` 中以 `result` 登记绝对地址并说明编码与限制。管理者核实输入后只派发 ONE，亲自读取登记文件，与启动前的字面预期逐字节比较。批准事实、最新 true 报告、约定输出和文件核对均满足后才 complete ONE；用非空 reason 记录地址与依据，再显式 finish。

观察 seam 是实际交付页面的发起 Run 入口、真实 manager/node 角色工具及其公共 Session 身份、node 实际文件、三份运行记录与 Herdr 原生 Chat。可区分的具体违规是“node 报告成功但未写文件或内容错误”：仅凭 report 或 completed 快照会漏过；独立读取实际文件并比较完整字节才能拒绝它。此真实场景未可执行时，不制造受控模型的 red/green 来声称覆盖。

### 隔离与执行顺序

1. 按[测试指引](agents/testing.md)建立独占服务、数据、manager zone、node zone 和 Herdr 实例。使用数值回环动态端口，禁用 mDNS 和真实关机，三份运行记录放在 node zone 外。先核实原生角色可加载且 omp 未启用 autoResume；不能隔离或加载时保留错误，不用用户会话、空扩展或手写记录代替。
2. 通过真实页面建立上述 `ONE` Map，保存启动前预期与实际定义。经 scheduler 启动；真实 manager 先 `prepare_inputs` 并查 status，再派发 ONE。记录两个独立原生身份、实际 cwd 和初始输入、Prompt 与工具交互。
3. 按上述字面值实际写入、批准、报告、核对原始字节、显式完成及结束。记录每次人工继续指示及原因，不能由脚本冒充角色，不能从最终 completed 推断字节、批准或通知已通过。
4. 保存产物原字节、实际对话与工具结果、三份记录及人工干预。检查首个完成说明、结束事实和报告历史，分别记录缺失、部分生效及通知失败。只清理本轮拥有的进程与资源，原始失败和清理错误分开。

## 节点报告与独立通知

Map node 的可选 `auto_approved` 只接受布尔，省略默认 true；派发时冻结为 `execution.autoApproved`，初始 `approved=false`、`approvalSource=null`。自动模式仍须执行者主动 approve，不能派发即批准；人工模式只有官方宿主原生确认中的显式 Yes 才提交 manual 来源。原生确认只询问“当前会话的成果是否已达到预期，可以交接并推进后续流程？”，不要求成果说明、报告或文件；普通聊天同意与报告声称批准均不能替代。拒绝、取消、超时或无 UI 不提交批准、不变 failed，也不自动重试。HTTP 的同机调用只能验证后端事实，不证明原生 Yes。

恢复原 Chat、recover 和派发核实沿用已保存执行模式与批准事实，不从当前 Map 改写；retry 是新执行，读取当前定义并重新从未批准开始，不继承旧批准或旧报告的完成资格。旧 execution 的模式、批准及报告仍保留；在途旧提交即使 retry 后才结束，也不能批准新 execution。正文与材料不冻结，批准后修改或报告不自动撤销批准；最新报告为 false 时仍拒绝 complete。

批准只在持久写入成功后返回成功。真实存储失败返回 `approval-save-failed` 并保留底层错误，公开读取仍未批准，不能以失败响应、日志或尚未保存的内存对象绕过 complete 门禁。同次重复／并发批准只有一次 `changed:true`，不产生额外报告、通知、完成或派发。确认发生在宿主，不占用后端 Run 操作资格；同 Run 查询、报告及无依赖分支不必等用户确认。

后端进程退出后，公开 Run 详情仍可读取同次 execution 的冻结模式、批准及来源；重启不重放业务输入、不接管旧 Run，也不为旧 Chat 新增批准或管理权限。原 Chat 的合法恢复与后端重启后的只读历史是不同边界。无旧 Run 字段适配、静默补批准或历史改写；不承诺继续或呈现旧版本 Run。

报告发送完全独立：批准前后都可 report，不需要原生发送确认，不改变批准；approve 不制造报告或通知 manager。正常成果交接由 node-chat 先 approve、后 report，后端不强制排序。manager complete 另行要求批准、当前最新 canComplete=true 和全部约定交付登记；批准本身不会 complete、dispatch 或 finish。结束节点不批准、不伪造 approved=true，由管理者显式 finish。节点无发送确认的规则不适用于 `chief_report`；后者保留独立确认，见[用户操作说明](usage.md)。

已关联的 node Chat 用一次 `report` 提交简短 `body`、必填 boolean `can_complete` 和可选 `outputs` 数组（每项 `id` 与本机绝对文件地址 `path`）；toolkit 应映射为 HTTP 的 camelCase `canComplete`。正文说明完成情况、阻塞、限制、部分效果及需要的干预，不重复完整成果。业务文件由 node 或外部程序生成；无需输出文件的工作也可声明 true。不要求 node 自报 Run、node、执行 ID 或接收者。角色扩展须从 toolkit 公共 Session 能力取得当前原生身份，并调用：

```http
POST /api/workflow/node/report
Content-Type: application/json

{"sessionId":"<公共 Session 能力取得的当前真实身份>","body":"结果已生成；限制与干预说明","canComplete":true,"outputs":[{"id":"summary","path":"C:\\results\\summary.md"}]}
```

新请求必须提供字符串 `body` 和严格 boolean `canComplete`；缺字段、null、字符串布尔值等错误类型明确拒绝，不保存报告，不为旧请求默认放行。`outputs` 若提供必须是 `{id,path}` 对象数组；`id` 必须非空、不重复且属于本次执行冻结的 `outputRule`，所有 `path` 必须为本机绝对地址且不含 NUL。旧对象格式、未知字段、未声明／重复 ID、相对地址或错误类型拒绝整次报告，不部分保存。省略或空数组不删除已有 ID，不增加删除协议；可发送无输出的进度或失败报告。约定 ID 缺失不阻止 report，但会阻止管理者首次 complete；完整门禁见[显式交接完成](#显式交接完成与结束-run)。沿用无查询参数、1 MiB 请求上限及同机来源限制。

后端按当前后端已管理 Run 的真实 node 执行关联归属，拒绝无关联、没有合法父 execution 节点角色的管理者身份、重复关联及调用方填写的 `runId`、`nodeId`、`from`、`to`、旧 `source`／`target` 等额外字段。系统生成报告 `id`、`at`、`type=report`，保存 `runId`、`nodeId`、`executionId`、真实发送方 `from` 和同 Run 管理者接收方 `to`。sessionId 不是模型参数，也不是秘密凭据；同机信任限制与 manager 工具相同。重启后的旧 Run 不接受 report。

输出登记唯一权威来源是 `messages.jsonl` 中报告的 `outputs` 数组；同一 Run/node/execution 按追加顺序按 ID 累积覆盖，例如 A=a1/B=b1 → 仅 A=a2 → 省略 outputs，足以推导 A=a2/B=b1。新 execution 不继承旧集合，保留的旧执行报告仍归旧 execution。Run 当前事实、背景、目录或状态索引不另存登记副本；派生查询不成为独立权威存储。登记仅校验地址表达，不读取或检查存在、可读性、格式、数量、完整性，也不生成、复制、移动、发布业务文件；同卷、异卷及 zone 外地址均可登记，不存在文件也合法。历史报告缺 `canComplete`／`outputs` 保持可读，不补出 true 或输出集合，也不因此恢复旧 Run 的写入／管理资格。

每次成功提交都在 `messages.jsonl` 追加独立报告，包括同一执行的重复报告；不修改 `run.json` 或执行历史，不自动完成、暂停、重试或派发。暂停或已完成现场的当前关联 node 仍可补充事实，不重新打开已完成工作。报告记录缺失、损坏或尾部没有换行时明确报错，不当空文件重建、不截断后继续写。

保存完成后才向目标管理者真实 agent 提交一次 Herdr 输入，正文明确标出 Run、node、报告 ID 和报告正文。文件锁及 Run 操作锁不跨投递。Herdr 提交确认仅表示接入层确认输入提交，不表示已阅读、严格顺序、原生 IRC 或必然打断正在进行的工作。

返回结果分阶段解释：

- 保存失败：HTTP 500，`code=report-save-failed`、`saved=false`、`notification.status=not-attempted`；不生成报告待办、不调用 Herdr。
- 保存成功：HTTP 200，`saved=true` 及系统生成的完整 `report`；`notification.status` 分别为 `submitted` 或 `failed`，`reason` 说明提交结果。投递等待时，已保存报告已可直接读取，不必等待通知完成。
- 诊断单独返回 `diagnostic.status=written|failed`。诊断写入失败也不撤销报告、不重投、不暂停。不因诊断失败把已保存报告假报为保存失败。

**仅通知失败或诊断失败时不要重发 report。** 已保存事实仍在，管理者应直接读报告并显式处理交接；report、停轮和退出 Chat 都不代表 node completed。确有新的结果或需要更正正文时可以再 report，会产生新的独立历史，不覆盖旧记录。

Run 详情返回 `reports` 完整追加历史及 `pendingReports`（node ID → 最新适用报告 ID），manager status/dispatch 的快照同样提供这些字段。待办仅取当前执行、当前仍可交接完成的 node（running，或 execution 阶段 failed）；completed 不列为待办。按追加顺序取同一 node 当前执行的最新报告，不借旧执行完成新执行。连续读取和 Herdr 通知提交都不清除待办。这是保存记录的派生视图，不是第四份业务记录；collect 的等待与检查语义应由 collect 能力单独验收。

投递诊断使用 JSON stderr 记录 `event=workflow-report-delivery`、Run、报告 ID、完整目标 Chat 引用、`result` 及类型化 `reason`，不记录正文或可能回显正文的宿主错误详情。输入确认原因是 `input-submission-confirmed`，失败原因沿 Herdr adapter 错误类别。调用者仍从通知返回值取得具体错误。诊断不写入三份业务记录，也不成为重试队列。

### 节点公共 Prompt 的部署要求

node-chat 的实际工具与公共 Prompt 属于独立 my-agent-toolkit 交付。其 `PromptDeclaration` 应使用 `first-enabled-run` 注入本节报告职责：何时交接、简短正文及限制、必填 boolean can_complete 和可选命名 outputs、报告不等于完成、保存与通知分离、仅通知失败不要重复 report；不以工具 description 代替注入。部署仍须将固定 node 角色入口接到公共 Session/HTTP/Prompt 与自动启用策略，配置稳定 endpoint 与预算，不依赖 node cwd 或 connect-bot 开关。本仓库的 HTTP 协议、静态入口配置或说明文字都不构成外部扩展已装配、原生工具实际调用或 Prompt 已注入的证据。

## 收取报告、等待与取消

管理者角色的 `collect` 没有必填业务参数。角色扩展必须从公共 Session 取得当前原生身份，而不是让模型填 Run/node/执行 ID。后端入口是 `POST /api/workflow/manager/collect`，传输体仅为 `{"sessionId":"当前原生身份"}`；身份必须唯一关联到本后端当前管理的 Run。未知/node 身份拒绝，额外业务字段拒绝；不接受自报归属。后端重启不恢复检查位置或等待者，旧 Run 不因存在已保存报告获得 collect 资格。

成功结果包括 `reason`、`snapshot`、`pendingReports`（node ID → 最新适用报告 ID），以及与 status 相同的 `ready`、`blocked`、`canFinish`。**collect 不返回 `reports`，也不重复报告正文。** 管理者按标识读取已传入的 `messages.jsonl` 或 Run 详情，然后亲自核对交接材料；通知检查不是已读回执，也不是 complete。连续 collect 不会清除尚未完成的待办。

| `reason` | 现场与下一步 |
| --- | --- |
| `paused` | Run 已暂停，立即返回当前快照；已有 Chat 可以继续 report/交接。由管理者决定后续 complete、resume 或满足条件后 finish。 |
| `completed` | Run 已显式结束，立即返回终态快照；不重新打开工作。 |
| `notification` | 至少一份保存成功的报告尚未经过 collect 检查；返回最新现场，通知可以合并，待办按 node 汇总。 |
| `no-running` | 没有未检查通知，也没有 running node，避免无限空等；根据 ready/blocked 显式派发、处理失败或 finish。 |
| `timeout` | 等待正常到期；返回当时快照，不把 Run/node 改成 failed/paused，不自主派发或重试。 |

暂停/结束优先返回自身原因；这些成功快照也检查此前已有通知。通知 generation 与检查位置仅在内存，无人等待时仍保留；报告必须先保存才产生通知，不依赖 Herdr 正文投递完成。一次检查只覆盖同一操作入口内的已保存现场，检查后的新通知留给后续 collect。状态变更可以唤醒等待以重新检查，但不能冒充报告通知。已检查通知后，即使 `pendingReports` 仍保留同一报告，只要仍有 running node 就继续等待新通知；门禁拒绝不增加 generation，不使等待立即返回。管理者遇到 false 或缺失声明时应等待后续真实报告，不反复对同一报告 complete。不额外规定“有 ready node 就必须立即返回”：仍有 running node、无新通知时可以等待。

后端通过既有服务配置 `[scheduler].collect_wait_seconds` 设定等待上限，默认 **5400 秒（90 分钟）**，正整数、最大 4294967295。更改后重启生效。等待不占用文件锁或 Run 状态操作入口；pause、complete 与 report 不需等 collect 到期。不存在 collect 游标文件、持久等待者、后台补投或业务轮询循环。

调用方公共 HTTP 的**整个请求预算必须大于后端期限**，默认建议 `timeoutMs=5460000`（91 分钟）；短配置如后端 1 秒，调用方可用 3 秒。不要复用 connect-bot 的短上报预算。HTTP 200 响应头可能先于等待结果到达，只有完整、合法 JSON 正文才是成功快照。等待阶段发生读取故障时后端终止未完成的响应体，不生成正常 timeout 或空快照；初始检查失败返回明确错误。调用方遇到取消、断线、预算耗尽或读取失败必须反馈错误，**不自主重试、不伪造正常结果、不据此改变运行状态**。需要再检查时由管理者明确发起新的 collect。

### 角色调用方及公共能力的部署边界

后端等待由 HTTP 响应体持有，不派生脱离请求的任务；真实连接断开会释放等待和通知接收者，随后新请求仍能收取新的通知。工具中断与 Chat 结束必须经 toolkit 公共 HTTP 的调用方取消能力断开正在进行的请求；取消不保证取得任何快照。manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 应注入上述原因、交接核实责任、预算与取消/网络错误处理，不以工具 description 代替实际注入。

角色入口 `manager-chat.ts` 与 `node-chat.ts` 由 my-agent-toolkit 提供。按部署的 Task 元数据加载对应宿主的入口，使用公共 Prompt、Session 和 HTTP 能力，不依赖 connect-bot 是否启用。公共 `HttpRequestOptions.signal` 传入工具生命周期的 `AbortSignal`，调用方区分 `cancelled`、后端正常 `timeout` 和请求失败。使用稳定的配置基准、有效 endpoint 与完整请求预算，不从 node cwd 推导。入口能找到或类型支持取消，不证明本次原生工具已接线；真实加载、指引注入、工具中断与 Chat 退出须分别观察。

## 启动失败

初始化失败不启动 Chat。原生身份或关联保存失败不提前提交指示。已启动的 Tab/agent 不会被假报回滚；初始指示提交失败也不代表外部端绝无接收效果。详情列出可得的 Workspace、Tab、pane、agent 及原生身份，说明哪些步骤已确认完成。先核实这些引用，再处理现场；不要盲目再次发起，异常窗口不承诺完整幂等。

管理者启动错误只保存在当前后端内存，刷新仍能查询。诊断输出包含 Run、失败阶段及已发生效果，不重复本次要求正文。错误不写入业务历史；后端重启后不重建页面错误，只保留持久事实与既有诊断。没有自动重试、自动关闭 Chat 或自动派发节点。
