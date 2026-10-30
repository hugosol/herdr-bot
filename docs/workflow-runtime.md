# Map Run 运行说明

需求、职责、批准承诺、排除项及验收责任见[长期需求与契约](workflow-contract.md)。本文是用户、管理者/节点及接入维护者的操作与协议参考；文中的检查命令不表示本次部署已通过，真实环境和人工验收状态见[证据边界](workflow-contract.md#验证边界与验收责任)。

## 公开接口与持久字段参考

以下是公开 HTTP/文件 interface，不要求用户手填请求；日常操作从 scheduler 发起、workflow 查看，再在已关联管理者 Chat 表达意图。静态 Map/Bot 的 snake_case 格式见 [scheduler](scheduler.md#定义职责与唯一格式)；运行 HTTP 与记录使用 **camelCase**，不要混用。

| 方法及路径 | 请求与结果 |
| --- | --- |
| `POST /api/scheduler/maps/{filename}/preflight` | 仅 `managerTaskId`、`zone`、`userInput` 三个字符串；空要求传 `""`。整图检查成功返回 `workspaceName`，不创建 Run/Chat。 |
| `POST /api/scheduler/maps/{filename}/runs` | 与 preflight 相同的严格请求，重新预检。完成启动返回 HTTP 201 详情包；启动失败可返回 HTTP 500 详情包，含已分配 `id`、可得事实及 `startupError`，不是“没创建过”的保证。预检/角色入口失败在分配前拒绝；不能忽略非 2xx 响应正文或盲目再次发起。 |
| `GET /api/workflow/runs` | 返回 `runs` 数组及有效 `pollIntervalSeconds`；每项 `id`、可空 `run`、`managed`、`readError`。按创建时间倒序，同时间以 ID 排序；不可读项明确错误，不假造空 Run。 |
| `GET /api/workflow/runs/{id}` | 详情包 `id`、可空 `run`、`managed`、`startupError`、`readError`；可读 Run 另含完整 `reports` 与 `pendingReports`。必要记录不可读明确失败；仅有启动错误但当前记录不可读时仍可返回错误现场。 |
| `PATCH /api/workflow/runs/{id}` | 严格 JSON 正文 `{"name":"显示名称"}`；去除首尾空白后必须非空，允许重名。HTTP 200 返回与 GET 相同的更新后详情包。当前管理与历史未接管的可读 Run 都可重命名；不改变运行状态、执行身份或管理归属。非法正文/空名返回 HTTP 400（空名 `run-name-required`），读取/保存失败返回 HTTP 500、`run-read-failed` / `run-save-failed`。 |
| `POST /api/workflow/manager/{action}` | action 为 status、dispatch、complete、fail、reconcile_dispatch、recover、retry、pause、resume、finish、collect；各自的严格业务参数及返回字段见下文，不是任意动作/状态修改入口。 |
| `POST /api/workflow/node/report` | 传输身份加 `body`，分开返回保存、通知与诊断结果，详见报告章节。 |

管理者角色公开 `run`（按 action 选择下文状态动作）与 `collect`，节点角色公开 `report`；HTTP 将 run 的各 action 分成上表路由，不表示另建一组可绕过角色约束的工具。运行角色传输请求中的 `sessionId` 由公共 Session 自动取得，模型仅给动作矩阵列出的业务参数。后端检查同机来源和已保存真实关联；不接受额外 Run/角色/执行/接收者归属。页面创建 Run 的配置请求与角色请求不是同一种身份协议。Map filename 和 Run ID 沿各自路径校验，不接受任意文件路径。

### 三份记录的字段

`run.json` 是一个完整对象；各 JSONL 文件每行一个完整对象，以换行结束。时间字段使用 Unix epoch **毫秒整数**，ID 是不透明字符串，不从名称/cwd/时间推导关联。以下可空项以 null 表示；初始化的节点执行、失败和完成均为空，旧执行数组为空。

| 对象 | 序列化字段及意义 |
| --- | --- |
| Run | `id`；可选 `name`（仅供人辨识的名称，未命名时省略，旧记录缺少此字段仍可读取）；`map`（Map 文件引用）；`createdAt`；`managerTaskId`；管理者 `zone`、`workspaceName`；原文 `userInput`；可空 `manager` Chat 引用；`status`；可空 `finished`；`nodes`（node ID → 节点事实）。不是定义快照。 |
| Chat 引用 | `sessionId`、`workspaceId`、`tabId`、`paneId`、`agentName`；原生身份与实际宿主关联，不是新 Tab 即独立身份的证明。 |
| 节点事实 | `status`；可空 `execution`、`failure`、`completion`；`previousExecutions` 数组，各项保留被替代的可空 `execution` 与 `failure`。 |
| execution | `id`、来源 `taskId`、实际宿主 `agent`、定义 `zone`、`reportPolicy`（`auto` 或 `manual`）、`autoClose`（布尔）、可空 `chat` 引用。两项选择在 dispatch/retry 解析当前 Map 时固定；旧记录缺少 `reportPolicy` 时按 `auto`、缺少 `autoClose` 时按 `false` 读取，无迁移、不因查询改写文件。新尝试拥有新执行 ID；没有 Chat 关联不代表外部没有发生效果。 |
| failure | `stage`、`message`；启动失败可含 `effects`，执行阶段失败不制造启动效果。详细阶段以实际响应为准，不能只看 failed。 |
| effects | `recordsInitialized`、可空 `workspaceId/tabId/paneId/agentName/sessionId`、`agentStarted`、`associationSaved`、`inputAttempted`、`inputSubmitted`；只表示已观察阶段，不证明未确认阶段绝无效果。 |
| completion | `id`、`at`、真实管理者 `actor` Chat 引用、原文 `reason`；保留首次交接事实。 |
| finished | `id`、`at`、真实管理者 `actor` Chat 引用；重复 finish 不覆盖。 |
| report | `id`、`type=report`、`at`、`runId`、`nodeId`、`executionId`、真实 `source`/`target` Chat 引用、`body`；只在 messages.jsonl 追加，不包含已读或投递成功事实。 |

管理者、node 与 retry 的共同启动阶段统一为 `workspace`、`tab`、`start`、`identity`、`association`、`history`、`submit`；管理者另有记录初始化阶段 `initialize`，node 另有定义解析阶段 `resolve` 与提交运行状态阶段 `state`。其中 `history` 指提交首次指示前写入 **Chat 历史会话列表**，不是追加 Run 的 `history.jsonl`。管理者在此失败也返回 `stage=history`（不再归入 `association`）；已保存的真实关联和 `associationSaved=true` 保留，`inputAttempted/inputSubmitted=false`，不会提交首次指示。node 与 retry 同样保留关联和失败事实；retry 不覆盖旧执行。Run 执行历史追加失败仍使用既有 `historyError`／部分生效反馈，不等同于这个启动阶段。

`history.jsonl` 保存按动作追加的执行/状态事实，不是重放协议：公共说明为操作 `id/at/actor/action`、目标 `nodeId`（节点动作）、`before/after`、说明 `reason` 及相关执行/失败引用。不同动作按需带 `execution`、`failure`、`previousExecution/previousFailure`、核实 `finding` 等；不要求所有事件有全部字段，不用历史字段缺席反推当前状态。具体 complete/fail/recover/retry/reconcile_dispatch/pause/resume/finish 的内容见对应章节。通知代次、collect 检查位置、managed、readError 和 manager startupError 都不是第四份持久业务记录。

当前事实读取以 `run.json` 为准，`reports` 为全部追加报告，`pendingReports` 为 node ID → 当前适用报告 ID。status/动作使用 `snapshot` 而非详情的 `run` 字段；collect 不带报告正文。HTTP 成功码不能掩盖 `historyError` 或逐项失败，错误码、已生效字段和已发生效果必须一起解释，见[部分提交](#独立读取与部分提交)。

## 发起与观察

在 scheduler 选中 Map，打开「+ 发起 Run」，选择 manager 类型的 omp 或 pi Task 与已登记 zone。Workspace 名称来自管理者 zone 的目录名，不退回默认或当前 Workspace。可先校验配置；「启动 Run」会重新执行同一整图预检，然后创建独立 Run。三份 Bot 文档允许空文本，不是业务质量门禁。

启动前请确认角色 Task 的固定扩展入口与所选宿主匹配，已在部署的 my-agent-toolkit 中交付并可加载；各宿主不自动续接其他会话，且管理者能按绝对路径读取 Map、本说明及运行记录。运行期间保持定义与 Task 配置不变。本服务不增加路径可读性探针，也不修改原生会话目录。缺失扩展入口会明确拒绝启动，不用空扩展代替。

新启动的 omp 可能已分配原生身份、但尚未创建会话文件。herdr-bot 仅在官方 `herdr:omp` 上报绝对原生路径、实际文件打开为 NotFound、文件名符合 omp 的时间戳与 UUID 约定且有实际进程 cwd 时，接受路径中的身份；文件存在时仍读取文件头，权限、损坏及非标准路径错误不降级。管理者、节点和节点 retry 均先保存身份关联与历史，再提交初始指示；不会为取得身份提前发送任务或创建原生文件。身份及历史中的预期文件路径不代表已经落盘或当前可恢复，信任边界见 [ADR 0014](adr/0014-unpersisted-omp-startup-identity.md)。没有后台落盘轮询、自动修复或对旧失败 Run 的补关联。

管理者 Task 的固定 prompt 定义职能与限制；初始 Skill、model/thinking 和固定 prompt 按 Task 原配置生效。本次输入只提供 Run ID、Map 路径、本说明及三份记录的路径，最后以独立段落附上非空本次要求的原文。空要求不附段落。Bot 文档、完整 Map 正文与 node.input 不隐式注入管理者。

取得 Run ID 后自动进入 workflow 的只读详情，链接形如 `#workflow/<Run ID>`。运行列表默认跨 Map 显示全部 Run，按创建时间倒序并以本地日期分组；每行没有表头，依次显示创建时间、Map 标识与 Run ID、状态、`completed 数量/节点总数`（例如 `1/4`），不展示管理归属列。支持按 Run ID/Map 搜索，以及 Map、Run 状态和管理归属的组合筛选；显示匹配数与总数，读取失败项和未知时间项不会在默认列表中消失。点击 Run 进入全宽详情并隐藏列表，返回全部 Run 或浏览器后退会恢复筛选与列表滚动位置。「刷新 Run」只重读列表和所选现场，不创建 Run 或 Chat，也不重试运行操作。管理归属仍在详情中明确说明，与 Run 生命周期状态分开。用户本次确认的浏览优化替代了旧版「不新增筛选、分组」的页面限制，不放宽任何运行写入边界。

workflow 仅在该入口可见且浏览器文档可见时轮询，默认在上一轮查询结束后等待 **5 秒**；离开 workflow、打开 server 面板或隐藏浏览器文档时暂停，返回可见时立即查询。一次刷新按顺序读取列表、所选 Run 和当前定义/名称目录；慢响应期间不启动下一轮，重复手动刷新合并到在途查询，不取消后重新发起重叠请求。离开前已发出的查询可以结束，但旧入口或旧 Run 的迟到结果不会覆盖新选择。

列表或 Run 查询失败时显示「更新失败」及最后一次完整成功查询时间，保留上次成功的列表、详情和图，不将 Run 改成 failed，也不显示假空列表。没有成功查询时明确显示尚无成功查询。可见时后续轮询或手动刷新成功后更新现场并清除更新错误；这是只读查询，不是重试派发、report、collect 或任何运行操作。当前 Map 本身缺失、非法或无法对应时仍遵守下文的不伪造历史图规则。

workflow 使用独立样式作用域，按已确认原型的字号、行高、间距、颜色和卡片层级呈现，不继承 scheduler 的按钮或图形外壳。桌面详情使用左侧保持可见、可独立滚动的节点列表，右侧纵向排列运行图和选中节点详情；窄屏使用紧凑节点选择框。运行图为原始字号的横向分层卡片，大图只在图区域内横向滚动，不使用 scheduler 的 SVG 节点缩放工具栏；细箭头仍逐条来自真实依赖，独立 node 不画虚假连线。首次选择依赖顺序中的首个节点，之后不因刷新或状态变化自动切换；点击列表或图上的 node 只替换详情内容，不主动滚动页面或移动焦点。当前执行适用报告优先展示，下方按时间线式阅读层级列出当前工作要求、已保存执行关联与完成说明；工作要求明确来自当前定义，不冒充执行快照。Chat 标识与 Resume 直接可见，额外保存的 Workspace/Tab/pane 等关联字段按需展开。当前完整定义与旧执行历史按需展开；本次要求、管理者配置及管理者 Chat 位于下方的「运行背景」折叠区。

只读图和 scheduler 共用依赖层级排序：前置层在前，同层沿当前服务返回的定义顺序稳定排列；图从左到右表示依赖，箭头从前置 node 指向依赖它的 node。此顺序不是执行调度顺序，不改写 Map 文件，manager 不作为 node。图、节点列表和详情展示 Task 名称：有当前执行时以保存的 execution.taskId 为准，否则使用当前 Bot 绑定；名称来自当前 Task 目录，名称不可读时显示标识，不冒充历史名称快照。

图中同时使用状态文字、独立符号与对应文字颜色，不依赖卡片整块填充表示状态；运行节点的独立箭头只在 Run 由当前后端管理且处于 running 时播放纯 CSS 动画，未接管或暂停时静止，减少动态效果设置下停用动画。选中态另用静态外框，不按 Chat 在线或工作状态推断完成。pending 节点仅按当前 Run 状态与当前定义的依赖显示 ready/blocked；这只是读取时的说明，不自动派发。节点详情继续区分「当前定义」的 Bot/Task、zone/input、前置依赖与阻塞原因，以及「已保存执行」的 Task/zone/真实 Chat 标识、失败阶段及效果、交接完成说明/时间和当前执行适用报告正文/时间。没有执行或 Chat 关联时明确提示，不隐式补建。

图始终注明来自**当前 Map，非历史定义快照**。当前 Map 丢失、不可解析或 node 标识集合无法与 Run 一一对应时，不画图也不按新名称误配旧 node；保留本次要求、保存的 Run/node 状态、失败、完成、当前/旧执行及报告与 Chat 关联。名称不可读时显示保存标识，不伪造过去的 Bot/Task/输入定义。运行期间仍应保持定义不变：同标识的定义修改不代表已保存历史定义，页面只将其标为当前定义。

首次打开 Run 链接、切换入口后返回、手动刷新只读取事实。浏览器整页刷新遵守[页面导航](usage.md#页面导航)规则回到 chat，保留 URL 指定的导航布局；重新进入 workflow 查看记录不会重新启动 Run。本次要求不可编辑；详情中的 Run/node 状态不是 Chat 在线或轮次状态。没有启动进度页、状态操作按钮或日志浏览器。需要核实、修复、恢复跟踪或交接时，请向已关联管理者 Chat 表达意图；报告本身不完成 node，普通 Chat Resume 不接管旧 Run。管理者、当前执行与默认折叠的「旧执行历史」保留真实 Chat 关联并提供 Resume；旧报告单独显示执行 ID、来源 Chat、正文与时间，不混入当前执行适用报告。旧报告的来源 Chat 也可独立 Resume。无执行或 Chat 关联时只说明缺失，不创建执行。

Resume 复用 chat 历史入口的实际定位／恢复：按所选原生 sessionId 查找，不按同名 Task、旧 Tab/pane、当前选中项或 cwd 猜测。已运行的 Chat 导航到 Herdr 内部焦点；需要新进程时加载指定原生对话并读取**来源 Task 最新 env config**，不重放初始 Skill、model/thinking、固定 prompt 或本次 input。恢复不是 recover、retry 或 Run 的 resume 状态动作，不改变节点状态、报告适用性或「历史现场 / 未接管」标记，也不恢复管理者写入资格。当前 Map 丢失、损坏或 node 无法可靠对应时不补造历史图，但这些已保存关联与折叠历史仍可使用。

定位／恢复失败与结果未知使用既有错误反馈：已发生 Tab/启动效果不假报回滚，已提交启动不等于已确认加载同一 Chat；未知时先核实 Herdr，不自动重试、再次启动或补发输入。页面成功仅表示响应中确认的 Herdr 内部导航，不保证 Windows 窗口置前或任务已继续。用户需要交接或修复时，在已确认进入的管理者 Chat 表达意图；角色指引不得把普通 Chat Resume 当成自动派发或接管许可。

可重复检查：`npm --prefix frontend run test:browser:build -- workflowHistory.spec.ts --workers=1`。P52.02/P57.02 用独立新旧执行、报告正文/时间和同名干扰 Chat，分别核对 Map 缺失、损坏和标识不对应时的实际 manager/当前/旧 Chat 入口、默认折叠历史与无关联节点；Resume 后观察 300ms，另对无关联节点观察 150ms。P15.04/P68.09 先经真实 HTTP 启动 Run 并派发节点，再修改来源 Task 的可选扩展与固定入口目录，通过 workflow Resume 核对最新 env argv、无 model/thinking/Skill/prompt/input 重放；当前管理与实际重启后的未接管场景各观察至少 350ms，并核对旧管理者写入资格仍被拒绝。P53.01 受控注入 manager 聚焦失败、旧 Chat 查找失败及当前 Chat 启动超时，核对明确失败与未知反馈、保留目标、状态与文件未变，之后观察 350ms 无自动重试。全部通过实际交付页面、HTTP、生产 Herdr CLI adapter 与持久文件核对；证据保存在 Playwright 输出目录中的截图与 JSON 文件/attachments，记录实际观察时长和外部调用。受控 Herdr 仅证明确定性调用与错误/未知语义，不证明真实宿主导航、扩展加载或原生 Chat 内容。真实 S3 复验需已部署 metadata 指定角色入口及独占命名 Herdr 实例，保存当前/旧/manager 原生身份、实际导航、来源 Task 环境变更前后加载和有起止的无输入重放记录；缺少这些前提应报告未验证，不将受控结果计作真实宿主通过，也不把有限窗口外推为无限期保证。

可重复页面契约检查：`npm --prefix frontend run test:browser:build -- workflowCurrent.spec.ts --workers=1`。P50.01 使用多 Map/时间/状态的独立字面记录核对完整倒序列表与完成数量；P51.01/P52.01 核对真实绘制的依赖方向、与在线工作中 Chat 不同的节点状态、独立选中态及当前定义/执行/报告详情；P57.01 先显示当前图，再分别删除、损坏、改成不对应的定义，手动读取并核对无伪造图、保存事实及旧关联可读。测试输出包含截图、记录与外部调用的前后对照，**750ms 有界观察窗口**内读取未启动执行；这不证明无限期无自动行为，也不证明真实 Herdr/omp 执行质量。

可重复轮询/错误契约检查：`npm --prefix frontend run test:browser:build -- workflowRefresh.spec.ts --workers=1`。P55.01 在实际页面记录默认 5 秒节奏，持有真实 HTTP 响应跨越两个自定义周期并重复手动刷新，检查不重叠和保留内容；离开入口观察 1500ms、原生 Chrome 隐藏文档观察 2300ms，返回均检查立即查询。隐藏场景使用独占临时 Chrome profile、动态数值回环调试端口和公共 CDP 的 `noDefaults` 连接，避免普通自动化始终可见的覆盖；不改写 `document.visibilityState`，也不冻结定时器。P56.01 在真实服务中将独占 Run 根暂时变为普通文件、再损坏所选 Run JSON，检查 HTTP 500、上次列表/详情/图/时间不变，恢复文件后下一轮查询恢复现场；不调用任何 Run 状态动作。P67.03 配置为 1 秒并观察实际页面请求节奏，无页面配置入口。每次在 `frontend/test-results/workflowRefresh-*` 保留 JSON 原始请求时序及失败恢复截图；受控响应门仅改变时序，不替换业务字节。这些有界检查不证明无限期无请求，也不证明真实 omp/Herdr 模型行为。

## 记录及配置

沿用服务 TOML 的 `[scheduler]`：

```toml
[scheduler]
map_directory = 'D:\service-data\maps'
bot_directory = 'D:\service-data\bots'
run_directory = 'D:\service-data\runs'
run_instructions = 'D:\herdr-bot\docs\workflow-runtime.md'
workflow_poll_interval_seconds = 5
```

`workflow_poll_interval_seconds` 是服务启动配置，默认 5 秒，允许 1..=4294967295 的整数秒；零、负数、小数或字符串会让启动失败。修改后重启服务，页面从 `GET /api/workflow/runs` 的 `pollIntervalSeconds` 读取有效值；没有页面设置、推送或独立配置平台。间隔从上一轮完整查询结束起算，慢响应不追赶漏掉的 tick。

相对目录在服务启动时按服务工作目录解析一次；向 Chat 提供绝对路径，不按 Chat 的 zone/cwd 重新推导。默认 Run 根为 `.data/workflow-runs`。部署时应显式设置稳定的 `run_instructions`，不要依赖编译机器保留源码；默认指向随源码构建的本说明。

每个 Run 目录只有三份业务记录：

- `run.json`：当前已提交事实，包含 Map 引用、时间、管理者 Task/zone/Workspace、本次要求、管理者真实 Chat 关联及节点状态。
- `history.jsonl`：追加执行/状态历史；新 Run 初始为空。
- `messages.jsonl`：追加报告；新 Run 初始为空。

### 执行宿主字段

节点当前执行 `nodes.<id>.execution` 及 `previousExecutions[*].execution` 的非空记录必须包含 `agent`（`omp` 或 `pi`），记录该次实际启动所用的宿主。派发核实读取这份执行事实，不根据管理者宿主或后来修改的 Task 推测；retry 新建执行时使用当前绑定 Task 的宿主，保留旧执行原值。

不再提供旧格式手动迁移工具，请使用当前数据格式。运行时不补缺省宿主；缺少 `agent` 的旧执行记录会作为不可读现场报告。

### 独立读取与部分提交

三类记录的读取入口就是初始指示给出的三个绝对路径：`run.json` 直接解析为完整当前事实，`history.jsonl` 和 `messages.jsonl` 逐行读取为追加历史和报告；不要通过重放历史或取最后一份报告来重建当前状态。`GET /api/workflow/runs/<Run ID>` 的 `run`/`reports` 提供当前事实与完整报告，管理者 `status` 另给派生准入及 `pendingReports`。历史从已有 `history.jsonl` 路径读取，不另建日志浏览 API。当前失败原因、首次完成说明与 `previousExecutions` 中的旧执行/失败仍保存在当前事实中；历史保留失败、recover、retry 和完成时的原因与说明。旧执行报告可晚到，但不能替代当前执行的交接依据。通知代次及待办索引是内存/查询派生值，不落成第四份业务记录，也不是已读或完成凭据。

Run 的单阶段状态动作由 `backend/src/runs/operation.rs` 集中拥有后台任务生命周期、同 Run 操作锁、锁内重读、事实提交及结果解释。`completion.rs` 和 `recovery.rs` 保留动作资格与业务变化，返回拒绝／幂等不写入、仅追加历史、提交事实并追加历史的明确意图，不自行排列保存步骤。`reconcile_dispatch=uncertain` 只追加核实历史，不重写当前事实；需要查询 Herdr 的核实仍持有业务操作锁，但不持有文件协调资格。事实提交仍先通知观察者、再追加历史。管理者启动、node 派发及 retry 保留独立的多阶段流程，只共用适用的拒绝与历史失败反馈，不将关联保存和初始输入合并成单次提交。

当前状态写入失败时，动作报错且不发布候选状态、不追加成功历史；应先查询已提交现场并排除文件问题。dispatch/retry 如果已经创建 Chat，其 `effects` 与 `persistenceError` 仍如实报告外部效果，不表示 Chat 已撤销。

当前状态提交后，仅历史追加失败属于**部分生效**：非批次状态动作的响应、dispatch 的每项 `results[]`、retry 的 `result` 都保留 `historyError` 原因，并明确返回 `code=history-append-failed`、`stateCommitted=true` 和“当前状态已提交，但执行历史追加失败；状态未回滚”的说明。HTTP 200 在这里表示动作结果可解析，不表示所有记录都写入成功；调用者必须同时检查这些字段。批次仍处理其他合法目标。不得因为这一次错误自动封锁 Run、回滚事实或自动重试；当前磁盘状态及后续查询均保持已生效结果，排除文件问题后可显式执行下一次合法动作。重复已生效动作不会补写缺失的首次历史。若历史尾部残缺，仍拒绝向残缺记录追加，不能自动截断“修复”。

用户应从 workflow 进入已关联的管理者 Chat，表达“检查当前事实与历史写入错误，核实后继续”，不要手填身份参数、修改运行文件或盲目重派。manager-chat 的公共 Prompt 必须解释上述已提交/未提交区别、读取三份路径与检查每项结果的责任；node-chat 应继续按真实执行身份报告，不把历史错误当作回滚或重做指令。本文描述的接口位于后端 HTTP 和三份持久记录；外部角色扩展的实际部署/Prompt 注入仍须独立 toolkit/Herdr 证据，不能以 HTTP 验收冒充。

可重复确定性入口：`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch records:: -- --nocapture`。`current_write_failure_preserves_queries_disk_and_success_history` 使用真实只读当前文件，联合检查返回、后续查询、原字节和成功历史；`history_failure_reports_committed_completion_and_allows_next_action` 在状态提交后使历史追加失败，检查明确部分生效、缺失历史和后续合法 pause；`batch_history_failure_explains_each_committed_target_without_stopping_batch` 检查两个合法派发目标及失败 retry 的部分生效反馈。`independent_records_read_current_facts_without_replaying_history` 使用相互独立、故意不一致的当前/历史/旧执行报告样例；`operated_records_preserve_old_execution_recovery_completion_and_reports` 经真实 HTTP 执行 fail/recover/retry/report/complete，再核对三个文件、旧事实与 collect 不生成新业务记录。测试只替换外部 Herdr 为隔离记录型副本，文件写入与 TCP 入口真实，不证明模型、原生宿主或角色 Prompt。

正常顺序为整图预检 → 三份记录全部初始化 → Workspace/Tab/agent 启动 → 获取原生 session_id → 持久保存 Run 的真实关联 → 提交初始指示。单文件更新使用同目录临时文件后替换，不先删除正式文件。不承诺三文件事务、断电恢复、多进程写入协议或外部 Chat 回滚。

### 损坏记录与 Windows 文件协调

三份记录都是必要文件。缺失不会被当作空文件重新创建；当前记录不可解析、报告结构损坏、历史不是 JSON 对象或 JSONL 最后一条没有换行时，相关读取/追加明确报错，错误包含文件路径。详情读取同时检查历史记录是否可读，但不重放历史；管理者 status 的当前事实仍以 `run.json` 为准，不依赖历史推导。运行身份扫描无法读取记录时返回 `run-read-failed`，而不是把文件故障伪装成身份未关联；另一个可正常识别的独立 Run 仍可工作。旧 Run 未接管规则不变。

历史的 dispatch、retry 和状态转换追加，以及报告追加，均在同一次文件集合协调内先检查已有完整记录，再执行追加；不向残尾或损坏 log 继续写，不自动截断、补换行、重建空数据或重试。若损坏是在外部启动等待期间出现，已发生的 Chat/已提交状态不能回滚；历史失败仍按上面的 `stateCommitted`/`historyError` 部分生效反馈处理。文件拒绝访问、只读或 Windows 不允许替换时，正式旧文件保持原字节；排除访问问题后须由调用者显式发起下一次合法操作。

协调仅在当前进程内，统一持久化 module 一次取得整组路径的协调资格，目录范围与后代文件范围相交时互斥；读取、初始化、整体替换和追加均通过受限访问对象进行，不向业务暴露文件锁。不相交的 Run 文件集合不会互相排队。文件访问锁不跨外部 Herdr 启动、初始输入、报告投递或 collect 等待；同 Run 的业务串行边界仍覆盖整个派发批次，不能把文件可读误解为状态操作可越过批次。没有新增跨文件原子、断电安全或多进程写入保证。

用户遇到损坏提示，应保留原文件及错误路径，从 workflow 进入原管理者 Chat 表达“先核实保存事实、文件故障和已有外部效果，不要重派或修改记录”。不要手填身份、编辑运行 JSON、删除坏行或用空文件覆盖；部署维护者应先备份原字节，再依据可信记录处理存储故障。manager-chat 指引应明确：读取失败不是空 Run，历史追加失败不是状态回滚，先查已提交事实并说明未知项；node-chat 遇到报告保存失败须如实说明未保存，不宣称通知成功，也不因投递失败重复报告。角色 Prompt 的真实注入仍需外部 toolkit/宿主证据，本段不是替代验证。

可重复 Windows 检查入口：`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch integrity_ -- --nocapture`（P47.01/P48.01）。独占临时文件与真实 TCP 检查必要文件缺失、当前/报告/历史损坏、残尾、原字节保持、外部等待后拒绝追加及部分生效；Windows 专有用例用不共享 DELETE 的真实文件句柄阻止替换，核对旧句柄/正式路径字节和失败后的显式成功替换；并发用例交错 80 次详情读取与 40 次状态替换。外部 start gate 验证同 Run 可读且另一 Run 能保存 pause。再运行同一测试目标中的 `serialized_batch_finishes_before_pause_without_blocking_other_run_or_chat`、`collect_retains_merges_notifications_before_failed_delivery_and_keeps_pending_work` 和 `collect_wait_releases_on_pause_and_last_handoff_and_completed_is_immediate`，联合验证批次、投递与 collect 的等待边界。gate/实际调用记录决定先后，负向窗口分别为 100/200 ms，不以任意等待推定排序。上述保持生产文件层及 Herdr 调用包装，外部 Herdr 使用记录型副本，只证明确定性文件访问/故障语义，不宣称原生宿主或模型行为通过。

## 基础管理者状态协议

`POST /api/workflow/manager/status` 的传输正文仅为 `{"sessionId":"<当前真实原生身份>"}`，返回 `snapshot`，内容来自当前已提交的 `run.json`，不保存第二套 ready 状态。角色扩展必须通过公共 Session 能力自动取得身份；这不是模型业务参数，模型不提供 Run ID、角色或归属。

后端先校验同机来源，再通过已保存的管理者原生身份解析唯一的当前后端 Run 归属；没有关联、重复关联、非管理者均拒绝，不按名称、Task、Tab 或 cwd 猜测。session_id 不是秘密认证凭据：接受同机进程冒用风险，不构成强认证或沙箱。重启后旧 Run 只读，普通 Chat Resume 不代表接管 Run。

### 后端重启：历史现场只读

历史 Run 仅允许通过上述 PATCH 修改人用名称；名称写入 `run.json`，重启后保留，不向执行历史或报告追加事件。名称不参与调度、身份解析、Chat 名称、Map 引用或管理权限；同 Run 的名称更新与工作流写入互斥，外部身份等待期间保存的名称也不会被迟到的启动关联覆盖。

管理归属仅存在于创建 Run 的当前后端进程；`managed=false` 是服务上下文事实，不是持久 Run 状态。真正重启服务后，旧 Run 仍可通过详情链接及 `GET /api/workflow/runs/<Run ID>` 读取，原 running/paused/completed 等状态、节点失败、执行与已知管理者/节点 Chat 关联、报告和三份记录保持原样。服务不扫描记录恢复控制权，不把 running 改写为 failed/completed，不发起执行，也不提供自动或手动接管入口。

重启前的管理者身份调用 status、dispatch、complete、finish、pause、resume、fail、recover、retry、reconcile_dispatch、collect 均拒绝（HTTP 403、`manager-unassociated`）；旧节点身份 report 同样拒绝（HTTP 403、`node-unassociated`）。已有报告或 pendingReports 不恢复写入资格，collect 不进入等待、不返回正常快照；普通 Chat Resume 也不恢复 Run 管理。用户可继续读取保存事实、核实原 Chat 与产物，但不能通过改文件、重放初始指示或将 Resume 当作接管来继续旧 Run。角色工具应将拒绝反馈给调用者，不自动重试或另起执行。

管理者启动失败的 `startupError` 仅保存在原后端内存，浏览器刷新可以查询；真正重启后为 null，不从业务历史重建到页面。已提交 Chat 关联仍保留，节点的持久 failure 则继续可见。需要跨进程查询启动诊断时，应由部署环境保留服务 stdout/stderr（含 Run ID、stage 和已发生效果）；三份业务记录不是诊断日志，也没有页面日志浏览入口。

可重复确定性验收入口（仅数值回环、独占 cwd/数据/Herdr 记录型副本、真实服务进程与交付页面）：

```text
npm --prefix frontend run test:browser:build -- workflowStart.spec.ts --workers=1
```

`P28.04 P49.01 P68.08 real restart preserves facts but rejects old Run tools` 在真实 Run 中派发 running/failed 节点并产生报告/历史，再重启；核对所有管理动作与 report/collect 拒绝、完整详情与三份文件逐字不变、页面未接管说明及外部调用记录。`P54.02 real restart drops manager startup errors without rewriting business facts` 在真实启动指示故障后保留进程诊断，重启并刷新页面，核对错误消失、已知 Chat 与空业务历史保留。二者各保留截图、前后事实/文件、外部调用及 **1000ms 有界无新增调用窗口**；不以有限窗口声明永远没有自动行为。受控 Herdr 仅证明确定性宿主响应处理，不证明真实 omp 原生 Chat 或 toolkit Prompt 注入；后者仍需部署角色扩展及隔离真实 Herdr 后单独验收。

status 除 `snapshot` 外返回派生的 `ready`（可派发 node ID）、`blocked`（每个 node 的当前阻塞原因数组）及 `canFinish`。依赖只在所有前置 node 为 completed 时满足；running、failed 等非 pending 节点不可首次派发。查询不保存第二套 ready 真相，不自动选择路线或创建 Chat。

### 管理者显式批量派发

`POST /api/workflow/manager/dispatch` 的传输正文为：

```json
{"sessionId":"<公共 Session 能力取得的当前真实身份>","nodes":["A","B"]}
```

模型业务参数只有目标 node ID 数组；不得让模型填写 sessionId、Run ID、角色或归属。后端按真实管理者关联解析本次 Run，逐项重新检查状态与依赖，返回 `results` 和与 status 相同的查询字段。每项包含 `nodeId`、`status`（`rejected`、`failed` 或 `running`）。拒绝项有 `reasons`，启动失败项有 `failure.stage/message/effects`；已发生的 Workspace、Tab、pane、agent、原生身份及指示尝试/确认均如实保留。`persistenceError` 表示失败事实未能落盘，`historyError` 表示当前事实已提交但追加历史失败；调用者不能把它们当作全量成功或自动重试理由。

每个合法目标沿 node → Bot → 普通（normal）Task（omp/pi） 解析完整 trigger config 和 env config，在定义中仍登记且有效的 zone 创建独立原生 Chat。目标 Workspace 只取已保存的管理者实际 Workspace ID；目标不存在或查询失败即报错，不按目录名重选，不退回当前/默认 Workspace。原生身份必须不同于本 Run 的管理者及已有 node Chat，宿主返回的 cwd 必须与定义 zone 一致。真实关联保存前不提交初始指示；运行文件锁不跨宿主调用。

节点初始输入按原有 Skill → Task 固定 prompt → 本次 input 组合规则提交。本次 input 包含自身 node 标识/名称、运行说明及三份记录的绝对路径，并在业务原文外提供独立策略控制段，明确本次 `report_policy: auto|manual` 及行为定义，最后保留 node.input 原文；不附 manager user_input、Bot 文档或完整 Map。node 应主动按需读取说明与记录，在自身 zone 工作；记录路径可以位于 zone 外，不按 cwd 推导替代路径，也不增加可读性门禁。node 不得通过自行退出、停轮或报告假定完成交接，流程推进由管理者显式决定。

同批拒绝或启动失败不取消其他合法目标，不关闭或回滚已创建的 Chat。成功派发后 node 为 running；启动失败为 failed，保留执行 ID、来源 Task、zone、可得 Chat 关联及失败阶段/原因，并追加执行历史。正常已记录的 running/failed 再次 dispatch 均拒绝，不增加外部启动、不覆盖首次事实。崩溃或关联保存失败窗口不承诺完整幂等，先核实已发生效果，勿盲目重发。同 Run 派发批次互斥；不同 Run 的身份与事实独立。

manager-chat 需要在独立 toolkit 中通过公共 `PromptDeclaration` 的 `first-enabled-run` 提供 status/dispatch 及下文 complete/finish 指引，并将工具接到对应协议。node-chat 同样通过公共 Prompt 能力给出节点职责与稳定路径读取指引。后端协议和页面不是该外部扩展已交付或真实注入已验证的证据。部署缺少 metadata 指定的任一角色扩展入口时先补齐实际扩展，不能用空函数、手填 sessionId 或 Tab 代替真实角色工具验收。

## 角色归属与来源集成检查

运行工具的归属不能由 Run ID、Task 类型或工具调用者自报决定。后端只按保存的真实原生 session_id 查找关联：管理者必须在当前后端内有唯一 Run 归属，且不能同时关联任何 Run 的当前或保留节点执行。出现跨 Run 的 manager/node 身份冲突时，所有管理者动作统一返回 `403 manager-ambiguous`，不按先找到的角色授权；应核实实际 Chat 关联，不能用同机来源或重复请求绕过。无关联节点/管理者身份也不能因来源在回环地址而取得权限。旧节点执行仍可提交归属于原执行的历史报告，但不能冒充当前执行或管理者，迟到时间不改变报告适用性。

可重复的后端契约检查入口：

```text
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch security:: -- --nocapture
node scripts/check-test-network-policy.mjs
```

`backend/tests/workflow_dispatch/security.rs` 通过既有 S2/S3/S4 interface 检查：

- `two_run_role_and_retained_execution_matrix_rejects_scope_forgery`：同 Map/Task 的两个 Run，分别关联管理者、当前节点与保留旧执行；报告接收者/适用待办隔离；全部管理者路由拒绝节点及无关联调用；两个方向的 Run/role/execution/source 冒充和节点报告的自报归属均被拒绝。拒绝前后对照公开详情、三份记录字节及 150ms 窗口内实际外部调用。
- `manager_authorization_rejects_cross_run_current_and_old_node_identity_collisions`：受控外部宿主返回跨 Run 重复身份，分别覆盖当前及保留旧节点执行与管理者角色冲突；全部管理者路由拒绝，不改两个 Run、不产生新外部调用。
- `source_matrix_enforces_local_transport_and_session_association_separately`：真实回环 HTTP 启动后，在同一生产应用的 HTTP 边界注入独立 TCP 来源样例；IPv4/IPv6 回环允许已关联管理者查询，却拒绝无关联身份；三个文档地址样例对全部角色路由返回来源拒绝，即使请求携带已关联身份。没有监听 LAN 或发布 mDNS，受控 TCP 对端不是实际网卡验收。

这些检查保留真实 HTTP 处理、持久化和生产 Herdr adapter；外部记录型 Herdr 是受控条件，测试中的入口占位文件只让后端配置检查可执行，**不证明角色扩展可加载、工具参数面正确、Prompt 已注入或 connect-bot 关闭时原生运行成功**。同机校验仍不是认证，session_id 仍不是秘密凭据，同机进程冒用风险不因测试通过而消失；没有新增 token。

双角色部署验收还必须在独占 Herdr/omp 环境实际加载 metadata 指定的 `manager-chat.ts` 与 `node-chat.ts`，关闭 connect-bot，验证普通 Task 独立启动 → 同一普通 Task 节点派发 → 节点 Chat Resume，并另行验证 manager Task 的 manager 角色，并使用两个不同 node cwd。通过真实工具与 Prompt/Session/HTTP 公共能力观察对应角色工具、首次启用指引、原生身份和实际请求目标；使用显式非默认 endpoint、config.path/configBaseDir 对照有效配置与真实请求，不按角色 ID 推导目录。还须在 toolkit 交付源码上检查角色 → common、common 不反向依赖角色、foundation 不依赖 business，且不引用 connect-bot 私有实现。缺少角色入口或源码时，这部分 P22.01 工具参数面、P58.01、P60.02、P67.04 必须报告未验证，不能用上述后端矩阵或入口文件存在替代运行证据。

## 派发验证入口与证据边界

确定性派发检查入口为 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture`，完整后端交付检查为 `node scripts/test-backend.mjs`（先按测试指引构建前端）。这些测试使用真实 HTTP TestServer、每例独占的实际文件及外部记录型 Herdr 副本，覆盖关联提交屏障、完整字面触发输入、批内拒绝/失败/成功、重复拒绝、Run 隔离、磁盘故障与 Workspace 不回退。夹具进程记录实际 cwd，但它不是 omp 原生 Chat，也不验证模型或公共 Prompt 注入。

真实宿主复验还需部署 metadata 指定的 manager-chat/node-chat，使用独占验收 zone、数据目录与 Herdr 实例：经 manager 实际工具先查 A/B ready、C 被 A/B 阻塞，再派发 A/B；保存原生 session_id、实际 cwd 和 Workspace ID、实际输入及三份记录。与 Task 的独立预期逐字段核对 model/thinking、初始 Skill、固定 prompt 和 node.input；管理者本次要求与 Bot/Map 文档使用互异标记，必须不出现在 node 实际初始输入中。让验收 node Chat 读取传入的 zone 外说明/记录路径，而不以服务端自己可读代替。最后重复派发，观察外部无新增启动。角色扩展未部署或无法隔离宿主时，这部分仍是未验证，不用上述受控结果替代。

## 显式交接完成与结束 Run

管理者检查结果确实可交接后才使用 `complete`，不必等执行 Chat 停轮或退出。管理者工具的业务参数只有 `nodeId` 与 `reason`；`reason` 必须为非空说明，应记录接受的结果、地址及限制，原文保留。传输身份仍由公共 Session 能力自动取得，不让模型或用户手填归属：

```http
POST /api/workflow/manager/complete
Content-Type: application/json

{"sessionId":"<当前管理者真实身份>","nodeId":"A","reason":"结果已可交接；清理工作仍在原 Chat 继续"}
```

Run running 或 paused 时，running node 可以完成；人工已处理并确认可交接的执行阶段 failed 也可完成，但 pending 或派发阶段 failed 不能借 complete 跳过工作或核实。完成不产生报告、不补发材料、不自主派发下游；原 Chat 默认继续存在，只有本次执行明确启用下述自动关闭时才尝试关闭其 Tab。所需结果地址与限制仍应通过原 Chat 的 report 补齐，后端不额外判断结果质量。A/B → C 中只完成 A 不够：C 仍由 B 阻塞；两者均 completed 后，管理者须另行显式 dispatch C。

成功后返回 `changed=true`、`snapshot`、`ready`、`blocked`、`canFinish`、`reports` 和 `pendingReports`。说明与操作 ID、时间、真实管理者 `actor` 保存在 `snapshot.nodes.<nodeId>.completion`，状态与说明在同次当前事实写入中提交；对应 history 的 `action=complete` 含前后状态、说明与执行引用。同 Run 动作与整个派发批次串行，动作快照在该边界内生成，后续动作仍检查最新事实。completed node 不再列入可操作报告待办；补充 report 不重新打开节点或 Run。

Run 尚未结束时重复 complete 返回 `changed=false`，不覆盖首次说明、时间、操作 ID 或成功历史。拒绝返回 HTTP 409，并附 `code`、原因、`changed=false` 和当前查询字段；例如 `node-not-completable`、`completion-reason-required`、`node-unknown` 或 `run-terminal`。无关联身份仍按管理者权限拒绝，不接受额外 Run/归属字段。

### 节点完成后自动关闭关联 Tab

Map node 的 `auto_close` 缺省为 `false`；dispatch/retry 读取派发时的当前 Map，并固定为本次 `execution.autoClose`。Run 创建时不冻结此选择，之后修改 Map 不改变已有执行；recover、核实或恢复原 Chat 沿用原执行设置。旧执行缺字段按 `false` 使用，不改写历史或补偿关闭。

只有 manager 合法 complete 使节点首次进入 completed，且当前完成事实保存成功、本次执行启用自动关闭时，才异步尽力调用一次 `herdr tab close <tab_id>`。目标仅取当前执行保存的 `chat.tabId`，没有可靠关联则跳过，不扫描、按名称猜测或使用 manager／其他节点／旧执行的目标。当前事实保存失败不关闭；事实已提交但历史追加失败仍可关闭，保留原有部分生效反馈。重复 complete 不产生新资格。

关闭不会等待 agent 停轮、空闲、退出或收尾文字，也不先发送 stop。complete 的响应与同 Run 后续合法操作均不等待关闭命令返回；外部等待不持有 Run 操作锁或文件协调范围。关闭会终止该 Tab 内终端及相关进程，不删除 Chat 历史。命令失败、不可用、超时或要求额外确认不撤销完成、不重试、不绕过确认；尝试与结果只记带 Run/node/执行/Tab 上下文的后端日志，不新增业务状态、关闭结果记录或 workflow 通知。report、停轮、等待输入、退出登记、fail、pause/resume、整体 finish 和读取均不独立触发关闭；当前 workflow 没有节点取消或停轮命令入口，不为此新增入口。

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch auto_close:: -- --nocapture` 通过真实 HTTP 和隔离 Herdr 命令副本核对 Map 双向修改、旧记录、权限及完成资格、当前执行精确目标、重复调用、工作中完成及非阻塞顺序。关闭 gate 在释放前保持命令未返回，测试先观察 complete 响应与同 Run 依赖节点真实派发，再释放并确认返回；10 秒有界等待仅防挂死，不是业务速度承诺。负向检查窗口为 150 ms；受控命令记录不证明真实 Herdr 的 Tab 移除或进程终止，真实外部效果需另行专属资源验收。

P14/P17/P20/P21 的浏览器与诊断覆盖位于 `frontend/browser-tests/autoCloseIsolation.spec.ts`，通过 npm 浏览器入口运行。场景分别提供成功、命令拒绝、结构化 `confirmation_required`、派发后移走测试专属执行文件，以及缺少 Chat/空白 Tab 的有效历史执行。生产输出保留 Herdr 的机器错误码与说明，日志区分尝试、成功、失败和缺少目标跳过；不可用时有一次后端执行尝试，但没有外部 CLI 进程启动。测试读取外部 `invocations.jsonl`（含不支持命令），不只数 adapter 的已知关闭记录，以检查无强制确认或其他绕过。所有场景经合法 complete、同 Run 下游真实派发及重复 complete，比较公开结果、三份持久事实与真实 workflow DOM；750 ms 有界窗口只用于观察未重试/未显示关闭结果，不承诺无限未来。该替身覆盖不证明真实 Tab 已移除或终端已结束（P22）。

### 结束 Run

全部节点 completed 后，管理者显式结束本次 Run：

```http
POST /api/workflow/manager/finish
Content-Type: application/json

{"sessionId":"<当前管理者真实身份>"}
```

finish 没有模型业务参数。running 或 paused 的 Run 全部节点 completed 才能成功；仍有未完成节点返回 HTTP 409、`code=nodes-incomplete`，不自动完成它们。成功返回 `changed=true` 及同样的查询字段，Run 变为 completed，`snapshot.finished` 保留首次操作 ID、时间与真实管理者，history 追加 `action=finish`。`canFinish` 表示**当前仍可执行 finish**，所以终态为 false；重复 finish 仍返回已有成功事实、`changed=false`，不追加历史。终态可查询、可接收当前 node 的补充报告，但拒绝 complete 和新 dispatch；dispatch 拒绝项不会调用宿主。活动中的 Chat 不阻止完成或 finish；finish 不关闭 Chat，complete 仅按本次执行的自动关闭选择尝试关闭关联 Tab。

当前事实保存失败返回 HTTP 500、`complete-failed` 或 `finish-failed`，不采用候选状态或追加成功历史。当前事实已保存但历史追加失败时，仍返回已生效快照与非空 `historyError`，不要当作全部未生效或盲目重试。三份记录不承诺跨文件事务；重复调用也不会修复或重复追加首次缺失的历史。

manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 应将 status → 读取报告/核实交接 → complete → 显式 dispatch 下游 → 全部完成后 finish 的职责与上述拒绝/部分生效语义注入真实管理者 Chat。node-chat 指引应明确 report、停轮、退出都不替代管理者交接。本仓库没有外部 toolkit 的角色工具实现；HTTP 路由与此说明不是工具已装配或 Prompt 已注入的证明，不通过手填 HTTP 身份冒充真实角色工具验收。

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
| **人工结果已可交接**：执行阶段 failed，人工已实际完成并核对结果。「A 的结果已人工补齐在绝对地址……，限制是……，已确认可交接；请记录人工完成，不再执行。」 | 亲自核实结果与限制，用 complete 的说明记录人工依据。需要补充正文时由已有关联 node Chat report，不伪造报告。 | A completed、完成说明可读、依赖满足才出现下游资格；不产生假 report，不自动传材料或派发下游；仅按本次执行的自动关闭选择尝试关闭关联 Tab。pending/派发阶段 failed 不可这样跳过工作。 |
| **仅通知失败**：node report 返回保存成功而通知失败。「报告已保存，报告标识是……；请读取该报告并核对交接，不要因为通知失败重报、重派或回滚。」 | 先 status/collect，再按报告标识读取正文与实际材料；确认后才 complete。node 保留原报告标识与投递错误，不仅因通知/诊断失败重发 report。 | 已保存报告与待办仍在，node 状态不被通知错误改变。collect 只给标识与快照，不含正文；读取/通知/collect 不清除未完成待办。输入已提交也不保证管理者已读或被打断。 |

**派发阶段 failed 先核实，不套用上表的执行恢复。** 有 Tab 不等于已有正确原生 Chat。向管理者分别说明「已确认原 Chat 存在且可跟踪」「已确认没有创建原生 Chat」或「目前无法确认」，并给出证据，由其 reconcile_dispatch：已创建只读核实原目标身份并恢复原执行跟踪；未创建恢复 pending，之后仍需显式 dispatch；不确定保持 failed。不要为取得 pending 而谎报未创建，不把确认参数当成回滚授权。核实不会重发可能遗漏的初始指示或补发材料。

### 角色责任与批准动作矩阵

以下是实际角色 Prompt 应覆盖的内容与工具业务参数审查基准，**不是已注入的声明**。身份与归属必须由公共 Session 能力和后端真实关联取得；模型不能提供 sessionId、Run/角色/执行归属或接收者。工具 description、Task 固定 prompt、本文路径存在或模型碰巧做对，都不能单独证明公共 Prompt 已注入。

| 管理者动作／业务参数 | 准入与状态责任 | 实际执行／交接边界 |
| --- | --- | --- |
| status；无 | 唯一可信管理者；终态仍可查当前快照、ready、blocked、canFinish。 | 读取不是派发授权，后续动作重新检查现场；不修改状态。 |
| dispatch；nodes | Run running，逐项 pending 且全部前置 completed；整批返回逐项结果。 | 创建独立原生 Chat，单项失败不取消其他项，不回滚已有效果。 |
| complete；nodeId、reason | Run 未结束，running node 或人工已确认可交接的执行阶段 failed；说明非空。 | completed 是交接完成并满足依赖，不要求 Chat 退出；不制造报告、传材料或自动派发。 |
| fail；nodeId、reason | Run 未结束、node running；保存执行失败与首次原因。 | 保留原 Chat；不暂停、重试或终止实际执行。 |
| reconcile_dispatch；nodeId、finding、reason | 仅派发阶段 failed；已创建／未创建／不确定按核实事实处理，不接受任意目标状态。 | 原派发目标与身份由系统核实，不由模型自报替换；不创建 Chat 或重放输入。 |
| recover；nodeId、confirmed、reason | 执行阶段 failed、有关联原 Chat、人工确认可继续；paused 也可用。 | 只恢复原执行跟踪，不修复现场、恢复进程、补发指示或撤销失败历史。 |
| retry；nodeId、confirmed、reason | running Run 的执行阶段 failed；人工核实同意，派发不确定先核实。 | 独立新执行、新 Chat，完整 trigger config 生效；保留旧事实与副作用，不转用旧报告。 |
| pause；无 | running → paused；等待同 Run 在途完整批次结束后生效。 | 只阻止新执行，已有 node 继续工作/report，管理者仍可核实、recover、complete。 |
| resume；无 | paused → running，只恢复派发资格。 | 不自动 dispatch/retry，也不是普通 Chat Resume。 |
| finish；无 | running/paused 且全部 node completed，显式结束；重复返回首次事实。 | 不等待或终止 Chat。终态拒绝后续状态变更，不重开 Run。 |
| collect；无 | 当前管理者收取通知，返回 reason、快照与报告标识；不修改 node 状态。 | 按标识读取正文，核实交接后才 complete；不把收取当成已读/完成。 |

管理者须逐项读批次结果、已发生效果及部分生效错误；保留原执行、材料地址与限制，显式选择后续动作。**后端**落实真实角色、当前执行、依赖、状态与持久化约束，不能以模型表现替代这些约束。node 的职责只有实际工作和 report（业务参数仅 body）：交接可用时写清地址/用法/限制，可以随后继续不影响结果的收尾；report、停轮、退出均不等于 completed。旧执行补充报告必须归原执行，不代表当前新执行完成。pause 不撤回 node 的已有工作。

collect 的正常原因分别是 notification、paused、completed、no-running、timeout；有 running 且无新通知时可等待，不保证有 ready 就即时返回。正常到期不改状态；取消、断线、预算耗尽或读取失败是错误，不是空快照或正常 timeout，不自主重试。管理者若要再检查须明确重新调用；工具中断/Chat 结束须经公共 HTTP 取消等待，整个请求预算大于后端期限（默认后端 90 分钟、调用方建议 91 分钟）。node 对 report 要区分保存失败、保存成功/通知失败和诊断失败，不能仅因通知/诊断失败重报。

### 可重复演练与 H2 证据要求

观察入口沿用真实交付页面（S1）和实际角色工具、公共 Prompt/Session/HTTP 与 Herdr（S3），不新增文档测试 seam。演练前先固定预期：recover 保持原身份；retry 创建新身份且保留旧事实；人工 complete 只完成交接不产生报告；仅通知失败保持已保存报告与状态。能区分的违规分别是偷偷另起 Chat、用旧报告交接新执行、伪造报告/自动传材料，以及丢报告/重复重报。只检查最终 completed 不能排除这些违规。

由执行 Agent 在新建的独占数据/cwd、manager/node zone、动态数值回环端口与独占 Herdr 实例内完成上面的四条用户路径；按[测试指引](agents/testing.md)通过 npm 浏览器入口构建页面，持续消费所属进程输出并清理所属资源。先核实 metadata 指定的真实 manager-chat/node-chat 可加载、omp 未启用 autoResume、Herdr 可隔离；缺任一前提就保存错误，不能操作用户默认 Herdr 或将空入口/记录型 adapter 当实演。不得修改运行文件制造成功；故障/修复只作用于演练拥有的工作材料或外部通知条件，记录具体操作，不引入生产故障开关。执行失败由管理者根据真实错误显式 fail 后再处置；各分支使用独立现场，避免前一分支的 completed 掩盖后一分支条件。

每条路径保留：查看失败的页面与阶段/原因 → 指定 Chat Resume 及实际定位结果 → 核实/修复依据与所有人工补充输入 → 管理者收到的自然语言意图 → 原生工具的实际参数/结果 → 刷新后的执行、状态、报告和历史。联合保存 Run ID、原生 Chat/执行关联、Task/Map/说明版本、三份原始记录、实际产物和外部调用。对禁止自动动作的检查写明观察起止、时长和全部外部调用；有限窗口不证明无限期无动作。通知失败还须同时保存 saved/report ID、notification 结果与管理者按标识读到的同一报告，不能只截错误提示。

Prompt 证据须捕获两种角色**实际收到**的公共 `first-enabled-run` 注入正文及对应会话/启用时机，与上表、collect、参数面和四条路径逐项对照；源码声明或 description 仅是候选内容，不能代替实际注入。分别标注「指南内容审查」「确定性后端约束」「真实角色注入/处置」的已验证与未验证项，发现的指南缺口修回本文。H2 最终只留用户对清楚/可行性的判断；不把实现、故障构造或实演转交用户，不因准备步骤齐全就称 P64/P65 通过。

## 失败核实、原执行恢复与人工交接

失败不等于 Chat 已退出，更不等于运行应暂停。后端不会因派发或执行失败自动 retry、pause、终止 Chat、补发初始指示或传递材料。先读取 status、原 Chat 与三份记录，再请用户核实或修复现场。Run running/paused 均允许以下非启动动作；completed 终态全部拒绝（HTTP 409、`run-terminal`），保留首次 finish 事实。

### 记录执行失败：fail

`POST /api/workflow/manager/fail`：

```json
{"sessionId":"<公共 Session 能力取得的当前管理者身份>","nodeId":"A","reason":"已核实：缺少上游源文件，原 Chat 暂不能继续"}
```

只有 running node 可首次 fail；`reason` 必须非空。成功保存 `status=failed`、`failure.stage=execution`、`failure.message` 和原执行 ID、Task、zone、真实 Chat，不改变 Run 派发资格。执行失败不凭空生成启动效果字段；派发失败的 `failure.effects` 仍保留启动时观察到的实际效果。history 的 `action=fail` 包含首次原因、前后状态、执行引用、管理者 actor、操作 ID 与时间。重复 failed 的 fail 返回 `changed=false`，不覆盖首次失败、当前文件或历史，不调用宿主；它也不能把派发失败改成执行失败。pending/completed 首次 fail 返回 `node-not-failable`，空原因返回 `failure-reason-required`。

### 只核实派发失败：reconcile_dispatch

`POST /api/workflow/manager/reconcile_dispatch`：

```json
{"sessionId":"<当前管理者身份>","nodeId":"A","finding":"uncertain","reason":"用户尚不能确认原派发是否创建了 Chat"}
```

模型业务参数仅 `nodeId`、`finding`、`reason`；没有任意目标状态或调用者自报 Chat 归属。只有派发阶段 failed 可核实；执行阶段 failed 必须 recover 或确认可交接后 complete，返回 `node-not-dispatch-failed` 防止绕过。非空原因必须说明人工核实依据。

- `finding=uncertain`：当前事实完全保持 failed，保留原失败原因与执行。`changed=false`，history 追加这次不确定核实及依据，但不触发宿主调用或派发。
- `finding=not_created`：人工确认未创建原生 Chat，恢复 pending，清除当前失败/候选执行；旧执行、原失败与依据仍在 history。已记录真实 session_id 或 Chat 关联时拒绝 `dispatch-chat-already-known`，不能用此动作抹除已发生事实。pending 只表示再次有资格，不自动 dispatch；已有 Tab 也不会被关闭。
- `finding=created`：用户确认原 Chat 存在并可跟踪，后端只对**原派发效果中记录的 agent** 调用只读原生身份查询。身份必须为真实 omp 会话、cwd 与原 zone 一致，不得替换已知原身份或复用管理者/其他节点身份；关联原执行的 Chat 后恢复 running。原执行 ID、Task、zone、Workspace/Tab/pane/agent 引用不变，不创建 Chat、不提交初始指示、不补发遗漏材料。缺少原派发现实目标或宿主身份不可确认时返回 `dispatch-chat-unverified`，身份/zone 冲突返回 `dispatch-chat-mismatch`，保持 failed。无已知原派发目标时不能通过猜测或直接编辑记录强行关联。

每次成功核实的 history `action=reconcile_dispatch` 保留 `finding`、依据、原 failure/执行和当前执行引用；确认后当前 failure 清空，但失败历史不删除。created 查询失败不伪造成功，也不自动再查或启动。

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

模型业务参数只有 `nodeId`、`confirmed` 和非空 `reason`；身份仍由 Session 自动取得。仅 **running Run 中的执行阶段 failed** 可以 retry。`confirmed=true` 是人工核实/同意的记录，不是自动安全判断或回滚授权。缺确认或空依据返回 HTTP 409、`retry-confirmation-required`；pending/running/completed node 返回 `node-not-retryable`。paused 明确返回 `run-paused`，必须先显式 resume；resume 本身不创建 Chat。终态优先返回 `run-terminal`。旧 node Chat、无关联身份和额外 Run/执行/角色字段不能调用管理者动作。

派发阶段 failed 返回 `dispatch-reconciliation-required`，包括未知原生身份的异常窗口：先 reconcile_dispatch，不以 retry 的 confirmed 代替核实。确认未创建则恢复 pending，之后另行 dispatch；确认原 Chat 已创建则恢复原执行 running，若核实该执行无法继续，再 fail 记录执行失败后选择 retry。不确定保持 failed，不盲目启动另一 Chat。若原执行可继续，使用 recover，不是 retry。

retry 复用首次 dispatch 的完整启动路径：当前 node → Bot → 普通 Task 的 trigger config/env config，并按节点执行上下文固定加载对应宿主的 node-chat、定义 zone、已保存管理者的实际 Workspace ID，创建独立新原生 Chat。model/thinking、初始 Skill、Task 固定 prompt 与 node.input 按原有组合规则全部生效，不只重放 node.input。实际目标不可用就失败，不回退目录或 Workspace。新身份不得复用管理者、当前执行或任何保留旧执行的身份；真实新关联保存成功前绝不提交初始指示。不会结束旧 Chat、撤销产物、清理现场、转发旧报告或自动派发下游。

当前 `snapshot.nodes.<nodeId>.execution/failure` 只属于新尝试；`previousExecutions` 依次保留每次被替代执行及当时的 failure，当前事实读取不依赖 history 重放。首次节点的该数组为空。解析目标失败的新尝试仍有独立执行 ID，并保留原来源 Task/zone 引用用于解释失败；它不是可用目标或自动回退。`history.jsonl` 追加 `action=retry`、管理者 actor、依据、`previousExecution/previousFailure` 和新 `execution/failure`，不覆盖既有 dispatch/fail/报告历史。

正常返回 HTTP 200，包含 `result` 与 status 相同的 `snapshot/ready/blocked/canFinish/reports/pendingReports`。`result.status` 为 running 或 failed；失败有 `failure.stage/message/effects`，实际新 `execution`、可得新 Chat、`persistenceError`、`historyError`。即使外部新 Chat 已启动而身份查询、关联保存或指示提交失败，也不把错误归到旧执行、不声称外部回滚。关联保存失败且失败事实也未能落盘时，snapshot 保持最后已提交的旧事实，result 与诊断保留新尝试和外部效果；不要因旧快照仍 failed 就盲目 retry。确认新 Chat 与磁盘后，按实际事实处理。普通读取/内部失败返回 `retry-failed` 或 `retry-internal`，不是自动重试许可。状态已提交而 history 追加失败仍是部分生效，不回滚新执行。

旧执行 Chat 可以继续 report；后端按持久保留的原执行身份归属，记录原 executionId/source，不把它伪装成新执行报告。旧/新/旧/新的交错报告全部追加到 messages.jsonl，status 的 reports 保留完整正文历史；pendingReports 只按每个 node 的**当前适用执行**汇总最新报告 ID，不按全局最新时间选择。retry 后旧报告不能成为新执行待办，也不能完成新执行；新执行仍须管理者核实其结果并显式 complete。completed 后旧/新补充报告均不重开待办或 Run。

用户处理路径：进入已关联管理者 Chat → 说明已核实的旧 Chat、产物位置、限制和另起执行意图 → 观察管理者 retry 结果的新执行/新 Chat 与保留的旧事实。不要手填 sessionId、编辑运行记录、为了获得 pending 状态而谎报原 Chat 未创建。manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 必须注入上述 recover/retry 区别、人工确认、暂停/终态拒绝及部分生效语义；node-chat 必须说明旧执行 report 只补历史，不能完成当前新执行。本仓库交付 HTTP 和运行说明；metadata 指定的外部角色入口未部署时，真实角色工具/Prompt/Herdr/omp 验收仍未完成。

确定性 retry 入口：

```text
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch retry:: -- --nocapture
```

`backend/tests/workflow_dispatch/retry.rs` 使用真实 HTTP TestServer、独占磁盘和外部记录型 Herdr 副本：

- `retry_saves_new_identity_before_full_trigger_and_retains_original_facts`：P13.03/P14.02/P15.03/P41.01/P68.07，初始指示门前的新身份落盘、独立字面完整触发配置、实际夹具 cwd/Workspace 和预留产物保留。
- `late_old_reports_remain_history_without_controlling_new_execution`：P27.02/P41.01/P68.07，旧新报告交错、collect 仅当前适用 ID、旧身份不能变更当前执行、显式交接。
- `retry_rejects_reuse_of_any_retained_native_identity`：P14.02/P41.01，多轮 retry 拒绝复用最早身份，原报告归属仍唯一。
- `retry_startup_failures_keep_both_attempts_and_do_not_rollback`、`retry_association_failure_exposes_new_attempt_without_overwriting_old_snapshot`：P13.03/P45.03/P41.01，身份/指示/关联失败的实际启动效果及旧失败保留。
- `paused_retry_requires_explicit_resume_and_human_confirmation`、`terminal_run_rejects_retry_without_new_chat_or_record_changes`、`uncertain_dispatch_cannot_retry_until_original_chat_is_reconciled`、`unavailable_retry_targets_fail_without_fallback_or_reusing_old_execution`：P35.04/P36.03/P41.01/P14.02，拒绝路径与后续合法显式启动。

这些检查保留 adapter 全部调用记录；无自主终止/重试/派发的断言覆盖请求与之后 **150ms 有界窗口**，不是无限期保证。夹具的启动记录不证明 omp 原生会话、真实工具注入或模型行为。真实 S3 复验需先部署角色入口并隔离 Herdr/zone/数据：让旧原生 Chat 留下互异标记产物并失败，经真实管理者工具 retry，核对新旧原生 session_id、zone、Workspace、实际完整输入和三份记录，再交错旧/新报告并观察当前待办。保存明确起止的无额外宿主动作窗口与人工干预；缺入口/隔离宿主时如实报告未验证，不以手填 HTTP 身份或受控身份代替。

### 人工已处理且确实可交接

用户已人工补齐结果且确认可交接时，管理者可对执行阶段 failed 显式调用上文 `complete`，用非空 reason 写清结果地址、限制与人工处理依据。pending/派发 failed 不能这样跳过核实。该动作只完成状态交接、解除依赖，不产生 report、不补发材料、不启动下游；仅按本次执行的自动关闭选择尝试关闭关联 Tab；失败和原执行历史仍可读。需要向管理者补充正文时，在**已有关联的 node Chat** 使用现有 report 写明地址与限制；completed 后补充报告不重新打开待办。管理者须在后续显式 dispatch 下游时自行提供所需材料，不依赖新增消息工具或自动传播。

### 错误、调用者与用户处理路径

上述 HTTP 只接受同机、唯一当前管理者真实 session_id；角色扩展必须通过公共 Session 能力自动取得它，模型和用户不填写身份/Run/角色/归属。未知字段、无效 finding 或缺少必要参数明确拒绝，不存在任意状态后门。成功返回与 complete 相同的查询字段、`changed`、`historyError`。状态写入失败返回 HTTP 500、`fail-failed` / `recover-failed` / `reconcile-dispatch-failed`，不采用候选状态或追加成功历史；状态已提交但历史追加失败时保留生效快照并返回非空 `historyError`，不假称全部未生效或自动重试。

用户从 workflow 查看 Run/node，进入已关联管理者 Chat 表达意图，而不是手填工具参数或编辑运行文件：

1. 派发失败：查看已发生效果与原 Chat → 必要时普通 Resume 进入已存在 Chat → 核实「已创建 / 未创建 / 不确定」→ 将结论和依据告诉管理者，由其 reconcile_dispatch。不确定继续 failed，不盲目重复 dispatch。
2. 执行失败：确认原 Chat 和当前原因 → 人工修复现场 → 告诉管理者「已修复，继续跟踪原执行」，由其 recover；需要恢复原生 Chat 进程时另用普通 Resume，它不代替 recover 或接管 Run。
3. 人工处理后已可交接：告诉管理者结果地址、限制与确认依据，由其 complete；原 Chat 可用现有 report 补充，不制造假报告。下游只获得资格，管理者必须再显式决定派发。

manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 必须包含上述动作矩阵、人工确认责任、禁止自动行为与错误/部分生效语义，node-chat 必须明确 report 不改变状态、失败后补充材料仍使用已有 Chat。**本仓库没有外部 toolkit 角色工具源码**，当前 HTTP 接线与说明不代表实际角色工具或 Prompt 已交付。metadata 指向的 `manager-chat.ts` 入口未部署时，真实工具/Herdr/omp 演练仍受阻；不能让用户手填 sessionId 或用受控原生身份宣称已验收。

### 可重复验证与证据边界

```text
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch recovery:: -- --nocapture
node scripts/test-backend.mjs
```

`backend/tests/workflow_dispatch/recovery.rs` 通过真实 HTTP TestServer、独占磁盘及外部记录型 Herdr 副本检验：

- `execution_fail_preserves_original_chat_first_reason_and_history_without_host_actions`：P20.04、P38.02，首次原因/原身份/查询/磁盘一致、重复 fail 不生效。
- `dispatch_reconciliation_keeps_uncertain_failed_then_confirmed_absence_pending` 和 `created_dispatch_reconciliation_observes_original_host_identity_without_start_or_input`：P39.01 的不确定/未创建/已创建三分支，原失败历史可读，不伪造提交效果。
- `confirmed_recover_restores_original_execution_while_paused_without_relaunch`：P40.01，人工确认前提、paused 可恢复原跟踪且身份不变。
- `manual_failed_execution_handoff_unlocks_dependency_without_fabricating_or_forwarding_reports`：P42.01，执行失败不可借核实重写，显式交接才解除依赖，无假报告或自动传材料，原 Chat 可补充地址/限制。
- `terminal_run_rejects_failure_reconciliation_recovery_and_manual_handoff`：P35.03，通过真实动作到达终态后拒绝上述全部状态变更，首次终态事实与外部调用不变。
- `reconciliation_rejects_unverified_or_conflicting_chat_facts_without_mutation`、`failure_actions_reject_wrong_role_unknown_fields_and_invalid_transitions` 和 `failure_actions_do_not_publish_uncommitted_state_or_success_history`：真实身份观察冲突、权限/参数/状态准入及真实只读文件错误反馈。

P68.06 由上述失败、三种核实、recover、人工 complete 的真实 adapter 全部调用记录支持：正常/拒绝/错误处理及之后 **150ms 有界观察窗口**无新增启动、终止、输入或自动重试；created 仅允许明确的只读 `agent_get`，测试中的显式原 Chat report 则只允许通知管理者。该证据不证明无限期无动作，也不证明真实 omp、模型决策或角色 Prompt 注入。

真实 S3 复验必须先部署 metadata 指定的角色入口并隔离 Herdr、zone、数据与原生 Chat，再由实际管理者工具分别处理三种核实、paused recover、人工 complete，保存原身份、输入/启动/终止记录、三份运行记录、明确起止的无新增动作观察窗口和用户意图/人工干预。缺入口或不可隔离宿主时报告未验证；HTTP 与受控 adapter 检查不能替代它。

## 暂停新执行与恢复派发资格

管理者的 `pause` / `resume` 都没有模型业务参数。传输正文仅由公共 Session 能力提供真实身份，后端仍按唯一管理者关联与同机来源检查；node、未关联身份及额外的 Run/node/角色字段不能借此修改状态：

```http
POST /api/workflow/manager/pause
Content-Type: application/json

{"sessionId":"<公共 Session 能力取得的当前管理者身份>"}
```

`resume` 使用相同正文，路径为 `POST /api/workflow/manager/resume`。不要把普通 Chat 的 **Resume**（定位或恢复原生对话）当作此状态动作；普通 Resume 不接管 Run，也不恢复派发资格。

- `pause` 将 running Run 转为 paused，排在同 Run 已接受的完整派发批次之后生效。它不关闭或终止任何 Chat、不等待它们停轮或退出。生效后 `ready=[]`，新 dispatch 的逐项结果为 `rejected`，原因含 `Run 状态为 paused`，不产生宿主启动调用。
- 已有执行仍可 report，管理者仍可在确认可交接后 complete。报告仍只通知管理者，不自动完成。paused Run 全部节点 completed 后可以直接 finish，无需先 resume，活动 Chat 不阻止成功。
- `resume` 只将 paused Run 转回 running，重新给出由当前事实与依赖派生的 `ready` / `blocked`。它不自动 dispatch、retry、补发输入或终止已有 Chat；管理者必须另外显式选择目标并 dispatch。
- 已在目标状态时重复 pause/resume 返回 `changed=false` 与当前查询字段，不重写当前记录、不覆盖首次操作 ID/时间/actor、不重复追加成功历史，也不产生外部调用。一次真正的反向转换后再次调用是新的显式状态动作，不是重复请求。
- completed 终态的 pause/resume 返回 HTTP 409、`code=run-terminal`、`changed=false` 和当前查询字段，不改变首次 finish 事实。重复 finish 仍返回首次结果。

成功动作返回与 complete/finish 相同的 `snapshot`、`ready`、`blocked`、`canFinish`、`reports`、`pendingReports`、`changed` 与 `historyError`。当前状态在 `run.json`，首次生效转换在 `history.jsonl` 中保留 `id`、`at`、真实管理者 `actor`、`action=pause/resume`、`before`、`after`。所有检查、写入和返回快照沿用同 Run 动作串行边界；文件锁不跨宿主调用。当前事实保存失败返回 HTTP 500、`pause-failed` / `resume-failed`，不采用未提交状态、不追加成功历史；历史追加失败则如实返回已生效状态和非空 `historyError`，不回滚、不自动修复或重试。

### 完整批次与并发调用

同 Run 的业务串行边界覆盖**整个显式派发数组**（包括逐项拒绝、启动失败、其余目标的实际启动及最终结果/快照），不是只覆盖某个 node 或一次文件写入。在途批次 `[A,B]` 尚未返回时提交 pause，pause 不能在 A 与 B 之间生效，也不能撤回 B 或终止 A；它在完整批次之后检查最新事实并保存 paused。批次响应仍描述该批次完成时的 running Run，后续 pause 响应描述 paused Run，两个响应都保留已启动 A/B 的事实，不互相覆盖。等待中的后续 dispatch 同样重新检查最新状态与依赖；先前 status 的 ready 或等待前的 pending 不是启动授权。

文件协调与业务排队不同：宿主启动/输入受阻时，同 Run 的只读 status 仍可读取**已提交**记录（例如初始输入尚未确认，节点仍 pending，但真实 Chat 关联已保存），不保证只读查询会等到整批完成。不要把这个中间快照当作可以绕过排队的理由。独立 Run 的动作不排在本 Run 的批次或 pause 后面；已启动 Chat 的活动上报也不受这条业务队列控制。节点 report/管理者 complete 等需要修改本 Run 业务事实的动作仍在同 Run 边界排队，不能把“Chat 继续工作”理解为业务写入可以穿插进批次。

遇到慢派发时，用户应保留现有 Chat 与现场，等待批次结果和 pause 的明确回执，不关闭 Tab、编辑记录或根据旧 ready 重发。manager-chat 应说明这些顺序及中间查询的含义；node-chat 应继续自己的工作/报告职责，不把管理者提交 pause 当作撤回已有任务。宿主报错、当前事实保存失败与历史部分失败继续按原有明确反馈处理，不通过自动重试或假快照隐藏问题。

可重复确定性检查：`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch serialized_ -- --nocapture`。

- `serialized_dispatch_rechecks_pending_snapshot_after_in_flight_launch`（P17.03）在 A 的外部初始输入门闩处先观察 pending/ready，再提交同目标 dispatch；释放后必须按 running 拒绝，结果/快照一致且无第二次宿主启动。
- `serialized_batch_finishes_before_pause_without_blocking_other_run_or_chat`（P21.01、P36.02，并覆盖 P17.03 的 pause 重检）分别在 A、B 初始输入处设门闩，观察同 Run pause 无响应、同 Run 文件查询成功、另一 Run 的 B 真正启动以及既有 Chat 事件经 SSE 可见；最终历史必须依次为 dispatch A、dispatch B、pause，批次与 pause 的快照分别保留对应边界事实。

门闩释放和外部调用记录确定先后，不以任意 sleep 推定排队次序。每道门闩的负向 HTTP 观察窗口为 **200 ms**，正向调用有独立有界期限；输出保留中间查询、动作快照、历史与外部调用列表。测试保留真实 TCP、文件写入/读取和生产 Herdr 调用包装层，仅外部 Herdr 可执行文件使用记录型副本；状态上报是受控输入，不是原生 omp 执行或真实角色工具/Prompt 已验证的证据。实际宿主复验仍需已部署的角色扩展与隔离 Herdr/zone，不能用本检查替代。

### 用户与管理者操作指引

人类需要处理现场时，先从 workflow 查看 Run 与节点事实并进入已关联的管理者 Chat，表达「暂停新派发」；由管理者调用 pause，而不是退出所有节点 Chat、关闭 Tab 或直接编辑运行文件。已有 node 可以继续提交含结果地址与限制的 report，管理者可以确认 complete；暂停不表示工作已经完成。处理后表达「恢复派发资格」，由管理者 resume、重新读取状态并决定下一次显式 dispatch。不要将 resume 工具成功当作下游已启动。若全部工作已交接，直接 finish，不必为结束而恢复派发。

manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 必须覆盖上述边界及重复/终态/部分生效反馈；node-chat 应明确 pause 不撤回其已有工作或报告职责。这是角色指引的交付要求，不是本仓库已交付外部工具或已验证真实 Prompt 注入的声明。当前 worktree 只交付 HTTP 能力和本文；metadata 指向的外部角色入口未部署时，真实角色工具与 Herdr/omp 链路仍未验证，不能让用户手填 sessionId 代替。

### 暂停与恢复的验证入口及证据边界

可重复检查为 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture`，完整后端检查为 `node scripts/test-backend.mjs`。`pause_blocks_new_dispatch_but_preserves_active_chat_handoff_and_first_fact` 覆盖 P18.03、P20.03 的 pause 分区、P36.01 和 P68.04 的 pause 分区；`resume_restores_only_eligibility_until_explicit_dispatch_and_preserves_first_fact` 覆盖 P20.03 的 resume 分区、P37.01 和 P68.04 的 resume 分区；`paused_run_finishes_with_active_chat_and_terminal_eligibility_actions_are_rejected` 覆盖 P35.02。测试复用真实 HTTP TestServer、独占临时记录及外部记录型 Herdr 副本，先实际派发/暂停，再尝试 dispatch、report、complete，并实际 resume 后观察，最后显式派发；另一路径在 paused 下直接结束。输出保留动作/查询、历史和真实外部调用列表。每个无自主行为观察窗口为 **150 ms**，不把有限窗口宣称为无限期保证，也不把受控 working 事件或宿主副本当作原生 Chat 活动证据。

真实角色复验须先实际部署 manager-chat/node-chat 并隔离 Herdr、数据及验收 zone。由真实 manager 工具 pause，保留状态、拒绝派发结果、实际外部启动/终止记录；已有 node Chat 继续 report 与 complete；由真实工具 resume，记录明确起止的观察窗口内无新增启动，之后显式 dispatch 才新增执行。再在 paused 且 Chat 仍活动的现场直接 finish，核对终态拒绝和首次历史。不具备真实角色入口/隔离宿主时如实保留未验项，HTTP 结果不替代真实工具、Prompt、原生身份或模型执行证据。

### 完成与结束的验证入口及边界

可重复确定性入口为 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture`，完整后端交付检查为 `node scripts/test-backend.mjs`。`explicit_complete_unlocks_only_satisfied_dependencies_and_preserves_first_fact` 核对独立 A/B → C 字面快照、只完成 A 的拒绝和首次说明/历史；`finish_requires_every_handoff_and_terminal_actions_preserve_success` 核对显式派发 C、结束条件、终态拒绝、查询和重复 finish；`reports_and_observed_chat_lifecycle_never_replace_explicit_handoff` 分别经 HTTP 提交 report、停轮状态事件、退出登记事件，再显式完成并验证迟到报告不重新打开工作。各负向观察窗口为 **150 ms**，测试输出保留实际外部调用列表、事件和动作/查询结果；有限窗口不证明无限期没有自动行为。会话事件是受控输入，外部 Herdr 副本不是原生 omp，不能宣称证明了真实 Chat 停轮/退出。

真实宿主复验需要 metadata 指定的 manager-chat/node-chat 实际交付：在独占数据、zone 与 Herdr 实例中，通过公共角色工具执行前置 report 后保持 running（让该 Chat 继续收尾），确认 C 拒绝；管理者 complete A/B 后显式 dispatch C，全部 complete 后 finish，保留原生工具结果、Prompt 注入、Chat 身份/活动顺序、三份运行记录和默认未启用自动关闭时无强制结束/自主派发的有界外部调用证据。角色入口缺失时，本仓库确定性检查可执行，但原生工具/Prompt/真实 Chat 仍未验证，不能用受控 HTTP 结果宣称完整宿主验收。

## 单节点真实宿主闭环验收

多节点后续入口见 [真实 A／B → C 验收与通知取证](workflow-fan-in-acceptance.md)：固定双材料/第三份产物预期、预先写入 C 的材料地址、三个真实通知现场与报告后收尾的顺序证据。该指南不是已通过声明，单节点或角色部署前置条件缺失时不得用多节点准备结果代替。

本节是进入多节点验收前的独立入口，只验证一个无前置 node 的 manager → dispatch → 实际文件/report → complete → finish。即使本节全部通过，也不代表 P62/P63 的 A/B → C、并行、汇合或多轮推进已经通过。HTTP TestServer、记录型 Herdr 副本和手填身份请求都不能代替本节。

### 独立预期与观察 seam

开始前先保存本轮预期，不能读取实际产物后再改预期。固定 node ID 为 `ONE`，`blocked_by=[]`，输出为 node zone 根目录的 `single-node-result.txt`。文件必须是无 BOM 的 UTF-8，行尾仅 LF，末尾有一个 LF，完整内容固定为：

```text
workflow-single-node
answer=42
```

这个字面预期独立于模型与运行实现；不要用模型报告、服务返回值或被测实现生成期望值。以下 PowerShell 在本轮实际 node zone 已保存为 `$nodeZone` 后核对原始字节；缺文件会报错，不生成替代产物，错误内容、BOM 或 CRLF 都不能通过。保存本段原文作为启动前预期，并在 manager 核对时保留实际执行结果；本次环境阻塞，尚未执行此产物检查。

```powershell
$expected = [Text.Encoding]::UTF8.GetBytes("workflow-single-node`nanswer=42`n")
$artifact = Join-Path $nodeZone 'single-node-result.txt'
$actual = [IO.File]::ReadAllBytes($artifact)
if ([Convert]::ToBase64String($actual) -cne [Convert]::ToBase64String($expected)) {
    throw "实际产物字节与独立预期不符：$artifact"
}
Write-Output "产物字节符合独立预期：$artifact"
```

node.input 应明确要求写上述文件、读取文件核对内容，然后通过真实 node 角色工具 report 绝对路径、编码和限制。管理者本次要求为：只派发 ONE；使用 status 与记录读取观察进展；收到 report 后亲自读取实际文件并与事先保存的预期逐字节比较；只有吻合才 complete ONE（非空 reason 记录文件地址与核对结论），再显式 finish。后端 collect 已提供，但真实角色工具尚未部署时不得伪造工具调用或以循环代替其承诺。

观察 seam 是实际交付页面的发起 Run 入口、真实 manager/node 角色工具及其公共 Session 身份、node 实际文件、三份运行记录与 Herdr 原生 Chat。可区分的具体违规是“node 报告成功但未写文件或内容错误”：仅凭 report 或 completed 快照会漏过；独立读取实际文件并比较完整字节才能拒绝它。此真实场景未可执行时，不制造受控模型的 red/green 来声称覆盖。

### 隔离与执行顺序

1. 按[测试指引](agents/testing.md)构建交付页面和安全关机壳。参考[静态样例的隔离配置](example/README.md#在隔离目录运行)，但不要对其用户配置或静态多节点样例直接发起 Chat。为本轮新建独占 service cwd、数据根、manager zone、node zone 和证据目录；配置数值回环动态端口、`mdns.enabled=false`、显式 `scheduler.run_directory` 和稳定的本说明绝对路径。三份记录留在 node zone 外。所有存储路径都归本轮所有，不能只把配置文件放到 Temp 却沿用用户数据。
2. 先核实当前 metadata 指定的 manager-chat/node-chat 是真实部署入口，能够加载公共 Session、角色工具及 `first-enabled-run` Prompt；同时核实 omp 未启用 autoResume。确认可以通过本轮独占的 Herdr 实例/命名 session 隔离 Workspace、Tab 和 agent。命名 session 或配置隔离未建立前不要调用用户的默认 Herdr 实例；只存在 Herdr 可执行文件不足以证明隔离可用。任一前提缺失就保存真实错误并停止，不修改 metadata 指向空扩展、不手写运行事实。
3. 在隔离服务的实际页面登记两个 zone，创建 manager 和普通（normal）类型的 omp Task，固定本轮 model/thinking、初始 Skill、prompt 和扩展选择并保存其预期。创建一个引用普通 Task 的 Bot 和仅含 ONE 的 Map，node.zone 为独占 node zone，输入采用上述字面任务。启动前保存这些实际定义及本次要求；运行期间不再修改。
4. 经 scheduler 页面显式发起 Run，保存页面结果及 Run ID。由真实 manager 使用角色工具查 status，确认 ONE ready，显式 dispatch ONE；不能由验收脚本手填 sessionId 冒充管理者。记录 manager 与 node 不同的原生 session_id、实际 cwd、Workspace/Tab/pane/agent 关联、公共 Prompt 实际注入与工具原始参数/结果。应由角色扩展取得身份，不允许模型提供归属。
5. node 在自身 cwd 写文件并调用真实 report。管理者用 status 与传入记录路径读取已保存报告，亲自读取该绝对文件，核对以上独立字节预期；随后显式 complete ONE，再 finish。若 manager 停轮，可人工要求继续核对/完成，但每次补充输入、时间、对象原生身份和原因都必须保留，不声称强制续轮。通知失败与保存失败分开解释，不盲目重报或重派。
6. 保存实际文件的原始字节、全部原生工具结果、两份原生 Chat 引用/可供复查的对话、最终 `run.json`、`history.jsonl`、`messages.jsonl` 和完整人工干预记录。用 status/记录确认 ONE completed、非空首次 completion reason、Run completed、首次 finished 事实，以及对应 report/complete/finish 历史。缺失的记录应标为缺失，不能生成空记录充数。任何保存/历史/通知部分失败均单列，不凭最终状态抹去。
7. 持续消费所属服务/agent 的 stdout/stderr；正常或失败退出都只清理本轮拥有的进程与资源。保留证据后再删除临时数据，不关闭用户已有 Workspace/Chat，不调用真实关机。保存准确的启动命令、配置、版本和清理结果，以便从新临时目录重复执行。

### 2026-10-28 实际前置检查：阻塞，未运行闭环

在当轮独占源码副本检查部署，实际结果为 `herdr 0.9.5`、`omp/18.13.2`，两者可执行文件均可找到。metadata 的两个固定角色入口分别为：

```text
E:\Developer\myhub\my-agent-toolkit\entrypoints\omp-auto\manager-chat.ts
E:\Developer\myhub\my-agent-toolkit\entrypoints\omp-auto\node-chat.ts
```

实际目录读取仅见 `codebase-tools.ts` 与 `connect-bot.ts`；对以上两个绝对路径执行 `Test-Path -PathType Leaf` 均为 `false`。对 toolkit 文件名另行查找也未找到这两个角色入口。可重复前置检查（PowerShell；这是部署检查，不是闭环通过证明）：

```powershell
$paths = @(
    'E:\Developer\myhub\my-agent-toolkit\entrypoints\omp-auto\manager-chat.ts',
    'E:\Developer\myhub\my-agent-toolkit\entrypoints\omp-auto\node-chat.ts'
)
$checks = @($paths | ForEach-Object {
    [pscustomobject]@{ path = $_; isFile = (Test-Path -LiteralPath $_ -PathType Leaf) }
})
$checks | ConvertTo-Json
if (@($checks | Where-Object { -not $_.isFile }).Count -gt 0) { exit 2 }
```

上述带退出码的前置检查已实际执行，退出码 **2**，原始结果为：

```json
[
    {
        "path": "E:\\Developer\\myhub\\my-agent-toolkit\\entrypoints\\omp-auto\\manager-chat.ts",
        "isFile": false
    },
    {
        "path": "E:\\Developer\\myhub\\my-agent-toolkit\\entrypoints\\omp-auto\\node-chat.ts",
        "isFile": false
    }
]
```

本次检查在创建服务、Run 或 Chat 之前停止，未接触用户对话/产物，未创建持续运行的验收子进程；版本/部署探针均已退出，无遗留清理对象。没有可交付的原生身份、工具调用、实际产物或三份运行记录；它们是**未产生**，不是验收通过。检查期间第一次 PowerShell 调用因外层 shell 展开 `$` 而语法失败，改用单引号包裹 PowerShell 程序后才得到两个 `false`；没有通过修改运行事实或模型输入干预结果。除此之外没有运行内人工干预，因为本轮尚未启动。

当前精确阻塞是独立 my-agent-toolkit 尚未部署上述真实角色入口，不能执行要求的原生身份/Prompt/工具路径。Herdr 隔离启动、模型访问与完整闭环也仍未验证，不能从可执行文件版本推断可用；没有证据据此修复宿主运行 bug。部署真实入口后须从新的独占环境按本节全程复验并另存实际证据；本次前置检查不能解除单节点宿主验收，也不能将 P62/P63 标为完成。

## 节点报告与独立通知

Map node 的可选 `report_policy` 只接受 `auto`／`manual`，省略默认 `auto`；派发时保存为本次 `execution.reportPolicy`，不是 Run 创建时的整图快照。auto 在可交接时直接报告；manual 每次准备报告都先在 node Chat 展示摘要、结果地址及限制，询问用户、停止当前轮次并等待此 Chat 的自然语言确认，然后才调用 report。用户要求修改时先修改，再展示、再确认；补充和更正也必须确认。业务 input 不是策略的唯一来源。这个约束只由提示词落实，不新增后端审批门禁、确认字段、状态或按钮，不改变 manager complete 的判断职责。

恢复原 Chat、recover 和派发核实沿用已保存执行的策略，不从当前 Map 改写，也不重放策略控制段；retry 是新执行，重新读取当前 Map，旧执行仍保留原值。已有 Chat 不热更新；旧执行缺字段按 auto 解释，无需迁移。若 retry 连当前 Map/node 都无法解析，失败来源记录没有可执行策略，默认 auto 不代表启动或允许回退；修正后再次 retry 才重新选择有效目标与策略。

已关联的当前 node Chat 使用 `report`，模型业务参数**只有正文**（`body`）。正文应包含可交接结果的地址、使用方法与限制；不要求 node 自报 Run、node、执行 ID 或接收者。角色扩展须从 toolkit 公共 Session 能力取得当前原生身份，并调用：

```http
POST /api/workflow/node/report
Content-Type: application/json

{"sessionId":"<公共 Session 能力取得的当前真实身份>","body":"结果地址与使用限制"}
```

后端按当前后端已管理 Run 的真实 node 执行关联归属，拒绝无关联、管理者身份、重复关联及调用方填写的 `runId`、`nodeId`、`target` 等额外字段。系统生成报告 `id`、`at`、`type=report`，保存 `runId`、`nodeId`、`executionId`、真实来源 `source` 和同 Run 管理者 `target`。sessionId 不是模型参数，也不是秘密凭据；同机信任限制与 manager 工具相同。重启后的旧 Run 不接受 report。

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

node-chat 的实际工具与公共 Prompt 属于独立 my-agent-toolkit 交付。其 `PromptDeclaration` 应使用 `first-enabled-run` 注入本节报告职责：何时交接、正文写地址和限制、仅提供正文、报告不等于完成、保存与通知分离、仅通知失败不要重复 report；不以工具 description 代替注入。部署仍须将固定 node 角色入口接到公共 Session/HTTP/Prompt 与自动启用策略，配置稳定 endpoint 与预算，不依赖 node cwd 或 connect-bot 开关。本仓库的 HTTP 协议、静态入口配置或说明文字都不构成外部扩展已装配、原生工具实际调用或 Prompt 已注入的证据。

### 报告验证入口与证据边界

确定性入口：`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture`。报告用例覆盖系统归属、真实文件追加、只读文件保存失败、受控外部投递失败/延迟、连续读取保留待办和真实关闭 stderr 管道的诊断写入失败。投递/诊断故障后均观察 **300 ms**，输出该窗口的外部进程调用记录，证明有限窗口无补投或自动重报，不声称无限期性质。受控 Herdr 副本只证明 adapter 的确定性处理，不证明真实模型、原生会话或通知质量。

真实复验必须使用隔离 Herdr/omp 与已部署 node-chat：记录公共 Prompt 实际注入内容和原生 report 调用（业务参数仅 body），同时核对源/目标真实身份、当前执行、追加消息、状态不变和 manager 实际输入。制造保存失败、投递失败/延迟和诊断失败；分别保留输入提交结果与无自主重投窗口。若外部角色入口缺失，本仓库部分可通过，但 P24.01 的真实工具、P59.02 的公共 Prompt 及真实宿主通知证据仍未验证，不能用手填 HTTP 身份或受控 adapter 宣称这些项完整通过。

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

暂停/结束优先返回自身原因；这些成功快照也检查此前已有通知。通知 generation 与检查位置仅在内存，无人等待时仍保留；报告必须先保存才产生通知，不依赖 Herdr 正文投递完成。一次检查只覆盖同一操作入口内的已保存现场，检查后的新通知留给后续 collect。状态变更可以唤醒等待以重新检查，但不能冒充报告通知。不额外规定“有 ready node 就必须立即返回”：仍有 running node、无新通知时可以等待。

后端通过既有服务配置 `[scheduler].collect_wait_seconds` 设定等待上限，默认 **5400 秒（90 分钟）**，正整数、最大 4294967295。更改后重启生效。等待不占用文件锁或 Run 状态操作入口；pause、complete 与 report 不需等 collect 到期。不存在 collect 游标文件、持久等待者、后台补投或业务轮询循环。

调用方公共 HTTP 的**整个请求预算必须大于后端期限**，默认建议 `timeoutMs=5460000`（91 分钟）；短配置如后端 1 秒，调用方可用 3 秒。不要复用 connect-bot 的短上报预算。HTTP 200 响应头可能先于等待结果到达，只有完整、合法 JSON 正文才是成功快照。等待阶段发生读取故障时后端终止未完成的响应体，不生成正常 timeout 或空快照；初始检查失败返回明确错误。调用方遇到取消、断线、预算耗尽或读取失败必须反馈错误，**不自主重试、不伪造正常结果、不据此改变运行状态**。需要再检查时由管理者明确发起新的 collect。

### 角色调用方及公共能力的部署边界

后端等待由 HTTP 响应体持有，不派生脱离请求的任务；真实连接断开会释放等待和通知接收者，随后新请求仍能收取新的通知。工具中断与 Chat 结束必须经 toolkit 公共 HTTP 的调用方取消能力断开正在进行的请求；取消不保证取得任何快照。manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 应注入上述原因、交接核实责任、预算与取消/网络错误处理，不以工具 description 代替实际注入。

本仓库没有 manager-chat/node-chat 的工具源码，也没有共享 toolkit 的修改工作区；不能复制 connect-bot 私有客户端或在本仓库造第二套客户端。实际部署目录 `my-agent-toolkit/entrypoints/omp-auto` 仅有 codebase-tools/connect-bot，缺少 manager-chat/node-chat；其公共 `framework/interface/general/http-contract.ts` 的 `HttpRequestOptions` 当前只有 method/headers/body/timeoutMs/proxy，没有调用方取消信号，失败类别只有 configuration/timeout/transport。外仓必须先补齐公共取消能力及角色接线、生命周期取消与较长预算，才能验证真实工具中断/Chat 结束及实际预算。上述部署要求不是这些缺失能力已实现的声明。

### 可重复的 collect 验证与证据范围

确定性入口：`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture`；完整后端交付检查用 `node scripts/test-backend.mjs`。用例通过独占数据、动态数值回环 TestServer 与隔离的 Herdr 记录型副本观察：

- `report_save_failure_has_no_message_pending_item_or_host_delivery`：真实只读文件保存失败后 collect 等待完整 **1 秒**并返回 timeout/空待办，无幽灵通知和投递调用（P25.02）。
- `collect_retains_merges_notifications_before_failed_delivery_and_keeps_pending_work`：无人等待时两份报告的通知保留/合并，外部投递被 gate 阻塞时先收取；连续 paused 检查仍保留待办，检查后新报告唤醒下次等待；投递失败后 **150 ms** 无自主补投（P26.02/P28.03/P31.01）。
- `collect_without_active_execution_returns_reason_and_body_free_snapshot` 及 `collect_wait_releases_on_pause_and_last_handoff_and_completed_is_immediate`：分别核对 no-running、等待中的 pause/最后一项交接唤醒、completed；仍有 running 时 ready 不强制立即返回（P30.01）。
- `collect_configured_expiry_and_tcp_disconnect_leave_facts_and_next_notification_intact`：核对默认 90 分钟/有效短配置、1 秒正常到期、实际读取响应头后连续三次关闭 TCP、**150 ms** 无状态/外部调用变化，后续新请求仍收到通知（P32.01、P33.01/P67.02/P68.05 的后端分区）。其泄漏敏感性通过临时将等待移入独立后台任务验证：旧请求错误消费新通知时，后续请求返回 timeout 而不是 notification，测试失败；正确实现恢复后复验。
- `collect_late_read_failure_breaks_transport_without_fake_snapshot_or_state_changes`：响应头之后制造真实文件损坏，观察未完成传输没有任何伪造 JSON 正文，恢复文件后新请求正常；**150 ms** 窗口内无额外 host 调用/状态修改。

这些检查只证明真实后端 HTTP/文件/受控宿主故障行为，有限窗口不代表无限期保证。**P33.01/P60.01 的公共 toolkit 调用方取消及真实角色生命周期、P32.01/P67.02 的实际调用方长预算、P68.05 的角色工具无重试/错误反馈仍未验证**；不能把 TCP 断开或测试客户端超时冒充公共 toolkit 取消。真实验收须先在独立外仓交付公共取消和 manager-chat，再在独占 Herdr/omp 中用实际角色工具验证中断、Chat 结束、网络失败与较长预算，保留原始工具错误、窗口与外部 HTTP 调用证据。未具备这些前提时不宣称整条 promise 通过。

## 启动失败

初始化失败不启动 Chat。原生身份或关联保存失败不提前提交指示。已启动的 Tab/agent 不会被假报回滚；初始指示提交失败也不代表外部端绝无接收效果。详情列出可得的 Workspace、Tab、pane、agent 及原生身份，说明哪些步骤已确认完成。先核实这些引用，再处理现场；不要盲目再次发起，异常窗口不承诺完整幂等。

管理者启动错误只保存在当前后端内存，刷新仍能查询。诊断输出包含 Run、失败阶段及已发生效果，不重复本次要求正文。错误不写入业务历史；后端重启后不重建页面错误，只保留持久事实与既有诊断。没有自动重试、自动关闭 Chat 或自动派发节点。
