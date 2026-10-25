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

## Bot 配置与原生存储

Bot 配置分为两组：

- **trigger config（发起配置）**：固定初始指示、初始 Skill 调用、model 与 thinking，仅在发起 Chat 时应用。
- **env config（会话环境配置）**：扩展配置；发起 Chat 时加载，Resume 需要新进程时也加载。

初始 Skill 调用是随初始消息提交的动作，不是加载型 Skill；Bot 固定 prompt 与本次 input 都属于用户输入，不是系统提示词注入。Bot 不提供加载型 Skill 或系统提示词注入。

发起 Chat 时，初始 Skill、固定 prompt 与本次 input 合成一条初始消息，随 `herdr agent start --initial-message` 一次交付，不再额外调用 `agent prompt`。有消息时，确认本次目标 agent 已进入原生执行轮次即算发起成功，不等待首轮结束，也不代表模型服务已经接受请求；三部分全空时只等待 agent 就绪，不发送空消息。页面只呈现统一的启动阶段。

此功能需要配套支持 `--initial-message`、`agent.start_with_message` 和启动轮次确认的 herdr，以及更新后的 Pi／OMP `herdr-agent-state` 扩展；CLI、运行中的 herdr 服务和 Bot 固定扩展入口都需使用配套版本。旧版本不受支持，不会回退到两步发送。若发起结果未确认，agent 仍可能已执行；已有目标保留，服务不会自动重试或补发。参见 [ADR 0012](docs/adr/0012-initial-message-with-agent-launch.md)。

原生会话文件和附属记录由 agent 管理，herdr-bot 不搬动文件或修改 agent 原生配置。发起 Chat 不保证新建原生对话；agent 自身的自动续接设置不属于 herdr-bot 的功能。

## 安全边界

**没有账号或身份验证，只应在可信局域网使用，不要暴露到公网。**
能访问服务的设备可以管理 Bot、启动 agent、执行配置脚本、操作 Git 和关闭整台 Windows 电脑。
connect-bot 会话写入另有限制：只接受同机来源，见 [ADR 0004](docs/adr/0004-session-write-boundary-same-host.md)。
页面确认不是服务端认证。

## 历史 Chat Resume

在 `chat` 的历史视图中使用 `Resume` 返回 Pi／OMP 原生对话，不要求该 Chat 仍在当前登记中。已有目标只做 Herdr 内部导航；需要新进程时，在历史工作目录加载旧对话，不自动继续中断任务，也不重放 Bot 的发起配置。Herdr 内部导航不保证 Windows 窗口置前。

新进程使用来源 Bot 的最新环境配置，而不是创建时的快照；没有来源 Bot 或来源已删除时，使用 agent 的原生默认扩展配置。修改 Bot 不会重新配置已经运行的 Chat。恢复中无法加载的扩展或位置回退会在页面提示。

历史记录支持多行纯文本备忘，电脑与手机共享；备忘不会发送给 agent 或修改原生聊天记录。

若页面显示“启动结果未确认”，外部目标仍可能已经启动。先检查页面提供的目标信息，不要把超时当作未执行；服务不会自动清理或重试。

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
- [架构决策](docs/adr/)
- [测试指引](docs/agents/testing.md)：运行入口、并行诊断、夹具隔离与手动网络验收。

源码在 `backend/`（Rust/Axum）和 `frontend/`（React/Vite）。
