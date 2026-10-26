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

## 修改测试或夹具

- **所有权**：每个用例独占可变临时数据与动态回环端口；同一竞争／重启场景保留必要共享。自动发现目标不等于自动消除资源冲突。
- **路径**：真实子进程使用测试拥有的临时 cwd 和显式持久路径，只读元数据可指向仓库。配置文件放在 Temp 不会自动改变相对数据路径的解析基准；启动期 `.data` 与迁移标记也要归测试所有。
- **环境**：`HERDR_FIXTURE_DIR`、`HERDR_BOT_FIXTURE_RECORD_DIR` 会覆盖默认夹具位置；继承环境时保证它们不把不同用例导向同一可变目录。
- **进程**：取得真实子进程端口后持续消费 stdout／stderr，直到进程退出。初始化失败与正常结束都执行所属资源清理；同时保留原始错误和清理错误。
- **Windows 清理**：等待服务退出事件后再解释 `taskkill` 失败状态，避免把已退出但尚未被观察到的进程误报为存活；仍存活时报告错误。临时目录删除不代替进程结束。
- **元数据**：修改 `meta-data/task-options.toml` 的显示标签时，同步检查摘要的独立预期值及动态元数据测试的替换样例，具体标签从元数据读取，不在本指引复制。
- **关机与会话**：关机自动验收使用记录型安全壳或替身；真实关机单独人工验收。会话探针使用隔离数据，不操作用户正在运行的对话。

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
