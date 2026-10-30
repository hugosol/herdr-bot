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

运行前检查 [`herdr-bot.toml`](herdr-bot.toml) 中的本机路径、监听地址及 Herdr Workspace，
以及 [`meta-data/task-options.toml`](meta-data/task-options.toml) 中的模型和扩展入口。
配置相对路径以进程工作目录为基准。请从控制台启动，以便脚本继承可用的控制台。

本机访问 `http://127.0.0.1:<端口>/`；局域网使用启动日志中的
`http://herdr-bot.local:<端口>/`，无法解析时使用服务主机的局域网 IPv4。
手机上的 `127.0.0.1` 指向手机自身。

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

跨模块引用指向实际所属文件，不通过旧路径兼容转发。前端全局样式暂集中在 `app/styles.css`；后端 `runs.rs` 与同名目录共同构成一个 Rust 模块。目录归类不改变 Task、Chat、Map、Run 的含义或执行时序。

系统持久数据的读写、枚举、删除，以及产品代码直接执行的文件变更统一经过 `files.rs`。普通 JSON 读改存与条件发布使用常用操作；复合操作通过 `FileStore::with`／`with_async` 的受限访问对象完成，不能嵌套取得协调范围，也不能在范围内调用 Herdr／Git 等外部操作。目录与后代文件重叠协调，不相关的数据仍独立推进；调用者保留业务规则及 Run 操作锁，不持有原生文件锁。文件格式、路径及部分生效语义不变，不提供跨文件事务或自动修复；适用范围和取舍见 [ADR 0016](docs/adr/0016-unified-file-persistence.md)。

测试继续位于 `frontend/tests/`、`frontend/browser-tests/` 和 `backend/tests/`，不随源码迁移；执行入口与安全约束见[测试指引](docs/agents/testing.md)。历史 ADR 和研究记录保留当时的路径语境。
