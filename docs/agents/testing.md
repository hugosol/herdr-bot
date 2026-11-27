# 测试指引

黑盒验收代码已迁至独立项目 `herdr-bot-tester`。按所用 shell 设置 `$TESTER` 和 `$BOT`，分别指向验证项目与本次被测产品 worktree 的绝对路径。两个项目不必相邻。

在 PowerShell 中，相对的 `CARGO_TARGET_DIR` 从 `$BOT` 解析。未设置时使用 `$BOT/backend/target`。

```powershell
$TESTER = 'C:/absolute/worktrees/herdr-bot-tester'
$BOT = 'C:/absolute/worktrees/herdr-bot'
$env:HERDR_BOT_WORKTREE = $BOT
Set-Location -LiteralPath $BOT
if ($env:CARGO_TARGET_DIR) {
    $env:CARGO_TARGET_DIR = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($env:CARGO_TARGET_DIR)
} else {
    $env:CARGO_TARGET_DIR = Join-Path $BOT 'backend/target'
}
```

`sh`（包括 Git Bash；行内 `NAME=value command` 示例也用此语法）：

```sh
TESTER='C:/absolute/worktrees/herdr-bot-tester'
BOT='C:/absolute/worktrees/herdr-bot'
export HERDR_BOT_WORKTREE="$BOT"
cd "$BOT" || exit 1
```

产品构建与常规回归从 `$BOT` 运行。`run.py` 从 `$TESTER` 启动场景，并按 `--herdr-bot-worktree "$BOT"` 绑定产品根。直接执行验证项目的 `scripts/` 设施自测时，使用上面设置的 `HERDR_BOT_WORKTREE`。传给验收脚本的文件路径使用绝对路径。要求独占构建目录的场景须在构建前设置本次独立的绝对 `CARGO_TARGET_DIR`，并在验收时使用同一目录。

完整入口见验证项目 `README.md`。本文记录入口选择与安全约束；测试结果和性能记录留在运行日志中。

## 验收脚本的共享设施

验证项目维护进程隔离、原生观察、模型传输、证据收集和设施自测。其 `README.md` 说明通用入口；本文和专项页保留产品场景的授权、配置和通过条件。网络护栏、Schema、文档检查和构建脚本仍由产品仓维护。

- 只操作本轮拥有的进程、窗口、会话、目录和端口。生产后端用于隔离业务验收；涉及系统关机的自动 dock 使用安全关机壳。安全壳不能替代指定的生产后端，自动验收不授权实际关机。
- 默认使用数值回环。Windows 原生场景显式选择 49152–65535，避免 `port=0` 不落在所需范围。仅启动前证实端口已被占用时可重新选端口；不得据此重放业务或模型请求。
- 真实模型、具体凭据、外网、LAN／mDNS、符号链接和现有用户会话的授权分别成立。不要发现、复制或刷新用户认证。将证据与密钥分开，清理前确认所属进程和相关后代实际退出；退出码、目录可删除或事后 PID 查询都不足以证明退出。

## 真实模型验收的范围

- **原生机制允许逐步驱动。** 按真实回执发出明确的用户指令，观察实际工具调用、必要的原生确认、正确的 execution 和公开读回。验收器不代模型调用工具、伪造确认、补写运行状态或材料。受控 provider 可证明机制，不能证明真实模型自主选择或业务质量。
- **自主交接不指定工具顺序。** 当场景要求模型自行判断何时交接，驱动只给业务目标和获授权的业务输入。普通节点应自行在成果 `report` 前决定 `approve`；进度 `report` 可以更早。不能把逐步教它 `approve` 再 `report` 算作证明。结束节点不承担普通节点的批准职责。
- **诊断可读内部实现，验收使用约定的公开界面。** 预期独立于被测算法。将原生身份、真实工具回执、公开持久事实和实际文件字节互相归因。批准、报告、命名交付和显式 complete／finish 分别取证。通知提交不等于模型已读，界面确认不等于持久化；有限窗口只证明窗口内的观察。
- **验收程序可用不等于产品通过。** 分开记录运行了哪些格、完整覆盖是否满足、证据及来源、必要仓库检查、原始失败和清理失败。过滤、native-only、缺权限或依赖、受控替身和部分通过不能补齐约定的真实矩阵。保留未覆盖项，不降低条件、不用重试改写失败，也不把缺失的 Agent 验收转成用户签收。

## 测试归属

保留后端集成测试、前端交互测试和真实浏览器验收三个运行层级，各层内部按业务所有者归组，而非按测试文件的历史前缀归组。Chat 历史归 `chats`，当前会话登记与观察归 `sessions`；Map／Bot 静态定义归 `scheduler`，Run 运行事实归 `workflow`；固定操作、关机与完成后关机监听归 `server`。真正跨入口的契约归 `app`，夹具自身行为归 `infrastructure`。

专用夹具留在所属测试模块，共享设施只收跨业务用途。迁移路径时同时更新导入、产物定位、证据清单、网络例外和命令选择器，保留已有 `--exact` 名称。`@time-contract` 与手动网络授权独立于业务目录。文件拆分不改变断言、故障预算或证据范围。

读取交付配置的测试必须显式绑定所用夹具的宿主类型及可执行路径，不能只替换 TUI 的 `executable` 却继承交付配置的 `backend = 'app'`。`runtime_data` 的交付配置场景固定使用受控 TUI，不连接已安装的 App／core；实际双宿主行为由对应专项验证。

## 选择入口

| 场景 | 入口 |
| --- | --- |
| 完整后端回归、后端共享代码或公共夹具变更、提交前的后端检查 | `node scripts/test-backend.mjs` |
| 前端组件与逻辑检查 | `npm --prefix frontend test` |
| 交付页面、真实 HTTP／SSE 与进程行为的浏览器验收 | `npm --prefix frontend run test:browser:build` |
| 单个后端目标的开发反馈 | 例如 `cargo test --manifest-path backend/Cargo.toml --test zone_commit` |
| 单个前端文件或当前改动的开发反馈 | `npm --prefix frontend test -- tests/zones/git/diff.test.ts` 或 `npm --prefix frontend test -- --changed` |

完整后端回归优先使用并行入口，一次执行即可；原生 Cargo 全量入口保留作对照和回退。定向检查和浏览器分层检查用于开发反馈，交付验收使用对应的完整入口。

### Herdr 宿主选择专项（S1／S3）

先构建本 worktree 的 `frontend` 与生产 `herdr-bot`，并使用独占绝对 `CARGO_TARGET_DIR`。运行 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-herdr-host-selection --backend <herdr-bot.exe> --herdr <herdr.exe> --app <herdr-app.exe> --core <hc.exe> --evidence-dir <新的证据目录>`，所有路径显式且绝对；证据目录使用 `%TEMP%` 下简短的独占名称，避免 App 的 `herdr-app.sock` 路径超过 Windows Unix socket 长度限制。专项用独占 TUI、App 和两个 core；从公开 HTTP 触发放置，以实际容器、Tab、Pane、终端归属及 core-local ID 碰撞核对配置路由和错配拒绝。固定操作分别核对脚本标记、HTTP 回执与独立宿主探测（成功脚本不必启动或停止进程），退出用所属进程和 core／holder 句柄确认。它不证明真实 Task 初始化或业务输入；有关 agent 行为另验。

### 配对 Task 发起、ready 与初始输入（S1／S2／S3）

在 Windows 交互桌面上，从参与的 `herdr-bot` worktree 根执行。准备 Windows Chrome 或已安装的 Playwright Chromium、Bun 1.4.4、已安装的 OMP／Pi、Herdr TUI／App／core；两套宿主不得连到已有会话。下面两个 worktree、配置、后端和证据路径必须是绝对路径；完整执行的 Cargo 目录必须是新建或空目录，证据目录不能已存在：

```powershell
$Toolkit = 'C:/absolute/toolkit-worktree'
$Config = 'C:/absolute/paired-launch-owned.json'
$Evidence = 'C:/absolute/paired-launch-new-evidence'
$env:CARGO_TARGET_DIR = 'C:/absolute/paired-launch-exclusive-cargo'
$Backend = Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe'
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-paired-task-launch `
  --toolkit-worktree $Toolkit --backend $Backend `
  --isolated-config $Config --combinations all --cases all --evidence-dir $Evidence
```

`$Config` 是本轮的显式安装选择，不是现有用户配置的副本。填入实际版本与路径（示例版本不表示支持范围）：

```json
{
  "authorization": "owned-local-mechanics-only",
  "herdr": "C:/absolute/herdr.exe",
  "app": "C:/absolute/herdr-app.exe",
  "core": "C:/absolute/hc.exe",
  "node": "C:/absolute/node.exe",
  "bun": "C:/absolute/bun.exe",
  "chrome": "C:/absolute/chrome.exe",
  "omp_cli": "C:/absolute/runtime/omp.cmd",
  "pi_cli": "C:/absolute/runtime/pi.cmd",
  "omp_package": "C:/absolute/runtime/node_modules/@oh-my-pi/pi-coding-agent",
  "pi_package": "C:/absolute/runtime/node_modules/@earendil-works/pi-coding-agent",
  "omp_state_extension": "C:/absolute/owned-omp/herdr-omp-agent-state.ts",
  "pi_state_extension": "C:/absolute/owned-pi/herdr-agent-state.ts",
  "toolkit_baseline": "C:/absolute/original-toolkit-checkout",
  "versions": {"herdr":"0.9.5","app":"0.1.16","core":"0.1.21","node":"v24.18.0","bun":"1.4.4","omp":"19.0.0","pi":"1.5.2"}
}
```

两 `.cmd` 必须解析到声明的同一目录下的实际 npm 安装。按发布版本取两份官方 TUI 状态扩展；可把 HOME／USERPROFILE、APPDATA、LOCALAPPDATA、`HERDR_CONFIG_PATH` 指向自有目录并分别设置独占 `PI_CODING_AGENT_DIR` 或 `OMP_CODING_AGENT_DIR`（OMP 还需建 `agent/extensions`，不要同时选 Pi 目录），再从安装二进制运行 `herdr integration install pi`／`herdr integration install omp`。Runner 不安装宿主、不找用户认证；业务只访问数值回环的自有机械文本 provider，证明原生模型请求、Skill 展开与原生历史，不证明真实模型判断。App 四组合格中的两个 App 格把 TUI 扩展路径明确指向未安装文件，不加载 TUI 扩展。

- 专项保留 frozen 依赖、页面和安全后端构建以及各仓独立原始日志。Toolkit 全量与受影响 lifecycle/connect-bot docks 只接本轮 `safe_shutdown_shell.exe`；`HARNESS_ACCEPTANCE_EVIDENCE_DIR` 将安装版本证据留在本轮目录。生产后端仅接自有实例上的产品 HTTP／页面，不触发系统关机。
- 默认名 `bot-playground`、另一个名字、zone 名称对应“当前”、旧配置省略 backend、仅切换 backend、core-local 地址碰撞却属于另一个实例分别取证。独立观察真实 Tab／Pane、core ID 与终端标签、实际 agent 进程和 cwd、模型/thinking、Suite 及固定扩展运行、命名模板、Skill 展开和原生文件，再与公开状态核对。
- 公开 framework 订阅和真实启动钩子记录 OMP composer 闸门与 Pi 初始化。原生确认在 framework ready 且宿主启动命令返回后打开；只延迟转发生产 ready 上报来确定提交时序，不更换 editor/handler，不改原生状态或强制输入。TUI 的 Pi 确认使用与生产工具相同的等待通报（`herdr:blocked`），由已安装官方扩展处理；确认可见且 `herdr agent get` 在目标 Pane 上报告 `blocked` 后才放行 ready。任意第三方裸 Pi 确认不一定会上报：Herdr 0.9.5 在官方扩展接管状态时会跳过屏幕检测，未识别的确认不受 `agent prompt` 的原生 blocked 防护；该场景不能冒充可识别的发送前拒绝。App 保留不借助 TUI 状态扩展的原生路径。保留实际拒绝进程的完成时间和同一目标下一次调用时间；仅显式 Escape 关闭自有确认。六次提交尝试均被拒绝、关闭／换原生目标、原页面刷新分别核查外部提交次数与原生记录。必要的断线、部分写入、超时及模糊回执由命名控制边界覆盖；`started:false` 另用真实发送后的受控回执，分别保留原始 core 响应与业务历史，不以没有即时历史推断未提交。
- `--native-only` 使用已经构建的同一 worktree 产物；`--checks-only` 仅跑独立回归；用 `--combinations pi-app --cases app-modal-release` 选择诊断格。所有子集均保留缺格、非零退出，不能替代整票。`summary.json` 区分失败和未验证；每格另有原生记录、HTTP、运行日志、可取得的页面截图和所属进程退出证据。保留独占状态根供复核；确认对应进程实际退出后才可删除，不用目录可删除或旧实验代替退出或当前验证证据。

### 同一历史 Chat 的双端恢复（S1／S3）

从参与的 `herdr-bot` worktree 根运行。先在 `frontend` 与 toolkit 根各执行 `bun install --frozen-lockfile`（Bun 1.4.4），然后在 PowerShell 中构建并启动专项：

```powershell
$env:CARGO_TARGET_DIR = 'C:\absolute\exclusive-ticket-cargo'
$toolkit = 'C:\absolute\toolkit-worktree'
$backend = Join-Path $env:CARGO_TARGET_DIR 'debug\herdr-bot.exe'
$paths = 'C:\absolute\isolated-history-paths.toml'
$evidence = 'C:\absolute\new-history-evidence'
Set-Location -LiteralPath $BOT
npm --prefix frontend run build
if ($LASTEXITCODE) { exit $LASTEXITCODE }
cargo build --manifest-path backend/Cargo.toml --locked --bins --example safe_shutdown_shell --example recovery_herdr_proxy
if ($LASTEXITCODE) { exit $LASTEXITCODE }
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-paired-history-recovery --toolkit-worktree $toolkit --backend $backend --paths-file $paths --directions tui-to-app,app-to-tui --evidence-dir $evidence
```

`$paths` 是另建的 TOML，只声明安装位置，不复制任何 agent 的常用配置、历史或认证。将下列绝对路径替换为本机实际安装值：

```toml
[roots]
herdr = 'C:/absolute/installed-herdr-directory'
herdr_app = 'C:/absolute/installed-herdr-app-directory'
herdr_core = 'C:/absolute/installed-core-directory'
omp_package = 'C:/absolute/installed-omp-package'
pi_package = 'C:/absolute/installed-pi-package'
omp_state_extension = 'C:/absolute/official-herdr-omp-agent-state.ts'
pi_state_extension = 'C:/absolute/official-herdr-agent-state.ts'
node = 'C:/absolute/node.exe'
bun = 'C:/absolute/bun.exe'
omp_cli = 'C:/absolute/omp.cmd'
toolkit_baseline = 'C:/absolute/original-toolkit-checkout'
```

需要 Windows 交互桌面（自有 TUI 客户端与真实 App 窗口）和可运行的 Chrome。`omp.cmd`、`pi.cmd` 必须可从声明的运行时目录解析到同一套已安装包。`toolkit_baseline` 只解析 toolkit 基线记录中的外部路径，运行的源码仍取自 `$toolkit`。专项核对本次后端的 Cargo depfile、版本、源码／锁文件及安装与产物哈希。业务 HTTP 使用生产后端；配对 dock 只使用旁置的 `examples/safe_shutdown_shell.exe`。整个调用仅在交互桌面和配对检查资源允许的窗口运行，不操作已有窗口或已有会话。已预先通过仓库检查时可用 `--native-only` 单独取原生证据；`--controlled-only` 只跑命名的后端 Resume 控制边界。`--agents pi`、`--scenarios cross-end` 等过滤与两个诊断模式始终保留缺格，非零退出，不认证整票。Toolkit 的标准入口可能独立刷新 `framework/docs/pi-acceptance.json`／`omp-acceptance.json`（即使专项其他格失败）。仅收集专项证据时，将本轮生成的记录备份到专项证据目录，再在独占 worktree 内恢复已跟踪基线；不把运行记录混入产品提交，也不丢弃已有修改。

原生专项用数值回环的自有机械文本 provider 只为初次发起生成可读历史，不访问真实模型或用户凭据；安装的 OMP／Pi、框架／connect-bot、宿主和后端走真实路径。先读取实际历史、保留文件头与原容器 ID；退出并持句柄核对旧 agent、旧端及 core 后，才配置另一端及第二个默认容器名。Resume 之后从实际新进程的 native `session_start` 取 SessionManager ID/file 和 `process.cwd()`，从安装包再读原 Chat 的正文，检查目标所属 Workspace/Space、Tab/Pane、core 与内部焦点。透明 CLI 记录器原样转发到本次安装，不替换宿主、原生上报或响应。记录参数只用于检查没有重放模型／思考覆盖、没有强加扩展、目录降级未指定 cwd，以及没有重发创建、启动或输入；不能用参数证明恢复、实际 cwd 或聚焦。两端回执都不证明 Windows 置前。更新来源 Task 的 Suite 和固定环境标记，观察最新扩展确实运行，并比较原生输入、历史和模型访问记录；独立的原生默认扩展观察器不向产品上报身份。

同端命中、跨端无匹配、人工同值、Git 主工作树与目录缺失、四种环境降级、原文件头和来源 agent 冲突均保留独立格子。Task 发起的 Chat 已记录原生文件时，再次登记不会覆盖历史 cwd；本次实际 cwd 由恢复回执和当前会话登记分别展示。未记录原生文件的普通接入继续按既有规则补充历史。未启动、未定位或原生恢复选择框仍待处理时，不将选择框、存在的终端或最近的回执计为成功，专项不替用户选择分叉或继续。人工同值／无来源只改变一次本格的自有历史输入，Resume 前后均保留原始字节，并核对公开记录、既有持久格式和原字段；同一 Windows cwd 的尾斜杠不算换目录。安装端若不能构造真实 Git 歧义或缺少实际退出记录，不用控制结果补格。App 没有可据以确认历史 checkout 的归属，默认 cwd 用同一 Space 的独立原生空白 Tab 取证。错误／畸形／空成功等确定性外部响应、被拒绝的启动／定位以及其他原生身份冲突另用 `chat_resume`、`chat_resume_environment`、`chat_resume_omp`、`chat_resume_pi`、`chat_resume_timeout` 的公开 HTTP＋控制 CLI 边界；它们不证明安装宿主通过。新证据目录包含原始错误、HTTP、native 布局／进程／退出、SessionManager 历史及各仓原始日志。每格另保留一个独占、短路径的临时状态根（见该格 `root`），可在证据归档后独立删除；仅根进程结束或目录可以删除不构成退出证明。CLI 边界回归可用 `python "$TESTER/scripts/accept-paired-history-recovery.test.py"`，它不启动宿主。

### Task 发起与初始输入（S1）

用独占绝对 `CARGO_TARGET_DIR` 运行 `cargo test --manifest-path backend/Cargo.toml --test chat_creation`。受控 TUI、App/core 的记录检查所选容器、固定扩展与 Suite、终端收到的原始输入，以及当前启动与目标的匹配；原生上报通过生产 `/api/sessions/events`，关机状态使用记录安全的替身，不调用系统关机。页面定向检查为 `npm --prefix frontend run test:browser:build -- chatCreation.spec.ts --workers=1`；受控上报独立于页面，浏览器只观察创建 HTTP 进度。以上不认证真实安装宿主、framework 或模型行为。

初始输入重试（P20、P24–P28、P50、P52）由 `chat_creation` 的 `initial_input_retry::` 及 `a_timed_out_submission` 通过真实 HTTP 和受控宿主验证：每次拒绝回执后的 0.999／1.000 秒、重复／乱序 ready、同目标最多五次重试（含首次共六次提交尝试）、退出／换会话／换进程取消，以及未知结果下的独立终端写入记录。`chatCreation.spec.ts` 对照页面等待、拒绝、未知和刷新时的实际创建／输入次数；`mobileWorkflow.spec.ts` 检查窄屏在六次明确阻塞后仍可查看现场、关闭弹窗，并确认没有输入写入。真实原生模态与终端另验。

Toolkit 的 `manager-dispatch-recovery.test.ts` 和 `uncertain-effects.test.ts` 用本工作树的安全关机壳及受控 Herdr 夹具回归 Run 的身份归属：同一原生会话已归属本 Run 的管理者或另一执行时，新的节点启动保留实际启动和身份事实，但不保存重复关联，也不提交业务输入。夹具通过隔离后端接收本次启动的身份和 ready 上报；这些故障场景不代表安装宿主实际产生了重复 ID。

### Run Chat 角色恢复（P42／S1）

用独占绝对 `CARGO_TARGET_DIR` 运行 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch --test workflow_start --test chief_delegation p42 -- --nocapture`。场景从公开发起／派发／委派产生真实关联，使用公开 Resume 与独立 Run／node／委派 HTTP 读回核对原身份、角色和数量，检查所选 TUI 或 App/core 的 argv 及终端输入记录。覆盖 OMP／Pi 的原角色、同 Task 普通 Chat、结束节点、子管理者双角色，以及配置错误或缺来源时的默认发现及未显式装配提示。另运行 `app_delegated_map_binds_manager_before_approved_input`，验证 App 的直接 Map 委派恢复仍保持原 Run／委派及管理者角色。既有 `roles::`、`end_node::` 和 `resume_restart::` 继续覆盖 TUI 的普通节点／结束节点、旧执行与错误归属。这些检查不认证实际安装宿主加载扩展、模型行为或跨端 Run 接管。

### 原生提醒专项（P47／P48）

控制边界：`cargo test --manifest-path backend/Cargo.toml --test chat_creation native_reminders` 和同目标的 `app_launch_uses_task_environment_and_selected_terminal_without_tui_files`。由公开 Chat HTTP 发起 OMP／Pi。TUI 独立核对 Pane 与加载的一份官方状态扩展；App 在 TUI 集成文件缺失时核对原生 agent argv 与所选 Space／Pane／core 终端。两端取完整控制 CLI 记录，确认 connect-bot 的 `working → awaiting-input → awaiting-input` 已在公开会话流可见，却未增加另一条宿主命令。控制记录不证明真实状态或播放。

**真实触发与配置。** 每轮使用独占的 Windows 交互桌面环境、HOME／APPDATA／XDG、agent 配置和会话目录、短路径的 `HERDR_CONFIG_PATH`／`HERDR_APP_HOME`、唯一 `HERDR_SESSION`／`HC_SESSION`。只连本轮宿主与 owned backend；每次只运行所选宿主。TUI 要连接一个可实际取得焦点的 TUI 客户端，不能只启动 headless server；App 要有连接到所选 core 的实际窗口，不能用 `hc` 代替窗口。保留已装 agent、宿主和 core 的版本，TUI 固定扩展按安装的官方入口接入；App 不用 TUI 扩展，OMP `terminal.programStatus` 启用，Pi 保持自动协商。

