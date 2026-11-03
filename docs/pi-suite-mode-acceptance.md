# Pi Suite 模式与模板真实验收

`python scripts/accept-suite-pi-mode.py` 负责 P14、P16、P23、P24、P26、P27。
入口经公开 Task／Chat HTTP 发起真实 Herdr pane，由 PATH 上已安装的 Pi 正常启动；
它不直接用 SDK 发起 agent，不替换 MCP manager，也不修改 Pi／Herdr 源码。
SessionManager SDK 仅用于事后读取原生历史。外部模型和 MCP endpoint 是数值回环受控服务，
因此结果证明工具／资源消费机制，不证明真实模型自主决策质量。

## 前提与执行

需要 Windows、Python 3.11+、Node.js、Bun 1.4.4、Rust、预先安装的 Herdr／Pi，
以及两个已准备依赖的参与 worktree。Pi 必须公开提供 codemode、原生 MCP 和模板接口；
toolkit 必须包含工厂期 `--pi-suite-config` 通道。runner 不安装宿主、不提权、不改 PATH。

从 **herdr-bot worktree 根**执行，变量均为显式绝对路径：

```powershell
$HERDR_BOT_WT = (Get-Location).Path
$TOOLKIT_WT = 'D:/worktrees/toolkit'
$ISOLATED_PATHS = 'D:/acceptance/isolated-paths.toml'
$EVIDENCE_DIR = 'D:/acceptance/pi-mode-run-unique'
$env:CARGO_TARGET_DIR = Join-Path $HERDR_BOT_WT 'backend/target'
npm --prefix frontend ci
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot
$BACKEND_BINARY = Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe'
python scripts/accept-suite-pi-mode.py `
  --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" `
  --backend "$BACKEND_BINARY" --paths-file "$ISOLATED_PATHS" `
  --evidence-dir "$EVIDENCE_DIR"
```

`$EVIDENCE_DIR` 必须尚不存在，禁止接管旧运行。隔离配置只声明真实、只读依赖：

```toml
[roots]
herdr = 'C:/installed/herdr'
pi_package = 'D:/node_modules/@earendil-works/pi-coding-agent'
```

路径根由当前 worktree 的 `herdr-paths` 解析，不读取 `paths.local.toml`，不需要用户业务根、
用户 Pi 配置或 auth store。声明的 Pi CLI 必须与 PATH launcher 旁实际包的字节身份一致，
不会静默切换安装副本。backend 必须在该 worktree 的独立 `CARGO_TARGET_DIR` 内；入口用
当前源码执行一次离线 Cargo build，并比对公开构建产物与传入二进制的 SHA-256。Cargo
编译使用既有工具链／缓存；真实 server／agent 使用完全隔离的 HOME、APPDATA、XDG、
Pi 状态及会话目录。静态页面必须由当前 worktree 前端构建产生，不生成占位页面。

## 场景与独立预期

| Coverage | 场景与观察 |
| --- | --- |
| P23 | Task 可选扩展始终为空；固定入口来自指定 toolkit。Suite 双向模式差异证明实际工厂消费启动配置，而不是只检查 argv。 |
| P24 | 全局 on、only 分别比较无 pi-suite 与有固定 pi-suite、无 Suite 的两次真实发起。两侧保留相同真实 model-cost 固定扩展；逐字比较模型收到的完整工具声明，并经 codemode 调用 native `mcp.json` 来源的 MCP。 |
| P26 | 全局 only + Suite on 的声明必须恰为 read、bash、edit、write、codemode；全局 on + Suite only 必须恰为 codemode。前后读取原全局 codemode，不能写回 Suite 模式。Pi 自己的 `lastChangelogVersion` 记账不等于改写模式，原始前后 settings 均保留。 |
| P27 | `codemode` 实际调用底层 `tools.read` 读取文件；结果须含独立字面 `PI-ONLY-REAL-FILE-731`，再核对源文件内容及哈希未变。 |
| P16 | 模板正文哨兵在初始模型请求及首次工具续轮均不存在，系统消息始终不得含正文。显式 `/suite-mode "red fox" blue` 后，原生位置参数展开的 user 文本必须为 `PI-TEMPLATE-BODY-827 red fox\|blue\|red fox blue`，同时核对原生 SessionManager 历史。 |
| P14 | 共享 Suite 声明一个有启动标记的受控 LSP 命令及 Python 文件。所属进程树没有该命令、启动标记不存在，而模板、真实文件及原生 MCP 确实被消费。 |

MCP 的 `tools/call` 还须在真实 endpoint wire 中恰出现一次，并返回独立受控文件正文
`PI-NATIVE-MCP-FILE-619`。测试没有用预期字符串替代 Pi 的文件读取或 native MCP 执行。
本矩阵选择空技能／可选扩展集合，只使用本票所需模板与原生资源，不建立复制／junction
回退，也不声称完成非空技能、扩展包或其符号链接验收。

默认 `--scenario all` 覆盖六次真实发起。`baseline`、`modes`、`template` 可用于窄反馈；
部分场景即使成功也非零退出，不代表全部 Coverage 完成。缺依赖、行为不符或清理失败均非零，
不 skip，不自动重试业务提交。CLI 缺当前二进制的 fail-closed 回归为：

```powershell
$env:PI_MODE_TOOLKIT_WORKTREE = $TOOLKIT_WT
python scripts/accept-suite-pi-mode.test.py
```

该回归只证明缺依赖时保留原错误、六项保持未验证且不启动服务，不代替 S4 成功证据。

## 证据及检查边界

`evidence.json` 将每个 Coverage 对应到场景，保留独立预期、源码 revision／diff／哈希、
二进制和 launcher 哈希、宿主版本、实际 HTTP 与模型请求／响应、MCP wire、工具声明、
settings 前后值、原生历史、terminal 和所属进程退出事实。stdout／stderr 持续消费到退出。
进程命令行证据只输出本次命名 Herdr server 的后代；不会导出用户其他进程的命令行。
正常及失败均关闭所属 pane/server，持有真实 Windows 进程句柄确认退出；原始失败与
`cleanupErrors` 分别保留。不会连接、关闭或登记用户现有 Chat／daemon，也不真实关机。

交付检查须另外从两个参与根分别执行，不能用专项矩阵或另一仓结果替代：

```powershell
# herdr-bot 根；源码变更后的页面由最后一条命令重新构建。
node scripts/test-backend.mjs
npm --prefix frontend test
npm --prefix frontend run test:browser:build
# toolkit 根；冻结依赖预先准备，临时 worktree 可按其规范设置 baseline 根。
bun run typecheck
bun run check:integrations
bun run test
```

toolkit 跨仓 regression 使用当前 herdr-bot 的安全壳而非生产关机二进制：将
`HERDR_BOT_REPO` 指向参与根，`HERDR_BOT_BINARY` 指向该根构建的
`backend/target/debug/examples/safe_shutdown_shell.exe`，`HERDR_BOT_METADATA` 指向
`meta-data/task-options.toml`，并先构建同根 `herdr-fixture`。这些 interop 配置不改变本专项
显式传入真实 backend 的启动路径。历史 baseline 的宿主版本失败不能通过改写 baseline 消除；
实际检查日志应明确区分版本缺口、依赖准备失败、产品失败和未派发检查。
