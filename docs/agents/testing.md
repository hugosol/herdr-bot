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
- Chat 的纯临时输入场景（空固定 prompt、无 Skill、无词典引用）应独立核对完整投递文本与输入逐字相等，不假定末尾空段或无条件追加 agent 指引。

## 修改测试或夹具

- **所有权**：每个用例独占可变临时数据与动态回环端口；同一竞争／重启场景保留必要共享。临时夹具目录须原子创建并跳过已有目录，不能因 Windows 重用 PID 而接管旧运行残留的数据。浏览器夹具探测端口后到子进程绑定前仍存在竞争；仅动态端口的已确认地址占用错误可重新分配（最多 32 次），固定地址及其他启动失败直接报错，不重试业务操作。自动发现目标不等于自动消除资源冲突。
- **路径**：真实子进程使用测试拥有的临时目录。普通相对路径按声明配置文件的父目录解析，扩展入口按元数据文件父目录解析；cwd 可以与配置目录不同。需要根引用时生成隔离根文件，不读取开发者的 `paths.local.toml`。启动期 `data_directory` 与迁移标记也要归测试所有。
- **环境**：`HERDR_FIXTURE_DIR`、`HERDR_BOT_FIXTURE_RECORD_DIR` 会覆盖默认夹具位置；继承环境时保证它们不把不同用例导向同一可变目录。
- **进程**：取得真实子进程端口后持续消费 stdout／stderr，直到进程退出。初始化失败与正常结束都执行所属资源清理；同时保留原始错误和清理错误。
- **并发历史持久化诊断**：跨来源并发 register 验收的 `historySaved` 断言保留完整回执及所属后端 diagnostics，区分事件接受与历史保存失败；后续通过不能抹去先前失败，也不能据此宣称已修复无法定位的间歇故障。
- **Windows 清理**：`taskkill` 完成后再开始服务退出事件的有界确认窗口，然后解释 `taskkill` 失败状态，避免终止命令在全局负载下耗时较长时，把已退出但尚未被观察到的进程误报为存活；确认窗口结束仍存活时报告错误。临时目录删除不代替进程结束。
- **目录初始化**：Map 与 Bot 目录可能由并发请求创建共同的缺失父目录；浏览器目录验收同时请求两个 catalog，确保创建竞争不产生 500，同时保留文件阻塞与链接拒绝检查。
- **元数据**：修改 `meta-data/task-options.toml` 的显示标签时，同步检查摘要的独立预期值及动态元数据测试的替换样例，具体标签从元数据读取，不在本指引复制。
- **关机与会话**：关机自动验收使用记录型安全壳或替身；真实关机单独人工验收。会话探针使用隔离数据，不操作用户正在运行的对话。

## 首批 Suite 与稳定 Task ID 迁移

`cargo test --manifest-path backend/Cargo.toml --test initial_suite_migration` 拥有 P39/P40/P41/P43，使用真实 `migrate-initial-suites` 交付命令、独占临时权威文件和状态目录，以及公开 Task／Suite HTTP。22 个旧 Task 的 ID、agent、旧选择与 17/1/4 结果来自独立字面样例；显示名全部改成干扰值，trigger 含用户新编辑。Bot 引用、原生 Chat 来源、Run／node 执行身份分别独立写入并读回不变。18/11 技能、按宿主具体扩展路径、MCP、四语言及探索 Pi `on` 用独立批准清单核对；实际宿主发现及命令消费仍归对应专项，不从配置通过推断原生加载成功。

失败覆盖 Windows 真实目标替换拒绝、计划发布后的真实子进程终止、重启、部分生效后的新编辑冲突和完成后新编辑。只有所有输出独立读回成功才能有 `complete.json`；已生效文件保留，重跑不重复生成，不自动回滚。中断进程写测试拥有的日志文件并在所有分支终止／等待退出；普通命令通过 `Command::output` 并行消费输出并等待退出。测试不运行任何用户部署迁移，不读取用户凭据、连接原生 Chat、启动用户 daemon 或操作集成工作区。源码预设的格式升级与用户部署的明确授权操作不同，发布工单本身不是生产操作授权。