- **OMP 额外通知渠道：** 在本轮独占的 `config.yml` 中设 `completion.notify: "off"`，关闭 OMP 在状态上报之外另外发送的完成 toast（在 Herdr TUI 中它自身也会触发声音）。保留 `terminal.programStatus: true` 和官方状态扩展；仍由实际 agent 状态触发本专项测量的宿主提醒，不改任何用户设置。
- **TUI：** 写入独占配置的 `[ui.sound]`：`enabled = true`，设置两个可区分的本地 MP3 `done_path`、`request_path`（相对配置文件目录或绝对路径）。确认未设 `HERDR_DISABLE_SOUND`，且没有适用的 `[ui.sound.agents]` 关闭项。默认资源对照移除两个路径和通用 `path`；关闭对照只改 `enabled = false`。
- **App：** 在独占 `settings.json` 合并 `"sounds": {"enabled":true,"done":"done.mp3","blocked":"request.mp3"}`（文件相对 `HERDR_APP_HOME`，也可用绝对路径）。确认未设 `HERDR_APP_NO_SOUND`。默认资源对照移除 `done`／`blocked`；关闭对照只改 `enabled`。
- **完成轮次：** 通过产品的 zone → Task 发起 Chat，在初始指示或后续原生输入提交带唯一标记的纯文本请求，例如 `仅回复 REMINDER-<本轮标记>`。用本机受控 provider 收到请求后暂缓响应，先核对目标已 `working` 并切到所需焦点，再放行正常响应。每次用新轮次，分别在目标 Tab 可见且宿主窗口有焦点、目标 Tab 不可见时完成。首次初始化和已停驻的旧状态均不算触发。
- **需要处理：** 为 owned Run 配置一个 `auto_approved=false` 的普通 node，经真实管理者派发到所选宿主。在 node Chat 正常发起一次成果批准（受控 provider 可固定返回该原生工具调用）。确认完整原生选择框仍显示且所选目标确实进入 `blocked`；在目标可见且宿主有焦点时也应走 request／blocked 音效。保持框打开跨越多次采样，同一状态不应重复响；用原生 **No** 解开，单独记下之后可能发生的完成转换。不要直接调用 `pane report-agent`、写入 OSC、模拟批准或暂停状态处理来制造提醒。
- **对照：** 两端后台完成各预期一次 done，正在查看时完成不播；实际需要处理的转换各预期一次 request／blocked，保持原状态不再播；关闭声音后重复这些真实转换，状态仍应到达但不得播音。报告投递及 ready 本身不是提示音；若投递使接收 agent 实际开始一轮，单独归因于该轮的原生状态。不要用设置试听或显式 `notification show` 充当任何格子的状态输入。

**时序与取证。** 每格保留选择的配置与声音文件哈希，记录含 UTC 与单调时间的输入提交、焦点变化、真实 provider 进出、原生状态转换、提醒 UI、播放迹象及进程退出。TUI 以 `herdr pane get <Pane>`／`herdr agent get <Pane>` 和 `herdr api snapshot` 保存 Pane、状态与焦点，可在所选实例公开 socket／命名管道订阅 `pane.agent_status_changed`（Windows 无 `herdr api subscribe` CLI）。App 保存 `herdr-app ls`、`herdr-app status`、必要时 `herdr-app shot`，用布局中的 Terminal 地址关联 `hc get <地址> --json` 和 `hc events --terminal <地址> --json`；核对 core 实例身份、`herdr.space`／`herdr.tab` 标签与所选 Pane，不以短地址或成功回执独立作证。保留一个未触发的其他 Pane／终端作为归属对照。

可用的 Windows 证据：保留本轮宿主日志和播放器错误；若有进程／文件访问跟踪，将已识别的宿主 PID（或其原生音频子进程）与打开所选 MP3 的时间关联，区分一次播放和宿主自己的资源回退。App Windows 音频可在进程内播放，找不到外部播放器不是静音证据。可用 Windows 音频端点的 WASAPI loopback 采集带时间基准的 WAV，记录设备、采样方式、其他音源与起止时间；不要让录制界面抢走被测焦点。缺设备、访问跟踪或录音时分别保留缺项；打开文件、Toast、截图或静音观测均不等同于可听。实际宿主自动取证按配对验收执行；两端音色与可听性仅在最终听音环节由用户确认，不预先判通过。

### 配对精确关闭与原生提醒（P43–P45／P47–P48／P54–P55）

在 Windows **交互桌面**上，从参与的 `herdr-bot` worktree 根执行。使用 Bun 1.4.4、已安装的 OMP／Pi、Herdr TUI／App／core、Chrome、FFmpeg；不要占用其他真实宿主验收的桌面时段。该专项会聚焦属于本次运行的两个宿主窗口。无需模型服务或用户认证：只用数值回环的受控 provider 延迟实际请求，或让实际 agent 调用已安装的原生 `approve` 工具。它不代写宿主状态，不走设置试听，也不执行系统关机。

配置 JSON 的值全部为显式绝对路径；准备 `herdr`、`app`、`core`、`node`、`bun` 可执行文件，`omp_package`／`pi_package` 安装目录，`omp_state_extension`／`pi_state_extension` 两个官方 TUI 扩展文件，和 `toolkit_baseline_root`（可解析历史 baseline 的原 toolkit 根，不作源码替代）。产品从 `PATH` 启动 OMP／Pi，入口确认 shim 对应声明的安装。`sound` 对象包含 `ffmpeg`、可区分的 `done`／`request` 自定义 MP3 和四个捆绑音效解码对照 `tui_done`、`tui_request`、`app_done`、`app_request`；从各安装版本的发布源码取 `assets/sounds/{done,request}.mp3`，不改上游。可独占创建自定义探针，例如 `ffmpeg -f lavfi -i sine=frequency=683:duration=0.65 done.mp3` 和 `ffmpeg -f lavfi -i sine=frequency=1129:duration=0.9 request.mp3`。录音分析使用 `PyAudioWPatch==0.2.12.8` 和 `numpy==2.2.6`，可通过 `python -m pip install --target <独占绝对目录> PyAudioWPatch==0.2.12.8 numpy==2.2.6` 准备，设置 `PYTHONPATH` 到该目录。要求 Windows 默认输出支持 WASAPI loopback；关闭其他音源，不切走焦点。没有音频设备、测不到波形或缺资源时保留原始错误，不以静音判通过。

```powershell
# 下列变量均改为本次绝对路径。target 必须是新建或空目录；证据目录必须不存在。
$ToolkitWorktree = 'E:/worktrees/toolkit'
$IsolatedConfig = 'E:/acceptance/paired-close-reminders.json'
$Evidence = 'E:/acceptance/paired-close-reminders-new'
$env:CARGO_TARGET_DIR = 'E:/acceptance/paired-close-reminders-cargo'
$Backend = Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe'
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-paired-close-reminders `
  --toolkit-worktree $ToolkitWorktree `
  --backend $Backend --isolated-config $IsolatedConfig `
  --close-cases all --reminder-cases all --evidence-dir $Evidence
```

- 入口保存两仓 frozen 安装、页面与本后端／安全关机壳构建、三个 herdr-bot 回归入口（后端限制为两个测试二进制并发，浏览器使用一个 worker，不缩小测试范围）、toolkit 四个入口及 `session-ready`／`binding-lifecycle`／`activity-facts`／`round-facts`／`native-node-report`／`node-report-contract` docks 的原始日志。配对 dock 的 `HERDR_BOT_BINARY` 指向同一独占目标目录中的 `safe_shutdown_shell.exe`。Toolkit harness 验收继续读取版本基线，只向本次证据目录的 `toolkit-harness-evidence/` 写检查结果，不改受版本管理的验收记录。失败继续留档，不用无关的绿项补齐；可按上述各入口的实际日志区分构建、测试和清理失败。
- 关闭用真实 Pi 创建经理／节点，由公开审批、报告、节点完成触发。透明记录器只转发已安装 CLI 的请求和原始回执。对照额外 Tab 和另一容器的存活终端、最后一个 Tab 消失及其容器实际移除；App 在前置负对照中验证未确认请求不产生效果，实际完成必须只发送 `tab.close` 且 `confirmed: true`。TUI worktree group 独占创建，父 Workspace 与关联 checkout 的唯一真实 group 必须可见，拒绝扩大时没有发出 close。TUI 用 `herdr pane process-info --pane <Pane>` 取得进程身份；另核对终端和 SDK agent 进程的真实退出，不把布局或成功回执误作 PID 退出。
- 提醒按 OMP／Pi × TUI／App × 自定义／捆绑／关闭运行，每格在实际 `working` 时切换焦点，再让同一真实轮次完成；后台完成观察一次 done，前台完成观察抑制。TUI 在连接客户端前按独占标题认领实际窗口，此后按窗口句柄和所属 PID 核对焦点。原生状态出现后继续录音四秒，覆盖 Windows 延迟启动播放器和完整捆绑资源；静音对照还检查信号幅度，避免把未匹配的声音计为静音。真实待审批 UI 可见且保持时观察一次 request，选择原生 No 后单独留档。公开会话状态、Pane/terminal 原生观察、CLI 请求、输入／释放时间、两种音效原文及哈希、WAV 和波形匹配时间分别保存；波形匹配不是用户的听音结论。
- `--close-cases app-pi-last-tab --reminder-cases none --native-only` 或 `--close-cases none --reminder-cases app-omp-custom --native-only` 用于已有本 worktree 构建产物的定向诊断；所有未运行格仍在 `summary.json`，定向诊断始终不返回全量通过。运行会保留未确认退出或证据归档失败的独占环境，供定位；不操作用户会话。`automatedPassed` 只代表自动范围，唯一人工项 `P48Listening` 始终待确认：用每格时间线与 WAV 核对两端实际可听和触发场景，仅由用户确认，不能提前计为通过。

### 配对 chief 与 Run 真实链路（P23／P37–P42／P52／P54–P55）

在 Windows 交互桌面、参与的 `herdr-bot` worktree 根运行。准备可运行的 Chrome、Bun 1.4.4 及实际安装的 Herdr TUI／App／core、OMP／Pi；只在桌面与检查资源独占时运行。隔离配置是一个 **不含认证** 的 JSON 对象：`herdr`、`app`、`core`、`node`、`bun` 是各已安装可执行文件；`omp_package`、`pi_package` 是 PATH 上的 `omp.cmd`／`pi.cmd` 对应的包目录；`omp_state_extension`、`pi_state_extension` 是与 TUI 版本对应的官方扩展文件；`toolkit_baseline_root` 是解析 toolkit 历史基线外部路径的原仓根，不替换本次 worktree 源码。配置的每个值均为绝对路径。

```powershell
$toolkit = 'C:/absolute/toolkit-worktree'
$config = 'C:/absolute/chief-run-isolated-installations.json'
$evidence = 'C:/absolute/chief-run-new-evidence'
$env:CARGO_TARGET_DIR = 'C:/absolute/chief-run-new-cargo-target'
$backend = Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe'
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-paired-chief-run `
  --toolkit-worktree $toolkit --backend $backend `
  --isolated-config $config --scenarios all --evidence-dir $evidence
```

完整调用要求新的证据目录和新的或空的绝对 Cargo 目录。先按冻结锁文件安装两仓依赖，构建页面、生产 backend、配对安全关机壳和 CLI 记录器，再运行本页三个 herdr-bot 入口、toolkit `typecheck`／`test:framework`／`check:integrations`／`test` 和 connect-bot、报告与 Run 角色 docks。关机相关 dock 只用同次构建的安全壳。原始入口日志、后端分目标和 Playwright 日志，与 `summary.json` 的命令、来源和独立退出码一起归档。可以用 `--scenarios tui-omp,app-pi --native-only` 定向复跑已有构建，或 `--checks-only` 单独复跑仓库检查；两种模式均保留缺格且不认证整票。

- 每格启动实际所选 Herdr 和 OMP／Pi、生产 HTTP、toolkit 和实际 TUI 客户端／App 窗口。只操作本格创建的容器、Pane、任务、Run、HOME、APPDATA、XDG、agent 和 core；不读用户认证，不接现存 Chat。每格保存真实安装指纹、公开与持久关联、原生历史、终端调用的请求／处置／原始回执及所属进程退出证据。`nativeRoot` 另保留本次独占短路径状态根，供复核后删除；留存目录不是进程退出证明，未确认退出的格不会通过。
- 第一条业务消息经实际宿主终端提交。外部 CLI 门闩在转发前读回 Chat／原生 header／Run 或委派关联，正常调用不改参数或回执。有分派的普通 Chat 第一次明确拒绝时不调用宿主，下一次通过实际宿主，同一目标／原文只有一次进入历史；比较独立创建、发起和输入次数。原分派 chief 保留为同名存活对照，更换当前 chief 后核对实际输入与报告读回。受控故障分别拒绝 chief／Run 通知，确认报告已保存且观察 46 秒无重投；成功业务和失败证据分开记录。
- 聚焦以所选宿主原生布局与焦点为准，不声称 Windows 置前。通过原生输入正常退出旧 agent 后再恢复；App 另核对实际会话自行发出的 `launch_ended`，不替它上报。App Pi 用原生 `/new` 先离开旧 Session、确认旧 owner 撤回后再退出；此格只证明已撤回 owner 后的恢复，不认证强制关闭或未送达退出上报后的旧 owner 回收。原生 Resume 后用真正的 manager／node 扩展工具读 Run 和交接，检查原执行；不做跨端 Run 迁移。另以明确聚焦失败核对没有复发 Resume，删除自有来源 Task 后检查降级提示、无显式角色装配和原 Run 不变。独立 CLI 门闩回归：把 `PAIRED_NATIVE_RECEIPT` 指向本次 `debug/examples/paired_native_receipt.exe` 后执行 `python "$TESTER/scripts/accept-paired-chief-run.test.py"`。
- 场景使用数值回环的受控机械 provider 和受控外部拒绝。它能驱动真实扩展工具、持久报告和手动推进的生产机制，不证明真实模型对需求的理解、结果质量或外部传播。该命令不触发真实模型网络；只有逐次明确授权的配置才能用于另外的真实网络验收，缺授权和未跑的原生格子均不能由全仓绿补齐。

### Chief 委派与报告（S1）

先构建本 worktree 的页面，用独占绝对 `CARGO_TARGET_DIR` 运行 `cargo test --manifest-path backend/Cargo.toml --test chief_delegation -- --nocapture`。公开 chief／Chat／历史／报告 HTTP 配合独占 TUI、App/core 记录确认委派授权、发起目标和外部输入；业务输入门闩期间读回已有原生身份的历史与委派关联。更换当前 chief 后保留原 Tab，从真实通知目标及报告读回确认路由。记录型宿主拒绝通知时，保留原报告，并在至少 46 秒窗口核对没有额外输入、重投或复发派；这不代表真实安装宿主业务链路已验收。

### 运行前提

- 工具与依赖安装见 [构建与运行](../../README.md#构建与运行)。后端测试也需要 Node.js，供跨前后端网络护栏使用。
- 浏览器验收需要本机 Chrome。源码更新后使用 `test:browser:build`，由它先构建前后端和安全关机壳再执行验收；仅调试已构建产物时使用 `npm --prefix frontend run test:browser`。
- 不同源码副本使用独立 `CARGO_TARGET_DIR`；测试程序会内嵌源码路径。性能比较顺序运行，避免多轮检查争抢同一资源。
- 原生可见性浏览器清理分别确认根进程和独占 profile 的进程退出。Node `exit` 事件或独立 Windows 句柄可确认根进程退出；随后观察进程须在独立时限内确认携带该 profile 路径的残留 Chrome utility 进程退出，才允许删除 profile。`taskkill` 返回码或输出不能代替退出确认。

## 动态 TypeScript 脚本验证

运行环境须已有 Bun；隔离夹具复制实际 `meta-data/scripts` 到临时 Bot 目录旁。`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch typescript_ -- --nocapture` 通过真实 HTTP、Bun 子进程和实际文件验证任意用户脚本、相对 import、日志、最小状态、异常退出、缺输出／文件、目录假输出、重复／未声明 ID 和失败部分效果。原 `rust_create_`／`rust_save_` 测试名保留，但执行的是迁移后的真实 TS 业务，检查严格材料、revision、并发保护、完整读回和交接。沿用既有独占静态页、绝对 CARGO_TARGET_DIR 与宿主进程清理规则；这些测试不调用真实模型。

## 四字段 Bot 定义与 backup 隔离

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch bot_targets::` 通过公开保存、编辑、预检和独立文件核实四字段、草稿、目标互斥与路径、未知／旧字段及 revision 拒绝后字节不变。`cargo test --manifest-path backend/Cargo.toml --test runtime_data bot_and_map_backup_stays_offline_after_real_startup` 在真实进程启动前隔离放置备份定义，启动后交叉核对在线目录、详情、预检、备份字节；既有历史布局的标记隔离另由 `definition_migration_preserves_bytes_and_never_reimports_retained_backups` 验证。`npm --prefix frontend run test:browser:build -- schedulerBotEditing.spec.ts --workers=1` 在真实页面编辑并重新加载，和公开 HTTP、实际配置文件交叉比较；受控传输分别保留明确拒绝与提交结果未知。隔离 backup 不代表部署已移动；不执行任何实际切换。

## 脚本表示验证

`python -m pip install -r scripts/requirements-contracts.txt` 安装单独的 Schema 验证依赖后，`python scripts/test-script-contracts.py` 从公开文件 seam 消费 `docs/contracts/script-bots.schema.json` 和独立 JSONL 字面实例。离线检查类型、必填、互斥、相对 TS 路径、完整配置形状、无 Chat 来源与失败效果表达；不启动宿主／服务、不写业务配置，不以源码快照验证接线。跨材料身份／revision／正文确认及真实保存仍是业务实现的公开 HTTP／文件验收，不能由 Schema 通过推断。

## TypeScript 空白创建公开 HTTP／文件回归

```sh
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch rust_create_ -- --nocapture
```

用本轮构建的页面、独占 Cargo 目录及已安装 Bun。选择器保留原名，实际运行用户维护的 TypeScript 创建脚本。核对 Task／Map 目标、公开完整对象和 revision、实际文件与下游命名材料；独立读回在途状态、冻结输入与完成门禁。Map 由定义文件提供，不是 HTTP 新建。输入拒绝不产生成功材料；写入已生效但读回或材料生成失败时，保留实际部分效果，不回滚、不提交可交接报告。受控外部宿主不证明真实模型或原生工具消费。

### 脚本报告通知失败隔离（P11）

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch rust_create_saved_report_survives -- --nocapture` 对实际 TS 创建分别观察已保存报告、命名输出、文件和外部通知失败。保留原报告，不自动重投；有限观察不表示 manager 已读或保证无限期无动作。

## 脚本 workflow 页面（P31／P5／P32 运行分区）

`npm --prefix frontend run test:browser:build -- workflowScripts.spec.ts --workers=1` 使用独占生产后端、真实 Map 和 Bun 脚本。分别核对运行中、成功和部分材料失败的页面与公开读回。观察不发起动作；无 Chat 的脚本执行及报告没有 Resume 或关闭入口。受控 Herdr 不证明真实宿主消费。

## TypeScript 四字段合并保存公开 HTTP／文件回归

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch rust_save_ -- --nocapture` 执行真实 Bun 保存。比较实际文件、完整 Bot／目标及新 revision。用独立原文验证 CRLF、LF、空行、tab 和首尾空格；下游只从声明的 ID 读取，handoff 不带正文。并发修改、过期 revision 或错误绑定应拒绝。已保存但材料失败时保留部分效果，不回滚或刷新 baseline。具体输入和授权见[共同业务材料契约](../business-materials.md)。

## 发布创建图迁移回归（P22／P23／P27／P28）

