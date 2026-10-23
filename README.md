# herdr-bot

Windows 上的 agent 对话主界面，本机浏览器与局域网设备访问同一个服务。

- `chat`：当前会话状态、历史会话及完成后关机监听。
- `bot`：管理可复用的 Bot；Bot 不是实际对话。
- `zone`：管理服务主机上的目录，浏览文件、审查 Git 变更，以及选择 Bot 发起 Chat。
- `server`：调用预先配置的启动／停止脚本。

## 构建与运行

需要 Rust 1.98+、Node.js 24+；发起 Chat 还需要已运行的 herdr 及已安装的目标 agent。

```bat
npm --prefix frontend ci
npm --prefix frontend run build
cargo build --release --manifest-path backend\Cargo.toml
backend\target\release\herdr-bot.exe --config herdr-bot.toml
```

运行前检查 [`herdr-bot.toml`](herdr-bot.toml) 中的本机路径、监听地址及 Herdr Workspace，
以及 [`meta-data/bot-options.toml`](meta-data/bot-options.toml) 中的模型和扩展入口。
配置相对路径以进程工作目录为基准。请从控制台启动，以便脚本继承可用的控制台。

Bot 的 Codex 模型选项包含 `GPT-6.1-sol`、`GPT-6-luna` 和 `GPT-6-astra`；
思考级别按所选 agent 与模型的能力提供，Pi 的 Luna 支持 `off`，Astra 不支持。
模型目录修改后下次打开编辑、保存或发起 Chat 时重新读取，无需重启。
删除 Bot 时先询问是否删除，取消不发出删除请求；删除不影响已经发起的 Chat。

本机访问 `http://127.0.0.1:<端口>/`；局域网使用启动日志中的
`http://herdr-bot.local:<端口>/`，无法解析时使用服务主机的局域网 IPv4。
手机上的 `127.0.0.1` 指向手机自身。

## 安全边界

**没有账号或身份验证，只应在可信局域网使用，不要暴露到公网。**
能访问服务的设备可以管理 Bot、启动 agent、执行配置脚本、操作 Git 和关闭整台 Windows 电脑。
connect-bot 会话写入另有限制：只接受同机来源，见 [ADR 0004](docs/adr/0004-session-write-boundary-same-host.md)。
页面确认不是服务端认证；测试或排障时不要调用真实关机，也不要用用户正在运行的对话做探针。

## 旧数据一次性切换

Bot／Chat 术语切换同时改变内部路由、字段、配置与存储格式，不提供旧格式兼容。
迁移需要 Python 3.11+。升级前停止旧服务，在仓库根目录运行：

```bat
python scripts\migrate-bot-chat-terminology.py --config herdr-bot.toml
```

迁移保留已有身份标识；不修改 zone、历史会话或 agent 原生对话文件。
完成后重新构建并启动服务。命名与迁移决策见 [ADR 0011](docs/adr/0011-bot-chat-terminology.md)。

## 文档

- [领域术语](GLOSSARY.md)
- [集成边界](docs/integration-boundaries.md)：Herdr 提交、扩展与原生会话的保证范围。
- [待人工确认的验收边界](docs/bot-acceptance.md)
- [架构决策](docs/adr/)

源码在 `backend/`（Rust/Axum）和 `frontend/`（React/Vite）。
自动检查入口为 `cargo test --manifest-path backend\Cargo.toml`、
`npm --prefix frontend test` 和 `npm --prefix frontend run test:browser`；
浏览器检查需要构建后的前后端和本机 Chrome。