Task API 的旧 `extensions` 字段（包括空数组）必须拒绝，固定扩展仍显示但不可勾选。资源装配并发／故障夹具及真实 Pi 模式／MCP runner 的 Task 请求同样不发送该旧字段；Pi Suite 扩展使用 `extensions: {pi: [...]}`，避免原生验收在启动前因旧请求或定义形状被拒绝。浏览器的精确 Pi argv 预期同时保留 `builtin:mcp`、`builtin:tool-search`、无 Suite 时的 `builtin:codemode` 以及元数据声明的固定 `pi-suite`；过滤恢复仍逐项断言文件扩展，不把内建扩展误当作失效文件。Suite `extensions` 只接受宿主键对象，不接受旧数组；现有浏览器 Suite 管理、Task 引用、三步编辑和窄视口检查使用相同新形状，原先只测试旧 Task 可选扩展勾选的场景已退役，不保留旧业务选择兼容分支。

## 首批 Suite 原生发现与实现命令

`python scripts/accept-initial-suites.py` 拥有 P11 的探索 OMP 18／探索 Pi 18／实现 OMP 11、P42 实际 `hyper-auto-coding` 注册及无任务调用，以及 P44 两仓完整交付清单和原生公开入口一致性。独立字面名称与来源清单不从装配器重算；真实 `pi.getCommands()` 包含手动调用的翻译技能，不能用模型提示中的较窄集合替代。入口、隔离依赖根、明确 Git 基线、构建命令、进程清理和证据含义见[首批 Suite 原生验收](../initial-suite-native-acceptance.md)。`python scripts/accept-initial-suites.test.py` 仅检查 S3 外来后端拒绝，不等于 S4 通过。缺链接权限、宿主能力或实际发现均保留非零和未验证；不安装、不提权、不读取用户凭据，不重跑其他票的资源/MCP/生命周期矩阵来冒充本票成功。

## OMP Suite 四语言 LSP 真实验收

`python scripts/test-real-herdr-suite-lsp.py` 拥有 P37/P38 的 S4 专项；S1 缺程序声明反馈由 `frontend/browser-tests/suiteLsp.spec.ts` 覆盖，S2/S3 生成产物由 `backend/tests/suite_omp_lsp.rs` 补充。后两者不替代真实服务启动。先构建当前 worktree 后端；四种服务、相关语言运行环境及 OMP／Herdr 必须已经安装好，runner 不安装、不提权、不修改 PATH，不用源码 CLI／SDK 或 Pi 替代已交付的 OMP 入口。

```powershell
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot
python scripts/test-real-herdr-suite-lsp.py `
  --herdr 'C:/installed/herdr/herdr.exe' `
  --omp-package 'D:/node_modules/@oh-my-pi/pi-coding-agent' `
  --backend 'backend/target/debug/herdr-bot.exe' `
  --rust-toolchain 'C:/installed/rustup/toolchains/stable-x86_64-pc-windows-msvc'
```

`--rust-toolchain` 只设置子进程 `RUSTUP_TOOLCHAIN` 为既有工具链目录，同时使用独占 `CARGO_HOME`，避免 HOME 隔离后 Rustup 代理找不到原先安装的组件；不安装组件、不读取用户 Cargo 凭据、不改 PATH。不使用 Rustup 代理的部署可省略。四种原生命令为 `rust-analyzer`、`typescript-language-server --stdio`、`jdtls`、`pyright-langserver --stdio`；TS 还需兼容 TypeScript，Java 需 JDK／JDT LS，不能仅凭 launcher 或版本号判断可工作。