用本轮构建的页面和独占 Cargo 目录运行 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch create_migration_ -- --nocapture`。发布 V1 Map 通过 Bun 执行创建和保存。核对命名文件、公开完整读回、revision、报告及显式 complete／finish；通过独立预期确认无关定义不变。这不证明模型遵从指引。原生材料消费和业务确认另跑下文创建及修订入口。修订可追加 `--cases task-no-materials` 定向复跑通用保存，不能代替完整矩阵。

## 发布成果批准职责与 V1／V2 独立性（P17／P31／P34／P35／P38）

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch create_migration:: -- --nocapture` 比较发布 Map 实际配置与独立的批准模式和结束节点预期。按 [运行时批准协议](../workflow-runtime.md#角色责任与批准动作矩阵) 判断职责，不能从旧报告开关机械推导批准模式。

V2 保留专用 authorization／confirmation 材料，独立运行并比较实际文件、完整公开对象及 revision。成功报告本身不能批准或完成节点。保留显式批准、complete／finish 的独立证据，并确认 V1 Run 和 request 不变。受控宿主与 HTTP 的人工来源不证明原生 Yes 或真实模型遵从。

## 后端并行与诊断

`node scripts/test-backend.mjs` 不带参数即可并行。后端集成测试按领域放在 `backend/tests/` 子目录，由 `backend/Cargo.toml` 的 `[[test]]` 显式登记，`autotests = false`。新增独立 target 时登记唯一的 `name` 与 `path`；现有 target 内新增 case 文件时由该 target 的 module 声明纳入。目录移动不改变 target 名称、二进制执行粒度、默认 ignored 状态、example 编译检查或 doctest。

- 资源紧张时用 `--binary-jobs 2` 降低外层并发。
- 排查跨二进制并发问题时用 `--binary-jobs 1`，并与 `cargo test --manifest-path backend/Cargo.toml` 对照；需要内部也串行时再加 `--test-threads 1`。
- 外层并发不限制 Cargo 编译并发；内部线程、环境继承及日志目录选项以 `node scripts/test-backend.mjs --help` 为准。
- 构建失败不启动测试；二进制全部成功才执行 doctest。首个失败或交互终端正常 Ctrl+C 后停止派发，等待在途目标结束，整体返回非零。
- 各目标日志与 `summary.json` 保留在入口打印的目录中。区分失败、取消、未派发和 doctest 未执行；部分完成不是完整通过。强杀进程或关闭终端不保证清理。

Windows 子进程保留普通启动方式；分离进程会改变 Git/MSYS 行为。正常 Ctrl+C 由终端输入处理，保留在途测试的清理机会。OMP／Pi 的临界场景在公开状态进入初始化后使用真实时钟，第 29 秒发布原生 header 和匹配的启动事件，核对事件前没有初始输入。缺失、无效或过期的启动证据阻止输入；写入历史失败仍不会撤销已提交的输入。

### 本次输入与消息分层（P1、P2、P4、P8–P10、P29、P33–P36）

`workflow_dispatch` 的 `input_preparation::` 检查公开 `mapInput` 与持久文件、全量替换、必需文件在准备及派发时的就绪门禁、已派发地址冻结；`dispatch_receives_background_direct_addresses_and_run_scoped_material_location` 同时观察独立实际提交、条件式文档入口和定义原字节不变。`roles::`、`end_node::`、`p42` 分别核对来源 Task 的独立用法、实际节点角色、原关联恢复和不重发。`workflow_start` 及 `chief_delegation` 的 Map 场景覆盖直接与委派发起；`missing_required_child_material_does_not_block_manager_but_blocks_work` 对照子 Run 准备门禁。`partial_child_submission_distinguishes_rejected_and_unknown_without_replaying` 及 chief Map 启动故障分别保留持久关联和受控宿主的实际提交记录，不将未知回执当未发送。

页面：`npm --prefix frontend run test:browser:build -- workflowStart.spec.ts workflowCurrent.spec.ts --workers=1`，对照表单发起、冻结图、实际输入和重读/刷新外部调用。原生安装与业务遵从另验，不由受控宿主记录替代。

### Run 冻结 Map 与历史保留（P11–P14、P30、P32、P55）

定向后端：`cargo test --manifest-path backend/Cargo.toml --test workflow_start manager_trigger_and_status_follow_committed_identity -- --exact` 检查副本先于外部启动及管理者实际收到的绝对路径；`initialization_and_identity_failures_stop_before_initial_instructions` 检查真实保存或外部失败。`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch frozen_map::` 检查旧／新定义、破坏副本及原样隔离归档；同目标的 `nesting::child_freezes_its_own_latest_map_without_freezing_referenced_bots`、`map_deletion::` 和 `history_integrity::deleting_source_maps_preserves_both_sides_of_nested_history` 检查子 Run 独立冻结及删除原定义保留整份历史。

真实页面：`npm --prefix frontend run test:browser:build -- workflowCurrent.spec.ts workflowRefresh.spec.ts workflowHistory.spec.ts --workers=1`；另用 `npm --prefix frontend run test:browser -- schedulerNodeDeletion.spec.ts --workers=1 --grep 'P30 deleting a Map'` 核对实际删除请求。新公开输入或成果门禁加入后，复跑同一冻结反例和显示场景，不用旧通过记录代替。隔离归档不代表执行过现场切换。

### 普通节点成果批准（S1）

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch approval:: -- --nocapture` 从公开 HTTP 分别检查批准、最新报告及约定交付三项门禁。夹具通过 `node/context` 和 `node/approve` 显式建立批准，不在 report 或 dispatch 中静默补齐。无交付约定时用空 `output_rule`；不能用旧 execution 或另一节点的输出补足。脚本仅在本次成功并通过输出校验后自动批准。人工 HTTP 只证明后端保存，不代替原生 Yes。

默认值及非法保存用 `report_policy::map_auto_approved_defaults_round_trips_and_rejects_invalid_saves_without_writing`（历史选择器名），结束节点用 `end_node::`。报告保存、独立通知和诊断失败见本页报告回归。

### 嵌套父节点整体批准（S1）

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch nested_completion:: -- --nocapture` 检查子 Run finish、父 execution 整体批准和父 manager complete 三项独立动作；内部批准不替代整体批准，整体批准不自动推进。`nesting::` 和 `nested_retry::` 分别检查真实父子归属与新旧执行隔离。公开 HTTP 和受控 Herdr 回归不证明原生确认。

### 成果批准 Map／Run 页面（S3）

`npm --prefix frontend run test:browser:build -- nodeSettings.spec.ts scriptDefinitions.spec.ts workflowScripts.spec.ts endNode.spec.ts --workers=1`。比对真实页面与公开读回，区分当前定义、冻结执行模式、批准来源、报告和累计输出。脚本只支持自动批准，结束节点不适用批准。保留无写入观察和实际关闭结果；受控 Herdr 和 HTTP 批准不证明原生 Yes 或模型行为。

### 真实模型成果交接与结束节点（S2）

从参与的 herdr-bot worktree 根运行：

```sh
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-node-approval-handoff --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$AUTHORIZED_MODEL_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"
```

参数必须是显式绝对路径，证据目录必须不存在；`--backend` 是本 worktree 生产 `herdr-bot.exe`，旁置同次构建的 `herdr-paths.exe`、`herdr-fixture.exe` 与 `examples/safe_shutdown_shell.exe`。使用独立绝对 `CARGO_TARGET_DIR`、两仓现有 frozen-lock 依赖及 Bun 1.4.4。paths 沿用共享原生验收设施的只读 `herdr`、`omp_package`、`pi_package`、`omp_state_extension`、`pi_state_extension`。授权模型 JSON 使用 `provider/model/baseUrl/apiKeyEnv`，可选参数沿用共享模型传输设施；不限定模型产品，不发现用户认证、不安装宿主、不回退受控响应、不重试模型业务决策。缺授权保留四格未验证并非零退出。

已明确授权某个只读 auth 文件时，可用既有 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-codex --scenario node-approval-handoff --hosts omp,pi --backend "$HERDR_BOT_BINARY" --toolkit "$TOOLKIT_WT" --paths-file "$ISOLATED_PATHS" --pi-package "$PI_PACKAGE" --auth-file "$AUTHORIZED_AUTH_FILE" --evidence-dir "$NEW_EVIDENCE_DIR"`。先用同一命令加 `--smoke`、不同的新证据目录独立核对 `serverModel=gpt-6-luna`。该既有传输仅支持 canonical `gpt-6-luna`，这是传输／本次授权边界，不是产品模型限制；不复制、刷新或修改认证，不将凭据正文写入证据。业务阶段保留真实 SDK 原生调用决策与安装宿主执行；不声称两宿主直接使用 Codex wire。专项不把烟测通过当作业务四格通过。已独立完成全仓检查时，可在交接专项或该 Codex wrapper 追加 `--native-only`，仅运行原生场景以避免重复全仓检查；可用 `--hosts omp --cases auto` 或 `--hosts pi --cases manual` 选择单格，`--cases` 默认为 `all`，仍运行所选宿主的自动／人工两模式。过滤或 native-only 始终保留完整覆盖清单中未验证项、`passed=false` 与非零退出，不单独认证整票，须与本次未受影响源码的全仓结果分别交付。

交接专项的模型身份检查仅将 `POST /chat/completions` 计为真实 completion，并在现有 30 秒请求预算内观察在途请求完成；其他流量原样保留并单独计数，不按 HTTP 状态筛除失败。真实 completion 的错误、缺失／错误模型身份及零 completion 均拒绝通过。离线边界回归为 `NODE_APPROVAL_HANDOFF_TOOLKIT="$TOOLKIT_WT" python "$TESTER/scripts/accept-node-approval-handoff.test.py" ModelIdentityBoundary`，不代表真实模型业务通过。

业务补充边界接受正常回答或合法的原生 Ask 表单，不要求 assistant 必须先 stop，也不要求先 report 再 Ask。表单必须归属当前 owned Session／同一冻结、未批准 execution，且仅询问本场景已知的折扣；使用安装宿主的 HTML parser 识别字段，经 owned pane 实际填写 25 并原生 Submit，再核对透传 `formDialog` 返回与实际 Ask `toolResult.details.answers`。已回答、错误、未知或歧义字段均不重填，不 Esc、不用普通 prompt 绕过 blocked UI，也不要求模型改用文本。`node-approval-observer.mjs` 只透传记录公共 UI／事件，不提供返回值或批准结果。离线回归为 `NODE_APPROVAL_HANDOFF_TOOLKIT="$TOOLKIT_WT" python "$TESTER/scripts/accept-node-approval-handoff.test.py"`，以及 `NODE_APPROVAL_HANDOFF_OMP_PACKAGE="$OMP_PACKAGE" bun test "$TESTER/scripts/node-approval-form.test.mjs"`；后者使用真实安装的 parser／表单组件，但没有模型或原生宿主运行，不是 S2 业务证据。

按已批准承诺逐项组合原始事实：P20 要求双宿主两模式的真实正常成果批准后报告、来源与独立成果，并保留进度可先报告的实际证据；P30-S2 要求双宿主原生结束节点及真实 finish，不额外要求每种普通节点批准模式重复 P30。保留各场景原始失败、未完成的 manager 推进与内部子标记；某格后续 finish 未发生不能抹掉它已独立实证的 P20，也不能补出该格没有发生的 finish。过滤场景或内部 16 子标记不是整体契约结论，组合结论必须引用实际工具、公开状态、材料、身份和清理证据，不能仅重写通过标记。

独占运行 OMP／Pi × auto／manual 四格。Map 明确声明 `order`，报价节点通过 `type=run` 读取真实订单。折扣可暂缺；未批准时保存原生进度报告。用户只补充 25% 折扣，不指定工具或顺序。独立预期为 7 件、单价 1200 分、实付 6300 分；核对真实 `quote.json`、命名输出、原生 approve 先于成果 report、真实回执和公开来源。人工模式实际选 Yes 并读回 manual，自动模式没有确认且来源为 auto。结束 Chat 等待发票抬头，不承担批准或报告；其他工作完成后 manager 显式 finish，结束 Chat 保持 running 且无批准或报告。使用覆盖 60 秒批准与 30 秒 HTTP 预算的 91 秒有界观察。

夹具输入回归为 `cargo test --manifest-path backend/Cargo.toml --test workflow_start handoff_order_requires_a_declared_map_input_before_manager_preparation -- --nocapture`；它不认证原生业务。

每格保留原生 SessionManager 历史、模型实际请求与返回身份、工具调用／结果、透传 HTTP、独立预期、真实材料、公开 Run、源码／锁文件／产物／安装身份、原始与清理错误。单宿主仅诊断；任一检查失败、coverage 缺失或清理失败，整体不得通过。入口顺序运行本仓 frontend build、指定 Cargo build、完整后端／前端／浏览器检查，以及 toolkit typecheck／test／check:integrations 和受影响 node docks；配对 dock 使用本次安全壳，绝不用生产后端执行关机回归。CLI 安全自测为 `NODE_APPROVAL_HANDOFF_TOOLKIT="$TOOLKIT_WT" python "$TESTER/scripts/accept-node-approval-handoff.test.py"`，只证明 fail-closed、旧证据保护、绝对路径和无模型产品限制，不证明真实模型遵从。

### 发布 V1 与非 V2 节点职责专项（S2／S3）

```sh
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-node-approval-v1 --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$AUTHORIZED_MODEL_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"
```

所有路径必须显式绝对，证据目录必须不存在。生产后端必须来自参与 worktree 的独立绝对 `CARGO_TARGET_DIR/debug`，不使用协调仓或旧产物。依赖按两仓既有 frozen lock 准备，Bun 1.4.4；paths 声明 `herdr`、`omp_package`、`pi_package`、`omp_state_extension`、`pi_state_extension`、`wayfinder`、`wayfinder_skills`、`pi_extensions`、`codebase_memory_mcp`、`chrome`，可选 `native_shell`。只读原生扩展与安装目录，不读取其 settings、models、历史或认证。模型 JSON 沿用 `provider/model/baseUrl/apiKeyEnv`，仅消费明确授权变量，不限制产品模型。

用户已经明确授权某个 auth 文件时，复用 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-codex --scenario node-approval-v1 --hosts omp,pi --backend "$HERDR_BOT_BINARY" --toolkit "$TOOLKIT_WT" --paths-file "$ISOLATED_PATHS" --pi-package "$PI_PACKAGE" --auth-file "$AUTHORIZED_AUTH_FILE" --evidence-dir "$NEW_EVIDENCE_DIR"`；先用同入口 `--smoke` 和另一个新证据目录检查真实返回身份。该传输只支持 canonical `gpt-6-luna`，每个实际 `serverModel` 必须匹配；不复制、刷新或修改 auth，不把传输烟测当业务验收。HTTP 200 的 `error`／`response.failed` 终帧保留白名单错误类别和请求结构计数，不记录原文；无终帧的超时保留已观察到的帧数与 SDK 状态，不推断上游错误或重发请求。

独占发布副本保留所有 node 的真实 Task、Suite、初始 Skill、宿主与业务职责，只重定位物理位置、传输及存储。两个 V1 Run 分别由 OMP／Pi 原生管理者协调，内部编写者／优化者仍使用原发布的混合宿主；不会将节点全改为管理者宿主。新 request 由真实模型生产，真实 Bun 创建与保存；保存只接收 baseline 和四份明确命名正文。完整 Bot／目标／新 revision、实际文件字节、原生确认真实返回及管理者显式 complete／finish 交叉核对，不能由成功文字或 Schema 代替。其余四张非 V2 发布 Map 也分别由 OMP、Pi 管理者独立执行。五张 Map 共十格；按管理者宿主分别记录 22 个发布节点的 S2 和 S3，加上两个宿主各自的 P31／P35，共 92 项。outer 中嵌套的 V1 子 Run 不能替代独立顶层 V1 格。Tibo 只读真实公开页面，临时目录调查仅指词典重定位的独占目录，不扫描用户 TEMP 或部署环境。两个调查节点没有命名输出约定，按各自保存的报告正文和真实来源验收；目录报告须给出项目绝对路径及 branch。静默观察保留所属 Session 的已接受 heartbeat 原始 wire 索引，不将其计为业务推进；其他写入仍拒绝，同时核对公开 Run、报告和待处理报告不变。

页面专项 `node-approval-v1-browser.mjs` 读取上述真实定义与真实 Run，逐节点比较独立字面模式、当前配置、冻结模式和实际来源，并保留截图／公开 JSON；不新增通用控件保存用例、不通过 HTTP 代原生批准。打开当前节点编辑器会以 `POST .../nodes/edit` 读取草稿；只接受当前 Map／node 的纯 `{id}` 查询，不把保存或其他写请求豁免为观察。打包页与业务判据分开收证；页面清理缺 Chrome 退出确认时不得继续派发。原生 Yes 只能作用于本次 owned pane，必须看到选中项并核对原始 UI true 返回；原生 Ask 业务问题不能通过取消表单或强迫工具顺序规避。外层创建 Task 可核对原生表单控件之前展示的完整候选，选项与默认值不充当授权；比较原生表单返回的选项值与 Ask 工具返回的可读标签，不将二者按裸字符串对比。单请求容量下，由真实 manager 逐个派发 V1 就绪节点（含嵌套 V1），不改发布依赖或模型并发。业务输入只供给当前 Run 及其真实子 Run；任一实际节点已失败时，本格不能以 retry 补齐原执行的覆盖，立即保留该节点、报告和当前公开事实，不继续等待整格超时。仍 running 的节点不按失败处理。顶层 V1、外层创建、修订和规划场景在各格独占的 MCP runtime/cache 中先索引参与 worktree 并核对 ready 与源码根，再启动业务；不修改共享索引配置。透明代理保留 manager collect 的原有 91 分钟原生传输预算，不按普通请求的 330 秒截断已开始的响应。规划场景通过公开 HTTP 准备和读回两个职责不同的隔离 Bot／Task，只规划并验证 `inspect → summarize` 的 `findings` 绑定，不运行生成的 Map。原生 Yes 前核对实际完整候选字节，不把目录中同字节的额外 proposal 文件都必须由该 actor 再次 read 当作批准前提；必要材料的完整消费仍按本 execution 的真实命名输入另行核对。

入口依次保存本仓 frontend build、指定 Cargo build、完整后端／前端／浏览器与 toolkit typecheck／完整 test／check:integrations、受影响 node-report／nested-role docks 的原始日志。paired dock 使用本次安全关机壳与本 worktree 元数据，不以生产后端执行关机回归。每次独占 HOME／APPDATA／XDG、原生存储、Herdr、动态数值回环及进程，保留两仓源码／锁文件／产物／安装身份、真实业务判断与原始／清理错误；确认所属进程退出后才能清理。完成后作 91 秒单调观察，覆盖 60 秒成果批准＋30 秒 HTTP 预算及 1 秒余量。缺依赖／授权、任一仓检查失败、单宿主、缺 coverage 或清理失败均非零且保持未验证，不回退受控模型、不重跑历史副作用、不改 V2 协议。原生资源窗口需与全仓检查分离时可显式追加 `--native-only`（直接入口及 Codex wrapper 均支持），只采集本次 native／页面证据，整票仍 `passed=false` 且非零，不替代两仓完整检查。在本 worktree 运行 `python "$TESTER/scripts/accept-node-approval-v1.test.py"` 验证选择器、覆盖清单及 CLI 拒绝边界；`python "$TESTER/scripts/node_approval_v1_matrix.test.py"` 通过实际本地子进程验证并发限额、按格归属、共享失败止发和清理。`python "$TESTER/scripts/node-approval-v1-behavior.test.py"` 验证完成／拒绝与对应 execution 的真实 HTTP 形状、页面候选归属、业务输入和 Ask 引用边界。这些本地检查不认证原生业务。

V1 入口及 Codex wrapper 支持 `--cases v1|outer|revision|planning|investigation`；例如 `--hosts pi --cases planning --native-only` 只选择 Pi 管理者的规划格。过滤调用保留全部 92 项，未选项不认证，整票仍非零。静态页和两仓检查只在父入口运行一次；每格独立进程、定义、后端、原生存储、模型流量和清理证据。汇总只认本次格子的宿主、case 和具名证据，不导入旧 ledger。Codex 矩阵的新证据目录放在两个 worktree 外或其 `.scratch/` 下，避免把在途证据误记成源码变更。

默认 `--jobs 1`。确有两个上游并发请求容量时，直接入口使用 `--jobs 2` 并在模型配置显式设置 `maxConcurrentRequests: 2`；Codex wrapper 使用 `--jobs 2 --model-capacity 2`。每个活动格最多占用一个上游 completion 槽，查询与非模型操作不占用它；不从 CPU 数量推断模型容量。Codex 格有独立 terminal 状态，认证丢失共享；普通业务或单格 HTTP 400 不重试、不污染兄弟格。确认共享资源失效或清理不安全时停止派发新格，在途格保留各自的结果。CLI／并发的离线测试不启动真实模型，不能证明 92 项业务通过。

传输离线验证：`python "$TESTER/scripts/workflow_completion_transport.test.py"` 使用回环 HTTP 检查在途请求、隔离故障和退出；`WORKFLOW_PI_PACKAGE="$PI_PACKAGE" python "$TESTER/scripts/workflow-codex-completion.test.py"` 使用测试自有凭据、安装 SDK 和截断在 fetch 的响应。HTTP 非成功时仅归档固定错误类别与转换后请求的结构计数；HTTP 200 后失败另记首帧、终止帧、事件计数和 SDK 结果。二者都不是业务交接；例如 manager 的 `409 output-missing` 应保留在该 execution 的业务证据里，不归为模型超时。

### 批准持久化、失败与执行隔离（S1）

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch approval -- --nocapture` 包含普通批准、`report_policy::approval_mode_is_frozen_recovery_keeps_approval_and_retry_reads_run_map` 及 `approval_restart::approved_execution_survives_actual_backend_exit_without_reclaim_or_input_replay`。冻结场景从公开派发、Map 文件修改、批准、fail/recover 到合法 retry，核对旧事实保留、新 execution 未批准且不继承报告资格；不再测试旧 reportPolicy 默认或兼容升级。现有 `retry::` 的执行／报告隔离、失败与材料回归继续保留。

`approval::approval_save_failure_preserves_original_error_and_cannot_authorize_complete` 用独占 Run 文件的 Windows deny-delete 句柄让真实原子替换失败；核对原生 OS 错误、HTTP 500、公开未批准及 ready 报告仍不能 complete。`repeated_and_concurrent_approval_has_one_fact_and_no_business_effects` 同时提交真实 HTTP，比较完整 Run／报告／外部调用；`approval_only_changes_the_callers_current_execution` 实际发起同 Task 独立 Chat、普通节点、另一 node／Run 及合法 retry，与根 manager／无关联身份对比。`material_changes_and_later_reports_keep_approval_without_bypassing_latest_report` 实际修改材料字节、追加 false／true 报告并验证独立完成门禁。

`inflight_old_approval_does_not_block_run_or_approve_retry` 是 **S1 传输门闩**：真实服务器返回 `100 Continue` 后，保持旧批准请求最后一个字节未提交；释放前公开查询、报告、独立 B 分支派发／完成及 A fail/retry 全部返回。查询、报告、分支派发／完成各 2 秒预算，释放后旧提交 2 秒内以陈旧执行拒绝，stdout 记录单调时序及实际调用。它不声称测试原生 UI 或后端 handler 内等待；OMP／Pi 真实确认在途属于原生专项。

重启场景在第一个真实后端进程中公开创建并批准 Run，kill 后 wait 确认退出、消费完 stdout，再启动不同 PID，经公开详情读回完整 execution、模式与来源；旧 manager／node 权限仍拒绝，外部调用无输入重放。每例独占定义、Run、zone、数值回环动态端口和受控 Herdr；真实后端子进程另隔离 HOME／APPDATA／XDG。原始失败与进程清理失败分别记录；确认退出后才清理。以上只证明 S1 机制，不替代原生批准、恢复或真实模型业务。完成检查仍分别执行本仓完整后端、前端和浏览器入口。

### 双宿主成果批准拒绝与继续工作（P5／P7／P8／P38）

从参与的 herdr-bot worktree 根运行：

```sh
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-node-approval-failures --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$AUTHORIZED_MODEL_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"
```

路径均为显式绝对路径，证据目录必须不存在。使用本 worktree 生产后端及旁置的 `herdr-paths`、`herdr-fixture`、`examples/safe_shutdown_shell.exe`，本次页面及独立绝对 `CARGO_TARGET_DIR`；两仓预装 frozen-lock 依赖和 Bun 1.4.4。paths 声明只读 `herdr`、`omp_package`、`pi_package`、`omp_state_extension`、`pi_state_extension`，不能指向用户会话数据。本专项机械路径可显式提供非秘密模型 JSON `{"mode":"controlled-native-failures"}`：受控 provider 仅提供工具指令，真实安装宿主执行原生确认、文件写入及报告；不要求外部模型凭据，也不声称真实模型选择。若使用真实模型，JSON 沿用 `provider/model/baseUrl/apiKeyEnv` 格式，只从显式授权变量读取凭据，不发现或刷新认证，实际请求及返回身份须匹配声明模型。不安装宿主、不回退模型、不重试业务决定。模型 transport 直接绑定 49152–65535 的数值回环端口，只在请求前的实际地址占用碰撞时重新分配；不依赖本机 `port=0` 的系统范围。最终身份核对仅将 `POST /chat/completions` 计为模型请求，其他流量保留原始 wire 证据并单独计数，不把无关 GET 错归因为模型失败；先在既有 30 秒请求预算内观察原生标题与 manager 的在途模型请求结束，再检查每条真实响应；不把尚在途当作失败，不跳过错误或重发模型请求。

原生矩阵按本机 CPU 数、所选场景数与模型配置显式 `maxConcurrentRequests` 容量中的最小值并行，记录实际 worker 数与传输容量；每例独占资源，聚合证据仅在主线程写入。未声明容量时使用 CPU 上限；既有 Codex transport 用锁串行消费原生决定，wrapper 明确声明其实际单请求容量，避免多个宿主排队超过原生传输预算，不修改产品时限。`node_approval_failures_acceptance.py` 每个宿主／No、Esc、真实 60 秒超时、无 UI 各建独占定义、Run、原生 Chat、HOME／APPDATA／XDG、Herdr、数值回环服务与材料。实际原生确认透传观察固定成果问题、选中值、真实返回或异常；按键成功不是批准。非 Yes 后核对实际工具反馈、公开 `approved=false`／`running`，随后连续观察原生调用、确认请求、HTTP 和公开事实 101 秒：超过 60 秒成果批准＋30 秒请求以及 10 秒在线＋30 秒保留＋5 秒扫描上界，记录单调时钟、每次样本与实际持续时间，不修改产品预算。

窗口结束后由新的显式用户输入要求原生宿主实际改写 `material.txt`、自由发送进度报告并从公开 Run 完整读回；此时仍未批准。再由另一条独立用户请求打开新确认并实际选择 Yes，核对批准成功、node 仍 running、报告不被改写。没有旧 report 发送审批预期，不把重新批准变成报告前提。

设置 `WORKFLOW_PI_PACKAGE` 为已安装 Pi 包路径，再运行 `python "$TESTER/scripts/workflow-codex-completion.test.py" CodexContentDiagnostics`，仅离线检查传输诊断不泄露凭据或未过滤内容。传输拒绝不触发重试或模型回退；离线证据不替代原生验收。

两入口可用 `--cases timeout` 或 `--cases unavailable`（也支持四个条件的逗号列表，默认 `all`）配合 `--hosts pi --native-only` 分别收集缺失场景。每次独立启动 wrapper 才创建独立 transport／配置／端口／新证据目录；失败决定在该次调用保持 terminal，不重发或重置。证据列出完整八项中的 `selectedCases`／`missingCases`；`selectedCasesPassed` 仅表示所选原生场景，部分场景或缺全仓检查仍 `passed=false`、退出非零，不认证整票。无效、空白或重复选择在创建证据与模型资源前拒绝。

真实模型可经 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-codex --scenario node-approval-failures --hosts omp,pi --backend "$HERDR_BOT_BINARY" --toolkit "$TOOLKIT_WT" --paths-file "$ISOLATED_PATHS" --pi-package "$PI_PACKAGE" --auth-file "$AUTHORIZED_AUTH_FILE" --evidence-dir "$NEW_EVIDENCE_DIR"` 运行：只读显式授权认证文件，沿用原生 Codex adapter，逐请求记录实际 `serverModel=gpt-6-luna`，不复制或刷新凭据。可追加 `--native-only` 分离原生资源窗口；该诊断保持非零且不认证全仓检查，必须另外保存两仓完整检查结果。

无 UI 使用受控 provider 制造机械故障：先确认原生 node 进程退出，再以同一真实 session 的安装宿主 print/JSON 模式执行实际 `approve`，保留 `hasUI=false`、原生结果、provider 请求及进程退出；真实后端执行、材料、之后新进程恢复和原生继续工作均不替换。headless 期间只临时替换独占 models registry 中已选 provider 的 endpoint／fixture key，不把 session 切换到另一个机械 model。headless 进程退出后，在关闭机械 provider 前逐字节恢复原 registry（异常也恢复），再走正常公开恢复，因此继续工作实际使用原授权模型及 UI；该 headless 请求仍明确是受控机械证据，不能按其保留的 model ID 冒充真实模型调用。其余 No／Esc／超时也可由显式机械 provider 选择工具，但原生确认结果从不受控。任何受控路径均不冒充真实模型选择或用不可用替代 UI 拒绝。每个场景清理原始错误与清理错误独立保留，确认所属进程退出后归档。

入口顺序记录本仓 build／后端／前端／浏览器与 toolkit typecheck／完整 test／check:integrations 及 `native-node-report.test.ts`、`node-report-contract.test.ts`。paired dock 强制本次安全关机壳和本 worktree 元数据，生产后端只用于隔离业务；suite 的证据不代替八项专项。源码／锁／产物／宿主身份入证，源码在原生运行期间变化、单宿主、任一检查失败、缺授权／宿主／coverage 或清理失败均非零且不认证整票。原生／Chrome 资源窗口独立时可追加 `--native-only` 只收集原生矩阵；它保留完整八项覆盖但整票 `passed=false` 且非零，不跳过认证所需全仓检查。离线回归显式设置 `WORKFLOW_PI_PACKAGE` 为已安装 Pi 包路径后运行 `python "$TESTER/scripts/accept-node-approval-failures.test.py"`：验证 CLI fail-closed／部分覆盖／旧证据保护，以及真实安装 Pi 通过字面本地 provider 进行 headless→原 session 恢复后回到原 endpoint；不读取用户认证、不调用外部模型，不替代专项业务验收。

### 双宿主批准隔离、恢复、并发与嵌套（S2）

从参与 worktree 根运行 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-node-approval-lifecycle --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$AUTHORIZED_MODEL_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。路径须显式绝对，证据目录须不存在；使用本 worktree 生产后端、旁置 herdr-paths／herdr-fixture／safe_shutdown_shell、本次静态页、独立绝对 CARGO_TARGET_DIR 和既有 Bun 1.4.4 frozen-lock 依赖。入口同时构建并复用现有 `--example recovery_herdr_proxy`，仅透明记录后端转发给本次安装 Herdr 的真实 argv，不启用其故障标志；原生诊断分离运行时先单独构建该 example。paths 沿用只读宿主／官方状态扩展安装路径；实际模型配置沿用 provider/model/baseUrl/apiKeyEnv，不发现认证、安装宿主、替换模型决定或自动重试。

显式授权只读 auth 文件时可复用 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-codex --scenario node-approval-lifecycle --hosts omp,pi --backend "$HERDR_BOT_BINARY" --toolkit "$TOOLKIT_WT" --paths-file "$ISOLATED_PATHS" --pi-package "$PI_PACKAGE" --auth-file "$AUTHORIZED_AUTH_FILE" --evidence-dir "$NEW_EVIDENCE_DIR"`，沿用 canonical gpt-6-luna／实际 serverModel 核对和 NativeCompletionTransport；不复制或刷新认证。`--native-only` 仅供诊断，缺全仓检查仍非零。两入口可用 `--cases persistence`、`nested-auto`、`nested-manual`、`lifecycle` 的互异逗号列表（默认 `all`）隔离独立分区；完整默认仍为两宿主×四分区，保留 selected／missing case 和每项原始结果，部分成功不认证整票、始终非零。实际 completion 失败后不通过同一 terminal transport 启动剩余宿主或重试决定；未尝试宿主明确保留未验证。

专项持有各自独占 Herdr、HOME／APPDATA／XDG、真实 Session、定义、动态高位数值回环、进程句柄和证据。P3/P14 实际关闭并确认 node 进程退出、合法恢复同一 Chat、比较原生业务历史／HTTP；修改 Map 后仍由原 execution 冻结人工模式打开真实 UI，合法 retry 才消费新自动模式。之后另实际重启后端，核对持久批准与冻结模式，旧身份不得接管旧 Run。P10 比较真实普通 node、同 Task 独立 Chat、根 manager、无关联 Chat、其他 Run／node、旧执行与嵌套双角色的实际模型工具目录、零业务参数调用、原生身份和公开拒绝，不编辑 Run 或制造身份。宿主可能加入 i/intent 或保留 additionalProperties，不把该 schema 外形当业务契约。多余身份参数分区由明确标注的 controlled 机械 provider 驱动实际安装宿主／adapter／业务 guard：`toolkit/scripts/node-approval-identity-fault.ts` 复用既有 RPC 与 public switch_session，打开本次真实历史的逐字副本并核对原生 Session，保留真实参数／返回、两 Run 读回与实际 HTTP，验证拒绝或剥离均不能选择另一个执行。它不冒充实际模型选择、P14 恢复成功或新产品 CLI；正常模型角色、批准、恢复与嵌套仍为真实模型，其他票的 malformed/dock 覆盖不重复认领。共享确认 driver 接受显式 Chat，不临时改写主 worker，错误恢复关联在终止／恢复进程或模型调用前经公开 execution.chat 拒绝。进程归属由实际 public `session_start` 的 Session／PID 与 Windows 实际安装 argv／存活句柄交叉核对，不以“操作前后多一个 PID”推测身份；原生 manager 已合法派发的同一新 Run 节点按真实公开事实观察，不重复派发后把拒绝冒充成功。

保持真实确认在途，核对公开查询、自由进度报告、独立分支报告及 complete 分别在 2 秒内返回。fail／retry 后，确认新执行确实 working、尚未发出首次业务模型请求，再在旧执行中实际选 Yes。记录原生 true 与后端陈旧拒绝；新执行仍未批准，也不得把已发生的独立动作归给旧 Yes。91 秒单调窗口覆盖 60 秒批准和 30 秒 HTTP 预算；释放调度后独立记录新执行的合法请求与报告，不改变产品时限或用提示词制造因果。

持久化场景须由原生工具实际改写材料和报告，验证批准保留，但不要求下游自动重读。嵌套场景验证修改父 Map 后仍按冻结模式批准；真实子 Run finish、整体批准与父 complete 各有独立回执。等内外 manager 停轮后再清理，不能截断 finish 后的响应。`NativeGateFixture` 仅证明安装宿主与隔离后端的调度隔离，不认证真实模型业务。

嵌套场景按工具契约逐项发送明确用户请求，由真实模型调用安装宿主的 prepare／dispatch／complete／finish／approve／report；不考核模型自主编排整套调用，不以直接 HTTP 写入或受控模型返回代替原生调用。场景请求 manager 收到通知后等待下一项用户指令，但不把模型完全不自主动作当作 oracle。每项推进前先观察当前原生轮收尾及公开事实；已由真实原生工具完成的合法动作保留其调用、返回与 HTTP，不重复提交。独立批准与父 complete 仍须各自首次成功返回 changed=true，不能以最终状态掩盖隐式推进。人工整体批准在观察或明确请求期间均沿共享原生确认 driver，在 60 秒成果批准预算内观察并选择真实 Yes。保留子 finish 返回、父批准返回、整体报告及父 manager complete 的独立原生／HTTP因果顺序，并核对内部批准和 finish 均未替代整体批准、整体批准未隐式 complete。原生调用可为直接工具，也可为安装宿主的 `write xd://<tool>`：仅按真实 toolResult 的 `details.xdev` 执行回执、匹配的路由和原始 JSON 参数归类，保留原调用 ID／路由／回执；schema help 不算执行，普通文件写入或缺少执行回执不能冒充批准，不使用已剥离身份参数的 renderer args 代替模型原始参数。P10 基础角色、旧 execution 和双角色分别保存；P28 两种模式分别保存，失败不会抹掉已完成的独立观察。

可重复传入 `--combine <原入口 evidence.json 的绝对路径>` 合成分次证据。不传模型配置，也不发送请求。仍提供两个 worktree、backend、paths、`--hosts omp,pi` 和新的 `--evidence-dir`，不得过滤场景。合成要求可核对的参与源码、运行指引、相同产物和安装宿主、实际模型身份及成功清理；不可重复记入同一成功场景，也不可改写旧失败。只有完整八格及所需检查齐全才通过。`--checks-from <原入口 evidence.json 的绝对路径>` 仅复用来源兼容且全部成功的仓库检查，不替代设施自测，不可与 `--native-only` 同用。`--native-only` 合成仅诊断，仍非零退出。

每个 coverage 保存独立预期、原生 UI／历史／工具返回、实际文件、HTTP／公开读回与进程退出，原始和清理失败分开。入口一次顺序执行本仓 build／完整后端／前端／浏览器与 toolkit typecheck／完整 test／check:integrations／node-report及nested-role具名 docks；paired dock 只用本次安全壳，生产后端仅用于隔离专项。toolkit baseline 通过显式 OMP_BASELINE_RECORD_ROOT 解析既有 oracle，不改 baseline 或锁文件；全仓测试 worker 由实际 CPU／可用内存按每原生文件2GiB预算决定，不将非canonical参数的执行冒称宿主 scope 晋级。单宿主、缺依赖／授权／coverage、任一检查或清理失败均非零。`python "$TESTER/scripts/accept-node-approval-lifecycle.test.py"` 的离线部分验证公开 CLI fail-closed／旧证据保护、显式 Chat 路由及原生 observer 协议。单独的 `CollectRegistrationFixture` 使用实际安装 Pi、隔离生产后端和本地机械 provider：先建立 running 内层节点，再保持真实 collect 在途，核对登记／目录观察返回后进度报告仍可保存并释放 collect。显式设置 `NODE_APPROVAL_LIFECYCLE_FIXTURE_TOOLKIT`、`NODE_APPROVAL_LIFECYCLE_FIXTURE_BACKEND`、`NODE_APPROVAL_LIFECYCLE_FIXTURE_PATHS`、`NODE_APPROVAL_LIFECYCLE_FIXTURE_EVIDENCE`（两个参与 worktree 的现有产物／隔离依赖及不存在的新证据目录），运行 `python "$TESTER/scripts/accept-node-approval-lifecycle.test.py" CollectRegistrationFixture`；未提供依赖时该设施用例跳过，不能据此认证票据。默认专项检查会显式提供这些配置。该机械设施回归不声明真实模型判断或任何 P 分区通过，不能替代真实专项。

### 发布 V2 双宿主原生与页面专项（P34／P35 V2）

从参与的 herdr-bot worktree 根运行：

```sh
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-node-approval-v2 --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$AUTHORIZED_MODEL_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"
```

路径必须显式绝对，证据目录必须不存在。使用本 worktree 的独立绝对 `CARGO_TARGET_DIR/debug/herdr-bot.exe`，旁置本次 `herdr-paths`、`herdr-fixture` 与 `examples/safe_shutdown_shell.exe`；两仓 frozen-lock 依赖及 Bun 1.4.4。paths 除原生设施的 `herdr`、`omp_package`、`pi_package`、两宿主 `*_state_extension` 外，须显式声明原发布 Suite 的 `wayfinder`、`wayfinder_skills`、`pi_extensions`、`codebase_memory_mcp` 、包含 chrome.exe 的 `chrome`，以及已安装 Windows 原生 Bash 可执行文件 `native_shell`（也可显式 `--shell-path`）。不继承 PATH 中的 WSL bash：Linux loopback 及盘符语义不能替代本 Windows 服务／文件。模型启动前，由所选 Bash 实际读取独占探针文件并访问同一生产后端 `/api/tasks`；可执行文件存在本身不是环境相通证据。预检核实原始 grill-with-docs、grilling、domain-modeling 和 writing-for-agents 指针实际存在，不复制 Skill 兼容别名、不改其正文。Suite 与初始 Skill 不删除，MCP 使用已有隔离 runtime/cache/allowed-root 设施，不接用户 daemon。每宿主只需一次可在原生历史中关联调用与成功返回的只读 `list_projects`，空列表有效；不创建索引或扩大 allowed-root。Task 的 `thinking=default` 仍是合法的沿用 Agent 默认值，无须出现在显式级别列表。以原生 session_start 的 PID、已观测的安装宿主 CLI 进程及原生历史 ID 交叉验证宿主，`agentName` 不代表宿主类型。阻塞状态说明不触发业务确认。模型沿用 `provider/model/baseUrl/apiKeyEnv`，不发现凭据、不安装宿主、不回退受控成功、不重试业务决定。

已明确授权只读 auth 文件时，使用既有 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-codex --scenario node-approval-v2 --hosts omp,pi --backend "$HERDR_BOT_BINARY" --toolkit "$TOOLKIT_WT" --paths-file "$ISOLATED_PATHS" --pi-package "$PI_PACKAGE" --auth-file "$AUTHORIZED_AUTH_FILE" --evidence-dir "$NEW_EVIDENCE_DIR"`；先以另一个新证据目录加 `--smoke` 核对实际 `serverModel=gpt-6-luna`。这仅是该传输的授权模型边界，不是产品限制；不复制、刷新或修改认证。临时 worktree 的历史 baseline 依赖可按 toolkit 指引显式设置 `OMP_BASELINE_RECORD_ROOT`，不改 baseline 或锁文件。

Codex 非成功响应仅记录固定错误类别和请求形状（数量、长度及工具调用／结果匹配布尔），不保存不可信错误正文、任意 code／param 或请求正文；分类不改变单次 POST、终止失败和禁止补发的语义。两个宿主可分别使用新独占 transport，避免一个终止失败掩盖另一宿主的独立诊断；单宿主结果不构成整票通过。

专项以发布 `create-task-v2.json` 及其真实两节点 `create-bot-v2.json` 独占副本运行，保留业务提示词、Skill、Suite、拓扑和专用 V2 authorization／confirmation 材料；只重定位物理 zone、契约地址和显式模型／宿主。两个宿主分别真实创建只读 CSV Task 与完整 Bot，三个人工职责观察原生完整问题、选中 Yes、真实 true 返回；子管理者 finish 后独立执行父层自动批准。原生历史、真实模型请求／返回身份、工具与透传 HTTP、命名材料字节、实际配置／revision、显式 complete／finish、独立 Run 身份及 V1 原定义未改证据均归档。批准、最新可交接报告及声明交付分别核对。驱动不发送 `complete` 探针：首次交接取真实 manager 工具结果及 `changed=true` 的公开快照，用快照的有序报告和累计 outputs 验证门禁，不按响应结束时间排序。无报告／最新 false 的拒绝由原 S1 `completion_gate` 覆盖。以 Run、node、execution 和 session 关联原生 Yes、公开来源与父子 finish；自动节点查原始成果确认记录，不以驱动未按键推断无弹窗。

业务交互驱动按实际 node 身份限定权限：Task 阶段不能授权 Bot 保存，需求阶段不能提前批准四字段摘要／最终保存，设计阶段仅批准真实展示且与本次目标一致的完整候选；没有结构化完整候选或动作不明时不授权；Task 候选可用结构化块，或用单一字段表格配唯一的完整 prompt 块展示，仍按同一规则核对全部字段。不把创建 Task 和写入材料合并授权。先校验整张原生表单，再操作终端。不合规表单以真实 Esc 拒绝并保留返回。将实际授权的候选与 Task／Bot 保存、完整读回及 revision 关联；按发布 Map 的已知接线核对 V2 request、双材料及父层最终交付，按角色在执行动作前验证实际读取。隔离副本仅额外授权读取 Map 明确绑定的 V2 格式契约文件。离线授权策略自测只证明驱动边界，不证明原生批准或模型业务成功。每个进程显式设置 TMP／TEMP／TMPDIR，业务消息明确独占材料／临时目录与只读文档／Skill 范围；沿用现有规划验收的参与 worktree 只读仓库绑定，使 Task 文档根与 `run_instructions` 同根，不建立第二份仓库副本或路径别名。禁止共享 Temp／`/tmp` 发现式查找和写入；这些是配置与授权约束，不是 OS 文件系统沙箱，观察到越界不能认证通过或事后扩权。

`node-approval-v2-browser.mjs` 使用本次真实打包页面：逐项消费四个发布职责、实际发起 Run、在选中节点的执行事实、适用报告及累计 outputs 区域对照独立 HTTP 的执行 ID、冻结模式、批准来源、报告 ID／正文和 output ID／地址，不重复通用编辑场景。Chrome profile、宿主会话、HOME／APPDATA／XDG、定义、进程、数值高位回环端口和材料全部独占；保留实际 Chrome 进程句柄退出及 profile 关联 utility 退出确认，清理错误与原始错误分开。完成后 91 秒单调窗口覆盖 60 秒成果批准＋30 秒 HTTP 预算及 1 秒余量。

入口顺序执行本仓 frontend build、指定 Cargo build、完整后端／前端／浏览器，以及 toolkit typecheck／完整 test／check:integrations 和 native-node-report、node-report-contract、nested-role-completion docks。配对 dock 只使用本次安全关机壳和本 worktree 元数据。任一检查失败、宿主／coverage／真实授权缺失、源码变化或清理失败都保持未通过并非零；单宿主及 `NODE_APPROVAL_HANDOFF_OMP_PACKAGE="$OMP_PACKAGE" python "$TESTER/scripts/accept-node-approval-v2.test.py"` 的窄回归通过不替代专项；`OMP_PACKAGE` 显式指向已安装 OMP 包。该入口同时经共享父类配置和安装宿主 `Settings.loadReadOnly().getShellConfig()` 检查 YAML 优先级，并检查本次真实 Esc／匹配 Ask call 的一次权限澄清边界，不将其当作四节点业务完成。仅调整验收设施、compiled/frontend/locks及实际产物未变时，可按既有 `--native-only` 约定另起新独占原生／页面窗口；其 `nativeCoveragePassed` 只表示本轮双宿主覆盖，`passed=false` 且退出非零，不认证全仓。必须与实际已完成的九项检查日志及两轮源码／锁／产物兼容证据分别交付，不能把旧失败入口说成成功，也不复用旧原生覆盖。不调用 V1、不迁移其材料、不认领 V1 验收，也不要求 V1 先通过。

## 节点报告共享保存／通知回归

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch report -- --nocapture`。分别核对保存、通知与诊断失败；调用者断线或通知受阻不能撤销已保存报告，也不能阻止读回或同 Run 的独立动作。嵌套收件范围另用 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch nested_completion`。该回归不认证原生确认或 chief 报告。

## Chief 入口与单次设置浏览器回归

`npm --prefix frontend run test:browser:build -- chiefEntry.spec.ts --workers=1` 检查真实打包页面的 chief 默认设置、窄屏缺 zone、单次覆盖不改 Task 预设，以及合法空输入启动不提交消息。与公开 Task 和会话登记交叉读回。普通 Chat 的空输入另用 `chat_creation` 的 `an_empty_trigger_starts_a_ready_chat_without_submitting_a_prompt` 定向检查。页面回归使用受控 Herdr，不证明安装宿主或真实模型行为。

## Chief 跨仓入口与生命周期专项

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-chief-entry --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"` 是 P7/P29/P30 的独立验收入口。从显式 Herdr worktree 根运行，所有路径须为绝对路径，证据目录必须不存在；Python 3.11+。使用本 worktree 构建的页面、生产后端及旁置 `herdr-paths`、`herdr-fixture`，独立绝对 `CARGO_TARGET_DIR`。两仓依赖按各自 frozen lock 安装，Bun 1.4.4。入口不安装宿主、不读用户认证、不回退模型、不重试业务决策。

隔离 paths TOML 使用已有 Rust 路径解析器，声明只读 `herdr`、`omp_package`、`pi_package`、`omp_state_extension`、`pi_state_extension`（后两项是具体官方扩展文件）、`chrome`（含 chrome.exe 的安装目录），以及交付固定扩展所需根。模型配置使用既有真实模型设施的 `provider/model/baseUrl/apiKeyEnv` JSON，model 严格为 `gpt-6-luna`，密钥仅来自显式授权的环境变量；返回 model 身份必须同为 `gpt-6-luna`。也可沿用下文 `NativeCompletionTransport` 与 `workflow-codex-completion.mjs`：仅在用户明确授权某个 auth 文件后，以已安装 Pi 原生 Codex provider 只读消费，不复制、刷新或修改认证文件；回环配置中的 apiKeyEnv 仅是独占传输标记。先独立烟测并检查真实 `serverModel`，再将真实决策送入本入口，由原生 OMP/Pi 执行工具；不证明两宿主直接使用 Codex provider 的 wire 合规性。adapter 通过 Node 的安装包解析定位该 Pi 声明的 pi-ai（含嵌套 npm 布局），使用 Node 22.19.0 或满足宿主要求的更新版本。缺授权、依赖或任一宿主证据均非零，不自动发现用户 auth store，不以受控响应替代真实模型。

P7 使用交付固定扩展清单的实际源码副本及独占 endpoint／model-cost 存储配置（副本中禁用指向正常服务的 Codex proxy），经公开发起和新进程恢复，各自让真实模型查询新发布的独立字面 Bot.when。原生 SessionManager 历史、chief 工具输入/真实结果、模型请求中的角色指引和最终回答共同作为消费证据；恢复前持有 OS 进程句柄确认退出，恢复后要求不同 PID 及相同原生会话身份。没有 Suite，不把配置文件或 argv 当作已加载；角色消费断言检查解码后的实际模型消息中的角色与禁止亲自执行业务边界，不要求只存在于原生历史中的 message marker 被传给 provider。查询没有原生批准操作，因此不伪造确认；只读用户输入和 HTTP/原生历史保留在证据。

`frontend/browser-tests/chief/chiefEntryAcceptance.spec.ts` 独占 P29/P30 的 S1/S2 场景。两个真实 browser context 同时从普通/专用入口发起不同 zone/Task，真实外部 CLI 门闩在登记前保持启动未完成，再叠加公开并发调用；仅一个 202、唯一公开权威和 qualify 胜者。P30 显式配置 10 秒在线、30 秒离线保留，沿用生产 5 秒扫描；真实单调时间观察离线（16 秒预算）和移除（46 秒预算），跨浏览器核对保留期不可另建、心跳恢复、超期可创建及明确注销。登记移除和页面反馈不证明旧进程结束，也不新增产品等待策略。浏览器的外部 Herdr 与原生引用受控，不能充当 P7 原生模型证据。

定向反馈：`npm --prefix frontend run test:browser:build -- chiefEntryAcceptance.spec.ts --workers=1`；CLI 安全回归：设置 `CHIEF_ENTRY_TOOLKIT` 为参与 toolkit worktree 后运行 `python "$TESTER/scripts/accept-chief-entry.test.py"`。后者只证明缺依赖 fail-closed、拒绝相对路径及不覆盖旧证据，不证明业务通过。完整专项逐项记录原始错误/清理错误、源码与锁文件/安装身份、HTTP、配置阈值与实际观察；退出并确认所属进程结束后才清理。

完成检查仍分别运行本仓 `npm --prefix frontend run build`、`cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`、`node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`；toolkit 顺序运行 `bun run typecheck`、`bun run test`、`bun run check:integrations` 和 `bun test tests/harness-dock/chief-role.test.ts`。配对 dock 的 `HERDR_BOT_BINARY` 必须是本次 `safe_shutdown_shell.exe`、`HERDR_BOT_METADATA` 为本次 worktree 元数据，不能以生产后端执行关机回归。全局回归、单宿主或 CLI fail-closed 通过均不代替双宿主专项成功。

## Chief 权威的进程与失败回归

`cargo test --manifest-path backend/Cargo.toml --test chief_authority -- --nocapture` 经公开 HTTP、真实独占后端进程及生产 CLI adapter 验证 P32/P33。`p32_real_restart_serializes_live_registration_and_resume_before_external_completion` 正常发起两份不同 zone／Task 的 chief 历史、注销、实际终止并重启后端，在外部 Herdr 查询门闩处保持 Resume 未完成，让存活登记与另一恢复调用竞争，核对当前权威与 qualify 的胜负；再次实际重启验证被替代身份不能复权。附带 S2 开发回归以实际默认 10 秒在线／30 秒保留策略和受控时刻验证离线占位、扫描移除及消费者失权，不替代 07 的浏览器 P29/P30 或 20 的原生交接 P31 全验收。

`p33_definite_start_failure_frees_slot_but_uncertain_external_start_survives_restart` 验证明确启动拒绝后能重试；受控 Herdr 已接受启动却返回 readiness timeout 时，保留 `uncertain`，实际重启后仍拒绝第二次外部启动；陈旧注销拒绝，明确解除当前 claim 返回原不确定事实并允许下一次合法发起。这些外部响应只证明协议与故障处理，不证明真实 Herdr／OMP／Pi 加载或 GPT-6-luna 的业务职责；本专项没有真实模型请求。

每例独占原子临时目录、定义、历史、权威存储、HOME／APPDATA／XDG 和数值回环动态端口。后端 stdout 持续消费，stderr 落在该例独占目录；Drop 终止并 wait 所属后端，确认结束再删除临时目录，不接触正常服务或用户 Chat。进程启动预算 30 秒，创建完成 20 秒，外部查询进入 5 秒、门闩 10 秒、释放后 Resume 完成 15 秒；不新增产品等待／重试策略。本专项是 S2 进程与公开操作证据，不以受控 CLI 充当 S3 原生宿主证据。完成检查仍需本 worktree 独立绝对 `CARGO_TARGET_DIR`、本 worktree 构建静态页及 `node scripts/test-backend.mjs`；真实 HTTP／进程检查另按入口要求运行 `npm --prefix frontend run test:browser:build`。

## chief 独立 Task Chat 委派回归

`cargo test --manifest-path backend/Cargo.toml --test chief_delegation -- --nocapture` 经过 S2 公开 HTTP、真实后端进程、独占临时 Bot／Task／zone／历史／委派存储及生产 Herdr CLI adapter，验证明确业务输入与材料地址、完整预览载荷、Task 目标／zone 改绑后的拒绝、旧 chief 失权、启动／投递失败和不确定反馈。外部 prompt 门闩在接受输入前读回真实 sender 关联；实际同 Task 的另一普通 Chat 不获关联；实际终止后端并启动新进程，再由公开 Resume 读回原关联且不重放业务输入。它不创建虚构 Run／node／execution。每个后端进程隔离 HOME／APPDATA／XDG，数值回环动态端口和 Herdr CLI 副本均独占；stdout 持续消费、stderr 保留日志，退出时终止并 wait 所属进程后才删除临时目录，清理失败不覆盖原失败。

门闩仅控制测试拥有的外部 CLI，进入预算 5 秒、阻塞预算 10 秒，释放后创建完成预算 20 秒；生产沿用 Chat 就绪／身份获取预算，具体身份失败不新增命令重试。专项没有真实模型请求，受控 Herdr 只证明协议／故障及持久事实，不代替 12 的 OMP／Pi 原生批准、GPT-6-luna 路由或实际模型业务完成前返回的 S3 证据；报告资格完整负例与跨进程恢复分别由 14／19／20 维护。本票完成检查使用本 worktree 构建静态页面及独立绝对 `CARGO_TARGET_DIR`，运行 `node scripts/test-backend.mjs` 和 `npm --prefix frontend run test:browser:build`。

## Chief Task 双宿主原生验收

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-chief-task --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"` 拥有 P8、P9(Task)、P10(Task)、P11、P13。Python 3.11+，从显式 Herdr worktree 根运行；路径均绝对、证据目录不存在。复用入口专项的 paths、实际模型配置、Cargo provenance 与 installed-host 检查；不安装宿主、不读用户 Chat、不发现凭据、不回退或重试业务决策。两仓 frozen 依赖须已存在，toolkit 的既有隔离 OMP 18.5.0 dock 安装也须已准备好。

独占 Herdr 使用 `headless_cols=200`、`headless_rows=220`，使完整事实与 Yes/No 同时可见；这只改变测试拥有的虚拟终端尺寸，不改变产品 20 秒确认预算。终轮按原生 assistant `stopReason=stop` 观察，再独立断言业务行为，不把模型复述测试标记当作验收条件。含糊需求可由真实原生 Ask 表单或普通回答澄清：先保留实际目标／要求问题，再由用户 Esc 放弃尚未知晓字段的表单并转向另一需求；不代填未知业务信息，也不把等待表单误判为模型未响应。后续用户输入先以 10 秒预算观察 chief 原生 ready，再只提交一次并用 `prompt --wait --until working --until blocked --timeout 10000` 证明本次活动；Herdr 写入成功不是模型已接纳，stalled 不补发或补 Enter，失败前保留屏幕。一次方向键只发送一次，再以 5 秒观察预算等实际选项重绘，不把写入按键成功当作已选中；不延长产品确认时限。启动结果按句检查是否存在未经否定的业务完成声明，保留“不表示业务已完成”等正确边界表述。真实 GPT-6-luna 自行决定工具：独立字面 Bot.when 与名称误导候选区分按适用范围选择；含糊、无匹配、已知匹配三轮均保留原生历史及工具结果。先提出初稿，用户修正 zone 与内部审计/禁止发布要求，再经实际原生 UI 核对最终完整预览。拒绝、Esc 取消、真实 20 秒超时、批准期间 Bot 改目标及 zone 改绑分别观察没有委派/新 Tab；每次工具结束后的 61 秒窗口覆盖 20 秒确认＋30 秒 HTTP 预算、10 秒在线＋30 秒保留＋5 秒维护扫描。用户明确请求新操作后才再预览，不自动重试。批准案例核对单次 preview ID 提交、公开委派事实、原生独立 Chat/cwd/完整输入、真实 Task 来源及无新增 Run；执行者未收到 RELEASE，chief 已返回启动结果并处理下一输入。业务哨兵逐阶段保持不变。

不可用确认复用 `tests/harness-dock/chief-delegation.test.ts` 的两个既有 Task headless case，明确标注为受控 provider 的原生协议证据，不重复认领模型职责；设置 `WORKFLOW_PROMPT_EVIDENCE_DIR` 保留原生输出和历史。CLI 安全检查：`CHIEF_TASK_TOOLKIT="$TOOLKIT_WT" python "$TESTER/scripts/accept-chief-task.test.py"`，仅证明缺依赖/错误模型 fail-closed、绝对路径与旧证据保护，不算专项业务通过。

明确授权某个 auth 文件时，可用现有 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-codex --scenario chief-task --hosts omp,pi --backend "$HERDR_BOT_BINARY" --toolkit "$TOOLKIT_WT" --paths-file "$ISOLATED_PATHS" --pi-package "$PI_PACKAGE" --auth-file "$AUTHORIZED_AUTH_FILE" --evidence-dir "$NEW_EVIDENCE_DIR" --node "$NODE" --bun "$BUN"`。它只读指定凭据，不刷新/复制/写入证据；实际返回 `serverModel` 必须为 `gpt-6-luna`。独立 `--smoke` 先核对身份。传输保留真实模型决策与原生工具，不证明宿主直接使用 Codex wire。

完成检查仍分别顺序执行本仓 frontend build、当前 worktree 的 `herdr-bot`/`herdr-fixture`/`herdr-paths`/`safe_shutdown_shell` build、`node scripts/test-backend.mjs`、frontend test 和完整 `test:browser:build`；toolkit 使用 Bun 1.4.4 的 typecheck/test/check:integrations 及受影响 chief dock。配对 dock 的 `HERDR_BOT_BINARY` 必须指向本次安全壳，metadata 指向本 worktree。单宿主、清理失败、缺任何真实依赖或 coverage 项均非零且保持未验证。原始失败与进程退出确认独立保留，不以删除目录代替清理，也不沿用源码变更前的证据。

## chief Map Run 委派回归

`cargo test --manifest-path backend/Cargo.toml --test chief_delegation map_ -- --nocapture` 拥有 09 的 S2 Map 发起与来源关联开发回归，复用独占真实后端进程、HTTP 和生产 Herdr CLI adapter：

- `map_bot_starts_real_root_run_and_preserves_explicit_manager_and_node_zones` 读回实际根 Run／manager／委派关联及完整预览投递正文；不虚构父执行，不把启动当业务完成。
- `map_confirmation_rejects_changed_map_manager_node_binding_and_material_meaning` 分别改变根／嵌套 Map、Bot、manager／node Task、仅内部 node 的 zone 绑定、命名／直接材料字节及材料删除；旧预览必须要求重新确认，且没有外部启动或 Run。
- `map_sender_is_saved_before_input_and_internal_node_keeps_workflow_gates` 在外部 prompt 门闩处公开读回已保存的真实 manager 关联；显式准备输入后实际派发 node，独立观察 node cwd、原生身份、管理归属和未报告时的完成拒绝。内部 node 无 chief 委派关联。
- `map_preview_requires_explicit_manager_and_all_required_named_materials` 拒绝缺 manager、错误用途、缺必需材料及未声明材料。
- `map_startup_failure_retains_run_provenance_and_real_submission_outcome` 覆盖 Run 初始化失败（无 manager）、agent 启动失败、明确投递拒绝和投递结果不确定，公开读取真实阶段／外部效果及持久启动状态。

专项沿用本文件上节的独占资源／进程退出规则；门闩进入预算 5 秒、外部阻塞预算 10 秒、创建完成预算 20 秒，后端启动预算 30 秒，没有新增产品等待或重试策略。外部 CLI 和原生文件是受控协议输入，没有模型请求；不能替代 13 的两宿主原生确认／GPT-6-luna／真实创建阶段审批、14 的报告资格或 19 的恢复验收。完成检查使用本 worktree 构建的静态页面及独立绝对 `CARGO_TARGET_DIR`，执行 `node scripts/test-backend.mjs` 和 `npm --prefix frontend run test:browser:build`。

## Chief Map 与能力创建跨仓专项

从显式 Herdr worktree 根运行 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-chief-map --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。所有路径必须绝对，证据目录必须不存在。模型配置、只读授权 Codex adapter／`NativeCompletionTransport` 和返回身份要求沿用上文入口专项；只允许 GPT-6-luna，不自动发现认证或重试模型决策。后端旁置当前 worktree 的 `herdr-paths`，页面与后端均须由参与源码构建。

`chief_map_native_acceptance.py` 独占 P9 的 Map 修正、P10 的 Map 原生确认、P12 的 manager／node 位置与内部管理关系、P14/P15 的完整创建能力选择与否决后授权、P16 的创建阶段批准和结果后无新业务。每个宿主／场景独占 Herdr、HOME／APPDATA／XDG、定义、存储、动态回环端口和原生会话；复用进程句柄退出确认与原始错误／清理错误分别保留。创建目标使用交付的完整 Task＋Bot／Map 创建定义，保留节点 Task 的真实 Suite／Skill／宿主和原有审批，只将物理位置、模型传输和存储移至独占根；原生 MCP 必须支持既有隔离 selectors，不能接入账户 daemon。

测试独占部署还加载 `scripts/chief-ui-observer.mjs` 的副本：仅透传观察安装宿主公开 `ui.confirm` 的原始参数、原始返回／异常与真实 abort 时刻，不选择工具、不修改确认内容或结果、不代用户批准。每条观察原子发布；专项把实际原生消息内的完整 JSON 与 HTTP 预览及提交载荷分别比较，观察缺失不能通过。观察器按公开 UI 自有方法 descriptor 保留真实函数，避免 OMP scoped Proxy 的转发闭包递归；`node --test "$TESTER/scripts/chief-ui-observer.test.mjs"` 只证明透传机制，不代替原生验收。专项通过公开 `terminal session control --cols 2048 --rows 2000` 调整本例独占 terminal，让完整 Map 原生确认与 Yes/No 按钮可见，不裁剪实际载荷。原生键选择和真实历史仍独立保留。

P15 的四种输入均先取得真实匹配复用建议，再分别仅否决、明确创建 Task、明确创建 Map、放弃；不能用本来没有复用候选的场景冒充否决边界。P16 分别保留实际阶段候选的拒绝／受阻返回，以及真实成功创建后无新业务的场景。成功案例只走所选既有创建流程需要的真实逐阶段正文／保存批准、独立原生报告批准和交接；核对实际新 Task＋完整 Bot 或多节点 Map，不追加创建产品的完整质量验收。创建观察预算为 `20 × --timeout` 秒（默认 3600 秒），与生产等待策略无关；拒绝／受阻不能冲抵成功创建后的授权边界。

原生确认预算 20 秒、请求预算 30 秒；拒绝／取消／超时／陈旧批准及创建结果后观察 46 秒，超过实际 10 秒在线＋30 秒保留＋5 秒维护上界。实际生产预算不改写为测试重试策略。`tests/harness-dock/chief-map-unavailable.test.ts` 是明确标注的受控 provider／peer、真实安装宿主无 UI 机械故障用例，不证明真实模型业务选择；其原生 2 秒预算使用真实进程时钟。其它业务场景须有 GPT-6-luna 返回身份、实际历史、完整工具／HTTP 及公开 Run 事实，不能用 dock 或全局套件结果冲抵。

`CHIEF_MAP_TOOLKIT="$TOOLKIT_WT" python "$TESTER/scripts/accept-chief-map.test.py"` 仅验证 CLI 的缺依赖／错误模型 fail-closed、拒绝相对路径与不覆盖旧证据。过滤单宿主不能通过整体双宿主覆盖。两仓仍分别完成票据要求的完整构建／后端／前端／浏览器及 Bun typecheck／test／check:integrations，配对 dock 必须使用本次安全关机壳与 worktree 元数据，不能用生产后端运行关机回归。

## Chief 报告持久化与资格回归

`cargo test --manifest-path backend/Cargo.toml --test chief_delegation reports:: -- --nocapture` 检查真实委派资格、公开读回、保存先于通知及后端重启后报告不变。保存失败不通知；通知不明不能推断无外部效果。报告已保存、通知已提交、chief 已接收和用户已读是不同事实。

## Chief 持久接收与人工恢复回归

`cargo test --manifest-path backend/Cargo.toml --test chief_delegation receipts:: -- --nocapture` 检查并发仅一次处理授权、重启保留接收、丢响应不重播、显式查看和人工重发保留原 ID／正文。先读回已提交的 receipt 才能把断线当作回执丢失。Resume 不重放输入或创建新委派。详见[持久接收、人工重发与查看](../usage.md#持久接收人工重发与查看)。两组后端回归使用受控 CLI，不替代下文真实模型和原生确认。

## Chief 双宿主报告专项

从参与的 Herdr worktree 运行 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-chief-reports --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。全部依赖使用显式绝对路径；证据目录必须不存在。先 frozen-lock 安装两仓依赖，以独立绝对 `CARGO_TARGET_DIR` 构建当前页面、生产后端、`herdr-fixture`、`herdr-paths` 与 `safe_shutdown_shell`。Bun 为 1.4.4；已有 baseline 依赖可用 `OMP_BASELINE_RECORD_ROOT` 定位，Pi 可显式设置 `PI_CLI`／`PI_NODE`。入口不安装宿主、不发现用户认证、不回退模型或重试业务决策。经单独授权的只读 Codex 传输可复用 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-codex --scenario reports`，沿用其显式 `--auth-file` 安全边界；先 `--smoke` 核对实际 `serverModel=gpt-6-luna`，不得把回环传输称为两宿主直接 Codex wire 合规。

P18 在当前 chief 存在时分别运行普通／实际 chief 委派的嵌套 Map，真实双角色工具继续向父 manager 报告，内部报告继续向自己的 manager 报告。P20 引用并实际执行既有 `native-chief-report` 的双宿主批准／拒绝／取消／超时／不可用及完整预览用例。新增 `chief-report-backend-faults` 用真实隔离 journal 目的地目录造成保存失败、受控外部 Herdr 明确拒绝，以及代理在实际后端响应后断开 TCP，核对原生结果及公开不可变事实；不是伪造后端成功响应。P36 执行既有 node 原生确认与公开报告门禁，双宿主真实通知拒绝后保存不撤销，观察46秒不新增报告／通知。P37 执行现有集成声明和 import-boundary 检查，并保留源码／锁文件／产物／安装宿主身份；源码审查仍需确认后端 files/report_delivery 唯一持久权威、扩展 common 共享确认与请求，不新增接线快照测试。

`chief_reports_native_acceptance.py` 的 P19/P28 使用真实 GPT-6-luna：独占子进程成功产物、退出7的部分产物、缺输入受阻及公开保存读回的能力定义作为独立业务证据，发送者实际读取并选择原生报告工具，驱动只提供用户输入／原生批准。批准期间替换为无原始对话的新 chief，逐字核对预览、实际请求、保存及读取事实；另对实际发送者进行新进程恢复后提交新的失败结果（不是重发原报告）。真实模型按预先固定的背景／执行对象、结果／限制、交付物／判断项、仅询问满足要求并等待、各类交付事实区分五项 rubric 评价真实呈现；背景与执行对象还必须提供实际 chief 答复中的非空逐字引文，驱动核对引文确实存在，不能引用原报告冒充已呈现；引文由评价器从原始 Markdown 字符串抽取，必须保持连续原文及其中格式符、标点和空白，不能以渲染后的去格式文本冒充逐字引文；程序仍逐字检查子串，不要求 chief 以引文格式回答。独立事实还保留实际产物读回；chief 准确转述权威报告即可，不要求重新执行业务或独立证明结果。缺项、虚构引文或混合结果失败，不重试。每种结果停轮后观察46秒，核对没有新增业务对话消息／工具、报告或 Chat；OMP 自身的标题／model-usage 元数据不是下一项业务。报告专项仅在独占 OMP `agent/config.yml` 中设置既有 `recap.enabled: false`，避免与报告无关的 idle recap 模型请求在清理时被进程退出取消；不改变业务确认、静默窗口或正常安装配置。窗口覆盖45秒隔离 Herdr／后端就绪观察、30秒原生启动／请求、20秒确认、10秒在线＋30秒保留＋5秒扫描预算，不以即时断言或新产品等待策略证明无自动工作。

所有原生历史、确认、工具结果、HTTP、公开读回、模型请求及实际身份、配置预算、原始失败与清理失败均归本次独占证据。HOME／APPDATA／XDG、原生目录、定义、持久化、数值回环端口和 Herdr 实例隔离；退出并确认所属进程结束后才清理。受控用例仅证明机制／故障；缺任何宿主、真实模型、能力或清理证据均非零，不能把 CLI fail-closed 或全局回归当作专项通过。完成检查仍分别运行票据要求的两仓全局入口；配对 dock 必须指向本次安全壳，真实专项 `--backend` 指向生产后端。

## Chief 双宿主人工恢复专项

从参与的 Herdr worktree 根运行 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-chief-recovery --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。路径必须绝对，证据目录必须不存在；依赖、页面、生产后端与 Cargo depfile 均核对参与 worktree。另以同一独立绝对 `CARGO_TARGET_DIR` 构建 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell --example recovery_herdr_proxy`。后者是验收专用外部 CLI 记录／故障代理，正常时透传到显式安装的 Herdr，仅由独占标记文件制造通知明确拒绝；不进入产品或正常部署。

`chief_recovery_native_acceptance.py` 独占 P23/P24/P25/P26/P27/P34/P35：分别观察通知失败、chief 创建／新进程恢复／重新登记、真实后端重启和时间流逝；真实 GPT-6-luna 在原发送方明确请求后选择原报告重发，完整原生预览、再次批准／拒绝和原 ID／正文逐项核对。故障代理只在实际后端提交之后持有／丢弃原响应，不伪造保存或接收成功；持久接收已可公开读回后、模型 admission 前终止所属原生进程。新 chief 不迁移旧上下文、不自动重播；用户明确查看才实际读取和呈现。重复／并发重发、丢失回执、更换 chief 和真实后端重启后核对原接收记录、最多一次处理授权及无新通知。独立 Chat 与直接 Map manager 分别新进程恢复原关联，恢复前后核对历史／模型请求，观察无初始输入重放、新委派或 Run；同 Task 的另一实际 Chat 恢复后仍无资格。改内容的原 ID 重发须拒绝，新内容的独立原生确认被拒绝时不保存／发送，旧记录保持原样。

每个新 chief 先接收一次无业务内容的用户就绪请求，等待原生历史实际持久化后才开始生命周期观察；新会话仍不包含旧报告 ID、正文或原委派上下文。无自动行为观察通过公开历史枚举实际 Session，逐个以 `GET /api/chief/delegations?sessionId=...` 读取关联，同时比较 Chat 身份集合与 Run；不使用不存在的无身份全局委派列表。原生 OMP 的字符串 `i` 仅作为宿主 intent 元数据记录，重发业务参数仍严格只含原 `id`，实际 HTTP 另行严格核对；已终止进程的 unregister 允许明确 `404 unknown-session`（后端重启后的内存登记可已不存在），其它错误仍失败。原报告预览按完整 JSON 对象比较，不把键顺序当内容差异；恢复前后和静默窗口的无业务模型调用观察使用同一计数边界，仅排除安装 OMP 无工具的独立标题／标题卡请求（其系统指引分别以 `You name coding-agent sessions.`／`You head coding-agent session titles with a small card.` 开始）；带业务工具、普通系统指引或 Pi 请求仍全部计入，仍保存并检查所有请求的实际模型身份；正常终止所属原生进程前，以既有30秒请求预算等待已在途请求完成，避免测试自身取消独立标题请求，不重试或忽略失败，所有用户／assistant／tool 业务消息必须保持不变。每个无自动行为窗口实际观察46秒，覆盖45秒隔离服务启动、30秒原生启动／请求、20秒确认，以及10秒在线＋30秒离线保留＋5秒维护完整上界；这些是测试观察预算，不是产品等待／重试策略。所有原生历史、确认原始参数／决定、HTTP、外部 CLI 调用、真实模型返回身份、源码／锁文件／产物／安装宿主身份及原始错误／清理错误保留在本次独占证据。HOME／APPDATA／XDG、定义、存储、原生会话、数值回环端口和 Herdr 均独占；持有进程句柄确认结束后才交还资源，删除目录不能替代退出。

模型配置沿用上文公开真实模型传输。用户单独授权的只读 Codex provider 可使用 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-codex --scenario recovery --hosts omp,pi ... --auth-file "$AUTHORIZED_AUTH"`，先 `--smoke` 核对实际 `serverModel=gpt-6-luna`；不发现、复制、改写或自动刷新用户认证，不回退或重试业务决策。它是实际工具决策的传输适配，不是两宿主直接 Codex wire 合规证据。入口实际执行依赖拥有的四份原生协议 dock，但受控 provider／HTTP／Herdr 只证明机制，不能冲抵本专项真实模型及双宿主矩阵。单宿主诊断、缺任一依赖／能力／证据或清理失败均不能整体通过。

可用两个独占证据目录分别运行 `--hosts omp` 与 `--hosts pi` 并行诊断；每次仍保持单宿主内的重启／接收因果顺序，单宿主入口退出非零且不能称整体通过。聚合必须核对两份宿主 `evidence.json` 全部七项 verified、各自 passed/cleanupErrors、实际模型身份与机械检查退出状态及同一源码／产物身份；不能仅看过滤后的进程退出码。`business-stderr.log` 按时间、host、阶段 started/completed 与已验证 coverage 实时 flush，最终 JSON 仍独立输出；宿主根证据记录准确失败。外部 CLI 记录在 `external-cli/calls/*.json` 以独占临时文件写完后 rename 发布，避免并发进程交错 JSONL 或读到半条记录。

CLI 安全与故障代理定向回归：`CHIEF_RECOVERY_TOOLKIT="$TOOLKIT_WT" python "$TESTER/scripts/accept-chief-recovery.test.py"`、`python "$TESTER/scripts/chief_recovery_faults.test.py"`；前者只证明 fail-closed／绝对路径／不覆盖旧证据，后者只证明原响应提交后的门闩和丢失行为。另设置 `CHIEF_RECOVERY_PROXY` 为本次 `recovery_herdr_proxy.exe`、`HERDR_FIXTURE` 为本次 `herdr-fixture.exe`，运行 `python "$TESTER/scripts/chief_recovery_cli.test.py"`，检查初始业务输入逐字透传、只拒绝 chief 报告通知、解除显式故障后原通知逐字透传。完成仍分别运行两仓要求的完整构建、后端／前端／浏览器及 Bun 1.4.4 typecheck／test／check:integrations；配对 dock 只使用本次安全关机壳与本次元数据，生产 `--backend` 不替代安全壳执行关机回归。全局套件不替代专项。
## Chief 存活旧进程与交接专项

从参与的 Herdr worktree 根运行 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-chief-authority --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。显式绝对路径、独占新证据目录、当前源码构建产物及隔离 roots 的要求与入口专项相同。模型严格使用 GPT-6-luna 并核对实际返回身份；授权只读 Codex provider 可沿用 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-codex --scenario authority`（先单独 `--smoke`），不发现用户凭据、不刷新认证、不回退或重试业务决策。

`chief_authority_native_acceptance.py` 独占 P31：两宿主分别保留旧 chief 真实进程与持有的 Windows 句柄；旧原生进程实际收到已提交报告通知并发起 `/receive` 时，在透明 HTTP 传输边界将该真实请求暂挂（最多18秒，短于原生20秒 admission预算），移除原登记并发起新 chief 后释放。真实后端必须拒绝旧原生接收，公开 receipt 仍为空；已提交的外部输入仍保留 `notification.status=submitted`，不能解释成已撤回或可靠接收。之后显式要求原发送者报告另一份新的独立结果，真实新 chief 必须获得持久 receipt 并呈现。此场景不重发同ID，不重复认领恢复专项的去重矩阵。

旧进程迟到登记、Task／Map 的旧预览提交及实际派发均须拒绝。新 chief 分别经真实原生预览与批准发起 Task／Map，公开读回 Chat／根 Run。保留有效 Map 的节点职责，不靠禁止所有节点执行来阻止向 chief 发送消息。`chief_report` 的预览和原生确认单独取证，不从节点成果批准或自由 `report` 推导。证据关联旧进程存活、竞争时序、原生身份、真实回执和公开报告／接收事实。

静默观察为46秒，覆盖实际10秒在线＋30秒保留＋5秒维护上界；配置、启动/请求/确认预算与原始失败、清理失败分别记录。所有 HOME／APPDATA／XDG、定义、持久化、动态数值回环端口、Herdr 和原生会话均独占；结束时关闭所属服务并以持有进程句柄确认退出，不以删目录代替。任何宿主/能力/模型/清理缺证据均非零，单宿主或 `CHIEF_AUTHORITY_TOOLKIT="$TOOLKIT_WT" python "$TESTER/scripts/accept-chief-authority.test.py"` 的CLI安全检查不算P31通过。

两仓完成检查仍分别执行票据要求的完整 frontend build、当前后端/fixture/paths/安全壳构建、`node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`，以及Bun 1.4.4的 `bun run typecheck`、`bun run test`、`bun run check:integrations` 和受影响 `native-chief-receipt-backend`／`native-chief-reception` dock。独立绝对 `CARGO_TARGET_DIR`；dock 的 `HERDR_BOT_BINARY` 只能指向本次 `safe_shutdown_shell.exe`、元数据指向参与源码，真实专项则使用生产后端。完整回归或受控dock仅证明其实际范围，不替代双宿主真实模型专项。

## 前端与浏览器

通过 npm 脚本运行，保留网络护栏；直接调用 Vitest／Playwright 会绕过 npm hook。脚本定义以 [package.json](../../frontend/package.json) 为准。

- Vitest 的环境和并发见 [vitest.config.ts](../../frontend/vitest.config.ts)。组件使用 jsdom，纯逻辑测试可使用 Node；保持测试文件 VM 隔离，调整 worker 数而非关闭隔离。
- Suite 技能说明的页面断言须区分最终技能白名单与原生目录发现：排除未选中技能不等于不扫描默认目录；空清单仍表示禁用技能。
- 浏览器调度见 [playwright.config.ts](../../frontend/playwright.config.ts)。文件内并行场景保持每个用例独占服务、可变存储和 Herdr 副本；浏览器由 Playwright 按 worker 复用。
- 资源受限或诊断并发问题时，给浏览器入口追加 `-- --workers=1`；Vitest 可追加 `-- --maxWorkers=4` 等覆盖参数。
- 用 `elementFromPoint` 核对 SVG 连线等真实绘制时，先将目标滚入视口；`toBeVisible` 不保证元素位于视口内，不能直接作为命中测试的前提。
- 以可访问名称、实际命中和公开 HTTP／SSE 读回验证浏览器行为，不用装饰符或无关序列化细节当判据。保存后核对完整对象，显示失败与空结果不能混用。
- 跨节点报告夹具须用真实来源 Chat 与其 execution ID；不可只改 `nodeId`。验证依赖资格时显式保存 `inputsReady=true`。普通节点 complete 的批准前提按[本页后端回归](#普通节点成果批准s1)建立，不由共享夹具补齐。
- 移动 request controller 时同步维护证据的源码哈希。即使采集或写入失败也要清理进程、恢复环境变量，并保留原始错误。
- 完整浏览器入口不加快速过滤。分层开发反馈使用 `test:browser:fast`／`test:browser:time-contract`；修改选择或标签后运行 `npm --prefix frontend run test:browser:partition`，核对完整集合是两分区的不相交并集。
- `@time-contract` 标记真实预算、确认超时、迟到结果及无自主刷新／重试／重放的观察窗口，不按文件名或断言 timeout 分类。新增真实时间承诺时同步审查标签。
- OMP／Pi 历史验收核对缺失启动证据的超时和无效官方身份的明确失败；页面不宣称输入已提交，迟到上报不触发读取、提交或补写历史。已提交后的历史保存失败仍只记录关联的服务端诊断，不改变创建结果。
- Chat 的纯临时输入场景（空固定 prompt、无 Skill、无词典引用）应独立核对完整投递文本与输入逐字相等，不假定末尾空段或无条件追加 agent 指引。

## 修改测试或夹具

- **所有权**：每个用例独占可变临时数据与动态回环端口；同一竞争／重启场景保留必要共享。临时夹具目录须原子创建并跳过已有目录，不能因 Windows 重用 PID 而接管旧运行残留的数据。浏览器夹具探测端口后到子进程绑定前仍存在竞争；仅动态端口的已确认地址占用错误可重新分配（最多 32 次），固定地址及其他启动失败直接报错，不重试业务操作。自动发现目标不等于自动消除资源冲突。
- **路径**：真实子进程使用测试拥有的临时目录。普通相对路径按声明配置文件的父目录解析；固定扩展入口相对元数据文件，Suite 扩展入口相对 Suite 文件解析；cwd 可以与配置目录不同。需要根引用时生成隔离根文件，不读取开发者的 `paths.local.toml`。启动期 `data_directory` 与迁移标记也要归测试所有。
- **环境**：`HERDR_FIXTURE_DIR`、`HERDR_BOT_FIXTURE_RECORD_DIR` 会覆盖默认夹具位置；继承环境时保证它们不把不同用例导向同一可变目录。
- **进程**：取得真实子进程端口后持续消费 stdout／stderr，直到进程退出。初始化失败与正常结束都执行所属资源清理；同时保留原始错误和清理错误。
- **并发持久化**：保留完整回执及所属后端诊断，区分事件接受与历史保存。文件原子替换前的身份观察可能已过期，不能用它证明最终文件授权。定向重跑不抹去首次失败。
- **Git 超时**：`zone_sync`／`zone_commit` 要验证实际超时步骤。不能用更早的 fetch 失败冒充 push 超时；核对已发生的 pull 不回滚、所属派生进程退出和后续可同步。
- **Windows 身份与可见性**：提前持有实际进程句柄，避免 PID 复用。使用真实窗口最小化／恢复验证页面 `hidden`／`visible`；聚焦不保证可见，不用始终可见模拟代替。
- **文件与元数据**：保留 `input_files` 的顺序、ID 与必填约束，不排斥合法 `description`。修改交付模型或标签时独立审阅预期，不从被测定义重算答案；这不改变固定模型专项的范围。
- **关机与会话**：关机自动验收使用记录型安全壳或替身；真实关机单独人工验收。会话探针使用隔离数据，不操作用户正在运行的对话。

## Suite 配置与 Task API 格式

按 [Task 选择与引用保护](../usage.md#task-单-suite-选择与引用保护) 和 [单宿主 Suite 决策](../adr/0020-single-harness-suite-types.md) 准备夹具。每份 Suite 只属于一个 `harness`；跨宿主对照用两份 Suite，且不可把 Task 的旧 `extensions` 当作有效配置。定义接受不证明原生资源已加载。浏览器的 Pi 启动／恢复参数检查单独保留 `builtin:mcp`、`builtin:tool-search`、适用的 `builtin:codemode` 和固定 `pi-suite` 的预期，不把内建扩展误当作缺失的文件入口。测试不授权迁移用户部署数据。

## 首批 Suite 原生发现与实现命令

`accept-initial-suites` 检查三套首批 Suite 的原生技能来源、实际注册并运行的 `hyper-auto-coding`，以及两仓交付来源。完整命令、独立名称／来源预期及 CLI 回归见 [首批 Suite 原生验收](../initial-suite-native-acceptance.md)。验证失败保留未验证，其他资源或生命周期专项不能补足。

## OMP Suite 四语言 LSP 真实验收

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-real-herdr-suite-lsp` 拥有 P37/P38 的 S4 专项；S1 缺程序声明反馈由 `frontend/browser-tests/suites/suiteLsp.spec.ts` 覆盖，S2/S3 生成产物由 `backend/tests/suites/suite_omp_lsp.rs` 补充。后两者不替代真实服务启动。先构建当前 worktree 后端；四种服务、相关语言运行环境及 OMP／Herdr 必须已经安装好，runner 不安装、不提权、不修改 PATH，不用源码 CLI／SDK 或 Pi 替代已交付的 OMP 入口。

```powershell
cargo build --manifest-path "$BOT/backend/Cargo.toml" --bin herdr-bot
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-real-herdr-suite-lsp `
  --herdr 'C:/installed/herdr/herdr.exe' `
  --omp-package 'D:/node_modules/@oh-my-pi/pi-coding-agent' `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --rust-toolchain 'C:/installed/rustup/toolchains/stable-x86_64-pc-windows-msvc'
```

`--rust-toolchain` 只设置子进程 `RUSTUP_TOOLCHAIN` 为既有工具链目录，同时使用独占 `CARGO_HOME`，避免 HOME 隔离后 Rustup 代理找不到原先安装的组件；不安装组件、不读取用户 Cargo 凭据、不改 PATH。不使用 Rustup 代理的部署可省略。四种原生命令为 `rust-analyzer`、`typescript-language-server --stdio`、`jdtls`、`pyright-langserver --stdio`；TS 还需兼容 TypeScript，Java 需 JDK／JDT LS，不能仅凭 launcher 或版本号判断可工作。

入口在独占 Temp 配置中声明批准的四服务，通过公开 Task／Chat HTTP → 真 Herdr → PATH 上安装版 OMP，为 Rust／TypeScript／Java／Python 分别创建项目标记和源文件。受控数值回环模型只调用真实 `write xd://lsp`，检查 `capabilities` 中实际 initialize 响应及随后 `status` 的 `ready`，并通过 Windows OS 查询独立观察所属新增语言服务进程。它证明原生启动和发现机制，不证明模型自主开发质量。缺可执行程序负例保留 Rust 项目标记，由测试拥有的用户 LSP 配置将命令指向确实不存在的程序：原生能力查询拒绝、status 不包含服务、OS 没有所属 rust-analyzer 进程。正例用项目配置覆盖该用户命令，核对项目 > 用户 > Suite 的原生优先级；不关闭其他默认语言服务。

每次输出独占目录内 `evidence.json`，记录源码 revision／diff／哈希、交付二进制与安装 CLI 哈希、版本、隔离来源、实际请求／响应、Herdr terminal、所属进程及每场景 `verified`。HOME／APPDATA／XDG、agent 数据、后端持久化、命名 Herdr server 和动态回环端口均隔离；固定扩展只记录原生进程身份，不替换 LSP 或内部 monkey patch。持续消费服务日志，正常及失败均停止所属服务、以持有的 Windows 进程句柄确认退出；原始错误与清理错误分别保留。不连接用户 Chat／daemon，不复制认证或操作真实关机。

默认 `--language all` 才是四语言 P37 验收；缺任何程序会记录 `blockers`、保持未验证并非零退出，已有声明、非空日志、零退出的其他场景均不能冲抵。`--language rust-analyzer` 等参数只用于较窄反馈，成功也不是完整 P37 通过。P38 UI 只说明声明／运行的区别，不伪造服务健康状态；运行事实以原生反馈和 OS 观察为准。

## Suite 装配发布与链接安全验收

`backend/tests/suites/suite_assembly.rs` 拥有 P34/P36 和 P35 的装配发布部分，入口仅为既有公开 Task／Chat HTTP 与独占部署目录（S2/S3），不增加测试专用产品 API。常规 `cargo test --manifest-path backend/Cargo.toml --test suite_assembly` 检查资源消失、Windows 独占读锁造成的真实不可读失败、未拥有文件保留，以及同 Suite／不同 Suite 的两套独立 LSP 与 MCP 配置哨兵；核对 MCP 原生资源包与技能配置、LSP 共属本次装配树，技能源路径保持在原位置，返回的 `--extension` 指向该包。外部 Herdr 夹具在启动效果处受控阻塞；第二个启动必须仍能取得完整产物，随后重读两套最终清单、释放门闩并等待公开完成状态，确保文件协调不跨外部效果。观察失败也先释放门闩并等待已接受的所属命令，原始错误与清理错误分开保留。这些夹具不是原生 agent 消费证据；P35 并发真实启动消费由独立专项负责。

非空技能的原路径成功发布属于默认回归，不依赖 `HERDR_BOT_MANUAL_SYMLINKS`、第二盘或 Windows 链接权限，也不能标为 ignored 后宣称完整通过。验证同 Suite／不同 Suite 并发启动中的独立源技能正文、相对资产和配置，失败只清理本次拥有的配置产物，保留源资源、旧产物及未拥有文件。

受控启动夹具不证明原生 agent 消费；完整交付仍需下述真实 OMP／Pi 场景，并记录实际已安装宿主及 OS 进程身份。OMP 使用现有全局安装，不部署新宿主构建；原生最终技能列表与真实消费不可由配置文件存在推断。完整后端回归使用 `node scripts/test-backend.mjs`。

## OMP Suite 技能真实消费验收

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-real-herdr-suite-skills` 专项覆盖 P5/P6/P7/P8/P10/P15 的 OMP 技能场景，必须先构建当前 worktree 的 herdr-bot 后端。它经公开 Task／Chat HTTP → 真实 Herdr → **现有全局安装的 OMP** 正常交付入口运行，不直接调用源码 CLI、SDK 或替代 MCP manager。依赖均须预先具备，runner 不改全局安装或 PATH、不提权、不修改开发者模式；不修改 OMP 源码、不构建或部署新的宿主。`--omp-package` 只声明已有安装包，不替换实际入口。Windows PTY 会从注册表重建 PATH，单独给父进程添加 PATH 前缀或放置另一份构建产物不能可靠指定宿主。runner 保留 OS 实际进程的 ExecutablePath 与 CommandLine，结合原有安装入口／包身份及实际技能消费核对运行身份；版本或文件哈希本身不能证明 Herdr 实际运行了所声明的安装。

验收检查现有 `--config` 对 `skills` 的高优先级局部覆盖：每次启动从所选集合重新取得有效直接子技能，生成最终名称 `includeSkills` 白名单，七个已有 `enable*` 来源开关均为 `false`、`ignoredSkills` 为 `[]`，有效名称为空时 `enabled: false`。核对原生最终可用列表及真实源路径，而不是要求默认目录完全未被扫描；不得把扫描行为与最终技能列表隔离混为一谈。原生 MCP、LSP、agent、扩展和项目指令不因技能覆盖而关闭。

```powershell
cargo build --manifest-path "$BOT/backend/Cargo.toml" --bin herdr-bot
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-real-herdr-suite-skills `
  --herdr 'C:/installed/herdr/herdr.exe' `
  --omp-package 'D:/node_modules/@oh-my-pi/pi-coding-agent' `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe')
```

默认 `--scenario selected` 创建独立根引用／声明相对路径／含空格的技能集合及名称含 `*` 的技能，在两个不同 Chat cwd 中实际 `read skill://...` 读取技能正文、scripts、references 与资产；第二次发起前向显式来源集合新增技能，验证后续发现将其纳入；未声明目录中的技能不参与。用户／项目／managed 默认技能、已加载插件的同名错误正文及可被错误通配符匹配的技能均为独立哨兵；原生最终列表必须排除它们。第二次启动还核对第一次的配置未被改写。受控回环模型检查原生最终技能列表，只要求真实 read／write 工具执行预定动作；通过实际原生 `xd://mcp__native_probe` 调用取得字面结果，并核对 MCP 服务器记录的 `tools/call`。同次先验证 OMP Suite 的空及非空 `prompts` 字段均被 API 拒绝，再观察另一份 Pi Suite 的模板哨兵未注入 OMP、原生 `AGENTS.md` 指令仍在。这里只证明协议与资源消费机制，不证明真实模型自主遵循业务职责。

runner 隔离 HOME／APPDATA／XDG、agent 配置与会话、命名 Herdr server、动态回环模型／后端和源／生成目录，不读取用户凭据或已有 Chat；静态页面使用测试拥有的占位文件，因为本入口不承担 S1 页面验收。正常及失败路径持续消费服务日志、停止所属进程，持有真实 MCP 进程句柄确认退出；证据 JSON 分开保留原始错误和清理错误。输出指向独占 Temp 的 `evidence.json`，保留实际请求／响应、公开入口结果、MCP wire、来源 revision／diff／哈希、源路径、配置产物及原始日志，不在仓库生成验收报告。

技能直接引用源路径，不需要链接权限或手动 opt-in。必须在不能创建符号链接的普通 Windows 环境实际消费非空技能及相对资源，参数检查不能代替成功证据。`--scenario empty` 是独立的较窄场景：无需建链接，验证明确空技能 Suite 的技能过滤、原生 MCP 实际调用、模板省略及原生指令保留，**不能**替代 selected 场景或证明非空技能／资产消费。缺任何真实依赖或必要宿主能力，完整专项仍是阻塞／未验证。

公开启动的受控失败回归为 `cargo test --manifest-path backend/Cargo.toml --test suite_omp_skills`，使用真实 Windows 独占共享锁制造预览后不可读资源，以及普通文件阻塞生成目录；检查 HTTP 明确拒绝且 Herdr 无调用。这些负例不需要 symlink 权限，不替代 S4 成功证据。

## OMP Suite 原生 MCP 真实消费验收

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-real-herdr-suite-mcp` 覆盖 Suite 契约 P17/P19/P20/P21/P22/P46 的 OMP MCP 场景。先构建当前 worktree 后端，使用实际安装的 Herdr 与 PATH 上的打包 OMP；不安装依赖、不修改 PATH、不替换 agentDir、不运行源码 CLI 或 SDK 组装。它复用技能专项的独占 HOME／APPDATA／XDG、配置、会话、命名 Herdr server、动态回环模型／后端和进程清理设施，但不认领所选技能验收。MCP 夹具只充当外部服务器，实际读取独立声明目录的字面文件；原生发现、优先级、完整 manager 与工具执行仍是实际交付宿主。先经公开 HTTP 发起原生管理 Chat（空 Task 固定 prompt、初始输入 `/mcp help`），在成功场景观察外部夹具初始化及原生界面后，用原生 Esc／Ctrl+U 退出帮助或补全并清空管理输入，再通过 Herdr 原生 agent 交互提交一次 MCP 读取指示；不承担初始化文本的终端粘贴传输验收，不重试业务提交、不补发 Enter。此等待和管理交互仅是测试前置条件，不写入产品启动路径；`unreachable` 独立验证产品没有全部就绪门禁。

```powershell
cargo build --manifest-path "$BOT/backend/Cargo.toml" --bin herdr-bot
foreach ($scenario in @('central', 'precedence', 'empty', 'full', 'unreachable')) {
  python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-real-herdr-suite-mcp `
    --herdr 'C:/installed/herdr/herdr.exe' `
    --omp-package 'D:/node_modules/@oh-my-pi/pi-coding-agent' `
    --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') --scenario $scenario
  if ($LASTEXITCODE -ne 0) { throw "Native MCP acceptance failed: $scenario" }
}
```

- `central`：空项目通过集中引用调用服务器，核对真实文件字节、声明相对 cwd／根引用命令及项目文件未改变。
- `precedence`：用户、项目、Suite 同次实际调用；独立指定项目同名胜者、等价异名只连一次、不同参数分别连接，以及项目禁用和用户 denylist 均不启动；另读原生 `/mcp list` 确认来源。
- `empty`：明确空增量下用户／项目服务器实际调用，原生来源未关闭。
- `full`：实际工具调用、资源列举和读取、服务器 instructions、`/mcp list/resources/prompts/notifications` 原生管理报告、prompt 获取及正文投递分别观察；通知后三个 catalog 真正刷新，资源 URI／prompt 描述改变，新工具实际可调用。不能以工具桥接或旧终端文字充当管理报告。
- `unreachable`：测试独占动态回环端口绑定但不监听，产生真实连接拒绝；Chat 发起仍被接受，其他服务器实际完成初始化，原生终端明确显示该服务器 `Failed: unreachable`／`ECONNREFUSED`。此场景不要求模型 turn 已开始或可用工具已调用，不将发起成功称为 MCP 就绪；可用服务器实际调用分别由其他场景承担。

每次输出独占 Temp 的 `evidence.json`，保留实际模型请求／结果、HTTP、MCP wire／连接次数、原生终端管理报告、源码 revision／diff／文件及产物哈希。MCP 夹具在初始化前等待 runner 持有真实 Windows 进程句柄，独立 watcher 持续观察，包括宿主退出时可能出现的晚到 reconnect；停止所属服务后清理并确认每个所属进程退出；来不及观察就已被宿主终止的晚到 PID，单独记录 Windows `OpenProcess` 的 `ERROR_INVALID_PARAMETER`（87）作为该 PID 已不存在的独立证据，访问拒绝等其他错误仍失败。原始验证失败与清理失败分别记录；缺实际依赖或任一能力不满足均非零，不 skip。回环确定模型只证明宿主协议与执行机制，不证明真实模型的自主业务遵循。此专项不要求技能目录链接权限，不替代非空技能专项、页面验收或 Pi MCP 场景。
## OMP Suite 扩展包真实消费验收

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-real-herdr-suite-extensions` 专项负责 P7（OMP 扩展）与 P9（OMP）的 S2/S3/S4 行为证据。先构建当前 worktree 后端，再使用已安装 Herdr 和 PATH 上正常交付的 OMP；依赖不由 runner 安装，不改系统 PATH，不读取用户凭据或启动用户 Chat：

```powershell
cargo build --manifest-path "$BOT/backend/Cargo.toml" --bin herdr-bot
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-real-herdr-suite-extensions `
  --herdr 'C:/installed/herdr/herdr.exe' `
  --omp-package 'D:/node_modules/@oh-my-pi/pi-coding-agent' `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe')
```

入口复用技能专项的独占配置、动态回环模型／HTTP 端口、持续消费日志和所属进程清理。一个 Suite 声明相对入口文件与集中根引用的具体非 index 源入口，均包含空格；直接传给原生宿主，不生成扩展链接或入口清单，也不自行展开 package.json。两个不同 Chat cwd 均放置错误相对入口哨兵。真实工厂 import 同包 sibling TS 和 `node_modules` 模块，真实工具读取 `import.meta.url` 相对 JSON 资产并计算，独立字面预期为 `RELATIVE-ASSET-412:64` 与 `ROOT-ASSET-905:80`。受控模型通过 OMP 原生 `write xd://...` 调用工具，不伪造工厂或工具执行；此证据只证明机制，不证明真实模型自主遵循。

验收核对没有生成扩展包链接或 `omp-extensions.json`，来源文件字节不变且固定扩展仍加载。输出的独占 Temp `evidence.json` 保留源入口、公开 HTTP 结果、原生模型请求／工具结果、源码与可执行文件哈希、原始日志和分开的清理错误。此空技能专项不需要符号链接权限；任何真实依赖缺失或工具消费失败均非零退出并记为未验证，配置列表存在或普通套件通过不能替代原生直接加载的 S4 成功证据。

P46 的扩展负例入口为 `cargo test --manifest-path backend/Cargo.toml --test suite_omp_extensions`：真实 Windows 独占共享锁阻止实际源入口读取，经公开预览／启动 HTTP 检查明确拒绝且 Herdr 无调用。装配器只检查入口可读性，不遍历包内资产或 node_modules；模块／资产由原生宿主从源目录读取。负例不需要链接权限，不代替上述真实成功场景。

## Pi Suite 启动开发烟测

开发烟测经公开 Task／Chat HTTP，由真实 Herdr 启动 PATH 上的安装版 Pi。预先准备依赖；Toolkit 必须提供公开的 `--pi-suite-config`。完整覆盖另见 [Pi 资源与 MCP](../pi-suite-resource-acceptance.md) 和 [Pi 模式与模板](../pi-suite-mode-acceptance.md)：

```powershell
cargo build --manifest-path "$BOT/backend/Cargo.toml" --bin herdr-bot
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-real-herdr-suite-pi `
  --herdr 'C:/installed/herdr/herdr.exe' `
  --pi-package 'D:/node_modules/@earendil-works/pi-coding-agent' `
  --toolkit 'D:/installed/my-agent-toolkit' `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe')
```

Pi Suite 直接引用含空格的技能源目录与具体的非 `index` 扩展源文件，不生成链接，也不通过包清单选择入口。受控模型让 Pi 实际读取技能正文，并调用扩展工具读取相对 JSON 资产。独立预期为 `PI-SKILL-BODY-731` 和 `PI-PACKAGE-ASSET-482:64`（19×3+7）。这只证明原生消费机制，不证明模型自主遵从。

持续消费进程输出，正常及失败均停止所属 Herdr/server/子进程，原始错误与清理错误分别写独占 Temp 的 `evidence.json`；记录源码 revision／diff／哈希、二进制和安装入口哈希、实际 HTTP／模型请求及 terminal。依赖／flag 未交付、实际资源未消费或清理失败均非零，不 skip、不退化为直读源码加载，不把产物／参数检查算作 S4 通过。

## Pi Suite 资源与原生 MCP 跨仓验收

实际资源路径、原生读取、MCP 优先级、禁用及失败矩阵见 [Pi 资源验收](../pi-suite-resource-acceptance.md)。另按目的选择 [Pi 模式与模板](../pi-suite-mode-acceptance.md)、[Suite 会话生命周期](../suite-lifecycle-acceptance.md) 或 [pstack 原生机制与离线包检查](../pstack-suite-native-acceptance.md)。各页保留独立预期和完整命令。这些受控模型专项不认证模型自主判断。

## 真实 Herdr 自动关闭验收（P22）

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-real-herdr-close` 是独立的真实依赖自动验收，不在普通替身回归中隐式启动。当前入口要求 Windows、已安装的真实 Herdr / Pi、Python 3.9+、Node.js，以及已安装依赖的 toolkit checkout。先构建当前 worktree 后端。路径参数可显式传入，或使用 `--paths-file`（默认 `$BOT/paths.local.toml`）：缺省参数从 `herdr/herdr.exe`、`pi_package`、`pi_agent/extensions/herdr-agent-state.ts`、`toolkit` 根派生；backend 默认取当前源码的 debug 产物并尊重 `CARGO_TARGET_DIR`。显式参数优先，全部依赖参数均提供时不读取根文件。原生会话验收 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-native-session-storage` 同样支持 `--paths-file`，缺省包路径使用 `pi_package`、`omp_package`。这些入口经 Rust `herdr-paths` 解析 TOML，不自行实现根引用；根可指向不同磁盘。

以下为显式参数方式：

```powershell
cargo build --manifest-path "$BOT/backend/Cargo.toml" --bin herdr-bot
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-real-herdr-close `
  --herdr 'C:\path\to\herdr.exe' `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --pi-package 'C:\path\to\node_modules\@earendil-works\pi-coding-agent' `
  --pi-state-extension 'C:\path\to\herdr-agent-state.ts' `
  --toolkit 'C:\path\to\my-agent-toolkit'
```

入口创建独占 Temp 根目录，清除继承的 Herdr 定位及模型凭证环境，显式隔离 HOME、APPDATA、XDG 配置/状态、Pi 原生存储、命名 Herdr server 和后端持久数据；不连接现有 Herdr，不读取用户 Chat。真实 toolkit 的角色入口及配置复制到临时目录，只修改副本的后端地址；官方 Pi Herdr 状态扩展使用显式只读路径。模型使用数值回环的确定响应服务，不访问真实模型或使用真实密钥。后端仅监听动态回环端口，关闭 mDNS。

验收通过真实发起 Run、manager dispatch 和 complete HTTP 路径触发自动关闭，绝不以直接 `tab close` 替代。先记录同一 Workspace 的专属保留 Tab、目标执行 Tab、其分割 pane 中实际存活的受控 Python 进程及原生 Chat 内容。之后独立查询 Herdr Tab 列表、持有 Windows 进程句柄确认进程退出，并使用已安装 Pi 的 `SessionManager.open(...).getBranch()` 再次取得原生 user/assistant 历史；不以历史列表登记或 completed 状态冒充效果，不把历史读取当作重新派发。保留 Tab 避免关闭最后一个 Tab 清空 Workspace，不强制确认，不承诺清理已脱离会话的 daemon。

stdout 返回 `evidence.json` 路径和真实结果；该文件包含命令原始输出、HTTP 响应、前后身份、存活及历史内容/哈希。Herdr/后端 stdout 与 stderr 持续消费并保存独立日志。成功或失败均停止且确认测试拥有的 server/子进程，原始错误与清理错误分别保留；Temp 数据保留供证据核对，不删除用户资源。缺失依赖、额外确认、目标仍在、进程仍活、原生历史不可读等均返回非零，不跳过且不算通过。文档仅说明入口，实际通过/失败以执行日志为准；本入口只验收 P22，不代表 P1–P21 已通过。

## 嵌套流程真实文件消费验收

独立入口 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-nesting-files` 经本 worktree 构建的真实页面发起 Run，加载指定 toolkit worktree 的真实角色入口。toolkit 使用 Bun 1.4.4 和 frozen lock；若 typecheck 明确报告缺少已锁定的 `@typescript/typescript-win32-x64`，可在该 toolkit worktree 执行 `bun install --frozen-lockfile --linker isolated --force` 修复本地依赖链接，不替换编译器、不改锁定版本或借用其他 worktree 源码。不要用旧的普通文件交接场景、受控 provider 或最终产物替代它。入口的隔离业务定义显式声明 Pi；`--hosts declared` 保留并执行这些 Task 声明，证据只记 Pi，不外推 OMP 或双角色生命周期通过。现有真实宿主设施不提供 OMP 文件消费能力，显式 `--hosts omp` / `--hosts omp,pi` 非零并保持未验证，不能以 Pi 成功抵消。

在构建前，将 `CARGO_TARGET_DIR` 设为本场景独占的绝对路径。

```powershell
Set-Location -LiteralPath $BOT
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --example safe_shutdown_shell
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-nesting-files `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --hosts declared `
  --paths-file 'D:/isolated-acceptance/paths.toml' `
  --model-config 'D:/isolated-acceptance/model.json' `
  --evidence-dir 'D:/isolated-acceptance/new-exclusive-run' `
  --shell-path 'D:/Program Files/Git/bin/bash.exe'
```

依赖声明是验收专属 TOML，使用 `[roots]` 下已安装的 `herdr`、`pi_package`、`pi_agent`（只读 `extensions/herdr-agent-state.ts`）及 `chrome`（包含 `chrome.exe` 的目录）。`--chrome <绝对chrome.exe路径>` 可显式选择已经安装的独立 Chromium；不自动发现或下载替代浏览器。Google Chrome 在完全隔离 Windows known-folder 环境中可能将显式独占 profile 误判为默认目录而拒绝调试，此时保留失败，不添加绕过调试限制的 feature flag、不借用用户 profile。已安装 Chromium 的成功只记录该实际浏览器身份，不覆盖 Chrome 失败。不安装宿主、不改 PATH、不读取用户认证。真实模型配置沿用下节的专用 `provider/model/baseUrl/apiKeyEnv` 声明或公开 `CompletionTransport`，不允许确定响应替代。后端必须来自同一独立绝对 `CARGO_TARGET_DIR`；两仓源码根、revision/diff、锁文件、后端身份、已安装依赖及真实 OS 进程均保留。缺依赖非零，不能以准备完成作为通过。

场景两次经真实浏览器分别提交独立业务文字和不同文件字节，运行 root → middle → leaf 命名重映射，再让子交付分别进入普通执行者及另一子流程。实际 native read 调用与对应 tool result、原生 Session/Run/执行身份、输入绑定和交付字节共同作为证据，不只比地址。根空输入与嵌套空输入执行者不得读取或猜测材料路径，可选 appendix 明确缺席。首次 leaf 执行者实际停轮等待业务 release 后，driver 原地改写已派发文件；另一次实际派发后删除输入，再明确 release，真实执行者必须保留读取错误、报告不可完成，1.25 秒窗口内不修复或重试。release 是记录在原生历史的显式业务输入，不编辑运行记录，也不替模型选择工具。

证据目录必须不存在，由 runner 独占创建；HOME/APPDATA/XDG/原生存储、命名 Herdr、定义和运行目录全部隔离，页面使用独立 Chrome profile，后端只绑定动态数值回环并关闭 mDNS。持续消费 stdout/stderr，正常与失败均停止并等待所属进程；原始错误与清理错误分别保留，清理失败仍非零。保留 browser 请求与确认截图、原始 HTTP/模型请求结果、原生历史、文件原字节、只读运行事实和逐项结论；删除带凭据的临时 models.json。专用 fail-closed CLI 回归：设置 `WORKFLOW_HANDOFF_TOOLKIT=$TOOLKIT_WT` 后运行 `python "$TESTER/scripts/accept-workflow-nesting-files.test.py"`；它不证明真实消费。

## 文件交接真实角色自动验收

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-workflow-file-handoff --scenario roles` 是独立的跨仓入口，加载指定 toolkit worktree 的实际角色源码及当前 herdr-bot worktree 构建的后端。它不使用确定响应服务替代真实业务模型，也不修改用户部署。当前真实模型链路仅支持 Windows / Herdr / **Pi** 和 OpenAI-compatible chat-completions 服务；结果不外推为 OMP 真实模型通过。参数保真、严格 report 校验和真实后端落盘另由入口内调用的 `tests/harness-dock/node-report-contract.test.ts` 同时覆盖 OMP/Pi；该 dock 使用受控 provider/Herdr，不能充当模型职责遵从证据。

先在两个参与源码根安装/构建依赖：toolkit 使用 Bun 1.4.4 与 `bun install --frozen-lockfile`；herdr-bot 执行 `bun install --cwd=frontend --frozen-lockfile`、`npm --prefix frontend run build` 生成本 worktree 的 `backend/static`（全后端进程检查必需）；后端使用独立的 `CARGO_TARGET_DIR` 构建 `cargo build --manifest-path backend/Cargo.toml --example safe_shutdown_shell --bin herdr-fixture --bin herdr-bot`。临时 toolkit worktree 的 frozen baseline 依赖定位可显式设置 `OMP_BASELINE_RECORD_ROOT` 为原 toolkit 根，不能因此替换实际被测源码。按 toolkit 的测试规范协调重型运行窗口。

提供**专用验收配置** `isolated-paths.toml`，不得指向用户业务、现有 Session 或用户 agent 配置。路径根仍经 `herdr-paths` 解析；例如：

```toml
[roots]
herdr = 'C:/installed/herdr'
pi_package = 'D:/node_modules/@earendil-works/pi-coding-agent'
# 仅只读加载此根下 extensions/herdr-agent-state.ts，不复制用户状态。
pi_agent = 'D:/isolated-dependencies/pi-state-extension'
workflow_model = 'D:/isolated-acceptance/model'
```

`workflow_model/model.json` 是专用模型服务声明，不是用户 `models.json` 或 auth store：

```json
{
  "provider": "acceptance-service",
  "model": "your-real-model-id",
  "baseUrl": "https://your-authorized-service.example/v1",
  "apiKeyEnv": "WORKFLOW_ACCEPTANCE_API_KEY",
  "contextWindow": 128000,
  "maxTokens": 8192,
  "reasoning": false
}
```

密钥只从显式命名的环境变量读取，不写证据/参数/头日志，退出时移除临时 credential-bearing models.json。没有已授权真实服务/密钥时，入口非零失败，不能拿 bridge token、受控 provider、用户全局 auth 或现有会话代替。`--herdr`、`--pi-package`、`--pi-state-extension`、`--model-config` 可覆盖依赖根派生路径；`--backend`、`--toolkit`、`--paths-file` 必须显式传入。

```powershell
$env:OMP_BASELINE_RECORD_ROOT = 'D:/original-toolkit'
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" test-workflow-file-handoff `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --scenario roles --paths-file 'D:/isolated-acceptance/isolated-paths.toml'
```

`--scenario plan-workflow` 的独立能力缺口阶段会通过真实 Chat 提交 `P28-GAP-SCOPE` 用户澄清：只确定税务申报表这一业务对象，系统、接口、材料、凭据及生产授权仍为未知且阻塞；要求保存并读回能力建设需求，不补齐缺失能力、不批准概要方案或 manual 报告、不授权创建或执行。原始输入及回答保留在 interventions 和原生历史中。

Windows 验收可显式追加 `--shell-path 'D:/Program Files/Git/bin/bash.exe'`，将实际安装的 Git Bash 写入本次隔离 Pi settings，不修改用户配置。不要让原生 Bash 工具落到 WSL：其 Linux 回环与 Windows 后端不同，`curl 127.0.0.1` 会报告连接失败，不能把该环境阻塞当业务能力缺口或强行推进。toolkit 全量跨仓检查还须将 `HERDR_BOT_BINARY` 设为本次 `CARGO_TARGET_DIR` 下 `debug/examples/safe_shutdown_shell.exe` 的绝对路径，将 `HERDR_BOT_METADATA` 设为 `$BOT` 下 `meta-data/task-options.toml` 的绝对路径；`HERDR_BOT_REPO` 不替代这两个 interop 参数，禁止使用生产可执行文件冒充安全壳。

重定位 `extensions/workflow-chat/config.yaml` 时保留全部必填配置。`approveConfirmationTimeoutMs` 用于人工节点成果批准，`reportConfirmationTimeoutMs` 用于 chief 报告和委派的独立确认，普通请求用 `requestTimeoutMs`；节点 `report` 不等待发送确认。模型已开始请求不等于角色已加载，须核对所属角色的 `run`／`collect` 或 `approve`／`report`。

透明观察只转发真实模型和 HTTP 数据。核对首轮旧文件及报告、明确授权的 fail／retry、新执行的部分文件与 `false` 报告、complete 拒绝及 collect 实际等待。外部业务文件释放只提供业务授权，不能替代原生节点 `approve`。当前验收还须证明新执行按冻结模式批准、报告最新 `true`、登记所需输出，随后由 manager 显式 complete／dispatch，消费者读取实际材料。旧执行的成功输出不能补格。

证据保留在输出的独占 Temp 根：源码根/revision/diff、锁文件与后端哈希、依赖版本、模型服务身份（仅密钥指纹）、实际模型请求/响应、工具参数/结果、HTTP 起止、原生 Session 历史、实际文件、只读运行记录副本及每项覆盖关联。独立背景、业务 input、当前地址和正文/聊天敏感标记核对原生初始化；manager 工具轨迹限定为 run/collect，不默认读输出正文。报告只允许简短编排结论，不复制详细行。观察窗口默认每阶段 300 秒（`--timeout 1..900`），外部业务等待 240 秒，保留报告后 collect 至少观察 1.25 秒仍未返回，再释放新报告；runner 不重试业务/模型选择，不延时掩盖失败。

若宿主只公开真实文本 `completion(prompt, model='default').wait()`，可用测试专属 `scripts/acceptance/host/completion_transport.py` 的 `CompletionTransport` 将完整实际原生历史和可用工具定义送给该真实模型；模型自主返回下一条文本/工具参数，薄传输只序列化为 Pi provider 响应，由 Pi 真正执行工具并写原生历史。它不读取验收预期、不选择业务动作、不模拟工具结果、不重试模型决策，也不修改产品 adapter；这类结果只证明该模型传输下的真实角色行为，不证明原生 OpenAI provider 的 wire 合规性。传输保留每次实际请求、发给 completion 的完整 prompt、原始输出和映射结果；错误非零返回且不生成替代动作。

在提供上述公开 `completion` 的 Python 宿主执行以下代码，不使用普通 Python CLI。把页首的 `$TESTER` 和 `$BOT` 作为 Python 变量 `TESTER` 和 `BOT`，并为 `TOOLKIT_WT`、`PATHS_FILE`、`EVIDENCE_DIR` 提供绝对路径。先按上文构建后端。将构建时的绝对 `CARGO_TARGET_DIR` 传入 Python 宿主的环境。`copy_context` 将宿主授权调用上下文传入 HTTP 工作线程，不读取私有 token：

```python
import contextvars, json, os, pathlib, subprocess, sys, tempfile
scripts = pathlib.Path(TESTER) / 'scripts'
sys.path.insert(0, str(scripts))
from acceptance.host.completion_transport import CompletionTransport
context = contextvars.copy_context()
def actual_completion(prompt):
    return context.copy().run(lambda: completion(prompt, model='default').wait())
with CompletionTransport(actual_completion, pathlib.Path(EVIDENCE_DIR) / 'completion-wire') as transport:
    with tempfile.TemporaryDirectory(prefix='isolated-completion-config-') as directory:
        config = pathlib.Path(directory) / 'model.json'
        config.write_text(json.dumps({'provider': 'public-completion-transport',
            'model': 'public-default', 'baseUrl': transport.base,
            'apiKeyEnv': 'WORKFLOW_COMPLETION_TRANSPORT_KEY'}), encoding='utf-8')
        env = dict(os.environ, HERDR_BOT_WORKTREE=str(BOT),
            WORKFLOW_COMPLETION_TRANSPORT_KEY='isolated-local-transport')
        result = subprocess.run([sys.executable, str(scripts / 'test-workflow-file-handoff.py'),
            '--backend', str(pathlib.Path(env['CARGO_TARGET_DIR']) / 'debug/herdr-bot.exe'),
            '--toolkit', TOOLKIT_WT, '--scenario', 'roles', '--paths-file', PATHS_FILE,
            '--model-config', str(config)], cwd=TESTER, env=env)
        if result.returncode:
            raise RuntimeError(f'roles acceptance failed: {result.returncode}')
```

这里的 local-transport 值仅满足 Pi 自定义 provider 的配置要求，不是模型凭据；真实模型访问仍使用宿主公开授权能力。传输本身的 HTTP/证据保真回归为 `python "$TESTER/scripts/workflow_completion_transport.test.py"`，其中受控 completion 只验证传输，不作为角色验收结果。

已安装 Pi 的 Codex 网页授权可通过独立入口 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-codex --scenario revise --hosts declared --backend <当前后端> --toolkit <toolkit根> --paths-file <验收依赖TOML> --pi-package <已安装coding-agent包> --auth-file <明确授权使用的auth.json> --evidence-dir <不存在的目录> --shell-path <已安装Git Bash>` 接入既有真实业务 runner；`--scenario roles` 对应双角色，`--hosts omp,pi` 对应两个宿主分别运行。入口固定 `openai-codex/gpt-6-luna`，不继承本机默认模型、不回退其他模型。先追加 `--smoke` 单独检查一次真实文本调用，再为业务运行使用新的证据目录；烟测通过也不是业务通过。修订入口可用 `--cases task-no-materials` 等逗号分隔的顶层 case 清单做定向反馈，默认 `all` 执行完整矩阵；子集成功仅记录 `subsetPassed`，`passed` 和完整 promise 覆盖仍保持未验证，不能据此关闭票据。仅双角色入口另支持 `--hosts omp`／`--hosts pi`，用于独立运行未进入的宿主覆盖，不重试另一个宿主的失败决策；单宿主通过不能替代 `omp,pi` 完整通过，证据保留实际 `requestedHosts`。原生证据归档使用 Windows 扩展路径保全超长 Session 文件名，并拒绝合并既有目标目录；`python "$TESTER/scripts/workflow_evidence.test.py"` 检查完整字节与旧证据不被覆盖。

此入口把原始 Chat 历史与工具 schema 映射给安装版 Pi 的原生 Codex provider，由模型直接返回 SDK `toolCall`，再由真实业务宿主执行。不再把整段会话塞入文本 prompt，也不要求模型生成包装 JSON；模型正文不会被解析或截取成工具决策。此映射仍不等于 OMP／Pi 本身直接配置了 Codex provider。每次保留纯 body 的原生请求、SDK 实际响应、服务端模型身份及工具调用 ID；历史工具结果按原调用 ID 绑定，不编造缺失结果。OAuth 只在独立子进程内读取，不进入业务 models 配置、命令行或证据；不刷新、改写用户登录文件。该子进程复用安装版 Pi 的 HTTP 调度器及已有 `PI_PROXY_OPENAI_CODEX`／HTTP 代理设置，不改全局代理或 PATH；SDK 单独调用并不会自动安装 CLI 的代理调度器。过期或距到期不足本次预算／原生刷新窗口的授权非零失败，须由用户自行恢复登录后再明确发起新一轮，不自动重试。模型响应必须具有服务端 `gpt-6-luna` 身份；Codex 不支持调用方服务端输出上限，`max_completion_tokens` 仅作为本地响应交付上限，超过时拒绝交付，不把它称为服务端计费预算。未知字段、非文本输入或缺失工具结果的历史明确拒绝，不静默丢弃。原生错误只保留固定安全类别、HTTP 状态与请求次数，不保存认证头或原始依赖错误。SDK 禁止重试，使用一次 SSE 请求；传输将模型调用、JSON 解析与失败登记串行化，模型或序列化失败后，排队请求也不再调用模型。退出传输时等待所属处理线程及其模型调用结束，清理异常保持非零；不同业务 case／宿主仍是独立覆盖项，不把继续执行另一覆盖项称为重试同一模型决策。业务证据仍需逐项满足所属票据，缺依赖、调用失败或清理失败均保持未验证。设置 `WORKFLOW_PI_PACKAGE` 为明确的安装包后运行 `python "$TESTER/scripts/workflow-codex-completion.test.py"`，验证过期授权拒绝、无凭据输出、认证文件不变，以及真实 SDK 在受控 HTTP 响应下的原生工具 schema／历史错误／调用 ID 关联；该受控外部 API 不证明有效授权或真实模型业务成功。

CLI 的缺依赖 fail-closed 回归可在设置 `WORKFLOW_HANDOFF_TOOLKIT` 为参与 toolkit 根后运行 `python "$TESTER/scripts/test-workflow-file-handoff.test.py"`；它只核对非零、原始错误、无虚构 Run 与无服务启动日志，不宣称真实角色链路通过。

通过必须同时具备两宿主 dock、真实模型链路与对应 worktree 常规检查证据。入口未取得模型、宿主缺失、行为不合预期、等待超时或清理失败均非零；缺依赖即明确未验证，不以准备完成、提示词存在或最终 completed 推断通过。正常/失败均停止所属服务与进程，分别保存原始与清理错误，不调用真实关机、不连接/关闭用户服务，不启用 LAN/mDNS。

## 嵌套原生新进程恢复验收

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-nesting-recovery` 是 P29／P32／P33 的跨仓原生专项；不重复服务端旧记录／碰撞矩阵。先按上文构建当前 worktree 静态页面、`herdr-bot`、`herdr-fixture` 与 `safe_shutdown_shell`，使用独立绝对 `CARGO_TARGET_DIR`。toolkit 使用 Bun 1.4.4、当前 worktree 依赖与角色源码；已有安装版 OMP、Pi、Herdr 和两宿主只读官方状态扩展必须可用，不安装、不修改 PATH、不读取用户认证。授权模型声明及公开 CompletionTransport 的格式、能力和证据限制沿用“文件交接真实角色自动验收”；不能用受控 provider 代替真实模型。公开 completion 传输可用结构化 schema 固定 `content: string` 与 `tool_calls: [{name: string, arguments: object}]`，只约束序列化格式、不预选业务动作；原始请求／结构化结果仍须保留，错误不重试。原生 OMP 的 SessionManager.open 需要 await，且 JSONL 可以先有 title 元数据再有 session 头；读取真实身份时不能假定第一行就是 Session。原生关闭可以追加真实 `session_exit` 元数据，因此同时保留关闭前历史和确认进程退出后的恢复基线；Resume 前后完整 branch 仍须逐项相等，不能过滤业务消息掩盖重放。

```powershell
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-nesting-recovery `
  --backend "$env:CARGO_TARGET_DIR/debug/herdr-bot.exe" `
  --toolkit $TOOLKIT_WT --hosts omp,pi `
  --paths-file 'D:/isolated-acceptance/paths.toml' `
  --model-config 'D:/isolated-acceptance/model.json' `
  --shell-path 'D:/Program Files/Git/bin/bash.exe' `
  --evidence-dir 'D:/isolated-acceptance/new-exclusive-recovery-evidence'
```

路径文件使用既有根解析器，声明 `herdr`、`omp_package`、`pi_package`、`pi_agent`、`omp_agent`；后两者分别只读加载 `extensions/herdr-agent-state.ts` 与 `extensions/herdr-omp-agent-state.ts`，隔离 HOME 后必须显式加载两宿主官方状态扩展，不能依赖用户默认发现。也可显式传 `--herdr`、`--omp-package`、`--pi-package`、`--pi-state-extension`、`--omp-state-extension`。证据目录必须尚不存在。Herdr 的原生状态和 IPC 使用原子创建的独占短 Temp 根，避免深层证据路径触发 Windows 路径限制；所属进程全部结束后将完整证据归档到该目录，`nativeRoot` 保留实际原生路径身份，归档失败也非零。`--hosts omp,pi` 分别运行两套同宿主真实业务；`--hosts declared` 保留固定业务声明的 OMP 父管理者与 Pi 子管理者，不修改声明来迁就 runner。单宿主参数只供较窄反馈，不等于两宿主验收。

场景先由真实父管理者 dispatch，再明确确认失败并授权 retry；独立核对新旧 execution／child／Session 与两份实际字节。随后分别关闭并确认旧、当前原生进程退出，经公开 Chat Resume 启动真正新进程，读取安装版 SessionManager 历史、只读来源 Task 与父子事实，核对无新增 Run、无输入重放。恢复后用真实 `run`／`collect`／`report` 的后端结果证明内外角色归属。真正终止并等待旧后端、启动新后端后，再恢复两条 Chat，实际原生状态／collect／pause／recover／report 必须拒绝且三份业务记录不变。完整服务端拒绝矩阵仍由后端专项唯一维护。

只读原生观察扩展仅记录 session_start／before_agent_start 的 PID、Session、cwd 和原生文件，不授予角色、不制造工具结果。证据包含两仓 revision／修改及锁文件身份、安装入口／二进制哈希、模型身份（无密钥）、原始用户授权、实际模型／HTTP 流量、原生历史、文件字节、前后记录哈希及持有的 OS 进程句柄退出确认。HOME／APPDATA／XDG／原生存储／命名 Herdr／定义与运行数据均独占，关闭 mDNS，持续消费 stdout／stderr；成功失败均停止并等待所属进程，原始与清理错误分开保留。任何宿主、模型、角色能力或清理失败都非零且未验证，不把启动成功、配置存在或最终状态当通过。

`WORKFLOW_HANDOFF_TOOLKIT="$TOOLKIT_WT" python "$TESTER/scripts/accept-workflow-nesting-recovery.test.py"` 只验证 CLI 缺依赖时保留错误、双宿主未验证且不启动服务；不替代原生专项。常规检查仍按票据分别执行 workflow_dispatch、完整后端、前端、toolkit workflow-chat／相关原生 dock、typecheck 和全量入口。

## 嵌套 manager 双角色真实验收

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-nesting-roles` 经本 worktree 后端、指定 toolkit 的实际角色入口、已安装 Herdr 和真实授权模型，观察嵌套 manager 的内部 `run`／`collect` 与外层 `report`。入口不安装宿主、不读用户认证、不改全局 PATH，不启用 LAN／mDNS，也不执行关机。

先按上文构建本 worktree 静态页和后端，在 toolkit 安装缺失的 frozen 依赖。依赖根文件必须显式指定，提供 `herdr`、`pi_package`、`omp_package`、`pi_agent`、`omp_agent`；后两者分别仅只读使用 `extensions/herdr-agent-state.ts` 和 `extensions/herdr-omp-agent-state.ts`，不读取同目录的认证或配置。模型声明沿用本页的独立 `model.json`（`provider`、`model`、`baseUrl`、`apiKeyEnv`），密钥仅从显式环境名获取。公开 CompletionTransport 也可作为真实模型的文本传输，但不证明原生 provider wire 合规性。HOME／APPDATA／XDG、原生历史、业务和运行目录、命名 Herdr server、动态数值回环端口均为本次独占。

```powershell
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-nesting-roles `
  --backend "$env:CARGO_TARGET_DIR/debug/herdr-bot.exe" `
  --toolkit $TOOLKIT_WT --hosts omp,pi `
  --paths-file 'D:/isolated-acceptance/paths.toml' `
  --model-config 'D:/isolated-acceptance/model.json' `
  --evidence-dir 'D:/isolated-acceptance/new-exclusive-run'
```

当前验证项目的 `accept-workflow-nesting-roles` 仍写入已拒绝的 `report_policy`，并期待 `report` 的发送确认。保留该入口供定位；直到驱动按当前批准和自由报告契约取证前，不将它认证为双角色通过，也不恢复旧字段或代发批准。

`--hosts omp,pi` 对两个宿主分别运行两套独立 manager／zone／原始业务字面样例；每套重复调用同一 Map，并经真实 manager 的 fail／retry 保留旧 execution 干扰。`--hosts declared` 是额外的混合宿主业务夹具：第一套 parent OMP／child Pi／worker OMP，第二套相反，严格保留入口中 `DECLARED_ROLES` 的实际 Task 声明；不是把用户业务 Task 统一改成一个宿主，也不是对部署业务的验收。该模式不替代双宿主完整矩阵。

子 manager 用 `run`／`collect` 管理子 Run，用 `approve`／`report` 面向父 execution。真实子 Run finish 后才能批准整体成果；人工模式观察原生 `approve` 的 Yes、实际回执及父 execution 的保存来源。取消旧批准确认不能产生批准，不禁止自由报告。分别检查进度与成果 `report` 的工具结果和公开保存，不期待发送确认。父 manager 只在独立完成门禁满足后显式 complete。核对父子身份、实际文件字节、原生双角色指引及目标 Tab 精确关闭；不以最终 completed 代替这些事实。`chief_report` 的独立确认不适用这里的自由节点报告规则。

`--evidence-dir` 必须不存在。原生运行状态使用原子创建的独占短 Temp 根，避免 Windows 已安装宿主的命名管道／存储路径长度限制；退出后完整复制到证据目录的逐宿主子目录，原始短根保留且记录为 `nativeRoot`，不把临时目录删除当作进程退出。成功或失败均保留 `evidence.json`、逐宿主独占子目录、两仓源码 revision／修改身份、锁文件／二进制／已安装宿主身份、模型／HTTP 原始请求结果、用户输入、原生历史、文件字节和分开的清理错误。模型配置在退出时删除。缺模型、宿主、能力、清理失败或行为不符均非零且未验证，不 skip，不重试业务或模型决策。`python "$TESTER/scripts/accept-workflow-nesting-roles.test.py"` 仅检查缺授权模型时的 CLI fail-closed 行为，不能代替真实专项通过。

## 共享 Bot 指南材料的启用检查

真实模型修订专项为 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-nesting-revise --backend <本 worktree 后端> --toolkit <toolkit worktree> --hosts declared --paths-file <独占依赖配置> --model-config <专用授权模型声明> --evidence-dir <不存在的独占目录>`；依赖声明、混合宿主／双宿主行为、四次 Task／Map 有／无材料矩阵、专属 Task 配对、修订负例及证据边界见[独立修订验收运行说明](../revise-bot-guides.md#真实模型跨仓修订验收)。设置 `WORKFLOW_HANDOFF_TOOLKIT` 后运行 `python "$TESTER/scripts/accept-workflow-nesting-revise.test.py"` 仅验证缺后端／模型时非零、覆盖保持未验证且不启动宿主，不替代真实业务通过。

修订定义和材料接线用 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch revise_bot_guides`。在 PowerShell 中设置独占绝对 `CARGO_TARGET_DIR`，再运行 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" check-revise-bot-guides --target-dir "$env:CARGO_TARGET_DIR"` 做安全进程烟测。独立入口、`task-revise-bot-baseline-v1` 和证据边界见[独立修订运行说明](../revise-bot-guides.md)。

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch shared_business` 检查公开定义与真实文件：直接绑定命名材料，不注入正文，比较实际字节、原路径及字符、完整目标和 revision；过期 revision 须拒绝。提示词存在不能证明模型遵从。材料协议见[共同业务材料契约](../business-materials.md)，真实创建和修订分别使用专项入口。

创建链真实业务矩阵入口为 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-nesting-create`，必须显式提供 `--backend`、`--toolkit`、`--hosts declared`（或完整 `omp,pi`）、`--paths-file`、`--model-config` 和独占 `--evidence-dir`。安装依赖、独立业务技能根、完整复跑命令及证据边界见[真实创建与配对验收](../create-bot-wrapper.md#真实创建与配对验收入口)。缺构建与重复证据目录的 CLI 回归为 `WORKFLOW_HANDOFF_TOOLKIT=$TOOLKIT_WT python "$TESTER/scripts/accept-workflow-nesting-create.test.py"`；该回归不认领真实模型、混合宿主或保存安全通过。

最小原生烟测可针对 Pi 的 `task-18e31d34797f1464-3`：将当前 Task.prompt 与明确的独立 Chat 委托组合为测试拥有的 `request.md`，提供字面 `source.txt`，只授权当前隔离目录的新 `text.txt`、`handoff.md`。安装版入口为 `node <pi-package>/dist/bundle/cli.js --offline --no-extensions --no-mcp --no-skills --skill <writing-for-agents/SKILL.md> --no-context-files --no-prompt-templates --no-themes --model <专用provider/model> --thinking off --mode json --print @request.md`。使用本节上文的专用模型配置或公开 CompletionTransport，隔离 HOME／APPDATA／XDG／PI_CODING_AGENT_DIR 与原生 sessions；传输的公开 completion 上下文必须覆盖整个子进程等待期，不能先退出宿主调用再留后台线程访问已失效桥接。HTTP handler 线程不继承宿主 completion 调用上下文；将请求排队交给拥有该上下文的主线程执行，再把结果返回 handler，避免 `Missing session/run/name`。测试不读取用户凭据，不复用用户 Chat，也不启动 daemon。

该烟测观察原生真实 read／write、两份材料完整落盘读回、原文与未授权文件保持，以及 handoff 不含重复正文；保留完整模型请求、native JSONL、错误及真实材料。进程退出零不是充分条件，原生 `stopReason: error`、缺文件或未完整读回均失败。工具必须真实执行，不可由模型或传输伪造结果；传输用完关闭并等待，移除隔离凭据配置。它只证明一个独立原生调用能处理其实际材料，不是六 Task 配对覆盖、公开 Chat 发起或两案例完整 P40–P45 证据。

## 创建 Task＋Bot 外层真实兼容验收

`python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-nesting-outer --backend <本 worktree herdr-bot.exe> --toolkit <toolkit worktree> --hosts declared --paths-file <专用已安装依赖声明> --model-config <明确授权模型声明> --evidence-dir <不存在的独占目录>` 拥有 Task 创建者／既有 Task.prompt 优化者的独立 Chat 与实际外层 workflow 配对，以及旧两个独立创建入口兼容和未迁移 Map 范围保持。运行方式、完整 Suite/MCP 隔离、独立业务全文确认、节点成果批准与自由报告、父子真实交接及证据边界见[包装能力运行说明](../create-bot-wrapper.md#真实外层与旧独立入口验收)。必须保留声明宿主，不能将原混合宿主改成单一 Pi 迁就 runner；`--hosts omp,pi` 要求两套完整场景各自通过。它不认领六共享 Task 独立配对或规划选择，也不能由组件、受控 provider、最终 completed 或全局回归代替。CLI fail-closed 回归设置 `OUTER_ACCEPTANCE_TOOLKIT` 后执行 `python "$TESTER/scripts/accept-workflow-nesting-outer.test.py"`；它仅验证拒绝边界，不是专项真实业务通过。

## softlink（符号链接）手动验收

真实文件／目录符号链接检查默认关闭，只有显式设置 `HERDR_BOT_MANUAL_SYMLINKS=1` 才尝试创建。普通浏览器回归明确报告 manual-only 跳过；后端 zone 浏览／diff 的混合边界测试仍检查路径穿越和 junction，只跳过其中的符号链接部分，不调用 WSL 创建链接。即使当前终端已有管理员权限，未设置开关也不会自动运行符号链接检查。

此开关仅用于测试，不修改产品的链接拒绝规则，不授权真实网卡监听，也不会自动提权或改变开发者模式。Windows 手动验收请使用具备符号链接权限的终端；缺少权限时仍须如实报告未验证，不能将 skip 当成通过。仅运行命令而不设置开关仍属于默认模式。

以下 PowerShell 从仓库根目录执行，浏览器使用已构建的交付产物；源码更新后先按前面的构建入口构建。所有测试继续使用独占临时数据，不操作用户数据：

```powershell
$previous = $env:HERDR_BOT_MANUAL_SYMLINKS
$env:HERDR_BOT_MANUAL_SYMLINKS = '1'
try {
  npm --prefix frontend run test:browser -- schedulerCrossOperation.spec.ts --grep 'file symbolic links' --workers=1
  cargo test --manifest-path backend/Cargo.toml --test zone_browse rejects_real_junctions_and_symlinks_without_reading_outside_content -- --exact --nocapture
  cargo test --manifest-path backend/Cargo.toml --test zone_diff diff_entry_enforces_the_zone_root_boundary -- --exact --nocapture
} finally {
  $env:HERDR_BOT_MANUAL_SYMLINKS = $previous
}
```

## 普通 node 双宿主批准与自由报告专项

从参与的 Herdr worktree 根运行：

```sh
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-node-approval-report --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$AUTHORIZED_MODEL_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"
```

所有路径必须绝对，证据目录不得存在。`CARGO_TARGET_DIR` 必须为本 worktree 独占绝对目录，`--backend` 指向其中的 `debug/herdr-bot.exe`。先在两个参与 worktree 以 Bun 1.4.4 frozen-lock 准备依赖；入口不安装宿主、不扫描凭据或读取用户 Chat。paths 显式声明已安装 `herdr`、`omp_package`、`pi_package`、`omp_state_extension`、`pi_state_extension`。机械场景的 model-config 必须显式为 `{"mode":"controlled","scope":"ticket-10-native-mechanics"}`；它只授权受控 provider 选择工具，不证明真实模型判断。真实模型职责使用其独立专项，不以本入口结果替代。

入口顺序执行 frontend build、生产后端／fixture／paths／安全关机壳和既有透明 CLI recorder 构建、后端／前端／浏览器完整回归，以及 toolkit typecheck／canonical test／integration check 和 chief／node 具名 dock。配对回归仅使用本次安全关机壳；生产后端仅用于专项独占业务实例。昂贵原生检查须遵守 toolkit quiet-window 约定。

专项复用既有原生安装、SessionManager、独占 Herdr、透传 UI observer 和外部 CLI recorder。两宿主真实 TUI 的 Yes 选中画面、确认原文、原生返回、公开 Run／报告、manager 原生输入、完整 HTTP 与外部调用分别留证。覆盖两模式无报告／无产物批准、批准前后四类自由报告、报告与批准正交、真实 Windows deny-delete 持久化故障、重复与实际重叠调用、chief 原生委派／报告确认。并发用例通过两个实际 native approve 到达透传 HTTP 门闩再共同放行，5 秒仍只有一个到达时保持失败，不能用串行重复补足。存储故障按独占记录内容定位，不固定临时文件名。并发到达与放行时间使用 `time.perf_counter()` 高分辨率单调时钟，避免 Windows 粗粒度时钟把先后事件记成同刻；第二宿主进程在原生历史的真实 cwd 启动并核对 switch 未取消。chief 使用交付的 connect-bot 扩展与独占后端配置真实登记，公开 current 达到 registered 后才请求委派，不伪造登记事件。

无自动行为窗口用单调时钟观察 46 秒，超过 30 秒请求及 10 秒在线＋30 秒保留＋5 秒维护上界；不修改产品确认／超时／重试策略。每项 coverage 独立输出字面预期、原始证据和 verified 状态；部分宿主、任何检查／覆盖缺失或清理失败均非零，原始错误与清理错误分开保留。CLI 自测 `python "$TESTER/scripts/accept-node-approval-report.test.py"` 仅证明 fail-closed 与旧证据保护，不代替 S2。

## 网络护栏与手动验收

普通测试复用 `TestServer`／`startServer`，仅监听数值回环地址，真实 TCP／SSE、历史持久化与进程重启仍可覆盖。非回环监听和真实 mDNS 仅走显式手动入口；环境授权不放宽普通 helper。

真实 LAN／mDNS 验收时：

1. 为 `herdr-bot.local` 安排独占使用，先停止占用该名字的服务。浏览器手动入口只执行已构建产物，前置构建要求与普通浏览器验收相同。
2. 使用以下专用入口。Rust 脚本构建并复制到稳定 runner 路径，避免重建换路径引发重复防火墙授权；直接 `cargo test -- --ignored` 不提供这一保证。

   ```powershell
   # 仅构建并打印 runner 路径，不打开监听。
   .\scripts\run-mdns-tests.ps1 -ListOnly
   # Rust 真实 LAN／mDNS 验收。
   .\scripts\run-mdns-tests.ps1
   # 浏览器真实 LAN 验收。
   npm --prefix frontend run test:browser:manual
   ```

3. 仅在用户要求配置防火墙时使用 Rust 脚本的 `-ConfigureFirewall`；它会请求管理员权限并配置入站规则。

手动入口设置 `HERDR_BOT_MANUAL_NETWORK=1`。可运行 `node scripts/check-test-network-policy.mjs` 检查监听 API、网络模块导入、manual helper 位置和 Rust ignored 标记；护栏自身回归为 `node --test scripts/check-test-network-policy.test.mjs`。
新增真实网络能力时，同时审查普通 helper、手动入口、固定 runner 与护栏例外。例外精确到函数／用例，不豁免整个目录；静态护栏只覆盖已知 API，不是任意外联的网络沙箱。
