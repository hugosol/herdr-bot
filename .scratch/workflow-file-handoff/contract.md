# Contract — Workflow 文件交接与 report 完成声明

Source: [spec](spec.md)
Acceptance mode: Full automation
Approval: approved — seam 选择、约束审查、验收模式均获用户显式确认（2026-11-01）。

本契约覆盖 herdr-bot 与 my-agent-toolkit，以及二者在实际 Chat 中的交接。批准表示承诺和验收分工已确定，不表示实现或验收已完成。Promise 完成情况由交付工作和验证证据推导，不手工维护状态列；交付票据映射由 /to-tickets 补充。

## Promises

所有 Coverage 均由 agent 承担。预期来自本契约、独立字面样例或明确的业务要求，不从被测实现重算。真实宿主证据不能由受控 adapter 冒充；缺失证据如实报告为未验证。

|#|Promise (one observable result)|Coverage|Source|Seam|
|---|---|---|---|---|
|P1|Map 可保存并读回命名输入，每项明确选择直接文件或来源 node 加输出名；既有无文件输入 Map 仍可使用。|自动：两类输入的独立字面样例、无输入样例及非法类型拒绝；保存失败不替换原定义。|stories 1–4,35|S1|
|P2|直接文件输入与 report 输出地址必须为绝对路径，且不因地址在任一 zone 之外而拒绝。|自动：同卷、不同卷及 zone 外的绝对地址；相对地址拒绝；类型或地址表达失败不产生登记。不得以文件存在性作为预期。|B1 approved；stories 2,12,14|S1|
|P3|Map 中引用不存在的来源 node 时保存被拒绝；输入关系不会隐式添加 blocked_by 或自动派发。|自动：拼错来源 node 的定义保存失败并保留原定义；合法输入引用与独立依赖声明按原样读回，未显式 dispatch 不启动。|B2 approved；story 4|S1|
|P4|节点初始化获得本次 userInput、原业务 input 和已解析的命名输入地址，不携带上游完整聊天或业务文件正文。|自动：捕获实际初始化输入，与独立背景、业务 input 和路径字面值比较；使用独立敏感标记辨别不应注入的正文与聊天。真实宿主验证实际进入 Chat。|stories 8,9|S1,S2|
|P5|上游输入地址只从本 Run 来源 node 当前 execution 的结构化输出登记解析，不从正文、时间或旧执行猜测。|自动：两个 Run、重试前后、同名输出与误导性正文的组合；下游实际获得独立预期地址。|stories 3,9,25；Implementation Decisions 2|S1,S2|
|P6|来源 node 存在但所需输出尚未登记时，下游仍可按既有派发规则启动，初始化明确指出该输入未解析。|自动：依赖已满足而输出缺失，观察实际启动及缺失逻辑名；不伪造地址，不撤销上游完成。|stories 10,11；B2 approved|S1,S2|
|P7|实际 node-chat 通过同一个 report 工具提交 body、必填 boolean can_complete 与可选 outputs，不要求模型提供运行身份。|自动：工具实际可发现并可调用；true/false、缺字段、错类型、省略 outputs；工具与后端请求一致。既有基本参数限制继续生效。|stories 13–17,31|S1,S2|
|P8|每条报告及其输出登记被绑定到调用者真实 Session 所属 Run、node、execution；伪造或不适用的归属不能改写当前事实。|自动：真实身份注入、非法调用者、两个 Run、保留旧 execution；检查返回与落盘归属，不以模型自报身份作为输入依据。|stories 14,25,31|S1,S2,S3|
|P9|同一 execution 的输出登记按名称累积更新；省略 outputs 或未提供某名字保留原登记。|自动：先登记 A=a1、B=b1，再只登记 A=a2，后续省略 outputs；下游获得 A=a2、B=b1；新 execution 不继承旧集合。|stories 15,16；Implementation Decisions 3|S1|
|P10|report 登记不检查文件存在、可读性、格式、数量或完整性，也不生成、复制、移动或发布业务文件。|自动：登记合法绝对地址但不存在的文件仍成功；报告前后业务文件保持独立字节预期且不创建目标；无输出的 true 报告合法。|stories 12,15,35；Implementation Decisions 3|S1,S3|
|P11|输出登记的唯一权威来源是报告记录，Run 当前事实和背景不出现新的权威副本或注册表。|自动：报告写入与派发解析的结果一致；检查实际持久记录及业务目录，无第二套登记、状态索引或背景副本。|stories 28,30；Implementation Decisions 1|S1,S3|
|P12|报告保存失败不产生输出登记、报告待办或宿主通知。|自动：在测试拥有的存储中制造保存故障，观察错误、原记录和宿主调用；恢复读取不出现失败候选事实。|Implementation Decisions 3；inferred: required to fulfill existing promise|S1,S3|
|P13|报告保存成功而通知失败时，登记与报告仍可查询，返回明确分阶段结果且不自动重报或回滚。|自动：控制通知失败并保留真实文件写入；查询报告和下游输入解析；在有界窗口观察无自主重投。|Implementation Decisions 3；inferred: required to fulfill existing promise|S1,S3|
|P14|首次 complete 只有在当前 execution 最新报告的 can_complete 严格为 true 时才通过新增门禁，并继续遵守既有状态规则。|自动：true/false、无报告、历史报告缺字段、旧执行 true、新执行 false、true 后 false；没有文件输出不影响该门禁。|stories 21,22,25,26|S1|
|P15|门禁拒绝不改变节点状态，并返回明确拒绝原因及相关最新报告信息（如存在）。|自动：比较拒绝前后当前事实与历史，核对独立原因和报告 ID；无报告时明确表达不存在，不制造引用。|story 23|S1,S3|
|P16|报告提交与首次完成具有一致先后：false 先保存则完成拒绝；完成先生效则后续 false 不追溯撤销。|自动：受控并发请求与已提交报告证据，确认结果可解释为上述先后关系；观察对外结果而非内部锁。|Implementation Decisions 4；inferred: required to fulfill existing promise|S1,S3|
|P17|report(true) 不自动完成或派发；manager 仍显式决定 complete 和后续 dispatch。|自动：报告后观察状态与宿主调用；随后显式 complete/dispatch 才发生相应转换。真实链路保存工具顺序。|story 21|S1,S2|
|P18|已 completed 节点重复 complete 保留既有幂等语义，补充报告不撤销完成或重开节点。|自动：完成后补 false 再重复 complete，首次完成事实不被覆盖，不重复触发完成副作用。|story 27|S1,S3|
|P19|can_complete 门禁不能被人工完成分支或 manager 单方面判断绕过。|自动：适用的既有人工完成入口在无当前 true 报告时同样拒绝；不新增 force 绕过参数。|Implementation Decisions 4；approved boundary review|S1|
|P20|collect 继续以新通知判断返回或等待，不因 pendingReports 中保留同一报告而重复立即唤醒。|自动：有 running 节点且已检查通知后开始等待，新报告唤醒；保留超时、取消及状态变化返回的既有行为。门禁拒绝不制造新报告通知。|story 24；Implementation Decisions 5|S1,S2|
|P21|实际 node-chat 公共指引要求正确声明完成情况并报告必要阻塞、部分效果和干预事项，而非重复完整成果。|自动：观察实际注入及真实工具调用；业务结果使用外部程序生成也可 report；结果全文只在文件中，报告保留足够编排信息。|stories 12,18,19,32|S2|
|P22|实际 manager-chat 阅读报告作编排判断，不默认验收输出正文；门禁拒绝后使用已有等待机制，不反复对同一报告尝试完成。|自动：隔离 Chat 场景中的完整调用和对话证据，覆盖先 false 后 true 及 complete 被拒绝的路径；不以指引文本存在代替运行行为。|stories 20,23,24,32|S2|
|P23|业务合并、转换与必要内容验收由 Map 安排的 node 承担，框架只解析登记地址而不解释文件内容。|自动：两个消费者读取同一登记输入，独立产出后由汇总 node 消费；独立字面内容证明路径指向正确结果，不新增框架合并动作。|stories 5–7,12|S1,S2|
|P24|manager zone 下按 Run ID 区分的中间资料位置不限制输出登记地址，也不与 node 的执行 zone 绑定。|自动：manager zone、node zone、业务产物目录互不相同；不同 Run 的资料位置可区分；外部产物直接登记且下游取得原地址。|story 29；B1 clarification approved|S1,S2|
|P25|历史报告缺新增字段仍可读取，持久报告和运行事实在后端重启后仍可观察，不因此接管旧 Run。|自动：独立历史样例及实际重启读取；缺字段不伪造 true；既有旧 Run 只读规则保持。|Implementation Decisions 6；inferred: required to fulfill existing promise|S1,S3|
|P26|工具和后端配套采用新 report 契约；缺少 can_complete 的新请求明确拒绝，不默认放行。|自动：实际 toolkit 新调用成功、旧形状新请求失败，且失败不写报告；历史读取与新请求校验分别验证。|Implementation Decisions 6；approved boundary review|S1,S2,S3|
|P27|精简后的 plan_workflow 在实际宿主中完成 outline → generate_map 交接，node 不再自行解析运行日志或筛报告找文件。|自动：隔离的真实 Run，记录原生身份、初始化内容、工具请求和结果、实际文件及 manager 显式推进；不通过手改运行记录补流程。|story 33|S2,S3|
|P28|plan_workflow 保留原有人工确认、九章节大纲要求、业务权限及不自动执行新业务 Map 的规则。|自动：实际 Chat 中受控用户明确确认，生成文件按独立业务要求核对，记录全部干预；不以 node 自述证明用户批准。|story 34；Implementation Decisions 6|S2|
|P29|现有 workflow 继续呈现同一 Run 的节点完成与报告事实，不引入新的状态来源或页面动作。|自动：复用既有 workflow 回归并核对其数据来自同一 Run；无需新增浏览器 seam，也不以 rendered completed 替代报告门禁证据。|story 28|S1|

