# herdr-bot

Windows 上的 agent 对话主界面，本机浏览器与局域网设备访问同一个服务。

## 主要功能

- **chat**：查看当前与历史会话，恢复原生对话，以及监听会话完成后关机。
- **task**：管理可复用的 Task 配置，用于发起 Chat；Task 本身不是实际对话。
- **zone**：管理服务主机上的目录，浏览文件、审查 Git 变更，并选择 Task 发起 Chat。
- **bot**：独立访问共享 Bot 能力定义，复用 scheduler 的原有管理功能。
- **scheduler**：浏览和编辑 Map、node 与共享 Bot，校验配置并发起独立管理者 Run（需部署角色扩展）。node 可配置 `report_policy: auto/manual`；Map 与 Run 图用圆角／切角节点区分策略，详情显示具体值。
- **workflow**：按日期倒序浏览和筛选全部 Run；全宽详情中用固定节点列表、横向状态图与报告查看现场，图中显示 Task 名称及独立运行标记，Resume 已关联 Chat；返回保留列表上下文，可见时轮询，查询失败保留上次内容，刷新不重新启动。需求、配置与限制见 [Map Run 长期契约](docs/workflow-contract.md)与[运行说明](docs/workflow-runtime.md)。
- **server**：调用预先配置的启动／停止脚本，以及手动关机。

## 构建与运行

需要 Rust 1.98+、Node.js 24+；发起 Chat 还需要已运行的 herdr 及已安装的目标 agent。

```bat
npm --prefix frontend ci
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
- `herdr` 等裸程序名继续通过 `PATH` 查找；显式程序路径相对配置文件解析。Git、Windows 系统工具和开发语言服务的安装与 `PATH` 设置仍由机器环境负责。当前 `.omp` 中的 MCP 命令也使用 `PATH` 中的 `codebase-memory-mcp`，不内嵌旧机器路径。
- 顶层 `static_directory` 指定页面产物，默认 `backend/static`；`scheduler.run_instructions` 默认 `docs/workflow-runtime.md`，均相对服务配置目录，不再使用编译时源码目录。交付配置通过 `roots.project` 显式定位两者。
- 顶层 `data_directory` 指定内部迁移标记目录，默认 `.data`。它不隐式覆盖其他持久化字段；交付配置将历史、zone 登记、Run 根及标记目录共同引用 `roots.data`。

可只读查看根解析结果：`cargo run --quiet --manifest-path backend/Cargo.toml --bin herdr-paths -- --paths paths.local.toml`。输出为 JSON，不启动服务、不创建业务目录。开发工具中真实依赖路径的覆盖方式见[测试指引](docs/agents/testing.md)。

本次路径集中管理不迁移旧 zone／Chat／Run，不改写 Task、Bot、Map 的业务文本与材料引用；保留这些定义不代表其中旧机器业务路径自动可用。

本机访问 `http://127.0.0.1:<端口>/`；局域网使用启动日志中的
`http://herdr-bot.local:<端口>/`，无法解析时使用服务主机的局域网 IPv4。
手机上的 `127.0.0.1` 指向手机自身。

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

省略字段时默认路径与上述相同。Run、Chat 历史、zone 登记仍属于 `.data`；Map 内每个 node 保存的具体 zone 路径、Task/Bot 引用及执行语义均不改变。定义可能包含本机路径、固定指示和业务内容，允许 Git 跟踪不代表适合公开提交，程序不会自动提交。

### 从旧定义布局升级

修改代码或配置本身不会搬动正在使用的数据。先停止旧服务并备份数据，将服务配置放在原数据布局的根目录，再启动新版本；不要让旧、新服务同时写入这组定义。旧源位置固定相对服务配置目录，不随 `roots.data` 移动。配置指向该目录下上述新默认位置时，启动会一次性将 `.data/tasks.json`、`.data/bots/`、`.data/workflow-maps/` 复制到对应新位置，保留所有原始字节、目录内容及旧源；不会迁移其他运行数据。显式配置的其他路径继续有效，不自动导入旧默认目录。

