# Contract — Bot 模板管理与从 Zone 创建 Bot

Source: [spec](spec.md)
Acceptance mode: Rapid iteration
Approval: Approved — 接缝选择、约束审查（含 B1 / B2）、验收模式及所提两组人工覆盖均已确认。

## Promises

Coverage 中“自动”由 agent 完成实现、TDD、自动检查及适用的运行验证。所有期望均来自 spec、独立字面样例或独立外部观察，不能调用实现自己的计算路径生成期望。受控 adapter 的记录不能冒充真实 Herdr / agent 的运行结果。

人工覆盖仅限各 Coverage 单元格明确列出的部分：H1 为真实 Herdr 下的四种 agent / model 组合（omp、pi 分别搭配两个已确认模型）的实际创建、目录、新对话和初始指示投递；H2 为用户日常手机的实机操作体验。它们不转移实现或其他自动检查责任。H1 不要求模型完成任务，也不把 Skill 的任务执行成功改成创建成功条件。

**Complete implementation and all assigned automated checks before the human-led acceptance round. Planned human-led checks do not pause otherwise executable work. At the final handoff, report the remaining checks together with their prepared evidence; they remain unverified until the user confirms them. Report genuine blockers when encountered.**

承诺的完成状态由交付工作、所有要求的检查结果及对应人工确认共同派生，不维护手工状态列。交付切片由 /to-tickets 关联，不在本契约预切实现顺序。