入口在独占 Temp 配置中声明批准的四服务，通过公开 Task／Chat HTTP → 真 Herdr → PATH 上安装版 OMP，为 Rust／TypeScript／Java／Python 分别创建项目标记和源文件。受控数值回环模型只调用真实 `write xd://lsp`，检查 `capabilities` 中实际 initialize 响应及随后 `status` 的 `ready`，并通过 Windows OS 查询独立观察所属新增语言服务进程。它证明原生启动和发现机制，不证明模型自主开发质量。缺可执行程序负例保留 Rust 项目标记，由测试拥有的用户 LSP 配置将命令指向确实不存在的程序：原生能力查询拒绝、status 不包含服务、OS 没有所属 rust-analyzer 进程。正例用项目配置覆盖该用户命令，核对项目 > 用户 > Suite 的原生优先级；不关闭其他默认语言服务。

每次输出独占目录内 `evidence.json`，记录源码 revision／diff／哈希、交付二进制与安装 CLI 哈希、版本、隔离来源、实际请求／响应、Herdr terminal、所属进程及每场景 `verified`。HOME／APPDATA／XDG、agent 数据、后端持久化、命名 Herdr server 和动态回环端口均隔离；固定扩展只记录原生进程身份，不替换 LSP 或内部 monkey patch。持续消费服务日志，正常及失败均停止所属服务、以持有的 Windows 进程句柄确认退出；原始错误与清理错误分别保留。不连接用户 Chat／daemon，不复制认证或操作真实关机。

默认 `--language all` 才是四语言 P37 验收；缺任何程序会记录 `blockers`、保持未验证并非零退出，已有声明、非空日志、零退出的其他场景均不能冲抵。`--language rust-analyzer` 等参数只用于较窄反馈，成功也不是完整 P37 通过。P38 UI 只说明声明／运行的区别，不伪造服务健康状态；运行事实以原生反馈和 OS 观察为准。

## Suite 装配发布与链接安全验收

`backend/tests/suite_assembly.rs` 拥有 P34/P36 和 P35 的装配发布部分，入口仅为既有公开 Task／Chat HTTP 与独占部署目录（S2/S3），不增加测试专用产品 API。常规 `cargo test --manifest-path backend/Cargo.toml --test suite_assembly` 检查资源消失、Windows 独占读锁造成的真实不可读失败、未拥有文件保留，以及同 Suite／不同 Suite 的两套独立 LSP 与 MCP 配置哨兵；核对 MCP 原生资源包与技能、LSP 共属本次装配树，返回的 `--extension` 指向该包。外部 Herdr 夹具在启动效果处受控阻塞；第二个启动必须仍能取得完整产物，随后重读两套最终清单、释放门闩并等待公开完成状态，确保文件协调不跨外部效果。观察失败也先释放门闩并等待已接受的所属命令，原始错误与清理错误分开保留。这些夹具不是原生 agent 消费证据；P35 并发真实启动消费由独立专项负责。

以下两例是**真正的、默认 ignored 的专项**，常规套件的 ignored 不等于通过。显式开关只能授权尝试测试，不修改操作系统权限。第二盘必须已经可写且与系统临时目录盘符不同；测试只在其根目录独占新建自己的临时子目录，清理只删除自己创建的目录。不得指向用户资源目录来代替测试数据。

```powershell
$previous = $env:HERDR_BOT_MANUAL_SYMLINKS
$previousDrive = $env:HERDR_BOT_SYMLINK_SECOND_DRIVE
$env:HERDR_BOT_MANUAL_SYMLINKS = '1'
$env:HERDR_BOT_SYMLINK_SECOND_DRIVE = 'E:/' # 换成已有的第二盘根目录
try {
  cargo test --manifest-path backend/Cargo.toml --test suite_assembly -- --ignored --nocapture
} finally {
  $env:HERDR_BOT_MANUAL_SYMLINKS = $previous
  $env:HERDR_BOT_SYMLINK_SECOND_DRIVE = $previousDrive
}
```

