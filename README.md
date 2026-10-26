# herdr-bot

Windows 上的 agent 对话主界面，本机浏览器与局域网设备访问同一个服务。

- `chat`：当前会话状态、历史会话及完成后关机监听。
- `bot`：管理可复用的 Bot；Bot 不是实际对话。
- `zone`：管理服务主机上的目录，浏览文件、审查 Git 变更，以及选择 Bot 发起 Chat。
- `scheduler`：浏览服务级 Map 目录中的顶层 JSON 文件，区分空目录与读取错误。

顶部 tab 顺序为 `chat`、`bot`、`zone`、`scheduler`；首次打开与整页刷新都回到 `chat`。
右上角的 `server` 按钮在同一页面打开独立内容页，不打开新窗口；点击“返回”回到进入前的 tab。
键盘可用 Tab／Shift+Tab 聚焦入口，Enter／Space 激活；顶部 tab 还支持左右方向键及 Home／End 导航。
进入 server 后焦点移到返回按钮，返回后恢复到此前 tab。

### server 固定操作

在 `server` 内容页点击“启动”或“停止”，调用服务端预先配置的脚本；页面仅反馈脚本调用，
即使退出码为 0，也不代表后台程序已经启动、就绪或彻底停止。请求未结束时两按钮禁用，
结果区分别说明完成、失败、忙碌拒绝或结果无法确认，不自动重试或排队补做。
切回 tab 再进入 server 仍可查看本页最近一次调用的反馈。
手动关机入口也位于 server 内容页，保留确认流程；返回 tab 不是取消关机，
全局关机提醒在各 tab 和 server 页仍可见，取消必须显式操作并等待服务端确认。

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

## scheduler Map 目录

进入 `scheduler` 后读取的是**服务所在电脑**上的 Map 目录；两个浏览器访问同一实例看到同一组文件，
切换 zone 不改变目录归属，也不需要在手机或其他访问设备上选择本地文件。

不配置时使用进程工作目录下的 `.data/workflow-maps/`。首次进入或刷新 Map 列表时会创建缺失目录及父目录，
保留已有文件，不覆盖定义、不自动安装示例。把自己的 Map JSON 文件放到该目录后，点击“刷新 Map 列表”；
列表只包括目录直接包含的 `.json` 文件（后缀大小写均可），不递归读取子目录，不混入文档样例。
已有目录没有 JSON 时显示正常空态；创建、权限或读取失败显示明确错误，恢复目录后可再次刷新。

可在配置文件中独立指定 Map 与共享模板目录：

```toml
[scheduler]
map_directory = '.data/workflow-maps'
node_template_directory = '.data/node-templates'
```

两个字段均可独立省略，分别使用上述默认值。**相对路径按进程 cwd 解析，不按配置文件所在目录解析**；
也可填写服务主机上的绝对路径。配置修改需要重启服务；目录内容变化只需刷新列表。
共享模板目录的实际内容读取由后续模板预览能力提供；本页目前仅列出 Map 文件，不解析或执行定义。
生产读取接口为 `GET /api/scheduler/maps`，不接受路径、zone、查询参数或请求体；
目录路径各级不跟随符号链接、Windows 目录联接或其他重解析点，列表也排除链接项。
仅允许准备必要目录，浏览不会修改定义、启动 Chat 或执行其中的脚本。

## Bot 配置与原生存储

Bot 配置分为两组：

- **trigger config（发起配置）**：初始消息、初始 Skill 调用、model 与 thinking，仅在发起 Chat 时应用。
- **env config（会话环境配置）**：扩展配置；发起 Chat 时加载，Resume 需要新进程时也加载。

初始 Skill 调用是随初始消息提交的动作，不是加载型 Skill；Bot 固定 prompt 与本次 input 都属于用户输入，不是系统提示词注入。Bot 不提供加载型 Skill 或系统提示词注入。

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