|#|Promise (one observable result)|Coverage|Source|Seam|
|---|---|---|---|---|
|P1|主界面提供独立的“Bot 模板”tab。|自动：真实浏览器进入该 tab，既有 tab 仍可操作。|spec story 1|S1|
|P2|新增模板后可读取完整的已保存值。|自动：公开操作保存独立字面样例，再经独立读取比对字段，不读取私有状态。|spec stories 2,7,11,23|S1,S2|
|P3|编辑模板改变后续创建使用的配置。|自动：先存旧样例再编辑为新样例，下一次创建的外部执行记录匹配预写的新配置。|spec story 3|S2,S3|
|P4|删除模板后，该模板不再出现在可选模板中。|自动：删除后由独立客户端读取和选择，目标条目不可用。|spec story 4|S1,S2|
|P5|独立客户端共享同一份已保存模板。|自动：客户端 A 保存、客户端 B 独立读取，不以同页缓存作证明。|spec stories 5,52|S2|
|P6|模板经 herdr-bot 重启后仍然保留。|自动：隔离真实存储，停止并重新启动进程，再读取预先写定的样例。|spec story 6|S2|
|P7|模板遵守既定的 name、bot-name 必填及 agent 固定选择规则。|自动：公开保存与页面选项分别验证；不借此新增 Skill / prompt / input 的非空组合检查。|spec Implementation Decisions：模板管理与字段|S1,S2|
|P8|命名规则以四个已承诺字段和固定文字生成 Bot 显示名。|自动：为给定 zone、模板、agent 和已知短标识预写完整期望名，例如“alpha · 审查 · a7f3c2”，不复用命名计算函数生成期望。|spec stories 8,10|S1,S2,S3|
|P9|编辑模板时可插入命名字段并查看示例预览。|自动：浏览器插入字段并观察对应示例内容。|spec story 9|S1|
|P10|本次创建预览的名称与实际 Tab 名一致。|自动：对同次创建独立观察页面预览和 Herdr 侧名称，动态标识可捕获但不能由同一命名函数计算两侧期望。|spec story 27 / inferred-required：本次预览须承载实际名称|S1,S3|
|P11|创建弹窗不提供手工覆盖 Bot 名称的入口。|自动：真实浏览器操作确认名称仅预览且由模板规则决定。|spec story 27、Implementation Decisions|S1|
|P12|同名 Tab 按规则原样创建，不附加去重后缀。|自动：移除命名规则中的实例标识后连续创建，在外部独立观察两个不同对象具有相同名称。|spec story 48|S3|
|P13|同名显示对象仍分别接收属于自己的创建配置和输入。|自动：两次同名创建使用不同字面任务，独立外部记录显示各自目标收到正确任务，控制别名不互相覆盖。|spec story 49|S3|
|P14|agent 的固定选项仅为 omp 和 pi。|自动：页面与公开调用分别验证，外部启动目标类型对应所选值；真实正常路径另由相关 H1 覆盖，不转移固定选项自动检查。|spec story 11|S1,S2,S3|
|P15|model 的选项仅为默认值和两个已确认的显式模型。|自动：公开选项和浏览器选择验证，不提供自由输入或额外模型。|spec stories 12–15|S1,S2|
|P16|显式模型使用约定的完整 provider / model 启动配置。|自动：记录四种组合的实际对外交互；GPT 固定为 openai-codex/gpt-6.1-sol，DeepSeek 固定为 deepseek/deepseek-flash；默认值不传模型覆盖。人工 H1：确认四种组合在真实环境启动并显示相应配置。agent 准备四组复现步骤、预期值、Tab / Pane 定位信息、启动记录及已完成检查；账号、费用和登录相关的最终真实运行由用户确认。|spec stories 13–15、Implementation Decisions|S3|
|P17|thinking 选项受当前 agent 和模型能力限制。|自动：使用目标代码的固定能力事实与独立样例核对界面和对外交互，Pi 不收到 auto，非推理模型仅保留默认值；不把参数解析成功当作模型支持证明。|spec stories 16–17、Implementation Decisions|S1,S2,S3|
|P18|model 沿用默认值时，thinking 也沿用默认值。|自动：浏览器及公开调用观察，实际启动不传 thinking 覆盖。|spec story 18|S1,S2,S3|
|P19|切换 agent 会将 model 与 thinking 重置为默认值。|自动：浏览器从已选配置切换 agent，观察两项重置。|spec story 19|S1|
|P20|切换 model 时保留仍适用的 thinking，否则重置并提示。|自动：浏览器分别操作适用与不适用的独立样例。|spec story 20|S1|
|P21|Skill 的可选值仅为无、/teach、/code-review。|自动：浏览器和公开调用观察固定选项，不允许任意 Skill 文本，同时保留“不选”。|spec stories 21–22|S1,S2|
|P22|多行固定 prompt 和用户 input 均允许为空。|自动：保存、编辑、创建使用空值和多行样例；对外投递正文与独立字面值比较，不强制用户补写。|spec stories 23,25–26|S1,S2,S3|
|P23|每个 zone 行的“+”打开该 zone 的创建弹窗。|自动：真实浏览器分别操作普通目录和 Git 工作树对应的 zone，弹窗关联正确目标。|spec stories 24,30|S1|
|P24|每次创建生成独立的 Tab、初始 Pane 和 agent。|自动：同一 zone 两次创建具有不同的外部对象标识，不复用目标。人工 H1：在四组真实运行证据中确认新建对象；agent 准备对象标识和创建前后对照。|spec story 28|S3|
|P25|创建对象位于唯一匹配配置名称的 Workspace。|自动：受控 Workspace 集合含不同焦点目标，仍选择配置目标。人工 H1：确认真实对象位于 bot-playground 或本次显式配置目标；agent 准备定位信息，不只提供成功截图。|spec stories 29,45|S2,S3|
|P26|新 Bot 的实际工作目录是所选 zone 的目录。|自动：普通目录和 Git 工作树使用独立预建目录作为期望，外部记录确认显式 cwd，故意让当前焦点目录不同。人工 H1：核对四组真实对象的实际 cwd；agent 准备预期目录、可独立读取的实际目录信息及复现步骤。|spec stories 29–30|S3|
|P27|目标 agent 就绪之前不会提交初始输入。|自动：受控 adapter 先接受启动但不报告就绪，独立记录无 prompt；目标就绪后才投递。人工 H1：真实运行交叉确认启动到可交互再提交；agent 准备就绪观察与投递的时序证据。|spec story 31|S2,S3|
|P28|Skill 选项生成真正的 /skill: 调用。|自动：两项分别与独立字面前缀 /skill:teach、/skill:code-review 比较。人工 H1：四组真实运行安排覆盖两项 Skill，确认原生 Skill 输入路径而非同名扩展命令；agent 准备输入原文、资源前提和终端证据，不要求证明任务完成。|spec story 33|S3|
|P29|Skill、prompt 和 input 按约定顺序仅作一次初始化提交。|自动：预写含多行正文的完整字面样例，对外记录确认文本、顺序和一次提交，不先发起独立 Skill 轮次。人工 H1：确认真实 TUI 收到一轮合成初始化输入；agent 准备预期文本与终端证据，不能用模拟回显代替。|spec story 32|S3|
|P30|未选择 Skill 时不补加 Skill 调用前缀。|自动：无 Skill 的非空多行样例按独立字面期望投递一次。|spec stories 22,32、Implementation Decisions|S3|
|P31|空值组合仍进入既定提交流程。|自动：Skill-only 与全空样例均从公开入口发起；全空由受控 Herdr 拒绝，不能提前组合校验、补造内容或跳过提交后假报成功。|spec stories 34–35、Implementation Decisions|S1,S2,S3|
|P32|页面分别呈现正在启动和正在提交初始指示的阶段。|自动：分别延迟就绪和提交响应，观察稳定的阶段，而不是碰巧截到瞬态。|spec story 36|S1,S2|
|P33|创建成功只在目标就绪且输入提交确认后出现。|自动：分别阻塞就绪和投递，两者均不能提前成功；均确认后才返回成功。人工 H1：确认真实提交结果与成功展示一致；agent 准备阶段与外部确认对照，不能以任务回答代替提交确认。|spec story 37|S1,S2,S3|
|P34|输入提交确认后，无需等待模型任务完成即可完成创建。|自动：受控输入已确认、agent 仍工作中时，创建返回成功，不等轮次结束或 Skill 任务完成。|spec story 38|S2,S3|
|P35|结果提供 Bot 名称、zone 及已知 Tab / Pane 标识。|自动：公开结果与外部对象独立记录比对，未取得的标识不伪造；H1 相关真实定位证据按 P16、P24–P26 准备。|spec story 39|S1,S2,S3|
|P36|部分失败明确区分已有现场与未确认投递。|自动：Tab 创建后、agent 就绪后及投递超时分别注入故障，核对已知对象及不确定性表述，不把“未知”说成“确定未发送”。|spec stories 35,40|S1,S2,S3|
|P37|错误或超时中断本次创建后续步骤，并保留已创建的对象。|自动：各阶段受控故障后经外部独立查询观察对象保留、后续阶段无动作；浏览器观察连接断开不按此规则取消已接受创建，见 P50。|spec story 42、Implementation Decisions；approved B1|S2,S3|
|P38|失败或超时不会触发自动重建、重发或删除现场。|自动：外部已产生副作用后返回不确定结果，独立记录确认无额外创建、重复投递或关闭动作；不只断言调用包装层返回值。|spec stories 41–42|S3|
|P39|模板编辑或删除不会改变已有 Bot。|自动：创建后修改、删除模板，独立外部观察对象名称、目标和运行不因此被重写或关闭。|spec story 43|S2,S3|
|P40|进行中的创建使用请求被接受时的模板快照。|自动：挂起一次启动，由另一客户端编辑模板，本次对外执行仍匹配旧字面配置，下一次匹配新配置。|spec story 44|S2,S3|
|P41|Workspace 配置缺失、为空或纯空白时，启动失败。|自动：真实 TOML 输入和进程启动结果验证，与合法显式值对照；不只测试私有验证函数。|spec story 46|S2|
|P42|Herdr 不可达或 Workspace 零个／多个匹配时不继续创建。|自动：三类受控情形分别观察公开失败及外部无 Tab 创建、无 Workspace 创建、无自动启动或目标回退。|spec story 47|S2,S3|
|P43|创建 Bot 不自动写入 connect-bot 会话或关机相关状态。|自动：独立读取既有公开会话／关机信息，无额外 connect-bot 上报时不新增登记或监听；原有同机来源限制保持。|spec story 50、ADR 0004/0006|S2|
|P44|创建结果保留在弹窗中，不自动切换到会话列表。|自动：真实浏览器分别观察成功与失败后的所在 tab，由用户操作关闭弹窗。|spec story 51|S1|
|P45|电脑与手机均可管理模板并创建 Bot。|自动：桌面／移动尺寸真实浏览器操作，独立客户端共享与局域网浏览器请求不被误套会话事件限制，不新增账号要求。人工 H2：日常手机实机确认触控、键盘输入、阶段及结果可用；agent 准备移动尺寸验证证据、访问入口和简短验收步骤，实机体验由用户确认。|spec story 52、Implementation Decisions|S1,S2|
|P46|浏览器不能将创建目标改成任意 Workspace。|自动：经公开创建入口提交意图，独立记录仍使用服务端配置目标，并执行唯一匹配。|spec Implementation Decisions / inferred-required：固定目标|S2,S3|
|P47|保存失败不被表示为已保存，也不破坏此前已保存的模板。|自动：隔离真实持久化失败或受控故障，观察公开失败与独立重新读取的旧值；不规定存储算法。|inferred-required：spec stories 2,5,6 的持久保存承诺|S1,S2|
|P48|新建 Bot 不因 agent 的隐式续接设置而加载旧对话。|自动：已存在同 cwd 旧对话时，验证实际新会话启动选择，不依赖“新 Tab”这一单一证据。人工 H1：启用 OMP autoResume 的环境交叉确认新对话；agent 准备旧／新对话独立标识或真实上下文对照，以及完整复现步骤。Herdr 未提供会话标识时不得伪造，不要求 connect-bot 接入。|inferred-required：spec story 28 的独立对话承诺；约束审查已确认|S3|
|P49|名称与正文不会被 herdr-bot 调用包装层先解释为 Shell 代码。|自动：引号、换行和 Shell 特殊字符的独立字面样例经过实际生产交互方式，外部接收值保真且包装层无额外执行；不限制 agent 收到任务后的正常行为。|inferred-required：spec stories 8,23,25,32 的数据保真；约束审查已确认|S2,S3|
|P50|后端已接受的创建不因浏览器断开、刷新或关闭而取消。|自动：确认后端已接受后，在就绪及投递前分别断开观察连接；独立查询外部结果，同一次创建仍继续且仅一次投递，不关闭、重建或重发；未收到结果的页面不假报成功。|approved B1：后端拥有创建责任，前端只发送请求|S1,S2,S3|
|P51|独立客户端同时提交的创建分别接受并独立执行。|自动：两个独立客户端同时对同一及不同 zone 提交，各有对象、快照和输入；无全局单飞拒绝、自动排队、合并或复用。不新增数值并发限制。|approved B2：分别接受、独立创建，目前不作更多约束|S2,S3|