`windows_true_file_directory_and_cross_volume_links_preserve_targets_on_failed_rebuild` 独立尝试跨盘文件真符号链接及目录真符号链接，再通过产品启动生成目录链接，使用 Windows 文件类型核对（junction 不算）。成功发布后增加非法 Windows 链接名触发后续装配失败；核对旧完整产物链接、真实源文件的独立哈希与未拥有文件均不变，本次失败树已经移除，无新增 Herdr 调用。`windows_same_and_different_suites_publish_complete_linked_resource_sets` 在同 Suite 和不同 Suite 的外部门闩竞争中，观察两套独立技能正文、扩展源码／资产、入口清单和 LSP 哨兵，防止只验证参数或空技能配置冒充全资源发布一致性。

缺 Windows 权限（如 `os error 1314`）、第二盘或任一证据时专项非零失败并保留未验证结论；不自动提权、不改开发者模式、不创建 junction、不复制替代，也不以常规成功抵消。输出原始断言／OS 错误，成功时打印真实链接路径、跨盘事实及独立内容哈希；输出应保留在运行日志中。真实 Herdr／安装版 OMP／Pi 消费不由这些受控启动夹具宣称通过。完整后端交付仍运行 `node scripts/test-backend.mjs`；只有后端与文档修改，不要求前端构建或浏览器作为本票的替代证据。

## OMP Suite 技能真实消费验收

`python scripts/test-real-herdr-suite-skills.py` 专项覆盖 P5/P6/P7/P8/P10/P15 的 OMP 技能场景，必须先构建当前 worktree 后端。它经公开 Task／Chat HTTP → 真实 Herdr → PATH 上已安装 OMP 的正常交付入口运行，不直接调用源码 CLI、SDK 或替代 MCP manager。依赖均须预先具备，runner 不安装程序、不改 PATH、不提权、不修改开发者模式。只声明依赖包路径不会替换真正启动的 `omp`，runner 会记录 PATH launcher、包入口和后端哈希。

```powershell
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot
$previous = $env:HERDR_BOT_MANUAL_SYMLINKS
$env:HERDR_BOT_MANUAL_SYMLINKS = '1'
try {
  python scripts/test-real-herdr-suite-skills.py `
    --herdr 'C:/installed/herdr/herdr.exe' `
    --omp-package 'D:/node_modules/@oh-my-pi/pi-coding-agent' `
    --backend 'backend/target/debug/herdr-bot.exe'
} finally {
  $env:HERDR_BOT_MANUAL_SYMLINKS = $previous
}
```

默认 `--scenario selected` 创建独立根引用／声明相对路径／含空格的两个既有技能目录，在两个不同 Chat cwd 中实际 `read skill://...` 读取技能正文、scripts、references 与资产；第二次发起前向来源集合新增未选技能。用户／项目默认技能及同名错误正文是独立哨兵。受控回环模型检查原生最终技能列表，只要求真实 read／write 工具执行预定动作；通过实际原生 `xd://mcp__native_probe` 调用取得字面结果，并核对 MCP 服务器记录的 `tools/call`。同次还观察 Suite 本地模板未注入、原生 `AGENTS.md` 指令仍在。这里只证明协议与资源消费机制，不证明真实模型自主遵循业务职责。

runner 隔离 HOME／APPDATA／XDG、agent 配置与会话、命名 Herdr server、动态回环模型／后端和源／生成目录，不读取用户凭据或已有 Chat；静态页面使用测试拥有的占位文件，因为本入口不承担 S1 页面验收。正常及失败路径持续消费服务日志、停止所属进程，持有真实 MCP 进程句柄确认退出；证据 JSON 分开保留原始错误和清理错误。输出指向独占 Temp 的 `evidence.json`，保留实际请求／响应、公开入口结果、MCP wire、来源 revision／diff／哈希、产物链接及原始日志，不在仓库生成验收报告。

真实目录符号链接需要明确开关及 Windows 已有权限。缺权限时必须非零并明确记录（例如 `os error 1314`），不以 junction、复制、skip 或参数检查冒充通过。`--scenario empty` 是独立的较窄场景：无需建链接，验证明确空技能 Suite 的技能过滤、原生 MCP 实际调用、模板省略及原生指令保留，**不能**替代 selected 场景或证明非空技能／资产消费。缺任何真实依赖或 selected 权限，完整专项仍是阻塞／未验证。

