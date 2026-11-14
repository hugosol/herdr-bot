# herdr-bot

Windows 上的 agent 对话主界面，本机浏览器与局域网设备访问同一个服务。

## 主要功能

- **chat**：查看当前与历史会话，恢复原生对话，以及监听会话完成后关机。
- **task**：管理可复用的 Task 配置，用于发起 Chat；Task 本身不是实际对话。场景资源只选择一个 Suite 或不选，不再编辑 Task 可选扩展。
- **suite**：维护场景工具集与按宿主声明的资源。探索／实现及 Pi 网页访问定义、旧部署的显式离线迁移见[使用说明](docs/usage.md#当前交付定义与旧格式更新)；发布不自动操作用户生产 Task。
- **zone**：管理服务主机上的目录，浏览文件、审查 Git 变更，并选择 Task 发起 Chat。
- **bot**：独立访问共享 Bot 能力定义，复用 scheduler 的原有管理功能。
- **scheduler**：浏览和编辑 Map、node 与共享 Bot，校验配置并发起独立管理者 Run（需部署角色扩展）。node 可配置 `report_policy: auto/manual`；Map 与 Run 图用圆角／切角节点区分策略，详情显示具体值。
- **workflow**：按日期倒序浏览和筛选全部 Run；全宽详情中用固定节点列表、横向状态图与报告查看现场，图中显示 Task 名称及独立运行标记，Resume 已关联 Chat；返回保留列表上下文，可见时轮询，查询失败保留上次内容，刷新不重新启动。需求、配置与限制见 [Map Run 长期契约](docs/workflow-contract.md)与[运行说明](docs/workflow-runtime.md)。
- **server**：调用预先配置的启动／停止脚本，以及手动关机。

## 构建与运行

需要 Rust 1.98+、Node.js 24+（含 npm）及 Bun 1.4.4；发起 Chat 还需要已运行的 herdr 及已安装的目标 agent。

前端依赖由 Bun 安装，`frontend/bun.lock` 是唯一依赖锁文件；构建和测试仍通过 npm 脚本运行，保留 Node.js 运行时及 `pretest` 网络护栏。`frontend/bunfig.toml` 启用 isolated linker 和 Global Virtual Store，新建 worktree 自动继承配置，但 Git 不会自动安装依赖：在各 worktree 根执行下面的冻结安装命令即可链接共享包，不需要复制 `node_modules`。

共享 store 默认位于 Bun 缓存的 `links/`；需要自定义位置时，在安装进程中设置 `BUN_INSTALL_CACHE_DIR`，多个 worktree 使用同一路径。不要直接改写共享包文件，也不要随意执行 `bun pm cache rm`：它会使所有引用该 store 的安装失效，直到各自重新安装。Vite/Vitest 缓存、`backend/static` 和 `CARGO_TARGET_DIR` 仍按 worktree 独立；只共享依赖，不共享构建产物。依赖变更在 `frontend` 中使用 Bun 更新，并提交 `package.json` 与 `bun.lock`，不要重新生成 npm 锁文件。

```bat
bun install --cwd=frontend --frozen-lockfile
npm --prefix frontend run build
cargo build --release --manifest-path backend\Cargo.toml
backend\target\release\herdr-bot.exe --config herdr-bot.toml
```

首次运行前，将 [`paths.example.toml`](paths.example.toml) 复制为仓库根的 `paths.local.toml`，按新机器调整根目录；已有本机配置时不要覆盖。样例保留旧机器已知路径，不代表新机器上存在这些目录。实际配置已被 Git 忽略，程序不会自动回退到样例。另检查 [`herdr-bot.toml`](herdr-bot.toml) 的监听地址、Herdr Workspace，以及 [`meta-data/task-options.toml`](meta-data/task-options.toml) 的模型与扩展选择。请从控制台启动，以便脚本继承可用的控制台。

### 本机根目录与解析规则

`herdr-bot.toml` 顶层 `paths_file = "paths.local.toml"` 指定本机根文件；该位置相对服务配置所在目录。根文件的 `[roots]` 可以使用独立绝对目录，也可以引用其他根，例如 `data = '${roots.project}/.data'`。外部项目不必在同一磁盘或父目录，改动一个根即可改变所有引用位置。

- `${roots.name}` 只允许出现在路径开头，后接可选的 `/相对位置`，不是通用字符串模板。不展开环境变量、Task 提示词、Bot 指南或 Map 内容。
- 普通相对路径以**声明它的配置文件所在目录**为基准：根相对根文件，服务路径相对服务 TOML，扩展 `entrypoint` 相对选项 TOML。切换启动 cwd 不改变这些位置。
- 根文件在启动时解析为快照。修改根或服务路径后重启；选项 TOML 仍按原规则动态读取，但使用本次启动的根快照。交给 agent 的扩展路径已经绝对化，不随 zone cwd 改变。
- 未定义根、循环引用、空路径、非法表达式或指定的根文件缺失会明确失败。只使用普通路径、不声明 `paths_file` 的隔离配置无需本机根文件。
- `herdr` 等裸程序名继续通过 `PATH` 查找；显式程序路径相对配置文件解析。Git、Windows 系统工具和开发语言服务的安装与 `PATH` 设置仍由机器环境负责。Suite 集中 MCP 的 `command` 使用 `${roots.codebase_memory_mcp}/codebase-memory-mcp.exe`，安装目录在 `paths.local.toml` 的 `[roots].codebase_memory_mcp` 配置，交给 OMP／Pi 前解析为完整路径，不依赖 PATH 定位该 exe。独立的 `.omp/mcp.json` 仍使用裸命令并优先于同名 Suite 定义；它不经过 herdr-bot 的 roots 解析，需单独维护。
- 顶层 `static_directory` 指定页面产物，默认 `backend/static`；`scheduler.run_instructions` 默认 `docs/workflow-runtime.md`，均相对服务配置目录，不再使用编译时源码目录。交付配置通过 `roots.project` 显式定位两者。
- 顶层 `data_directory` 指定内部迁移标记目录，默认 `.data`。它不隐式覆盖其他持久化字段；交付配置将历史、zone 登记、Run 根及标记目录共同引用 `roots.data`。

可只读查看根解析结果：`cargo run --quiet --manifest-path backend/Cargo.toml --bin herdr-paths -- --paths paths.local.toml`。输出为 JSON，不启动服务、不创建业务目录。开发工具中真实依赖路径的覆盖方式见[测试指引](docs/agents/testing.md)。

程序路径根、zone 本机绑定与提示词词典是三套不同职责：前者解析程序配置路径，zone ID 决定执行和直接文件输入的位置，词典替换指定配置提示词中的本机参考信息。Run／Chat 历史仍为本机事实，不保证跨 server 使用。

本机访问 `http://127.0.0.1:<端口>/`；局域网使用启动日志中的
`http://herdr-bot.local:<端口>/`，无法解析时使用服务主机的局域网 IPv4。
手机上的 `127.0.0.1` 指向手机自身。

## 本机提示词词典

服务 TOML 顶层 `prompts_file = 'prompts.local.toml'` 指定独立词典，缺省为同名文件，位置相对服务配置目录。与 `paths_file` 不同，它用于提示词正文，不解析程序变量或决定执行目录。复制 `prompts.example.toml` 为本机文件并按当前 server 调整值，不覆盖已有本机配置：

```toml
herdr-bot-path = 'E:/Developer/myhub/herdr-bot'
herdr-bot-api-url = 'http://127.0.0.1:18787'
```

- Task 固定 prompt、Bot 的 `when`／`plan_guide`／`node_guide`、Map `description` 与 node `input` 保存 `${herdr-bot-path}` 等原始标签。用户临时输入、Run 本次要求、报告及 agent 产物不参与替换。
- 系统在发送配置字段前通过统一词典 module 替换一次，再与其他消息拼接；每条初始消息读取一份最新词典，不要求重启，不递归解释值。`$${key}` 输出字面量 `${key}`。
- agent 直接读取 Map/Bot 原始文件时仍会看到标签。系统发起的 Chat（含流程派发）仅在替换前的 Task 固定 prompt 含未转义且完整闭合的 `${key}` 时获得词典绝对路径和读取说明，由 agent 自行映射。用户输入、其他消息段、替换值、转义写法 `$${key}` 与未闭合写法都不触发注入；不强制覆盖所有读取入口，不修改原始定义，不自动注入 Bot 指南全文。
- 保存定义允许本机词典尚不完整；实际发送时发现未知标签则报出字段与标签并停止该次发送。缺失词典文件视为空词典，使用标签仍会失败；非法 TOML 或不可读文件明确报错。不要猜测缺失值。
- 词典仅提供提示词参考信息，修改其中的 API URL 不会自动修改 toolkit 扩展的实际网络连接配置。不要在词典中放入不应向 agent 暴露的密钥。

## 定义数据与运行数据

Task、Bot、Map 是可复用的定义数据，可在页面中编辑，也可手工维护和纳入 Git；不是每次执行产生的运行事实。默认路径如下，均以**服务配置文件所在目录**为基准；交付配置通过命名根表达相同布局，可将各根放在独立位置。修改配置后须重启：

```toml
[tasks]
storage_file = 'meta-data/tasks.json'
metadata_file = 'meta-data/task-options.toml'

[scheduler]
map_directory = 'meta-data/workflow-maps'
bot_directory = 'meta-data/bots'
run_directory = '.data/workflow-runs'
```

省略字段时默认路径与上述相同。Run、Chat 历史、zone 登记仍属于 `.data`；Map 内每个 node 保存稳定 zone ID，直接文件输入保存相对路径与可选 zone ID，由执行 server 的本机绑定解析。定义可能包含本机路径、固定指示和业务内容，允许 Git 跟踪不代表适合公开提交，程序不会自动提交。

### 从旧定义布局升级

修改代码或配置本身不会搬动正在使用的数据。先停止旧服务并备份数据，将服务配置放在原数据布局的根目录，再启动新版本；不要让旧、新服务同时写入这组定义。旧源位置固定相对服务配置目录，不随 `roots.data` 移动。配置指向该目录下上述新默认位置时，启动会一次性将 `.data/tasks.json`、`.data/bots/`、`.data/workflow-maps/` 复制到对应新位置，保留所有原始字节、目录内容及旧源；不会迁移其他运行数据。显式配置的其他路径继续有效，不自动导入旧默认目录。

- 所有待迁项目先检查冲突：新旧同时存在时，完整内容必须一致；缺少或多出文件也算目录冲突，不自动合并。冲突、链接／junction、异常文件类型或 I/O 错误都会明确终止启动，不当作空数据继续。
- 成功切换分别记录在 `data_directory` 下的 `.definition-layout-v1-tasks`、`-bots`、`-maps`（默认目录为 `.data`）。此后只读写配置的新位置；旧源只是备份，新数据修改或删除后不会从旧源回灌。不要删除这些标记来尝试刷新定义。
- 若报冲突，保持停服，备份并人工确认唯一权威版本，使目标与旧源完全一致或将冲突目标移到其他备份位置，再重启；不要盲目覆盖任一份数据。
- 不提供跨文件事务。中途 I/O 失败可能留下已完整复制的项目；保留现场并排除错误后重启，完整一致的项目可确认切换，不覆盖现有内容。

升级后核对 task、bot、scheduler 列表。原生 agent 会话、Run 历史和 UI 入口不受这次布局调整影响。

### zone 稳定 ID 与旧格式

当前 zone 登记使用 `{id,path}`，node 的 `zone` 使用稳定 ID，直接文件输入使用相对 `path` 与可选 `zone_id`。新接口只接受 ID 引用，不兼容旧路径格式；正常启动不会自动迁移旧格式。历史离线迁移工具已归档，不再随仓库提供。已有旧数据须先停服并备份，明确每个旧目录的稳定 ID 和本机绑定，完成显式格式更新后再启动，不根据目录名猜测或让旧、新服务混写。

新 server 的 zone 登记入口要求 ID 与本地目录；登记 JSON 使用 `{ "id": "hugosol/herdr-bot", "path": "E:/Developer/myhub/herdr-bot" }`。取消登记按 ID，改绑只影响新 Run，不删除目录。Map 可在绑定未完成时保存，启动前必须补齐。

### 文件交接规则格式升级

静态 node 现在使用 `input_rule`／`output_rule` 数组；执行固定规则并记录实际 `inputs: [{id,path}]`；报告保存 `from`／`to` Chat 关联及 `outputs: [{id,path}]`。规则 ID 是逻辑引用，不是文件名。完整字段与校验见 [scheduler](docs/scheduler.md#文件输入与输出规则) 和 [运行说明](docs/workflow-runtime.md)。本次同时更新 toolkit 角色工具与指引，不能混用新后端和旧 report 参数。

普通启动与请求不兼容旧字段，不在读取时自动转换。历史离线迁移工具已归档，不再随仓库提供。已有数据须先停服，核实配置中实际使用的 Map／Run 目录并备份，显式更新格式后核对 Map 可读、规则引用、历史报告与执行事实，再启动新后端并部署同版本 toolkit，不混合新旧数据继续运行。

无法确定的历史执行规则与实际输入保持未知，不从当前 Map 伪造派发时快照。更早历史事件中缺失的非交接元数据（如 `agent`）保持原样，不猜测补填；当前 Run 记录仍按当前结构完整校验。格式更新不应改变工作状态、报告归属或业务文件名称／地址，也不恢复旧 Run 的管理资格。

## 安全边界

**没有账号或身份验证，只应在可信局域网使用，不要暴露到公网。**
能访问服务的设备可以管理 Task、启动 agent、执行配置脚本、操作 Git 和关闭整台 Windows 电脑。
connect-bot 会话写入另有限制：只接受同机来源，见 [ADR 0004](docs/adr/0004-session-write-boundary-same-host.md)。
页面确认不是服务端认证。

## 文档

- [使用说明](docs/usage.md)：页面导航、server 操作、Task 配置与历史 Chat 恢复。
- [scheduler 使用与参考](docs/scheduler.md)：目录配置、定义管理、JSON 格式、接口和问题排查。
- [Map Run 长期需求与契约](docs/workflow-contract.md)：职责、全部承诺、状态规则、接口/记录索引、重要取舍、排除项与 Agent/人工验收边界；无需临时设计材料。
- [scheduler 样例与体验指引](docs/example/README.md)：可复制样例、隔离运行与人工验收。
- [workflow 运行与故障处理](docs/workflow-runtime.md#故障处理速查先看现场再向管理者表达意图)：查看失败、Resume、核实/修复与向管理者表达恢复、重试或人工交接意图。
- [领域术语](GLOSSARY.md)与[架构决策](docs/adr/)。
- [测试指引](docs/agents/testing.md)：验证入口、夹具隔离与手动网络验收。

## 源码归属

请求 controller 集中放置并按业务模块命名，便于查找全部请求；页面、hook 与业务实现仍按功能模块组织。前端 controller 发出 HTTP 请求，后端 controller 接收 HTTP 请求，两者分别位于各自的 `controllers/`，不共用实现：

| 功能 | 前端 `frontend/src/` | 后端 `backend/src/` |
| --- | --- | --- |
| 应用装配 | `main.tsx`、`app/` | `lib.rs`、`main.rs` 及启动配置文件 |
| HTTP 请求入口 | `controllers/*Controller.ts` | `controllers/*_controller.rs` |
| Task 与选项目录 | `tasks/` | `tasks/`（含 `catalog.rs`） |
| Chat 发起、恢复与历史 | `chats/` | `chats/` |
| Herdr 宿主集成与位置映射 | — | `herdr/`（私有 `client.rs` CLI adapter、`identity.rs` 原生身份实现与 `placement.rs` 位置映射） |
| 当前会话登记与观察 | `chats/` 中的 session feed 与会话视图 | `sessions/`（含共享观察流） |
| zone、文件浏览与 Git | `zones/`，内部按 `browser/`、`git/`、`reader/` 归类 | `zones/`，Git 操作位于 `zones/git/` |
| Bot 静态定义与编辑 | `scheduler/bots/`、`controllers/botController.ts`；展示仍由 scheduler 组合 | `scheduler/bots/`（含固定 Rust 能力） |
| Map／node 静态定义与编辑 | `scheduler/maps/`、`controllers/schedulerController.ts` | `scheduler/maps/`；跨定义协调留在 `scheduler/mod.rs` |
| Run 发起与预检 | `workflow/launch/`、`controllers/workflowController.ts` | `runs/preflight.rs` 与现有 Run 启动逻辑 |
| Run 运行事实 | `workflow/`、`controllers/workflowController.ts` | `runs.rs` 与 `runs/` |
| 系统文件持久化与协调 | — | `files.rs`（常用操作与受限协调范围） |
| 固定操作与关机 | `controllers/serverController.ts`、`server/shutdown/` | `operations.rs`、`shutdown/`（实际关机动作在 `power_off.rs`） |

前端普通请求集中在 `frontend/src/controllers/`，按业务命名为 `chatController.ts`、`botController.ts`、`taskController.ts`、`schedulerController.ts`、`workflowController.ts`、`suiteController.ts`、`zoneController.ts` 与 `serverController.ts`。文件浏览、Git（含 diff）、HTML 阅读资源地址和关机分别由 `zoneBrowserController.ts`、`zoneGitController.ts`、`zoneReaderController.ts`、`shutdownController.ts` 拥有；HTML 净化与链接安全规则仍归 reader 实现。这些 HTTP adapter 收拢请求地址、序列化、响应解析和错误分类，跨分区调用同一个业务 interface，不复制请求，也不保留旧目录或 `*Api.ts` 转发层。不增加全局万能请求封装或自动重试，保留各业务的拒绝、不可达和结果未确认语义。

后端 `controllers/*_controller.rs` 声明 HTTP 输入、调用业务操作，并保留必要的响应编排及 SSE 传输编码。重复请求契约位于 `controllers/extractors/`，通过 Axum `FromRequestParts`／`FromRequest` 提取；相同契约共享实现，不合并不同的查询串规则、body 上限、Content-Type 接受范围或拒绝顺序。已有原生 `Json`／`Query`／`Path` 契约继续保留，来源检查先于受保护请求的 body 解析，Run 重命名仍先检查路径身份。

领域结果与错误的 HTTP 转换位于 `controllers/responses/`，按领域实现 `IntoResponse`；`HttpError<E>` 只承载原领域错误，不引入全局错误枚举或统一响应信封，controller 不互相调用错误转换。chief 委派的陈旧预览错误需要异步读取原始错误响应，保留专用异步转换；SSE、文件及 Run collect 流式响应不转成普通 JSON。权限判定、业务校验、锁、状态转换、持久化及外部执行仍由 `tasks`、`chats`、`chief`、`scheduler`、`runs`、`sessions`、`suites`、`zones`、`operations` 与 `shutdown` 等业务 module 拥有；业务 interface 返回领域结果与错误，不依赖 Axum。路由注册沿用 `lib.rs` 的装配位置，`AppState` 不变，不新增 Controller 类、万能调度器或旧 handler 转发层。HTTP interface 仍是测试面，业务约束在原 module 内保持 locality。

页面与 hook 保留加载／草稿状态、取消与竞态控制、刷新及导航协调。会话与 zone 的 SSE 分别保留 `chats/sseSessionFeed.ts`、`zones/zoneWatch.ts` 专门 adapter，不并入普通请求。测试沿用 fetch、事件源注入及真实浏览器 HTTP／SSE seam，不 mock controller；路径迁移不改变行为断言。

Map／Bot 编辑协调集中在 `frontend/src/scheduler/useSchedulerEditor.ts`：草稿归属、离开许可、读取结果归属、保存结果与删除后的状态由同一 module 拥有，interface 提供可展示状态与用户动作，不暴露原始 setter。`SchedulerPanel.tsx` 保留渲染、确认框展示与焦点，App 保留主导航；Map 请求使用 `controllers/schedulerController.ts`，Bot 请求使用 `controllers/botController.ts`。Bot／Map 专属展示分别归 `scheduler/bots/` 与 `scheduler/maps/`；`BotTargetSelect.tsx` 表达 Task、Map、固定 Rust 能力三种互斥目标。Run 发起表单归 `workflow/launch/`，预检与发起请求归既有 `controllers/workflowController.ts`，HTTP 路径不变。共同草稿、刷新所有权及共享样式不随目录拆散。

后端 `scheduler/bots/` 与 `scheduler/maps/` 分别拥有 Bot 和 Map／node 定义行为，Run 预检归 `runs/preflight.rs`。跨定义引用、协调范围及 chief 委派所需定义核对仍由 scheduler 持有；chief 授权与实际委派不移入 scheduler。归属调整不改变 revision／引用检查顺序、文件协调与 Run 操作锁范围、部分失败语义或数据格式，也不保留旧路径转发。

Herdr 集成统一通过 `crate::herdr` 使用；`client` 与 `identity` 保持私有实现，宿主调用与原生身份类型由模块接口导出，内部位置映射通过 `crate::herdr::placement` 使用。本次仅调整源码归属，不改变运行行为。

位置映射集中在 `backend/src/herdr/placement.rs`：`Scenario` 枚举显式区分普通 Chat、chief、chief 委派 Task、根 Map 管理者、chief 委派 Map、node、嵌套 Map 管理者和历史 Chat 恢复，共用按名称放置、跟随管理者、恢复历史位置三套规则。interface 只接收位置依据，不接收整个 Task／Run／AppState；zone 仍是稳定逻辑工作位置，执行目录要求与宿主 Workspace 选择分开表达。映射只进行必要的只读查询，不创建 Tab、启动 agent 或修改历史。

Resume 先解析目录，完成原生加载目标与环境检查后再查询 Workspace；映射结果区分明确 cwd 与交给 Herdr 选择目录，后者仍须在启动后确认实际 cwd。位置降级事实交回 `resume.rs` 的 `degradation::Feedback`，与环境、原生文件降级统一呈现；查询失败不当作不存在，不新增重试或缓存。CLI adapter 与独立进程 Herdr fixture 的测试 seam 保持不变。

Run 专用 Chat 启动集中在 `backend/src/runs/startup.rs`：管理者启动、node 派发与显式 retry 共用启动、真实身份确认、关联保存、Chat 历史记录和首次指示提交协议，并由该 module 完成成功状态与失败收尾。位置选择交给上述映射 module，身份校验和关联写入差异仍在启动协议内部明确处理，不通过阶段回调拼装。调用者保留授权、预检、批次顺序及 node 的 Run 操作锁；独立 Chat 发起、会话恢复和人工恢复事实不进入此协议。

跨模块引用指向实际所属文件，不通过旧路径兼容转发。前端全局样式暂集中在 `app/styles.css`；后端 `runs.rs` 与同名目录共同构成一个 Rust 模块。目录归类不改变 Task、Chat、Map、Run 的含义或执行时序。

系统持久数据的读写、枚举、删除，以及产品代码直接执行的文件变更统一经过 `files.rs`。普通 JSON 读改存与条件发布使用常用操作；复合操作通过 `FileStore::with`／`with_async` 的受限访问对象完成，不能嵌套取得协调范围，也不能在范围内调用 Herdr／Git 等外部操作。目录与后代文件重叠协调，不相关的数据仍独立推进；调用者保留业务规则及 Run 操作锁，不持有原生文件锁。统一文件入口不改变文件格式及部分生效语义，定义路径按上文单独切换，不提供跨文件事务或自动修复；适用范围和取舍见 [ADR 0016](docs/adr/0016-unified-file-persistence.md)。

测试保留 `frontend/tests/`、`frontend/browser-tests/` 和 `backend/tests/` 三个运行层级，内部按领域与行为归组。后端通过 `backend/Cargo.toml` 的 `[[test]]` 显式登记入口，目录迁移保留原 target 名称与执行粒度；新增 target 需要登记。共享夹具与业务用例分开，跨入口契约归 `app`，夹具自身回归归 `infrastructure`。执行入口与安全约束见[测试指引](docs/agents/testing.md)。历史 ADR 和研究记录保留当时的路径语境。