## Seam decisions

|Seam|Exposes|Hides|Alternatives considered|Reversibility|
|---|---|---|---|---|
|S1 主界面交互 interface|既有 tab / 表单 / zone 操作入口上的模板管理、字段联动、名称预览、创建阶段和结果、移动使用。|页面内部组织、状态管理和请求封装。|只观察公开 HTTP 无法证明真实页面交互、阶段与手机可用性；仅检查私有计算又不足以覆盖用户行为。选择已有最高用户接缝。|内部交互实现可替换；若改变可见流程或字段承诺，先修订契约。|
|S2 herdr-bot 公开运行 interface|启动输入及启动结果、公开 HTTP 调用意图和结果、共享持久化、进程重启、快照、后台所有权及独立并发。|存储格式、进程内分工、HTTP 资源命名、进度编码和调度实现。|仅看页面无法证明保存、启动失败、重启和多客户端；把存储或每个步骤单独公开会增加接缝而不增加必要观察。复用当前公开运行接缝并扩展本功能意图。|内部实现保持自由；未来增加创建回执、恢复或版本冲突策略将扩展公开承诺，需要先修订契约。|
|S3 Herdr 公开控制 interface|Workspace 查询、Tab 创建及查询、agent 启动／就绪／输入、对象身份和外部副作用。|herdr-bot 内部客户端、CLI 或 socket 选择、请求编码及等待实现。|仅信成功文字或模拟返回不能证明正确 cwd、启动参数、实际单轮投递和现场保留；不修改 Herdr，也不为每个私有步骤建公开接口。|CLI / socket adapter 可替换，但必须保持同一目标、就绪和副作用语义；外部版本变化若推翻承诺，先更新契约。|

