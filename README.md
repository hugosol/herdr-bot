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

## 历史 Chat Resume

在 `chat` 的历史视图中，每条记录都有 `Resume` 入口，不要求当前会话仍登记或保留原 Herdr 绑定。
独立调用者可发送 `POST /api/chats/resume`，JSON 只包含 `{"sessionId":"历史记录的完整身份"}`；
额外的命令、参数、路径或输入字段会被拒绝，原生身份及 Herdr 可执行文件均由后端决定。

每次操作重新查询 Herdr，以 Pi／OMP 原生 ID 匹配；路径引用读取原生文件头以核实 ID，不按名称或 cwd 猜测。
找到目标后导航到其实际 Workspace／Tab／Pane，不搬迁、不创建副本、不提交输入。
多个目标按 Workspace ID、Tab ID、Pane ID 的字典序稳定选择一个，并在页面提示。
影响匹配的原生身份暂不可读、Herdr 查询异常或聚焦未确认都会明确失败；不以失败冒充无匹配。

只有成功查找且没有匹配时，才在历史 cwd 中新建目标，使用当前配置的 agent 命令和必要集成加载指定原生对话。
优先使用身份一致的可用会话文件，文件不可用时仅回退到同一原生 ID；Pi 使用 `--session`，OMP 使用 `--resume`，不使用会自动继续中断任务的 `--restore`。
恢复不提交新输入、不重放 Bot 初始指示，也不重新应用 Bot 的模型／思考设置覆盖原生对话。缺失工作目录、不可用原生对话、启动或聚焦失败均明确反馈，不以空白对话冒充成功。
缺少原 Workspace 的旧记录回退到配置的唯一默认 Workspace，并说明回退；默认位置不可用时不创建替代位置。
`located` 表示已确认定位已有对话，`restored` 仅在确认同一原生身份、实际 cwd 和聚焦后返回；两者都只确认 Herdr 内部导航，不保证 Windows 桌面窗口置前。
真实原生加载及 OMP 未自动继续任务由专用 H-Pi／H-OMP 人工验收确认；受控参数记录不代替该证据。详见 [集成边界](docs/integration-boundaries.md#历史-chat-resume)。
缺少受支持 agent 类型的旧记录仍提供入口并明确反馈原因。会话事件同机写入限制保持不变。

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
浏览器检查需要构建后的前后端和本机 Chrome。Cargo 检查也需要 Node.js，以执行跨前后端的测试网络护栏。

### 测试网络边界

默认测试可以启动真实进程及 HTTP／SSE 服务，但只能监听数值回环地址；不得监听所有接口、本机网卡地址或发布真实 mDNS。
Rust `TestServer::start_app_on` 和浏览器 `startServer` 在绑定前拒绝非回环地址，即使已设置手动授权也不放宽。
普通测试应复用这些入口，不得自行导入网络模块或创建 listener。历史持久化、进程重启与多浏览器上下文测试仍走回环。

真实 LAN／mDNS 验收须显式手动运行：

```powershell
# Rust：构建并复制到固定路径，再运行 ignored LAN／mDNS 用例。
.\scripts\run-mdns-tests.ps1
# 仅构建并打印 runner 路径，不打开监听。
.\scripts\run-mdns-tests.ps1 -ListOnly
# 浏览器：只运行真实 LAN 验收；前置构建要求同普通浏览器检查。
npm --prefix frontend run test:browser:manual
```

Rust 脚本沿用原文件名，但现在覆盖 `mdns_failure`、`mdns_conflict`、`session_write_boundary` 和 `monitoring` 的手动用例。
它把带哈希的 Cargo 测试程序复制为 `backend\target\debug\<测试名>_runner.exe`，避免重建换路径引起重复授权；
直接 `cargo test -- --ignored` 不提供这一保证。手动入口设置 `HERDR_BOT_MANUAL_NETWORK=1`；这不是普通测试的绕过开关。
真实 mDNS 用例需要独占 `herdr-bot.local`，运行前停止正在使用该名字的服务。只有用户明确需要配置规则时，才单独使用脚本的 `-ConfigureFirewall`；该选项会请求管理员权限并配置入站放行。

`node scripts/check-test-network-policy.mjs` 检查跨前后端测试中的直接监听 API、网络模块导入、手动 helper 的调用位置及 Rust 手动用例的忽略标记。
它通过 Cargo 的 `network_policy` 测试和 npm 的 `pretest`／`pretest:watch`／`pretest:browser`／`pretest:browser:manual` 自动执行；直接调用底层 Vitest／Playwright 不会执行 npm hook。
护栏自身的回归检查为 `node --test scripts/check-test-network-policy.test.mjs`。
例外限定到脚本内列出的具体基础设施函数和手动用例，不豁免整个目录。新增真实网络能力时必须同时审查手动入口、默认收集规则与固定 runner；不能只增加豁免。
静态检查针对已知 API，配合运行时地址约束防误用，不是能识别任意第三方库或动态代码的网络沙箱。