## Seam decisions

|Seam|Exposes|Hides|Alternatives considered|Reversibility|
|---|---|---|---|---|
|S1 — 现有 Map／Run HTTP interface|静态声明、报告调用与身份归属、完成和派发结果、状态查询、collect、错误与取消语义；herdr-bot 为主要验证对象，toolkit 是实际调用者。|输入解析、报告归并、存储协调和状态判断的内部组织。|拒绝以私有 helper 测试代替调用者可见承诺：它不能证明完整请求链或状态效果。复用现有 interface，不新增测试专用入口。|验证组织容易调整；外部请求字段和错误语义变化会影响 toolkit 及既有调用者，变更须同步契约。|
|S2 — toolkit 角色工具与实际 Chat interface|真实可调用 report、自动身份传递、角色指引、初始化信息、通知与 manager 行为，以及两个仓库实际组成的交接闭环。|toolkit 内部工具装配、HTTP 转换和宿主 adapter 的组织。|仅验证 HTTP 会漏掉注册、部署与实际注入；仅检查提示词会漏掉运行行为。保留实际 Chat 观察，不为每个仓库重复划一个 seam。|角色内部实现可调整；工具字段与指引变化影响模型调用，需要保留跨仓运行证据。|
|S3 — 既有持久记录文件 interface|报告登记的真实保存、历史可读、单一事实来源、损坏或写入失败的外部结果，以及重启后的可观察事实。|序列化与协调实现细节，不把私有锁、缓存或临时文件命名规定为契约。|仅通过返回值无法证明落盘与无第二套来源；保留既有持久记录观察，不新增产物 registry interface。|运行记录是既有外部可读格式；未来迁移载体会影响直接读取者和故障证据，需要重新评估本 seam。|