公开启动的受控失败回归为 `cargo test --manifest-path backend/Cargo.toml --test suite_omp_skills`，使用真实 Windows 独占共享锁制造预览后不可读资源，以及普通文件阻塞生成目录；检查 HTTP 明确拒绝且 Herdr 无调用。这些负例不需要 symlink 权限，不替代 S4 成功证据。

## OMP Suite 原生 MCP 真实消费验收

`python scripts/test-real-herdr-suite-mcp.py` 覆盖 Suite 契约 P17/P19/P20/P21/P22/P46 的 OMP MCP 场景。先构建当前 worktree 后端，使用实际安装的 Herdr 与 PATH 上的打包 OMP；不安装依赖、不修改 PATH、不替换 agentDir、不运行源码 CLI 或 SDK 组装。它复用技能专项的独占 HOME／APPDATA／XDG、配置、会话、命名 Herdr server、动态回环模型／后端和进程清理设施，但不认领所选技能验收。MCP 夹具只充当外部服务器，实际读取独立声明目录的字面文件；原生发现、优先级、完整 manager 与工具执行仍是实际交付宿主。先经公开 HTTP 发起原生管理 Chat（空 Task 固定 prompt、初始输入 `/mcp help`），在成功场景观察外部夹具初始化及原生界面后，用原生 Esc／Ctrl+U 退出帮助或补全并清空管理输入，再通过 Herdr 原生 agent 交互提交一次 MCP 读取指示；不承担初始化文本的终端粘贴传输验收，不重试业务提交、不补发 Enter。此等待和管理交互仅是测试前置条件，不写入产品启动路径；`unreachable` 独立验证产品没有全部就绪门禁。

```powershell
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot
foreach ($scenario in @('central', 'precedence', 'empty', 'full', 'unreachable')) {
  python scripts/test-real-herdr-suite-mcp.py `
    --herdr 'C:/installed/herdr/herdr.exe' `
    --omp-package 'D:/node_modules/@oh-my-pi/pi-coding-agent' `
    --backend 'backend/target/debug/herdr-bot.exe' --scenario $scenario
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

`python scripts/test-real-herdr-suite-extensions.py` 专项负责 P7（OMP 扩展）与 P9（OMP）的 S2/S3/S4 行为证据。先构建当前 worktree 后端，再使用已安装 Herdr 和 PATH 上正常交付的 OMP；依赖不由 runner 安装，不改系统 PATH，不读取用户凭据或启动用户 Chat：

```powershell
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot
$previous = $env:HERDR_BOT_MANUAL_SYMLINKS
$env:HERDR_BOT_MANUAL_SYMLINKS = '1'
try {
  python scripts/test-real-herdr-suite-extensions.py `
    --herdr 'C:/installed/herdr/herdr.exe' `
    --omp-package 'D:/node_modules/@oh-my-pi/pi-coding-agent' `
    --backend 'backend/target/debug/herdr-bot.exe'
} finally {
  $env:HERDR_BOT_MANUAL_SYMLINKS = $previous
}
```

入口复用技能专项的独占配置、动态回环模型／HTTP 端口、持续消费日志和所属进程清理。一个 Suite 声明相对入口文件与集中根引用包目录，均包含空格；包目录的原生 manifest 选择非 index 入口。两个不同 Chat cwd 均放置错误相对入口哨兵。真实工厂 import 同包 sibling TS 和 `node_modules` 模块，真实工具读取 `import.meta.url` 相对 JSON 资产并计算，独立字面预期为 `RELATIVE-ASSET-412:64` 与 `ROOT-ASSET-905:80`。受控模型通过 OMP 原生 `write xd://...` 调用工具，不伪造工厂或工具执行；此证据只证明机制，不证明真实模型自主遵循。

