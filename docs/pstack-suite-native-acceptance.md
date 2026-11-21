# pstack Suite 跨仓原生验收

黑盒验收代码已迁至独立项目 `herdr-bot-tester`。本页验收命令使用显式设置的 `$TESTER` 与 `$BOT`，不要求两个项目相邻。产品构建与自身回归仍从产品根运行。

```powershell
$TESTER = 'E:/Developer/myhub/herdr-bot-tester'
$BOT = 'E:/Developer/myhub/herdr-bot'
```

验收设施的 `scripts/` 路径现属于验证项目。参数、授权、覆盖及非零退出约束不变。完整入口见验证项目 `README.md`。

在 Windows 上用实际安装的 Herdr TUI 和 OMP，从当前 herdr-bot worktree 的公开 Task／Chat HTTP 发起和恢复。Runner 使用真实 OMP 原生交互及 Session 历史，受控模型只返回明确的本地工具调用，不替宿主执行工具，也不填写历史、状态或工具结果。它不使用用户 credential、既有会话或全局配置。不要同时运行其他占用已经配对的原生／全仓验收。

## 安装选择与构建

用 Bun 1.4.4 及 Python 3.11+。先将下列 PowerShell 变量中的所有路径改为本轮绝对路径，Cargo 目录必须是新建或空目录；证据目录不得存在。Windows TEMP 指向无需引号即可交给 Bun preload 的短路径（无空白），runner 在其下创建独占 Herdr、agent、Git 和发布副本；技能副本所在目录故意含空格。保留已安装宿主，不运行宿主安装、更新或修补。

```powershell
$Toolkit = 'C:/absolute/toolkit-worktree'
$Config = 'C:/absolute/pstack-owned-installations.json'
$Evidence = 'C:/absolute/pstack-new-evidence'
$env:CARGO_TARGET_DIR = 'C:/absolute/fresh-exclusive-pstack-cargo'
$Backend = Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe'
$SafeShell = Join-Path $env:CARGO_TARGET_DIR 'debug/examples/safe_shutdown_shell.exe'

Set-Location "$BOT/frontend"
bun install --frozen-lockfile
Set-Location $BOT
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --locked --bins --example safe_shutdown_shell --example recovery_herdr_proxy
Set-Location $Toolkit
bun install --frozen-lockfile
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-pstack-suite `
  --toolkit-worktree $Toolkit `
  --backend $Backend --isolated-config $Config --cases all --evidence-dir $Evidence
```

逐项保留命令、退出码和未截断的原始输出。构建失败应停止专项，不用旧产物替代。`recovery_herdr_proxy` 是既有的透明 Herdr CLI 记录器；不启用故障注入，所有命令转发给声明的已安装 Herdr。实际业务 HTTP 只使用生产后端；此专项不会请求系统关机。

配置示例（版本只是示例，不是支持承诺）；所有路径显式选择，配置不含认证、生产网络地址或模型厂商凭据：

```json
{
  "authorization": "owned-local-mechanics-only",
  "herdr": "C:/absolute/installed/herdr.exe",
  "core": "C:/absolute/installed/hc.exe",
  "node": "C:/absolute/installed/node.exe",
  "bun": "C:/absolute/installed/bun.exe",
  "omp_cli": "C:/absolute/installed/omp.cmd",
  "omp_package": "C:/absolute/installed/node_modules/@oh-my-pi/pi-coding-agent",
  "omp_state_extension": "C:/absolute/owned-preparation/.omp/agent/extensions/herdr-omp-agent-state.ts",
  "versions": {"herdr":"0.9.5","core":"0.1.21","node":"v24.18.0","bun":"1.4.4","omp":"19.0.1"}
}
```

`omp.cmd` 必须指向声明的发布包，实际 OMP 进程使用所选的 Bun。Core 显式绑定并记录版本／哈希；本专项选 TUI，不能认证 App/core 的会话行为。使用与 Herdr 版本配套的官方 OMP 状态扩展：在独占准备目录设置 HOME／USERPROFILE、APPDATA、LOCALAPPDATA、`HERDR_CONFIG_PATH`，建好 `.omp/agent/extensions`（清除 `PI_CODING_AGENT_DIR`），再用实际安装的 `herdr integration install omp` 导出。只传该官方扩展文件，绝不复制用户的 `.omp`、模型目录或认证。Toolkit worktree 使用未进行过本机角色绑定的交付源码，缺失生成交付资源不会替你构建；仅 `role-lifecycle` 在 owned 发布副本上显式确认、生成。

