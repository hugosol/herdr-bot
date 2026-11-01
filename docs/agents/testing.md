# 测试指引

命令均从仓库根目录执行。本文记录入口选择与安全约束；测试结果和性能记录留在运行日志中。

## 选择入口

| 场景 | 入口 |
| --- | --- |
| 完整后端回归、后端共享代码或公共夹具变更、提交前的后端检查 | `node scripts/test-backend.mjs` |
| 前端组件与逻辑检查 | `npm --prefix frontend test` |
| 交付页面、真实 HTTP／SSE 与进程行为的浏览器验收 | `npm --prefix frontend run test:browser:build` |
| 单个后端目标的开发反馈 | 例如 `cargo test --manifest-path backend/Cargo.toml --test zone_commit` |
| 单个前端文件或当前改动的开发反馈 | `npm --prefix frontend test -- tests/diff.test.ts` 或 `npm --prefix frontend test -- --changed` |

完整后端回归优先使用并行入口，一次执行即可；原生 Cargo 全量入口保留作对照和回退。定向检查和浏览器分层检查用于开发反馈，交付验收使用对应的完整入口。

### 运行前提

- 工具与依赖安装见 [构建与运行](../../README.md#构建与运行)。后端测试也需要 Node.js，供跨前后端网络护栏使用。
- 浏览器验收需要本机 Chrome。源码更新后使用 `test:browser:build`，由它先构建前后端和安全关机壳再执行验收；仅调试已构建产物时使用 `npm --prefix frontend run test:browser`。
- 不同源码副本使用独立 `CARGO_TARGET_DIR`；测试程序会内嵌源码路径。性能比较顺序运行，避免多轮检查争抢同一资源。

## 后端并行与诊断

`node scripts/test-backend.mjs` 不带参数即可并行。Cargo 自动发现测试目标，普通新增 case 或集成测试文件无需登记；保留默认 ignored 状态、example 编译检查及 doctest。

- 资源紧张时用 `--binary-jobs 2` 降低外层并发。
- 排查跨二进制并发问题时用 `--binary-jobs 1`，并与 `cargo test --manifest-path backend/Cargo.toml` 对照；需要内部也串行时再加 `--test-threads 1`。
- 外层并发不限制 Cargo 编译并发；内部线程、环境继承及日志目录选项以 `node scripts/test-backend.mjs --help` 为准。
- 构建失败不启动测试；二进制全部成功才执行 doctest。首个失败或交互终端正常 Ctrl+C 后停止派发，等待在途目标结束，整体返回非零。
- 各目标日志与 `summary.json` 保留在入口打印的目录中。区分失败、取消、未派发和 doctest 未执行；部分完成不是完整通过。强杀进程或关闭终端不保证清理。

Windows 子进程保留普通启动方式；分离进程会改变 Git/MSYS 行为。正常 Ctrl+C 由终端输入处理，保留在途测试的清理机会。

## 前端与浏览器

通过 npm 脚本运行，保留网络护栏；直接调用 Vitest／Playwright 会绕过 npm hook。脚本定义以 [package.json](../../frontend/package.json) 为准。

- Vitest 的环境和并发见 [vitest.config.ts](../../frontend/vitest.config.ts)。组件使用 jsdom，纯逻辑测试可使用 Node；保持测试文件 VM 隔离，调整 worker 数而非关闭隔离。
- 浏览器调度见 [playwright.config.ts](../../frontend/playwright.config.ts)。文件内并行场景保持每个用例独占服务、可变存储和 Herdr 副本；浏览器由 Playwright 按 worker 复用。
- 资源受限或诊断并发问题时，给浏览器入口追加 `-- --workers=1`；Vitest 可追加 `-- --maxWorkers=4` 等覆盖参数。
- 用 `elementFromPoint` 核对 SVG 连线等真实绘制时，先将目标滚入视口；`toBeVisible` 不保证元素位于视口内，不能直接作为命中测试的前提。
- 完整浏览器入口不加快速过滤。分层开发反馈使用 `test:browser:fast`／`test:browser:time-contract`；修改选择或标签后运行 `npm --prefix frontend run test:browser:partition`，核对完整集合是两分区的不相交并集。
- `@time-contract` 标记真实预算、确认超时、迟到结果及无自主刷新／重试／重放的观察窗口，不按文件名或断言 timeout 分类。新增真实时间承诺时同步审查标签。
- OMP 历史身份超时验收检查创建关联、agent 类型及 `identity-timeout` 类别，并验证创建成功、页面不泄露内部诊断、身份恢复后不重读／补写历史及初始化仅提交一次；诊断原因的自然语言措辞不作为契约。

## 修改测试或夹具

- **所有权**：每个用例独占可变临时数据与动态回环端口；同一竞争／重启场景保留必要共享。浏览器夹具探测端口后到子进程绑定前仍存在竞争；仅动态端口的已确认地址占用错误可重新分配（最多 32 次），固定地址及其他启动失败直接报错，不重试业务操作。自动发现目标不等于自动消除资源冲突。
- **路径**：真实子进程使用测试拥有的临时目录。普通相对路径按声明配置文件的父目录解析，扩展入口按元数据文件父目录解析；cwd 可以与配置目录不同。需要根引用时生成隔离根文件，不读取开发者的 `paths.local.toml`。启动期 `data_directory` 与迁移标记也要归测试所有。
- **环境**：`HERDR_FIXTURE_DIR`、`HERDR_BOT_FIXTURE_RECORD_DIR` 会覆盖默认夹具位置；继承环境时保证它们不把不同用例导向同一可变目录。
- **进程**：取得真实子进程端口后持续消费 stdout／stderr，直到进程退出。初始化失败与正常结束都执行所属资源清理；同时保留原始错误和清理错误。
- **Windows 清理**：`taskkill` 完成后再开始服务退出事件的有界确认窗口，然后解释 `taskkill` 失败状态，避免终止命令在全局负载下耗时较长时，把已退出但尚未被观察到的进程误报为存活；确认窗口结束仍存活时报告错误。临时目录删除不代替进程结束。
- **目录初始化**：Map 与 Bot 目录可能由并发请求创建共同的缺失父目录；浏览器目录验收同时请求两个 catalog，确保创建竞争不产生 500，同时保留文件阻塞与链接拒绝检查。
- **元数据**：修改 `meta-data/task-options.toml` 的显示标签时，同步检查摘要的独立预期值及动态元数据测试的替换样例，具体标签从元数据读取，不在本指引复制。
- **关机与会话**：关机自动验收使用记录型安全壳或替身；真实关机单独人工验收。会话探针使用隔离数据，不操作用户正在运行的对话。

## 真实 Herdr 自动关闭验收（P22）

`python scripts/test-real-herdr-close.py` 是独立的真实依赖自动验收，不在普通替身回归中隐式启动。当前入口要求 Windows、已安装的真实 Herdr / Pi、Python 3.9+、Node.js，以及已安装依赖的 toolkit checkout。先构建当前 worktree 后端。路径参数可显式传入，或使用 `--paths-file`（默认仓库 `paths.local.toml`）：缺省参数从 `herdr/herdr.exe`、`pi_package`、`pi_agent/extensions/herdr-agent-state.ts`、`toolkit` 根派生；backend 默认取当前源码的 debug 产物并尊重 `CARGO_TARGET_DIR`。显式参数优先，全部依赖参数均提供时不读取根文件。原生会话验收 `scripts/test-native-session-storage.py` 同样支持 `--paths-file`，缺省包路径使用 `pi_package`、`omp_package`。这些入口经 Rust `herdr-paths` 解析 TOML，不自行实现根引用；根可指向不同磁盘。

以下为显式参数方式：

```powershell
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot
python scripts/test-real-herdr-close.py `
  --herdr 'C:\path\to\herdr.exe' `
  --backend 'backend/target/debug/herdr-bot.exe' `
  --pi-package 'C:\path\to\node_modules\@earendil-works\pi-coding-agent' `
  --pi-state-extension 'C:\path\to\herdr-agent-state.ts' `
  --toolkit 'C:\path\to\my-agent-toolkit'
```

入口创建独占 Temp 根目录，清除继承的 Herdr 定位及模型凭证环境，显式隔离 HOME、APPDATA、XDG 配置/状态、Pi 原生存储、命名 Herdr server 和后端持久数据；不连接现有 Herdr，不读取用户 Chat。真实 toolkit 的角色入口及配置复制到临时目录，只修改副本的后端地址；官方 Pi Herdr 状态扩展使用显式只读路径。模型使用数值回环的确定响应服务，不访问真实模型或使用真实密钥。后端仅监听动态回环端口，关闭 mDNS。

验收通过真实发起 Run、manager dispatch 和 complete HTTP 路径触发自动关闭，绝不以直接 `tab close` 替代。先记录同一 Workspace 的专属保留 Tab、目标执行 Tab、其分割 pane 中实际存活的受控 Python 进程及原生 Chat 内容。之后独立查询 Herdr Tab 列表、持有 Windows 进程句柄确认进程退出，并使用已安装 Pi 的 `SessionManager.open(...).getBranch()` 再次取得原生 user/assistant 历史；不以历史列表登记或 completed 状态冒充效果，不把历史读取当作重新派发。保留 Tab 避免关闭最后一个 Tab 清空 Workspace，不强制确认，不承诺清理已脱离会话的 daemon。

stdout 返回 `evidence.json` 路径和真实结果；该文件包含命令原始输出、HTTP 响应、前后身份、存活及历史内容/哈希。Herdr/后端 stdout 与 stderr 持续消费并保存独立日志。成功或失败均停止且确认测试拥有的 server/子进程，原始错误与清理错误分别保留；Temp 数据保留供证据核对，不删除用户资源。缺失依赖、额外确认、目标仍在、进程仍活、原生历史不可读等均返回非零，不跳过且不算通过。文档仅说明入口，实际通过/失败以执行日志为准；本入口只验收 P22，不代表 P1–P21 已通过。

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