验收同时核对生成位置中的包目录真符号链接、入口清单、来源文件字节不变及固定扩展仍加载。输出的独占 Temp `evidence.json` 保留公开 HTTP 结果、原生模型请求／工具结果、源码与可执行文件哈希、原始日志和分开的清理错误。链接权限不足（如 Windows `os error 1314`）或任何真实依赖缺失时非零退出，明确记为未验证；源码直接加载成功、生成列表存在、junction 或普通套件通过均不能替代最终链接装配的 S4 成功证据。

P46 的扩展负例入口为 `cargo test --manifest-path backend/Cargo.toml --test suite_omp_extensions`：真实 Windows 独占共享锁分别阻止入口文件及包内资产读取；另用普通文件只阻塞扩展输出子目录，保持技能装配可写。全部经公开预览／启动 HTTP 检查明确拒绝且 Herdr 无调用，不需要链接权限，不代替上述真实成功场景。

## Pi Suite 启动开发烟测

`07` 的开发烟测为公开 Task／Chat HTTP → 真实 Herdr → PATH 上安装版 Pi；完整 Pi 契约矩阵仍由资源／MCP和模式／模板验收拥有。先构建当前 worktree 后端，预先安装依赖，toolkit 必须提供公开工厂期 `--pi-suite-config`。不安装、不提权、不修改 PATH，不用 SDK 或替代 launcher：

```powershell
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot
$previous = $env:HERDR_BOT_MANUAL_SYMLINKS
$env:HERDR_BOT_MANUAL_SYMLINKS = '1'
try {
  python scripts/test-real-herdr-suite-pi.py `
    --herdr 'C:/installed/herdr/herdr.exe' `
    --pi-package 'D:/node_modules/@earendil-works/pi-coding-agent' `
    --toolkit 'D:/installed/my-agent-toolkit' `
    --backend 'backend/target/debug/herdr-bot.exe'
} finally {
  $env:HERDR_BOT_MANUAL_SYMLINKS = $previous
}
```

独占临时目录隔离 HOME／APPDATA／XDG、Pi 状态、后端配置、命名 Herdr server 和动态回环端口；不读取用户凭据或会话。共享声明相对路径含空格，扩展包 manifest 指定非 index 入口。受控回环模型请求 Pi 真实 read 消费技能正文，并调用真实包工具读取相对 JSON 资产，独立字面结果分别为 `PI-SKILL-BODY-731`、`PI-PACKAGE-ASSET-482:64`（19×3+7）。这只证明启动接线与消费机制，不证明真实模型自主遵循。

持续消费进程输出，正常及失败均停止所属 Herdr/server/子进程，原始错误与清理错误分别写独占 Temp 的 `evidence.json`；记录源码 revision／diff／哈希、二进制和安装入口哈希、实际 HTTP／模型请求及 terminal。缺真符号链接权限（如 `os error 1314`）、依赖／flag 未交付、实际资源未消费或清理失败均非零，不 skip、不退化为直读源码加载，不把产物／参数检查算作 S4 通过。

## Pi Suite 资源与原生 MCP 跨仓验收

完整 S2/S3/S4 矩阵入口为 `python scripts/accept-suite-pi-resources.py`，显式接收两个 worktree、当前后端、隔离 paths 文件、场景和独占证据目录。覆盖集中根／相对路径／空格／不同 cwd、真实技能与扩展依赖、builtin 与 Suite MCP 同次调用、文件优先／等价／禁用／空清单，以及配置／装配拒绝和原生不可达状态。参见 [Pi 资源验收](../pi-suite-resource-acceptance.md) 的依赖、独立预期和完整复跑命令。缺真实依赖、真链接权限或清理失败非零；受控模型只验证消费机制，不证明模型自主质量，全局套件不能替代逐场景证据。

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

## 文件交接真实角色自动验收

`python scripts/test-workflow-file-handoff.py --scenario roles` 是独立的跨仓入口，加载指定 toolkit worktree 的实际角色源码及当前 herdr-bot worktree 构建的后端。它不使用确定响应服务替代真实业务模型，也不修改用户部署。当前真实模型链路仅支持 Windows / Herdr / **Pi** 和 OpenAI-compatible chat-completions 服务；结果不外推为 OMP 真实模型通过。参数保真、严格 report 校验和真实后端落盘另由入口内调用的 `tests/harness-dock/node-report-contract.test.ts` 同时覆盖 OMP/Pi；该 dock 使用受控 provider/Herdr，不能充当模型职责遵从证据。