## 两个仓库的独立检查

沿用上面两个 worktree、已建立的冻结依赖和同一次独占 Cargo 构建。分别保留每个命令、cwd、源码 revision／锁文件和相关源码哈希、版本、退出码、原始日志及产物身份；不要让一次后续绿覆盖较早的失败。

```powershell
Set-Location $BOT
node scripts/test-backend.mjs
npm --prefix frontend test
npm --prefix frontend run test:browser:build

Set-Location $Toolkit
$env:HERDR_BOT_REPO = $BOT
$env:HERDR_BOT_METADATA = Join-Path $BOT 'meta-data/task-options.toml'
$env:HERDR_BOT_BINARY = $SafeShell
$env:OMP_BASELINE_RECORD_ROOT = 'C:/absolute/original-toolkit-checkout'
$env:HARNESS_ACCEPTANCE_EVIDENCE_DIR = 'C:/absolute/owned-toolkit-check-results'
bun run typecheck
bun run test:framework
bun run check:integrations
bun run test
```

离线[交付包与分层证据](pstack-delivery-evidence.md)与本专项分开运行：包检查不会启动宿主，本专项不重复检查整个资源闭包。

Toolkit 只对 dock 使用 safe shell，绝不将生产后端设为 `HERDR_BOT_BINARY`。`OMP_BASELINE_RECORD_ROOT` 仅用于解析历史 sibling 依赖；不会以旧工作区替代当前 worktree。依照 Toolkit worktree 的 `docs/agents/testing.md` 和 `pstack-suite/README.md` 选已安装的 OMP／Pi、可支持的 Node 及显式 Java、TLC 等配套工具。仓库检查失败、过滤和未运行项都必须保留；这两个检查组与专项互不替代。

## 场景和证据

| `--cases` 场景 | 证据边界 |
| --- | --- |
| `resource-failure`（P46） | 先创建真实历史，再移走／真实拒绝读取同一个已声明入口；公开发起及 Resume 均明确失败。分别对照外部启动调用、实际 Tab、原生事件、文件和请求。不是依赖递归预检。 |
| `assembly`（P2/P3/P14） | 其他 Suite 先成功使用同名异源技能，pstack 发起／恢复检查原生最终可见目录、真实 `read` 原文件头及正文、项目指令、官方状态上报和固定 `/budget` 执行。已验证 ACL 的只读交付、前后快照、Herdr/Bun/Git 调用与声明上游的回环观察核对启动不构建、不拉取。不是文件或网络沙箱。 |
| `mode-resume-on`、`mode-resume-off`（P17） | 在两个独占宿主分别真实保存 on/off，公开 Resume 同一原生历史后独立读首轮请求及保存值、比较新旧提示贡献。不是 H1 人工签收。 |
| `source-base`（P20） | 通过 Suite 装配的主 Chat 与实际独立 cwd 的 native task 分别明确读取，核对技能原路径、相对配套资源和子历史，排除 cwd 诱饵或父对话继承。 |
| `role-lifecycle`（P24/P28/P31） | 无绑定实际进入 setup；停止 owned 会话后用原生命令目录及明确 `--confirm` 生成 A，重新公开发起并调用原生角色。再次停用、确认 B、发起且实际消费 B；通过 Herdr 发起其他 Suite 的原生默认模型作前后对照。 |

`--help` 列出参数。逗号分隔的子集只用于诊断，始终非零并清楚保留缺格状态。场景的 `summary.json` 中，`passed` 仅代表所有专项格通过，不表示仓库检查、H1/H2 或真实模型质量。外层 `run.py` 的 `tester-run.json` 记录所选产品根、验证代码身份和场景退出码，不判定覆盖。证据目录保留原生模型 wire、公开 HTTP 和构建页面正文、Herdr 请求、实际历史和输出；每格 `evidence.json` 给出独占短路径及所属程序退出记录。清理或取证失败不算通过。专项保留状态供复核，不访问既有会话；确认所属程序已退出后才删除本轮目录。
