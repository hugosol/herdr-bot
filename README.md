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

- **trigger config（发起配置）**：初始消息、初始 Skill 调用、model 与 thinking，仅在发起 Chat 时应用。
- **env config（会话环境配置）**：扩展配置；发起 Chat 时加载，Resume 需要新进程时也加载。

初始 Skill 调用是随初始消息提交的动作，不是加载型 Skill；Bot 固定 prompt 与本次 input 都属于用户输入，不是系统提示词注入。Bot 不提供加载型 Skill 或系统提示词注入。

原生会话文件和附属记录由 agent 管理，herdr-bot 不搬动文件或修改 agent 原生配置。发起 Chat 不保证新建原生对话；agent 自身的自动续接设置不属于 herdr-bot 的功能。

## 安全边界

**没有账号或身份验证，只应在可信局域网使用，不要暴露到公网。**
能访问服务的设备可以管理 Bot、启动 agent、执行配置脚本、操作 Git 和关闭整台 Windows 电脑。
connect-bot 会话写入另有限制：只接受同机来源，见 [ADR 0004](docs/adr/0004-session-write-boundary-same-host.md)。
页面确认不是服务端认证；测试或排障时不要调用真实关机，也不要用用户正在运行的对话做探针。

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
- [测试优化方案](docs/test-optimization.md)：已测瓶颈、问题原因、后续方案、隔离边界及验收标准。
- [架构决策](docs/adr/)

源码在 `backend/`（Rust/Axum）和 `frontend/`（React/Vite）。
自动检查入口为 `cargo test --manifest-path backend\Cargo.toml`、
`npm --prefix frontend test` 和 `npm --prefix frontend run test:browser:build`；
浏览器验收需要本机 Chrome。源码更新后的完整验收使用 `test:browser:build`：依次构建前端、
全部后端二进制及 `safe_shutdown_shell`，再调用带网络护栏的 `test:browser`；任一步失败即停止。
连续调试可独立使用 `npm --prefix frontend run test:browser`，它只验证已构建产物，不会自动重建；
旧二进制可能导致整轮验收使用旧协议并失败，不能靠修改断言绕过。组合入口支持追加筛选参数，
例如 `npm --prefix frontend run test:browser:build -- --workers=1`；完整验收不追加快速过滤。
Cargo 检查也需要 Node.js，以执行跨前后端的测试网络护栏。
真实子进程测试读取启动端口后仍须持续消费 stdout，直到进程退出；提前关闭管道会与剩余启动日志竞争，导致进程异常退出。
浏览器检查使用随仓库交付的 `meta-data/bot-options.toml`；修改显示标签时，须同步更新摘要的独立预期值及动态元数据测试中的替换样例（当前模型标签为 `GPT-6.1-Sol`）。
Windows 浏览器 fixture 清理会等待服务退出事件后再判断 `taskkill` 的失败状态，避免将已退出但尚未被 Node 观察到的进程误报为清理失败；进程仍存活时继续报告错误。

### 测试反馈速度

Vitest 使用 `vmThreads`，按可用 CPU 数最多开启 8 个 worker；保持 `isolate: true`，不共享测试文件的 VM 全局状态。
`diff.test.ts` 与 `zoneDisplay.test.ts` 是纯逻辑测试，显式使用 Node 环境，其余组件测试仍使用 jsdom。
这减少重复环境初始化与并发资源争抢，不省略用例或降低断言。需要比较其他机器上的并发设置时，可通过 CLI 覆盖：

```powershell
npm --prefix frontend test -- --maxWorkers=4
# 开发时仅运行相关文件，或按当前 Git 改动选择；仍会先执行网络护栏。
npm --prefix frontend test -- tests/diff.test.ts
npm --prefix frontend test -- --changed
# 完整回归仍使用默认入口。
npm --prefix frontend test
```

本机 Windows／Node 24、20 个可用逻辑 CPU 上，纯逻辑测试分流后的三轮交替测量中，
`forks`／19 workers 与 `vmThreads`／8 workers 的命令耗时中位数分别为 11.93 秒和 11.02 秒；
每 100 毫秒采样的进程树峰值 RSS 中位数分别约为 3862 MiB 和 2018 MiB。
RSS 是各进程工作集的相加值，不是独占物理内存；这些是未清空缓存的重复运行结果，并非冷启动比较，也不是其他机器的性能保证。

浏览器回归分层入口（均复用正式 `test:browser` 的网络护栏；只执行，不负责构建）：

```powershell
npm --prefix frontend run test:browser:fast
npm --prefix frontend run test:browser:time-contract
# 核对完整集合 = 快速集合与时间契约集合的不相交并集，输出逐 case 清单。
npm --prefix frontend run test:browser:partition
# 最终验收必须运行完整默认集合，而不是只运行 fast。
npm --prefix frontend run test:browser:build
```

`@time-contract` 按行为标记真实预算、确认超时、迟到结果及无自主刷新／重试／重放观察窗口，
不是按文件名或普通断言的 timeout 分类。新增未标记 case 默认进入 fast，分区核对会自动包含它；
新增真实时间承诺须同时审查标签。完整默认入口没有 grep 过滤，继续覆盖两个层次。

浏览器测试继续使用独立临时目录、存储、Herdr 副本和真实进程重启，不共享可变状态。
默认启用 2 workers，但保留文件内顺序；真实 LAN 手动配置仍为 1 worker。
本机三轮交替完整回归每轮 159 项首次通过，1／2 workers 命令耗时中位数为 427.55／227.37 秒，
峰值进程树 RSS 中位数约 1204／1901 MiB；墙钟减少 46.8%，但内存代价增加，较小机器可传 `--workers=1` 回退。
采样方法、范围、首次失败与回退边界见[测试优化实测记录](docs/test-optimization.md#本轮实测与决策)。
Playwright 已按 worker 复用浏览器；没有显著只读准备成本的证据，因此不增加自制缓存。
初始化失败（含监听／接口就绪等待失败）与正常结束共用清理逻辑，清理后才返回启动错误；清理失败会同时保留原始错误。

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