### 批准记录与已选取舍

- Seam 选择：用户明确批准 S1＋S2＋S3，覆盖两个仓库和实际跨仓交接。不单独增加浏览器 seam；保留相关既有回归。
- 约束审查 B1：直接输入和登记输出使用绝对路径；与 node zone、manager zone 解耦，不因 zone 外地址拒绝。拒绝隐式按 cwd／zone 猜相对路径，因为同一地址会在不同 Chat 中产生歧义。绝对路径表达不构成授权或存在性保证。
- 约束审查 B2：不存在的来源 node 是静态定义错误，保存拒绝；存在 node 但未登记输出是业务交接缺失，仍可派发。拒绝把两者合并为运行时门禁，避免改变已批准的 node 侧处理职责。
- 验收模式：用户选择 Full automation；无 human-led coverage 例外。真实业务中的用户确认动作由隔离场景的明确受控输入实现，不用 agent 自述代替确认。
- 唯一事实来源、登记不验证文件、完成门禁和无绕过入口均继承 spec；不另引入审批、发布、版本或自动恢复。
- 不新增 ADR：本次明确选择复用既有 interface，没有另引入需要独立 ADR 承载的高成本基础设施取舍。未来实际实施推翻 seam 时，先修订契约，再调整票据；满足 ADR 条件时另行记录。

