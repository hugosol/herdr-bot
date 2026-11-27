# herdr-bot

Windows 上的 agent 对话主界面，本机浏览器与局域网设备访问同一个服务。

## 主要功能

- **chat** 查看当前与历史会话，恢复原生对话，或监听会话完成后关机。
- **task** 管理发起 Chat 的可复用预设。**suite** 管理 Pi 或 OMP 的场景工具集。
- **zone** 登记服务主机的目录，浏览文件、审查 Git 变更，并在该目录发起 Chat。
- **bot** 管理共享能力。**scheduler** 编辑 Map 与 node，校验定义并发起 Run。**workflow** 查看 Run 状态、批准和报告，或恢复关联的 Chat。
- **server** 调用预先配置的启动或停止脚本，也可安排手动关机。

## 构建与运行

构建需要支持 Rust 2024 edition 的工具链、Node.js（含 npm）及 [前端 `packageManager` 指定的 Bun 1.4.4](frontend/package.json)。项目没有声明经验证的最低 Rust 或 Node.js 版本。发起 Chat 另需已运行的 Herdr 和已安装的目标 agent。运行[动态 TypeScript 脚本](docs/usage.md#动态-typescript-脚本)要求后端的 `PATH` 中有 Bun，服务不会打包或安装它。

在仓库根构建：

```bat
bun install --cwd=frontend --frozen-lockfile
npm --prefix frontend run build
cargo build --release --manifest-path backend\Cargo.toml
```

每个 worktree 单独执行冻结安装。Bun 共享依赖缓存，不共享构建产物；如设置 `BUN_INSTALL_CACHE_DIR`，各 worktree 使用同一目录。清空共享缓存会使引用它的安装失效。更新依赖用 Bun，并提交 `frontend/package.json` 与 `frontend/bun.lock`；测试用 npm 脚本，保留网络护栏。

首次运行前，将 [`paths.example.toml`](paths.example.toml) 复制为仓库根的 `paths.local.toml`，按本机调整根目录，不覆盖已有配置。服务不会回退到样例文件。检查 [`herdr-bot.toml`](herdr-bot.toml) 的监听地址及 [Herdr 宿主与默认容器](docs/usage.md#选择-herdr-宿主)。使用提示词标签时，配置[本机提示词词典](docs/usage.md#本机提示词词典)。

已有部署先读[从旧定义布局升级](docs/usage.md#从旧定义布局升级)，并分清[定义数据与运行数据](docs/usage.md#定义数据与运行数据)的存放位置。修改[本机根目录或服务路径](docs/usage.md#本机根目录与解析规则)后重启。

从仓库根的控制台启动，让固定操作脚本继承控制台：

```bat
backend\target\release\herdr-bot.exe --config herdr-bot.toml
```

本机访问 `http://127.0.0.1:<端口>/`。局域网设备使用启动日志中的 `http://herdr-bot.local:<端口>/`，无法解析时改用服务主机的局域网 IPv4。手机上的 `127.0.0.1` 指向手机自身。

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

| 功能 | `frontend/src/` | `backend/src/` |
| --- | --- | --- |
| HTTP | `controllers/` 发出请求 | `controllers/` 接收请求，业务模块持有领域规则 |
| Chat、Task、Suite 与 zone | `chats/`、`tasks/`、`suites/`、`zones/` | 同名模块；Herdr 集成在 `herdr/` |
| 会话登记与关机 | `sessions/`、`server/shutdown/` | `sessions/`、`operations.rs`、`shutdown/` |
| Bot、Map 与 Run | `scheduler/`、`workflow/` | `scheduler/` 管理定义，`runs/` 管理执行与脚本协调 |

文件协调及部分生效边界见 [ADR 0016](docs/adr/0016-unified-file-persistence.md)。Chat 发起和恢复的原生身份信任边界见 [ADR 0014](docs/adr/0014-unpersisted-omp-startup-identity.md)。