先在两个参与源码根安装/构建依赖：toolkit 使用 Bun 1.4.4 与 `bun install --frozen-lockfile`；herdr-bot 执行 `npm --prefix frontend ci`、`npm --prefix frontend run build` 生成本 worktree 的 `backend/static`（全后端进程检查必需）；后端使用独立的 `CARGO_TARGET_DIR` 构建 `cargo build --manifest-path backend/Cargo.toml --example safe_shutdown_shell --bin herdr-fixture --bin herdr-bot`。临时 toolkit worktree 的 frozen baseline 依赖定位可显式设置 `OMP_BASELINE_RECORD_ROOT` 为原 toolkit 根，不能因此替换实际被测源码。按 toolkit 的测试规范协调重型运行窗口。

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
$env:CARGO_TARGET_DIR = Join-Path $HERDR_BOT_WT 'backend/target'
$env:OMP_BASELINE_RECORD_ROOT = 'D:/original-toolkit'
python scripts/test-workflow-file-handoff.py `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --scenario roles --paths-file 'D:/isolated-acceptance/isolated-paths.toml'
```

`--scenario plan-workflow` 的独立能力缺口阶段会通过真实 Chat 提交 `P28-GAP-SCOPE` 用户澄清：只确定税务申报表这一业务对象，系统、接口、材料、凭据及生产授权仍为未知且阻塞；要求保存并读回能力建设需求，不补齐缺失能力、不批准概要方案或 manual 报告、不授权创建或执行。原始输入及回答保留在 interventions 和原生历史中。

Windows 验收可显式追加 `--shell-path 'D:/Program Files/Git/bin/bash.exe'`，将实际安装的 Git Bash 写入本次隔离 Pi settings，不修改用户配置。不要让原生 Bash 工具落到 WSL：其 Linux 回环与 Windows 后端不同，`curl 127.0.0.1` 会报告连接失败，不能把该环境阻塞当业务能力缺口或强行推进。toolkit 全量跨仓检查还应显式设置 `HERDR_BOT_BINARY` 为本 worktree 的 `backend/target/debug/examples/safe_shutdown_shell.exe`，`HERDR_BOT_METADATA` 为本 worktree 的 `meta-data/task-options.toml`；`HERDR_BOT_REPO` 不替代这两个 interop 参数，禁止使用生产可执行文件冒充安全壳。

入口沿用 `test-real-herdr-close.py` 的独占 HOME/APPDATA/XDG/Pi 存储、命名 Herdr server、动态回环后端、关闭 mDNS 与所属进程清理设施，但覆盖模型 provisioning，绝不启动原入口的受控业务模型。透明回环 observer 仅转发实际模型/HTTP 字节，不预设模型工具选择。场景由原生 manager dispatch → 首次执行生成旧文件/true → 经明确业务授权 fail/retry → 新执行生成部分文件/false → 独立 complete 门禁拒绝 → collect 真等待 → 外部批准文件释放 → 新 true 唤醒 → manager 显式 complete/dispatch → 所需输出已登记的 consumer 启动并完成；所需输出缺失的拒绝派发边界由后端隔离回归覆盖，不再把 unresolved 地址注入 Chat。业务批准由隔离 fixture 明确给出，不把用户手工确认当成自动验收替代。

证据保留在输出的独占 Temp 根：源码根/revision/diff、锁文件与后端哈希、依赖版本、模型服务身份（仅密钥指纹）、实际模型请求/响应、工具参数/结果、HTTP 起止、原生 Session 历史、实际文件、只读运行记录副本及每项覆盖关联。独立背景、业务 input、当前地址和正文/聊天敏感标记核对原生初始化；manager 工具轨迹限定为 run/collect，不默认读输出正文。报告只允许简短编排结论，不复制详细行。观察窗口默认每阶段 300 秒（`--timeout 1..900`），外部业务等待 240 秒，保留报告后 collect 至少观察 1.25 秒仍未返回，再释放新报告；runner 不重试业务/模型选择，不延时掩盖失败。