- 所有待迁项目先检查冲突：新旧同时存在时，完整内容必须一致；缺少或多出文件也算目录冲突，不自动合并。冲突、链接／junction、异常文件类型或 I/O 错误都会明确终止启动，不当作空数据继续。
- 成功切换分别记录在 `data_directory` 下的 `.definition-layout-v1-tasks`、`-bots`、`-maps`（默认目录为 `.data`）。此后只读写配置的新位置；旧源只是备份，新数据修改或删除后不会从旧源回灌。不要删除这些标记来尝试刷新定义。
- 若报冲突，保持停服，备份并人工确认唯一权威版本，使目标与旧源完全一致或将冲突目标移到其他备份位置，再重启；不要盲目覆盖任一份数据。
- 不提供跨文件事务。中途 I/O 失败可能留下已完整复制的项目；保留现场并排除错误后重启，完整一致的项目可确认切换，不覆盖现有内容。

升级后核对 task、bot、scheduler 列表。原生 agent 会话、Run 历史和 UI 入口不受这次布局调整影响。

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

源码按功能模块组织，不按视图、请求、hook 等技术类型分散目录：

| 功能 | 前端 `frontend/src/` | 后端 `backend/src/` |
| --- | --- | --- |
| 应用装配 | `main.tsx`、`app/` | `lib.rs`、`main.rs` 及启动配置文件 |
| Task 与选项目录 | `tasks/` | `tasks/`（含 `catalog.rs`） |
| Chat 发起、恢复与历史 | `chats/` | `chats/`（含 Herdr adapter） |
| 当前会话登记与观察 | `chats/` 中的 session feed 与会话视图 | `sessions/`（含共享观察流） |
| zone、文件浏览与 Git | `zones/`，内部按 `browser/`、`git/`、`reader/` 归类 | `zones/`，Git 操作位于 `zones/git/` |
| Map、Bot 静态定义与发起预检 | `scheduler/` | `scheduler/` |
| Run 运行事实 | `workflow/` | `runs.rs` 与 `runs/` |
| 系统文件持久化与协调 | — | `files.rs`（常用操作与受限协调范围） |
| 固定操作与关机 | `server/operationsApi.ts`、`server/shutdown/` | `operations.rs`、`shutdown/`（实际关机动作在 `power_off.rs`） |

Map／Bot 编辑协调集中在 `frontend/src/scheduler/useSchedulerEditor.ts`：草稿归属、离开许可、读取结果归属、保存结果与删除后的状态由同一 module 拥有，interface 提供可展示状态与用户动作，不暴露原始 setter。`SchedulerPanel.tsx` 保留渲染、确认框展示与焦点，App 保留主导航，`schedulerApi.ts` 作为 HTTP adapter。

Run 专用 Chat 启动集中在 `backend/src/runs/startup.rs`：管理者启动、node 派发与显式 retry 共用启动、真实身份确认、关联保存、Chat 历史记录和首次指示提交协议，并由该 module 完成成功状态与失败收尾。管理者／node 的 Workspace、身份校验和关联写入差异在内部明确处理，不通过阶段回调拼装。调用者保留授权、预检、批次顺序及 node 的 Run 操作锁；独立 Chat 发起、会话恢复和人工恢复事实不进入此协议。

跨模块引用指向实际所属文件，不通过旧路径兼容转发。前端全局样式暂集中在 `app/styles.css`；后端 `runs.rs` 与同名目录共同构成一个 Rust 模块。目录归类不改变 Task、Chat、Map、Run 的含义或执行时序。

系统持久数据的读写、枚举、删除，以及产品代码直接执行的文件变更统一经过 `files.rs`。普通 JSON 读改存与条件发布使用常用操作；复合操作通过 `FileStore::with`／`with_async` 的受限访问对象完成，不能嵌套取得协调范围，也不能在范围内调用 Herdr／Git 等外部操作。目录与后代文件重叠协调，不相关的数据仍独立推进；调用者保留业务规则及 Run 操作锁，不持有原生文件锁。统一文件入口不改变文件格式及部分生效语义，定义路径按上文单独切换，不提供跨文件事务或自动修复；适用范围和取舍见 [ADR 0016](docs/adr/0016-unified-file-persistence.md)。

测试继续位于 `frontend/tests/`、`frontend/browser-tests/` 和 `backend/tests/`，不随源码迁移；执行入口与安全约束见[测试指引](docs/agents/testing.md)。历史 ADR 和研究记录保留当时的路径语境。