### 依赖类别与验证约束

- In-process：地址引用解析、报告归并和门禁判断通过批准的 interface 观察，不把内部 helper 纳入契约。
- Local-substitutable：文件系统使用测试拥有的隔离目录及故障夹具，不创建面向假想数据库的 interface。
- Remote but owned：toolkit 与 herdr-bot 复用既有 HTTP adapter；不增加第二套归属或协议。受控检查不取代配套运行。
- True external：Herdr 与 OMP／pi 的受控 adapter 承担确定性故障场景；实际角色注册、初始化注入及 plan_workflow 交接需要真实宿主证据。
- 所有运行遵循仓库的独占存储、动态数值回环端口、进程所有权、用户数据保护与显式环境授权规则。缺少真实依赖时报告具体阻塞，不降低承诺或声称已通过。

## Not yet specified

无阻止本契约批准的决策。

- 具体 Map 字段拼写、返回错误标识和物理中间目录名称，留给实施按现有风格选择并更新调用者及文档；不得改变两类来源、绝对路径、未解析输入可派发等已批准行为。这些不是新增 seam。
- 执行器若未来增加脚本、产物若增加版本或发布，再讨论相应 interface；本次不预先拆票或提供占位实现。
- 未来若需要相对路径、远程机器文件或非文件产物，必须重新评估地址契约；当前只承诺本机绝对文件地址，不预设新 adapter。

## Out of scope

继承 [spec 的 Out of Scope](spec.md#out-of-scope)，不增加或缩减其范围。特别保留“登记不验收文件”“输入未登记不阻止派发”“不建设第二套事实来源”三项限制；本契约 B1 的路径表达校验、B2 的静态来源 node 校验不构成业务文件校验。

## Coverage review

- 每条 promise 均映射至少一个 seam：P1–P29，无例外。
- 每个 seam 均承载 promise：S1、S2、S3，无空 seam。
- 六类表面均已检查：entry 覆盖定义、工具与动作调用；data and state 覆盖登记、当前执行、唯一来源与历史；external dependencies 覆盖宿主、文件系统和跨仓调用；errors and failure 覆盖拒绝、保存／通知分段与取消；output and observability 覆盖初始化、报告、错误信息和实际成果；resources and concurrency 覆盖隔离资源、执行归属、报告／完成先后与 collect 等待。
- 无整类不适用表面。局部不新增事项：浏览器无新交互；文件字节并发、冲突检测与资源配额并非本次承诺，不因本轮审查扩张范围。
- Not yet specified 仅含实施命名和未来扩展；无批准阻塞项。
- Acceptance mode：Full automation。Human-led coverage：none。