若宿主只公开真实文本 `completion(prompt, model='default').wait()`，可用测试专属 `scripts/workflow_completion_transport.py` 的 `CompletionTransport` 将完整实际原生历史和可用工具定义送给该真实模型；模型自主返回下一条文本/工具参数，薄传输只序列化为 Pi provider 响应，由 Pi 真正执行工具并写原生历史。它不读取验收预期、不选择业务动作、不模拟工具结果、不重试模型决策，也不修改产品 adapter；这类结果只证明该模型传输下的真实角色行为，不证明原生 OpenAI provider 的 wire 合规性。传输保留每次实际请求、发给 completion 的完整 prompt、原始输出和映射结果；错误非零返回且不生成替代动作。

以下代码在提供上述公开 `completion` 的 Python 宿主执行（不是普通 Python CLI）；`HERDR_BOT_WT`、`TOOLKIT_WT`、`PATHS_FILE`、`EVIDENCE_DIR` 均为显式绝对路径，backend 和依赖须先按上文准备。`copy_context` 将宿主授权调用上下文传入 HTTP 工作线程，不读取私有 token：

```python
import contextvars, json, os, pathlib, subprocess, sys, tempfile
sys.path.insert(0, str(pathlib.Path(HERDR_BOT_WT) / 'scripts'))
from workflow_completion_transport import CompletionTransport
context = contextvars.copy_context()
def actual_completion(prompt):
    return context.copy().run(lambda: completion(prompt, model='default').wait())
with CompletionTransport(actual_completion, pathlib.Path(EVIDENCE_DIR) / 'completion-wire') as transport:
    with tempfile.TemporaryDirectory(prefix='isolated-completion-config-') as directory:
        config = pathlib.Path(directory) / 'model.json'
        config.write_text(json.dumps({'provider': 'public-completion-transport',
            'model': 'public-default', 'baseUrl': transport.base,
            'apiKeyEnv': 'WORKFLOW_COMPLETION_TRANSPORT_KEY'}), encoding='utf-8')
        env = dict(os.environ, WORKFLOW_COMPLETION_TRANSPORT_KEY='isolated-local-transport')
        result = subprocess.run([sys.executable, 'scripts/test-workflow-file-handoff.py',
            '--backend', str(pathlib.Path(env['CARGO_TARGET_DIR']) / 'debug/herdr-bot.exe'),
            '--toolkit', TOOLKIT_WT, '--scenario', 'roles', '--paths-file', PATHS_FILE,
            '--model-config', str(config)], cwd=HERDR_BOT_WT, env=env)
        if result.returncode:
            raise RuntimeError(f'roles acceptance failed: {result.returncode}')
```

这里的 local-transport 值仅满足 Pi 自定义 provider 的配置要求，不是模型凭据；真实模型访问仍使用宿主公开授权能力。传输本身的 HTTP/证据保真回归为 `python scripts/workflow_completion_transport.test.py`，其中受控 completion 只验证传输，不作为角色验收结果。

CLI 的缺依赖 fail-closed 回归可在设置 `WORKFLOW_HANDOFF_TOOLKIT` 为参与 toolkit 根后运行 `python scripts/test-workflow-file-handoff.test.py`；它只核对非零、原始错误、无虚构 Run 与无服务启动日志，不宣称真实角色链路通过。

通过必须同时具备两宿主 dock、真实模型链路与对应 worktree 常规检查证据。入口未取得模型、宿主缺失、行为不合预期、等待超时或清理失败均非零；缺依赖即明确未验证，不以准备完成、提示词存在或最终 completed 推断通过。正常/失败均停止所属服务与进程，分别保存原始与清理错误，不调用真实关机、不连接/关闭用户服务，不启用 LAN/mDNS。

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
