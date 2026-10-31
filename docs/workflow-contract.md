# Map Run：长期需求与契约

本页承接 Map Run 的需求、职责、可观察承诺与重要取舍；它与下列长期参考共同构成维护入口，不需要设计草稿、实施票据或历史会话才能解释规则。编号 P1–P70 是稳定的需求追溯标识，**不是完成状态**。契约内容已批准不表示产品、真实 Agent 或人工验收已通过。

- 用户：从[日常使用](usage.md)进入 [scheduler 定义与发起](scheduler.md)，在 [workflow 观察与故障速查](workflow-runtime.md#故障处理速查先看现场再向管理者表达意图)处理现场。
- 接入维护者：[运行接口、记录与配置](workflow-runtime.md#公开接口与持久字段参考)，以及同页各动作、report、collect 协议。
- 角色扩展维护者：[角色动作矩阵](workflow-runtime.md#角色责任与批准动作矩阵)、[公共调用方与取消边界](workflow-runtime.md#角色调用方及公共能力的部署边界)。实际源码在独立 my-agent-toolkit，不能用本文或入口文件存在代替部署与注入证据。
- 验证者：[测试安全与入口](agents/testing.md)、[单节点真实闭环](workflow-runtime.md#单节点真实宿主闭环验收)、[真实双材料汇合与通知取证](workflow-fan-in-acceptance.md)、[故障指南实演与 H2](workflow-runtime.md#可重复演练与-h2-证据要求)。

## 需求与责任

静态依赖图不足以表达一次运行中的独立执行、交接、失败及人工恢复。系统让人类从已有 Map 发起独立 Run，由管理者 Chat 显式决定动作，后端校验并保存事实，节点 Chat 做实际工作并报告；workflow 负责观察，不是第二套执行器。

| 参与者／对象 | 责任与边界 |
| --- | --- |
| 规划者、Map、node | Map 是服务级静态定义；node 给出 Bot 引用、input 原文、zone 路径与同图依赖。边不传材料、不自动启动。运行期间保持定义与 Task 不变。 |
| Bot | 跨 Map 共享的业务能力文档；when、plan_guide、node_guide 分别供选用、规划与工作理解，可独立为空；不是执行实例、运行门禁或自动注入。 |
| Task、Chat | Task 是可复用发起预设，Chat 是实际原生对话。Task 用途仅 normal/manager，均可选择 omp 或 pi；node 是本次 Chat 的执行角色，普通 Task 可同时用于独立发起与节点派发，不是运行归属。trigger config 仅在首次发起/retry 生效，普通 Resume 使用来源 Task 最新 env config。 |
| 人类 | 选择 manager Task、zone 与本次要求；核实、修复实际环境和产物，通过管理者 Chat 表达明确意图，不填身份参数、不编辑运行记录。 |
| 管理者 Chat | 独占状态操作，读取记录与材料并判断交接，显式派发、完成、恢复或结束；不把报告、停轮、退出当完成，不强制持续续轮。不是业务 node。 |
| 节点 Chat | 在定义 zone 做工作，按本次 `reportPolicy` 以 report 正文说明结果地址、用法与限制：auto 自行报告，manual 每次先展示摘要/地址/限制、询问并停轮等待本 node Chat 用户自然语言确认；修改后及补充更正同样重新确认。可在交接后继续不影响结果的收尾；不能自改流程状态。 |
| 后端 | 检查同机来源、真实原生身份与 Run/执行归属、依赖和状态；串行同 Run 业务动作，保存当前事实与历史，提供报告和 collect。模型判断不能替代这些约束。 |
| Herdr／agent／toolkit | Herdr 托管原生 Chat 与输入提交，各 Task 选择的 omp 或 pi 执行；toolkit 经公共 Prompt/Session/HTTP 能力自动取得身份、注入指引、调用并取消请求，不依赖 connect-bot 开关。 |
| Run、workflow | Run 保存本次事实而非定义快照；同一 Map 可有多个独立 Run。workflow 只读观察执行事实、定位/恢复已关联 Chat，并允许人类重命名 Run；不暗中创建、接管或改状态。 |

新启动 omp / pi 的文件可能尚未落盘（pi 在首次用户或助手消息出现后创建文件）；仅对官方引用、绝对标准原生路径、实际 NotFound 与已报告 cwd 的组合接受路径内身份，文件存在时仍以文件头为准。这是显式命名约定信任而非文件头验证；照常先关联再提交指示。仅凭可信历史保存的同一路径与 ID 可以定位未落盘当前 Chat，新进程恢复仍须确认原身份；身份冲突拒绝，不自动改绑、轮询或补关联旧 Run。见 [ADR 0014](adr/0014-unpersisted-omp-startup-identity.md)。

术语以[领域词汇](../GLOSSARY.md)为准；尊重 [Task 命名](adr/0012-rename-bot-preset-to-task.md)、[Bot 命名](adr/0013-rename-node-template-to-bot.md)、[zone 与 Workspace 区分](adr/0009-rename-workspace-to-zone-for-herdr.md)和[同机写入边界](adr/0004-session-write-boundary-same-host.md)。历史 ADR 中旧词由其现行术语注记解释，不恢复旧别名。

## 状态与关键取舍

Run 仅有 running、paused、completed；node 仅有 pending、running、failed、completed。ready、blocked 和 canFinish 从当前事实与当前定义派生，不保存第二套生命周期。completed 表示结果可交接并满足依赖，不表示 Chat 退出。详细准入见[批准动作矩阵](workflow-runtime.md#角色责任与批准动作矩阵)。

- 启动整图预检必须通过；保存定义则允许外部绑定未完善。空图可保存但不能启动。只支持当前唯一格式；不再提供旧格式手动迁移工具，不做运行时兼容或角色猜测。
- 三份记录先初始化，真实 Chat 身份关联保存后才提交初始指示。每次派发/retry 是独立原生 Chat，不以新 Tab 证明隔离。用户确认 omp 未开 autoResume；不改原生存储目录。
- Task 固定职能、公共 Prompt 工具指引、本次工作 input 各司其职。manager 末尾追加 user_input 原文；node 不自动获得它、Bot 文档或整张 Map。所有必要路径由环境保证可读，不新增服务端可读性门禁。
- 同 Run 动作串行到完整派发批次结束，单项失败不取消其他合法项；另一 Run 和已有 Chat 工作不被该业务队列阻止。文件协调只覆盖实际访问，不跨启动、投递或 collect 等待。直接读三文件不承诺跨文件一致快照。
- Map node 的 `report_policy` 为可选 snake_case 字段，仅 auto/manual，默认 auto，非法值拒绝；dispatch/retry 读取当前 Map 并固定为执行的 camelCase `reportPolicy`。旧记录缺字段按 auto，无迁移；恢复原 Chat/执行沿用保存值，不热更新。初始输入在业务 input 外独立给出策略控制段。manual 只是提示词约束，不新增审批门禁、状态或按钮，不改变管理者 complete 职责。
- Map node 的 `auto_close` 缺省 false，dispatch/retry 固定为执行的 `autoClose`；旧执行缺字段按 false，不因查询改写。只有 manager 首次合法 complete 且当前事实保存成功，才按本次选择尽力关闭当前执行保存的 Tab；历史追加失败不撤销资格或原反馈。外部命令不占用业务推进锁或文件协调，响应及同 Run 后续合法动作不等待关闭返回；不要求停轮/空闲，不扫描无关目标，不重试、补偿、绕过确认或持久化关闭结果。关闭会终止 Tab 内终端及相关进程，不删除 Chat 历史；日志之外不新增 workflow 结果展示。详见[完成后自动关闭](workflow-runtime.md#节点完成后自动关闭关联-tab)。
- report 先保存后独立通知。输入提交成功不等于阅读、严格顺序、原生 IRC 或打断保证；保存成功/投递失败不撤回报告、不重投、不仅为此重报。collect 的通知检查不清交接待办。
- 状态保存失败不发布候选事实；当前状态提交后历史追加失败明确部分生效，不回滚、不持续封锁 Run。已启动 Chat 不假报回滚；崩溃/关联失败窗口不承诺完整幂等，先核实副作用再行动。
- 每 Run 仅有当前事实、追加历史、追加报告三份业务记录；便于直接读取但不提供数据库事务、跨文件原子、断电安全或多进程协议。Windows 替换不先删正式文件，残缺记录拒绝追加，不自动修复。
- 后端重启后旧 Run 的执行事实只读，保留原状态、报告和 Chat 关联，拒绝动作/report/collect；人类仍可修改名称。普通 Chat Resume 不接管。未接管是服务上下文事实，不是持久 Run 状态。
- Run 可选 `name` 仅供人类识别，不参与 Map、身份、依赖或执行逻辑；旧记录缺字段无需迁移。详情标题优先名称，否则沿用当前 Map 标题 / 保存 Map 标识；列表优先名称，否则沿用保存 Map 标识，名称、ID 与 Map 均可搜索。标题旁「重命名」展开以当前显示标题为初值的输入；保存去除首尾空白、拒绝空名称、允许重名，取消不写入，失败保留草稿。`PATCH /api/workflow/runs/{id}` 接收 `{ name: string }`，成功返回与 GET 相同的详情；保存后同步列表/详情并刷新，切换 Run 不应用迟到结果。历史/未接管 Run 同样可重命名，不恢复管理或修改执行事实。
- 同机来源限制不是认证，session_id 不是秘密；接受同机进程冒用风险，不引入 token 或沙箱。未来提高认证、归属、事务或存储保证须同步接口、迁移及验证，不能静默改变调用者依赖。
- 图来自当前定义而非历史快照，定义无法可靠对应时保留保存事实但不画假图。轮询默认 5 秒、collect 默认 90 分钟；有效配置、稳定路径和较长 HTTP 预算见[记录及配置](workflow-runtime.md#记录及配置)。

## 可观察承诺与长期规则索引

以下逐项说明独立应满足的结果及其规则位置；不是关键词检查或测试通过清单。关联章节保留输入、错误、时序与负向边界。P66 的审查不能替其他编号宣布通过。

### 定义、启动与输入

| 编号 | 应满足的结果／不可弱化的边界 | 长期参考 |
| --- | --- | --- |
| P1 | Map/node/Bot/Task/Chat 职责及唯一新格式不混淆，静态定义不存运行事实。 | [定义格式](scheduler.md#定义职责与唯一格式) |
| P2 | 三份 Bot 文档独立读取、独立可空，不成为业务门禁。 | [定义格式](scheduler.md#定义职责与唯一格式) |
| P3 | 内部合法、外部 Bot/Task/zone 未完善仍能保存，不补绑定。 | [保存边界](scheduler.md#保存与校验边界) |
| P4 | 未知字段、错类型、漏字段、非法依赖或环拒绝且保留原定义。 | [保存边界](scheduler.md#保存与校验边界) |
| P5 | Task 类型仅 normal/manager；普通 Task 可独立发起及节点派发，节点固定扩展由执行上下文强制选择，角色支持 omp/pi、不可取消/混装，HTTP 同样拒绝非法组合。 | [Task 类型](usage.md#task-类型与固定角色扩展) |
| P6（已退役） | 旧 Task 手动迁移工具及其升级承诺已移除；使用当前 Task 格式，不做运行时兼容或角色猜测。保留编号，不重新分配。 | [Task 类型](usage.md#task-类型与固定角色扩展) |
| P7 | 空图或无效整图启动前拒绝，不创建 Run/Chat。 | [预检](scheduler.md#从选中-map-校验启动配置) |
| P8 | zone 按已登记路径重检；注销拒绝、重登记再检，不自动跟随搬迁。 | [预检](scheduler.md#从选中-map-校验启动配置) |
| P9 | 选定 Map 只选 manager 配置与本次要求，不临时覆盖 node 或新增运行名。 | [发起入口](scheduler.md#从选中-map-校验启动配置) |
| P10 | user_input 原文持久保留，只追加 manager 初始输入，启动后只读。 | [发起与观察](workflow-runtime.md#发起与观察) |
| P11 | 同 Map 多 Run 身份和事实独立。 | [记录契约](workflow-runtime.md#公开接口与持久字段参考) |
| P12 | 取得 ID 进入详情，刷新/重进只读取；预检失败留发起页。 | [发起与观察](workflow-runtime.md#发起与观察) |
| P13 | 三文件初始化与真实身份关联保存均成功后才提交初始指示；外部 Chat 不假报回滚。 | [记录与提交](workflow-runtime.md#独立读取与部分提交) |
| P14 | 每次 dispatch/retry 独立原生 Chat，使用定义 zone 与管理者实际 Workspace，不回退。 | [派发](workflow-runtime.md#管理者显式批量派发)、[retry](workflow-runtime.md#独立新执行retry) |
| P15 | 首发/retry 完整保留 trigger config，普通 Resume 不重放。 | [派发](workflow-runtime.md#管理者显式批量派发)、[Resume](usage.md#历史-chat-resume) |
| P16 | Task 职能、工作信息、工具指引分工；不注入 Bot/整图，提供可读绝对路径。 | [派发输入](workflow-runtime.md#管理者显式批量派发) |

### 状态、报告与等待

| 编号 | 应满足的结果／不可弱化的边界 | 长期参考 |
| --- | --- | --- |
| P17 | status 和动作快照反映已提交现场、准入和结束条件；后续动作重新检查。 | [状态查询](workflow-runtime.md#基础管理者状态协议) |
| P18 | 依赖/状态不准入 dispatch 不创建 Chat。 | [派发](workflow-runtime.md#管理者显式批量派发) |
| P19 | 批量逐项拒绝/失败/成功，失败不取消其他项或回滚成功项。 | [派发](workflow-runtime.md#管理者显式批量派发) |
| P20 | 已记录派发与重复状态动作不重复效果、不覆盖首次事实；异常窗口不承诺完整幂等。 | [派发](workflow-runtime.md#管理者显式批量派发)、[暂停/恢复](workflow-runtime.md#暂停新执行与恢复派发资格) |
| P21 | 同 Run 完整批次串行，其他 Run 和已启动 Chat 工作可继续。 | [并发边界](workflow-runtime.md#完整批次与并发调用) |
| P22 | 工具归属来自真实会话与当前/保留执行，不接受模型自报或跨 Run 越权。 | [归属检查](workflow-runtime.md#角色归属与来源集成检查) |
| P23 | 同机来源与会话关联分别检查；session_id 不充当秘密认证。 | [归属检查](workflow-runtime.md#角色归属与来源集成检查) |
| P24 | report 仅正文，系统确定报告 ID/时间/来源/执行/接收者，不改状态。 | [报告](workflow-runtime.md#节点报告与独立通知) |
| P25 | 报告保存失败不产生待办/通知。 | [报告阶段结果](workflow-runtime.md#节点报告与独立通知) |
| P26 | 保存成功、投递失败仍可 collect，明确分阶段结果，不回滚/暂停/补投/仅为此重报。 | [报告](workflow-runtime.md#节点报告与独立通知) |
| P27 | 重复报告独立追加；待办按 node 当前适用执行汇总最新报告，旧报告只作历史。 | [retry 报告归属](workflow-runtime.md#独立新执行retry) |
| P28 | 读取、通知、collect 不消除未完成待办；completed 后不重开；重启后执行事实只读，名称可改。 | [报告](workflow-runtime.md#节点报告与独立通知)、[重启](workflow-runtime.md#后端重启历史现场只读) |
| P29 | 管理者显式 complete 才交接/满足依赖，report/停轮/退出均不能替代，pending 拒绝。 | [交接](workflow-runtime.md#显式交接完成与结束-run) |
| P30 | collect 返回原因、快照、报告标识而非正文；无 running/通知不无限等，不承诺 ready 即返回。 | [collect](workflow-runtime.md#收取报告等待与取消) |
| P31 | 通知在无人等待时保留、可合并，检查后新通知留到下次，不当已读。 | [collect](workflow-runtime.md#收取报告等待与取消) |
| P32 | 配置期限正常到期不改状态；默认 90 分钟，整个请求预算更长。 | [collect 配置](workflow-runtime.md#收取报告等待与取消) |
| P33 | 工具中断/Chat 结束取消请求并清等待；网络失败报错，不假快照、不自动重试。 | [公共取消](workflow-runtime.md#角色调用方及公共能力的部署边界) |
| P34 | 工作中/空闲/collect 的 Herdr 通知仅承诺实际输入提交阶段，不承诺阅读/顺序/打断。 | [三类现场](workflow-fan-in-acceptance.md#三个真实通知现场p3401) |

### 结束、恢复与持久化

| 编号 | 应满足的结果／不可弱化的边界 | 长期参考 |
| --- | --- | --- |
| P35 | 全部节点 completed 才 finish；running/paused 均可，活动 Chat 不阻止，终态拒绝变更。 | [结束](workflow-runtime.md#显式交接完成与结束-run) |
| P36 | pause 在完整批次之后生效，禁新 dispatch/retry，已有工作/report/交接保留。 | [暂停](workflow-runtime.md#暂停新执行与恢复派发资格) |
| P37 | resume 只恢复派发资格，须另行显式 dispatch。 | [恢复资格](workflow-runtime.md#暂停新执行与恢复派发资格) |
| P38 | failed 保留派发/执行阶段与原因，不自动暂停/重试/终止 Chat。 | [失败](workflow-runtime.md#失败核实原执行恢复与人工交接) |
| P39 | 派发失败核实：确认创建→原执行 running，未创建→pending，不确定→failed；不新建 Chat。 | [核实](workflow-runtime.md#只核实派发失败reconcile_dispatch) |
| P40 | 人工确认后 recover 只恢复原执行跟踪、身份不变/失败历史保留，paused 允许。 | [recover](workflow-runtime.md#恢复原执行跟踪recover) |
| P41 | retry 是独立新执行，保留旧 Chat/事实/副作用；paused 拒绝，不确定派发先核实。 | [retry](workflow-runtime.md#独立新执行retry) |
| P42 | 人工 complete 执行失败仅交接，不能伪造报告/传材料；已有 Chat 可补地址和限制。 | [人工交接](workflow-runtime.md#人工已处理且确实可交接) |
| P43 | 当前状态写失败不发布内存候选或成功历史。 | [部分提交](workflow-runtime.md#独立读取与部分提交) |
| P44 | 状态已提交、历史失败明确部分生效，不回滚、不因该次错误持续封锁。 | [部分提交](workflow-runtime.md#独立读取与部分提交) |
| P45 | Chat 已启动后的身份/关联/输入失败暴露真实效果，不假称回滚或完整幂等。 | [启动失败](workflow-runtime.md#启动失败) |
| P46 | 当前事实、追加历史、报告独立可读；不靠历史重放、不增加通知业务文件。 | [三份记录](workflow-runtime.md#独立读取与部分提交) |
| P47 | 必要文件缺失/损坏/JSONL 残尾明确报错，保留字节、不重建/截断/继续追加。 | [损坏记录](workflow-runtime.md#损坏记录与-windows-文件协调) |
| P48 | Windows 单文件更新不先删正式文件；文件锁不跨外部启动/投递/等待，不承诺跨文件事务。 | [文件协调](workflow-runtime.md#损坏记录与-windows-文件协调) |
| P49 | 真实后端重启保留旧 Run 原执行事实，执行只读并拒绝动作/report/collect，名称仍可修改，Resume 不接管。 | [重启](workflow-runtime.md#后端重启历史现场只读) |

### 观察、部署与交付

| 编号 | 应满足的结果／不可弱化的边界 | 长期参考 |
| --- | --- | --- |
| P50 | workflow 默认跨 Map 倒序、按日期列全部 Run；时间在前，无表头/归属列，完成数仅 n/m；提供显式组合筛选，进详情隐藏列表，切换其他 Tab 再返回保持已选 Run 与详情 hash 一致；「返回全部 Run」清空选择、使用 `#workflow` 并恢复列表筛选/滚动。详情区分未接管事实，允许人类重命名元数据，不增加运行控制。 | [观察](workflow-runtime.md#发起与观察) |
| P51 | 只读图用文字与视觉标记，不以颜色/Chat 状态独自替代 node 状态；manager 不是 node。 | [观察](workflow-runtime.md#发起与观察) |
| P52 | 详情区分当前定义/执行/适用报告/旧历史及各 Chat，无关联不补建。 | [观察](workflow-runtime.md#发起与观察) |
| P53 | Resume 指定 Chat，如实成功/失败/未知，不改节点、不接管旧 Run。 | [观察](workflow-runtime.md#发起与观察)、[普通恢复](usage.md#历史-chat-resume) |
| P54 | manager 启动错误仅当前后端内存，刷新保留、重启后只查诊断；node 失败持久化。 | [重启与错误](workflow-runtime.md#后端重启历史现场只读) |
| P55 | 可见 workflow 才按有效配置轮询，默认 5 秒、同页不重叠，返回立即读取。 | [轮询](workflow-runtime.md#发起与观察) |
| P56 | 查询失败保留上次成功内容，显示更新失败/最后成功时间，恢复后更新，不改状态。 | [查询反馈](workflow-runtime.md#发起与观察) |
| P57 | 图注明当前定义；丢失/损坏/不对应不伪造历史图，保存事实/关联仍可读。 | [历史降级](workflow-runtime.md#发起与观察) |
| P58 | manager 角色随 manager Task 启用，node 角色随节点派发及关联 Resume 启用，不同 node cwd 下读取稳定配置。 | [部署检查](workflow-runtime.md#角色归属与来源集成检查) |
| P59 | 实际 Chat 通过公共 Prompt 获得角色指引，工具参数不暴露自报身份/归属。 | [角色矩阵](workflow-runtime.md#角色责任与批准动作矩阵) |
| P60 | 扩展经公共能力工作、支持取消，不依赖 connect-bot 开关或私有实现。 | [公共能力](workflow-runtime.md#角色调用方及公共能力的部署边界) |
| P61 | 报告投递诊断含 Run/报告/目标/结果/原因，不重复正文；诊断失败不撤销/重投。 | [诊断](workflow-runtime.md#节点报告与独立通知) |
| P62 | 真实 A/B→C 中 C 确实读取两份指定材料，三产物符合启动前独立预期，不能用手改事实替代。 | [真实汇合](workflow-fan-in-acceptance.md) |
| P63 | 前置交接后可继续收尾，manager 显式推进 complete/dispatch/finish；干预如实记录。 | [活动顺序](workflow-fan-in-acceptance.md#显式交接产物与活动顺序p6201p6301) |
| P64 | Agent 沿真实页面/工具演练故障查看→Resume→核实修复→表达意图→结果，不让用户填参数/文件。 | [故障演练](workflow-runtime.md#可重复演练与-h2-证据要求) |
| P65 | 角色实际注入与指南明确状态/实际执行/交接之别，覆盖 recover/retry/人工完成/仅通知失败。 | [角色矩阵与演练](workflow-runtime.md#角色责任与批准动作矩阵) |
| P66 | 长期需求、契约和指南在移除临时材料后独立可用，逐项审查规则、链接与必要依赖。 | [本页审查方法](#文档审查方法与本次结论) |
| P67 | 有效配置约束记录路径、轮询、collect 和预算，不依赖 node cwd/偶然默认。 | [配置](workflow-runtime.md#记录及配置)、[公共能力](workflow-runtime.md#角色调用方及公共能力的部署边界) |
| P68 | 正常/错误/重启均不隐式加入已排除自动动作；证据写明有界窗口，不冒称无限期证明。 | [边界](#明确排除的能力)、[验证边界](#验证边界与验收责任) |
| P69 / H1 | 用户最终判断真实汇合推进、三类通知与全部干预是否符合预期；不是替 Agent 补执行。 | [H1 证据要求](workflow-fan-in-acceptance.md#h1-交接与证据完整性) |
| P70 / H2 | 用户最终判断故障指南和实际工具指引是否清楚可行；实现/真实演练须先完成。 | [H2 演练要求](workflow-runtime.md#可重复演练与-h2-证据要求) |

## 明确排除的能力

以下不是尚待补齐的隐含产品承诺，也不能由正常操作、失败或重启偷偷引入：

- 自动生成 Map/LLM Plan、规划编排、运行中改图、专门循环系统、定时调度、声明式自动重试、后台选路。合法既有动作组合不被额外禁止，但不据此新增返工机制。
- omp/pi 之外的宿主、非 Chat adapter、通用宿主/策略插件平台，或内置 Git/worktree/TDD/自动集成业务阶段。
- Bot 文档自动注入、插值、动态传参、user_input 下发、Delivery/产物绑定/复制/校验/快照或额外质量门禁。
- 独立放行、stop/graceful shutdown、pausing/stopping/stopped 生命周期、除本次执行明确选择自动关闭外的强制结束或等待所有 Chat 退出；pause 只阻止新执行。
- manager→node/同伴消息、广播、普通双向协作、extension 接收长连接、原生 IRC、强制续轮、联系人/归属查询工具。
- 通用人工审批/通知/接管产品、恢复按钮、任意 setStatus/JSON patch、完整运维平台、checkpoint、已完成节点返工/下游失效传播。
- 强认证/独立凭据、会话切换自动重绑定、原生 session-dir 迁移、完整崩溃窗口幂等。
- 配置快照/版本锁/变更检测/编辑锁/动态加载保证、多格式运行时兼容、稳定 zone ID、目录搬迁跟随、历史定义还原。
- 跨文件事务、自动回滚/修复/补投/接管、断电恢复、多进程平台、数据库引擎、全仓库文件层迁移或 transcript 汇总。
- 重启后自动或手动接管旧 Run，或用普通 Resume 隐式恢复管理权。
- 启动进度页、manager 启动错误业务持久化、日志浏览入口、额外运行名、Run 列表搜索/筛选/分组。

## 验证边界与验收责任

四个观察 seam 是证据位置，不要求四套新抽象层：

| Seam | 可以证明什么 | 不能单独证明什么 |
| --- | --- | --- |
| S1 交付用户界面 | Task/发起/workflow/实际 Resume 的可达性、图和详情、轮询与错误反馈。 | 单独页面成功不证明落盘、真实工具/原生身份或模型读取材料。 |
| S2 对外 HTTP | 请求/响应、准入、身份、状态与部分效果、串行/等待/取消的对外行为。 | 不证明真实角色装配、Prompt 注入或使用效果；不新增私有 helper seam。 |
| S3 omp 工具与宿主 | 实际 Prompt/Session/HTTP、Herdr 启动/输入/Resume、原生身份、工具参数和实际产物。 | 受控 adapter 只能证明如何处理其响应，不能替代真实宿主/模型质量。 |
| S4 持久记录与迁移 | 三记录、迁移输入/输出、重启保存事实、真实 Windows 替换/损坏效果。 | 不证明 UI/Prompt 可用；不把内部锁/临时命名固定成公共契约。 |

六类表面（入口、数据/状态、外部依赖、错误/故障、输出/可观察性、资源/并发）都需要相关证据，无新增人工例外。仅 P66 是已批准的**非运行 seam 文档审查**，不是第五运行 seam。P1–P65、P67–P68 的指定 Agent 检查不得转交用户；只有 P69/H1、P70/H2 保留最终使用判断。

确定性检查复用隔离 HTTP、文件、进程与浏览器入口；预期来自批准规则和独立字面样例，不由被测实现重算。真实检查使用独占数据/cwd/zone、数值回环动态端口和隔离 Herdr，保存原生身份、完整工具结果、实际注入、产物、三文件、页面及全部干预。LAN/mDNS、符号链接、关机仍遵循测试指引的显式门，不因本功能默认授权。有限窗口没有调用不等于无限期不存在自动动作。

**2026-10-29 文档交付时的证据边界：** 真实汇合与故障指南入口已交付，但已记录的真实部署前置检查因 manager-chat.ts/node-chat.ts 未部署而停止。独占 Herdr、模型能力、真实角色注入/故障实演、三类通知和真实汇合未验证；不能将指南准备、受控 HTTP 或工作已集成当作这些检查完成。部署前置原始结果见[运行说明的有日期检查](workflow-runtime.md#2026-10-28-实际前置检查阻塞未运行闭环)。这是历史检查结果，不是每次部署的实时探测；环境改变后按指南从新独占目录复验。H1/H2 仍未获用户最终确认，且不能以“只待人工”掩盖缺失的 Agent 真实验证。

## 文档审查方法与本次结论

P66.01 的观察对象是以上长期文档，不运行无关产品测试。独立审查基准为已批准的 P1–P70、继承的状态/输入/持久化规则和排除项；对照每行的**条件、结果、失败/副作用与限制**阅读对应段落，而不是搜到名称就判定完整。

可重复入口（仓库根目录）：

```text
node scripts/check-workflow-docs.mjs
```

机械检查从 README、本页、运行/故障/汇合/scheduler/使用/样例/词汇八个入口追踪 Markdown 本地链接，核对文件与标题锚点，拒绝仓库外以及临时目录链接；代码块不作导航链接。它不访问外部网站，不证明外仓部署路径存在、不理解散文语义，不把关键词出现或链接通过冒充内容通过。可传隔离副本根目录检查移除临时目录后的材料；不要删除真实设计或用户数据。

内容复审步骤：

1. 沿上述读者入口阅读职责、定义格式、启动输入和所有动作，按 P1–P23 核对保存/启动区别、完整 trigger、身份、批次与重复/异常窗口。
2. 按 P24–P42 分别核对报告保存/通知/待办/交接、collect 原因与取消、paused/终态和各恢复路径，尤其检查原执行与新执行、Chat Resume 与状态动作是否混淆。
3. 按 P43–P61、P67–P68 核对提交阶段、三文件、损坏/Windows/重启、图降级/刷新、配置和外部角色依赖，不能由最终状态推断不存在副作用。
4. 按 P62–P65、P69–P70 阅读真实汇合/故障演练的固定预期、真实观察、干预、失败及证据要求；明确 Agent 检查缺失与用户最终判断两层，不能新增人工豁免。
5. 按本页排除项与关键取舍核对边界，再运行链接检查；逻辑移除临时材料后重新沿入口解释规则。历史 ADR 的可选历史出处不承载现行规则，外仓部署绝对路径是环境依赖而非临时设计依赖。

本次逐项内容审查发现并修复：缺集中长期需求/责任/排除项及四 seam 解释；缺完整 P1–P70 规则索引；缺明确的创建/查询路由、响应包和持久字段参考；README 的 workflow 描述只覆盖早期基础详情。上述缺口已在本页、运行参考及 README 补齐，不改批准行为、不增加新入口/兼容层。原长期材料中的恢复/报告/collect/损坏/轮询/降级规则及故障自然语言路径保留；其命令是复验入口，不被本次文档审查冒称重新运行通过。

2026-10-29 实际检查记录：初始 `node scripts/check-workflow-docs.mjs` 因缺本页退出 1；补齐后退出 0，遍历 15 份 Markdown、154 个本地链接，零失效文件/锚点、零临时导航依赖。另在 Python 标准库 `tempfile`/`shutil` 创建的独占副本中排除临时设计目录、Git 元数据与构建/用户数据后，以 `node scripts/check-workflow-docs.mjs <副本根>` 重跑，同样退出 0（15/154）。分别向另外三个独占副本的 README 新增不存在的文件链接、不存在的运行规则锚点、临时设计文件链接，检查均退出 1，并分别报告 `broken link`、`missing anchor`、`non-durable dependency`；这些负例能检测具体验证违规，未改变真实工作树，所有副本已清理。运行相关源码/配置及长期规则文档的临时设计路径引用检查无匹配；ADR 的历史出处仅作可选背景，其决策正文完整保留。

链接计数记录的是本轮材料版本，不是未来固定通过阈值。复验应重新运行入口并完成上面的内容审查，不要求计数恒定。内容审查结论是长期规则可独立解释，不是产品实现、真实 Agent 或 H1/H2 验收通过声明；本次未运行无关产品测试或重新探测外部部署。