### 已批准的行为取舍

- **B1：后端拥有已接受创建。** 拒绝“浏览器断开就取消后续步骤”，因为手机网络波动不应改变已接受任务；断开仅失去观察，不是 Herdr 执行失败。本次不增加刷新后找回结果或后端崩溃恢复。未来若需要这些能力，应扩展 S2 的回执／恢复承诺，而非偷偷改变本次所有权。
- **B2：分别接受、独立创建。** 拒绝全局单飞忙碌拒绝和自动排队，因为它们会让电脑与手机互相阻塞并增加未要求的接纳限制；不复用既有固定操作的执行资格。未来若需要有意的限流或排队，须显式变更 S2 的接纳／错误承诺。
- **验收模式：Rapid iteration。** 相比 Full automation，把真实账号／模型环境与日常手机体验的最终确认留给用户；实现、TDD、适用的运行验证、自动检查和证据准备仍由 agent 负责。人工范围仅为 Coverage 中明确分配的 H1、H2 部分，不能扩大成整条承诺或其他自动工作的豁免。

### 依赖与观察纪律

- 名称和输入合成属于 **In-process**；不为这些内部计算额外公开接缝。
- 本地持久化属于 **Local-substitutable**；隔离真实存储和进程重启可观察承诺，不要求新增公开存储接口。
- 浏览器到自有 herdr-bot 属于 **Remote but owned**；生产交互与受控响应可验证各自职责，但页面 mock 不能证明真实保存或创建。
- 本 effort 不修改 Herdr、OMP、Pi，其交互属于 **True external**。生产 adapter 和可脚本化测试 adapter 支撑真实变化：后者覆盖可控时序和故障，前者支撑正常路径证据。接口是测试表面，不以私有状态、源文本断言或复制实现计算充当结果证据。
- 六表面均已走查：entry、data and state、external dependencies、errors and failure、output and observability、resources and concurrency。整类不适用项为无；没有新增独立日志／metrics 格式或性能容量 SLA 的承诺。

## Not yet specified

- **批准阻塞项：无。** 三个决策关卡均已完成。H1、H2 是未来实现交付时的人工覆盖，不是契约批准阻塞，也不暂停其他可执行工作。
- **保持自由的实现细节：**具体公开资源名称、进度传输、连接配置字段、持久化格式及内部实现。不为它们预切票或新增私有接缝；它们不得削弱上表的可观察结果。
- **尚未承诺的编辑行为：**未知命名占位符的具体诊断，以及同一模板并发编辑的版本控制／合并策略。它们不扩展四个已定义占位符，也不成为擅加用户限制的理由；若实现需要新增可见拒绝或改变已批准行为，必须先修订契约。
- **未来能力：**数值性能／容量 SLA 尚未指定。后端重启后的创建恢复与结果检索仍按 spec 的既定范围处理，本次不预建相应系统。
- **后续交付信息：**由 /to-tickets 记录每项承诺的 delivering work；不是在此维护完成状态。任何实现证据推翻承诺或接缝时，先修订契约，再重开受影响票。

## Out of scope

完整继承 [spec 的 Out of Scope](spec.md#out-of-scope)，不新增或缩减范围。尤其不得将 Rapid iteration 理解为允许省略实现、强制 Skill 必选、添加空值组合校验、引入自动重试／回滚／清理，或把真实外部结果用 mock 冒充。既有 connect-bot 会话写入来源限制、会话登记与关机语义不因本契约改变。
