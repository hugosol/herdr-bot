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
- 原生可见性浏览器清理通过独占 Chrome 的 Node `exit` 事件或独立 Windows 进程句柄确认退出；任一确认后回收句柄观察进程。`taskkill` 返回码或输出不能代替退出确认。

## 固定 Rust 表示验证

`python -m pip install -r scripts/requirements-contracts.txt` 安装单独的 Schema 验证依赖后，`python scripts/test-script-contracts.py` 从公开文件 seam 消费 `docs/contracts/rust-script-bots.schema.json` 和独立 JSONL 字面实例。离线检查类型、必填、互斥、已知能力、完整配置形状、无 Chat 来源与失败效果表达；不启动宿主／服务、不写业务配置，不以源码快照验证接线。跨材料身份／revision／正文确认及真实保存仍是业务实现的公开 HTTP／文件验收，不能由 Schema 通过推断。

表示票不提前注册可保存但不能运行的能力，不改现有 Task／Map 运行代码；回归仍先构建本 worktree 静态页，再用独立绝对 `CARGO_TARGET_DIR` 执行 `node scripts/test-backend.mjs`。实例与长期文档只是新表示，不代表两种 Rust 业务或 S2／S3 已交付。

## Rust 空白创建公开 HTTP／文件回归

先构建本 worktree 静态页面，使用独立绝对 `CARGO_TARGET_DIR`，开发反馈运行 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch rust_create_ -- --nocapture`；完成检查运行 `node scripts/test-backend.mjs`。`workflow_dispatch/rust_create.rs` 通过公开 S1 HTTP／文件 interface 测定义、真实 Run、manager dispatch、完整配置／revision、自动报告、collect、完成门禁及真实临时材料；Map 在独占临时定义目录经文件 seam 发布并以 HTTP 读回／预检，不声称存在 HTTP 新建 Map 接口。正式创建流程定义未迁移。

- `target_round_trips`：固定目标互斥、未知能力拒绝及拒绝后原定义不变。
- `dispatch_writes`／`map_target`：normal Task 与完整真实 Map 目标、新 Bot 三空指南、真实 revision／baseline、独立 handoff、报告归属；实际派发下游受控宿主进程读取两份命名 JSON，内容和完整公开对象交叉核对。
- `returns_while`：真实 Windows oplock 暂挂业务目标读取，门闩仍持有时独立取得 HTTP dispatch 返回、running lifecycle、无报告和首次 complete 拒绝；改写 node.input／当前规则后仍用已冻结 inputs／inputRule／outputRule。
- `rejects_bad_requests`：格式、根／嵌套重复 key、缺项、未知字段／版本、null、授权、脚本／歧义／缺失目标、身份／绑定和目录歧义；独立完整对象与实际文件不变，无成功输出。
- `write_readback_and_material_failures`／`refuses_binding`：真实文件阻塞合法 Bot 保存、两个实际文件门闩制造保存后目标读回失败、材料父目录阻塞／同名文件碰撞，以及真实运行 Map 的禁写；核对部分效果，不回滚或提交 true。
- `report_provenance`／`enforces_automatic_policy`：报告来源／execution／Run 一致性，拒绝伪造来源、manual／关闭／manager Task 策略及未声明交付 ID。原 `report_outputs` 回归继续通过同一报告保存实现覆盖未声明／重复 ID、执行级更新与隔离。

每例独占临时定义、zone、Run、文件和数值回环服务；外部 Herdr 为记录型测试宿主，非真实 manager／模型。门闩进入预算 5 秒、业务在途派发返回预算 2 秒、自动报告观察预算 10 秒；结束前等待所属通知 CLI 退出再清理独占根。stdout 保留实际成功 Run／execution／report、baseline／revision、消费者读取对象和拒绝分类，完整入口日志归自身输出目录。通过不证明 S2 原生工具／S3 页面或 H1 人工验收；这些仍按各自分区交付。

### 脚本报告通知失败隔离（P11）

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch rust_create_saved_report_survives -- --nocapture` 复用上述真实 Rust 创建、S1 HTTP／实际文件和既有报告诊断子进程／Herdr prompt gate。两种外部明确拒绝分别立即返回或在最多 10 秒门闩后返回；通知进入预算 5 秒，等待中公开读回预算 2 秒。报告保存及业务写入不使用替身。等待中／结束后核对同一完整报告与全部 outputs，并真实读取 baseline／handoff；失败后至少 1 秒持续取样配置 revision、Run／execution、报告与材料及完整宿主调用，确认一次业务派发、一次报告、一次外部通知，无自动重投。stdout 输出实际窗口、样本数、配置／报告身份、真实材料、调用记录与进程退出确认；stderr 保留共享通知分类诊断。它不声称真实 manager 已接收或用户已阅读，也不认领 11 的原生会话覆盖。

完成检查先构建本 worktree 页面，设置独立绝对 `CARGO_TARGET_DIR`，再运行 `node scripts/test-backend.mjs`。日志不是第二份报告存储，不为此引入自动恢复或消息队列。

## Rust 三路合并保存公开 HTTP／文件回归

先构建本 worktree 静态页，使用独立绝对 `CARGO_TARGET_DIR`，开发反馈运行 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch rust_save_ -- --nocapture`；完成检查运行 `node scripts/test-backend.mjs`。`workflow_dispatch/rust_save.rs` 独占临时定义、zone、Run、文件和数值回环服务，通过 S1 真实 Map／manager dispatch／自动报告和实际下游进程读取命名 saved／handoff；正文、非目标字段及确认 digest 使用独立字面值，不调用业务内部函数。

- P24/P25/P26：三段互异正文含 CRLF、LF、空行、tab 和首尾空格；完整 Bot／真实新 revision／完整目标及实际 saved 逐项比较。下游只从显式 outputs ID 消费，handoff 不含正文／完整配置。
- P7/P8/P33：非法格式、缺项、重复 key、错字段／身份／目标／digest／授权、真实并发 Bot 修改、旧 revision、完整目标配置变化及活动 Run 禁写均拒绝；读回完整对象，不能因 HTTP 成功或正文相同宣称业务成功。
- P15：真实文件阻塞 saved 或 handoff 生成，保存已生效时保留 write/readBack=succeeded 和已读回 outputs，不自动回滚或刷新 baseline。解除故障后由另一个隔离真实 Run 的 manager 显式派发，原 baseline 仍以 revision-conflict 拒绝。该场景不依赖同节点 fail/retry，后者由独立生命周期覆盖拥有。

门闩只控制测试拥有的真实文件 open／外部宿主；进入预算 5 秒，自动报告观察预算 10 秒，所属通知 CLI 退出确认预算 5 秒，确认后再清理独占根。stdout 记录实际 Run、saved、报告、下游文件读回及拒绝／失败分类；完整入口保留日志。受控外部消费者证明命名文件传递，不证明原生模型／manager 行为或 H1/H2；本入口不迁移正式定义，也不改通用替换者或修订流程。

## 发布创建图迁移回归（P22／P23／P27／P28）

先构建本 worktree 静态页，以独立绝对 `CARGO_TARGET_DIR` 运行 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch create_migration_ -- --nocapture`；交付检查运行 `node scripts/test-backend.mjs`。`create_migration.rs` 经真实隔离后端、发布八节点 Map、两种 Rust 能力、实际文件和公开报告／complete／finish 完成闭环。迁移前完整发布定义为 `fixtures/rust-create-migration/before.json`；用例保留独占副本及真实无关 Run／Chat，完整比较原通用替换者 Bot／Task／修订 Map 和无关定义。三个固定独立字面稿为 `abc`、`hello`、空串，确认 digest 独立固定；记录型外部进程真实读取三路 baseline 与各 text/handoff，最终真实保存和 revision／完整目标读回不使用替身。严格 request 缺项／Markdown、guide handoff Markdown、缺显式 when_text 绑定均拒绝，不从邻居文件、自然语言或来源索引寻址。stdout 保留真实 Run、报告、baseline／saved／revision 及拒绝；所属通知 CLI 退出确认后才清理。

这不证明模型遵从指引。P28 的共享三编写者创建 v1 消费／确认由更新的 `accept-workflow-nesting-create.py` 实际原生 workflow 场景覆盖，原非 v1 独立材料／通用替换与真实普通文件安全场景也保留；创建入口在独立／创建图／通用文件安全场景后，还实际组合调用修订入口既有 task-no-materials 业务场景，只有原格式三路消费与通用保存也成功才标 P28；可用 `accept-workflow-nesting-revise.py --cases task-no-materials`（或完整 all）独立复跑定位。两入口沿用本页授权模型、完整 Suite、独占依赖、动态回环、原生历史／HTTP／字节及进程退出要求；三编写者保持原宿主，脚本节点不虚构 Task、Chat 或模型 read。须实际执行对应场景；缺授权／依赖保留未验证，不转交 H1/H2，不以 CLI fail-closed 或定义相等补足行为证据。

## 后端并行与诊断

`node scripts/test-backend.mjs` 不带参数即可并行。Cargo 自动发现测试目标，普通新增 case 或集成测试文件无需登记；保留默认 ignored 状态、example 编译检查及 doctest。

- 资源紧张时用 `--binary-jobs 2` 降低外层并发。
- 排查跨二进制并发问题时用 `--binary-jobs 1`，并与 `cargo test --manifest-path backend/Cargo.toml` 对照；需要内部也串行时再加 `--test-threads 1`。
- 外层并发不限制 Cargo 编译并发；内部线程、环境继承及日志目录选项以 `node scripts/test-backend.mjs --help` 为准。
- 构建失败不启动测试；二进制全部成功才执行 doctest。首个失败或交互终端正常 Ctrl+C 后停止派发，等待在途目标结束，整体返回非零。
- 各目标日志与 `summary.json` 保留在入口打印的目录中。区分失败、取消、未派发和 doctest 未执行；部分完成不是完整通过。强杀进程或关闭终端不保证清理。

Windows 子进程保留普通启动方式；分离进程会改变 Git/MSYS 行为。正常 Ctrl+C 由终端输入处理，保留在途测试的清理机会。

### 节点报告共享保存／通知回归

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch report -- --nocapture` 覆盖公开节点报告的保存失败、保存后通知失败、诊断写入失败、输出登记及既有完成门禁；嵌套收件对象与完成资格另由 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch nested_completion` 覆盖。共享编排位于 `backend/src/report_delivery.rs`，身份、目标、正文、存储范围与业务反馈仍由节点报告路径持有，不新增存储或公开测试入口。

`disconnected_report_caller_keeps_saved_fact_and_does_not_lock_run_during_delivery` 使用真实 TCP 和受控外部 Herdr 门闩：通知阻塞预算 10 秒，进入预算 5 秒，已保存事实读取与同 Run 暂停分别须在 2 秒内完成；断开调用者后仍读回同一报告，释放通知后观察 300 毫秒无新增报告或通知。它只证明该有界开发回归，不替代 chief 报告或 OMP/Pi 原生确认的跨仓验收。后端共享代码完成检查仍为 `node scripts/test-backend.mjs`，使用本 worktree 构建的静态页面与独立绝对 `CARGO_TARGET_DIR`。

## Chief 入口与单次设置浏览器回归

`frontend/browser-tests/chiefEntry.spec.ts` 经真实打包页面、独占后端进程及生产 Herdr CLI adapter 验证 S1/S2 的 P1–P6（P3 仅本票负责的重复合法发起读回分区）。P1 使用真实位置和中心命中测试；P4 独立断言三个字面默认值并核对提交、外部 Tab。P5 只列 chief Task，改选 Task／zone／模型／思考／名称，读取外部 argv、cwd、名称及公开 Task 定义证明本次覆盖不改预设。P6 在手机视口核对缺默认 zone 时空选择与禁用、公开登记和测试目录不变，再手动选择有效项发起。P2/P3 使用两个浏览器 context，三次以空 input 合法发起／登记／注销，核对真实默认 chief 的空 prompt／无 Skill 可启动、公开结果 `inputSubmitted: false`、页面等待用户输入且外部无空消息或补造指令，以及当前 chief 定位目标和每次公开 Task 的身份、数量不变；不重复认领 07 的并发／时间阈值案例。`backend/tests/chat_creation.rs` 的 `an_empty_trigger_starts_a_ready_chat_without_submitting_a_prompt` 通过公开 HTTP 分别覆盖 OMP/Pi 的普通空指示启动，已有非空正文和 Skill-only 用例继续约束真实提交。

定向反馈：设置本 worktree 独立绝对 `CARGO_TARGET_DIR` 后运行 `npm --prefix frontend run test:browser:build -- chiefEntry.spec.ts --workers=1`。完成检查仍为不带过滤的 `npm --prefix frontend test` 和 `npm --prefix frontend run test:browser:build`。各用例独占 HOME／APPDATA／XDG／原生会话引用、定义、登记、动态回环服务及记录型 Herdr 副本；使用共享清理设施终止并确认所属进程结束后才删测试目录。启动预算 30 秒、页面断言预算 10 秒；不新增产品等待或重试策略，不承担无自动行为时间承诺。

附件 `chief-entry-evidence` 记录源码、锁文件、当前 worktree 二进制与页面哈希、Chrome channel／实际版本、外部调用与诊断；专项另保留默认窗口／缺 zone 截图及公开定义／提交读回。Herdr 响应受控、原生文件是测试独占协议输入，没有实际模型请求，不证明真实 Herdr／OMP／Pi 消费或 GPT-6-luna 角色行为；这些仍由其具名跨仓验收拥有。不得把本专项通过外推为 S3 通过。

## Chief 跨仓入口与生命周期专项

`python scripts/accept-chief-entry.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"` 是 P7/P29/P30 的独立验收入口。从显式 Herdr worktree 根运行，所有路径须为绝对路径，证据目录必须不存在；Python 3.11+。使用本 worktree 构建的页面、生产后端及旁置 `herdr-paths`、`herdr-fixture`，独立绝对 `CARGO_TARGET_DIR`。两仓依赖按各自 frozen lock 安装，Bun 1.4.4。入口不安装宿主、不读用户认证、不回退模型、不重试业务决策。

隔离 paths TOML 使用已有 Rust 路径解析器，声明只读 `herdr`、`omp_package`、`pi_package`、`omp_state_extension`、`pi_state_extension`（后两项是具体官方扩展文件）、`chrome`（含 chrome.exe 的安装目录），以及交付固定扩展所需根。模型配置使用既有真实模型设施的 `provider/model/baseUrl/apiKeyEnv` JSON，model 严格为 `gpt-6-luna`，密钥仅来自显式授权的环境变量；返回 model 身份必须同为 `gpt-6-luna`。也可沿用下文 `NativeCompletionTransport` 与 `workflow-codex-completion.mjs`：仅在用户明确授权某个 auth 文件后，以已安装 Pi 原生 Codex provider 只读消费，不复制、刷新或修改认证文件；回环配置中的 apiKeyEnv 仅是独占传输标记。先独立烟测并检查真实 `serverModel`，再将真实决策送入本入口，由原生 OMP/Pi 执行工具；不证明两宿主直接使用 Codex provider 的 wire 合规性。adapter 通过 Node 的安装包解析定位该 Pi 声明的 pi-ai（含嵌套 npm 布局），使用 Node 22.19.0 或满足宿主要求的更新版本。缺授权、依赖或任一宿主证据均非零，不自动发现用户 auth store，不以受控响应替代真实模型。

P7 使用交付固定扩展清单的实际源码副本及独占 endpoint／model-cost 存储配置（副本中禁用指向正常服务的 Codex proxy），经公开发起和新进程恢复，各自让真实模型查询新发布的独立字面 Bot.when。原生 SessionManager 历史、chief 工具输入/真实结果、模型请求中的角色指引和最终回答共同作为消费证据；恢复前持有 OS 进程句柄确认退出，恢复后要求不同 PID 及相同原生会话身份。没有 Suite，不把配置文件或 argv 当作已加载；角色消费断言检查解码后的实际模型消息中的角色与禁止亲自执行业务边界，不要求只存在于原生历史中的 message marker 被传给 provider。查询没有原生批准操作，因此不伪造确认；只读用户输入和 HTTP/原生历史保留在证据。

`frontend/browser-tests/chiefEntryAcceptance.spec.ts` 独占 P29/P30 的 S1/S2 场景。两个真实 browser context 同时从普通/专用入口发起不同 zone/Task，真实外部 CLI 门闩在登记前保持启动未完成，再叠加公开并发调用；仅一个 202、唯一公开权威和 qualify 胜者。P30 显式配置 10 秒在线、30 秒离线保留，沿用生产 5 秒扫描；真实单调时间观察离线（16 秒预算）和移除（46 秒预算），跨浏览器核对保留期不可另建、心跳恢复、超期可创建及明确注销。登记移除和页面反馈不证明旧进程结束，也不新增产品等待策略。浏览器的外部 Herdr 与原生引用受控，不能充当 P7 原生模型证据。

定向反馈：`npm --prefix frontend run test:browser:build -- chiefEntryAcceptance.spec.ts --workers=1`；CLI 安全回归：设置 `CHIEF_ENTRY_TOOLKIT` 为参与 toolkit worktree 后运行 `python scripts/accept-chief-entry.test.py`。后者只证明缺依赖 fail-closed、拒绝相对路径及不覆盖旧证据，不证明业务通过。完整专项逐项记录原始错误/清理错误、源码与锁文件/安装身份、HTTP、配置阈值与实际观察；退出并确认所属进程结束后才清理。

完成检查仍分别运行本仓 `npm --prefix frontend run build`、`cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`、`node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`；toolkit 顺序运行 `bun run typecheck`、`bun run test`、`bun run check:integrations` 和 `bun test tests/harness-dock/chief-role.test.ts`。配对 dock 的 `HERDR_BOT_BINARY` 必须是本次 `safe_shutdown_shell.exe`、`HERDR_BOT_METADATA` 为本次 worktree 元数据，不能以生产后端执行关机回归。全局回归、单宿主或 CLI fail-closed 通过均不代替双宿主专项成功。

## Chief 权威的进程与失败回归

`cargo test --manifest-path backend/Cargo.toml --test chief_authority -- --nocapture` 经公开 HTTP、真实独占后端进程及生产 CLI adapter 验证 P32/P33。`p32_real_restart_serializes_live_registration_and_resume_before_external_completion` 正常发起两份不同 zone／Task 的 chief 历史、注销、实际终止并重启后端，在外部 Herdr 查询门闩处保持 Resume 未完成，让存活登记与另一恢复调用竞争，核对当前权威与 qualify 的胜负；再次实际重启验证被替代身份不能复权。附带 S2 开发回归以实际默认 10 秒在线／30 秒保留策略和受控时刻验证离线占位、扫描移除及消费者失权，不替代 07 的浏览器 P29/P30 或 20 的原生交接 P31 全验收。

`p33_definite_start_failure_frees_slot_but_uncertain_external_start_survives_restart` 验证明确启动拒绝后能重试；受控 Herdr 已接受启动却返回 readiness timeout 时，保留 `uncertain`，实际重启后仍拒绝第二次外部启动；陈旧注销拒绝，明确解除当前 claim 返回原不确定事实并允许下一次合法发起。这些外部响应只证明协议与故障处理，不证明真实 Herdr／OMP／Pi 加载或 GPT-6-luna 的业务职责；本专项没有真实模型请求。

每例独占原子临时目录、定义、历史、权威存储、HOME／APPDATA／XDG 和数值回环动态端口。后端 stdout 持续消费，stderr 落在该例独占目录；Drop 终止并 wait 所属后端，确认结束再删除临时目录，不接触正常服务或用户 Chat。进程启动预算 30 秒，创建完成 20 秒，外部查询进入 5 秒、门闩 10 秒、释放后 Resume 完成 15 秒；不新增产品等待／重试策略。本专项是 S2 进程与公开操作证据，不以受控 CLI 充当 S3 原生宿主证据。完成检查仍需本 worktree 独立绝对 `CARGO_TARGET_DIR`、本 worktree 构建静态页及 `node scripts/test-backend.mjs`；真实 HTTP／进程检查另按入口要求运行 `npm --prefix frontend run test:browser:build`。

## chief 独立 Task Chat 委派回归

`cargo test --manifest-path backend/Cargo.toml --test chief_delegation -- --nocapture` 经过 S2 公开 HTTP、真实后端进程、独占临时 Bot／Task／zone／历史／委派存储及生产 Herdr CLI adapter，验证明确业务输入与材料地址、完整预览载荷、Task 目标／zone 改绑后的拒绝、旧 chief 失权、启动／投递失败和不确定反馈。外部 prompt 门闩在接受输入前读回真实 sender 关联；实际同 Task 的另一普通 Chat 不获关联；实际终止后端并启动新进程，再由公开 Resume 读回原关联且不重放业务输入。它不创建虚构 Run／node／execution。每个后端进程隔离 HOME／APPDATA／XDG，数值回环动态端口和 Herdr CLI 副本均独占；stdout 持续消费、stderr 保留日志，退出时终止并 wait 所属进程后才删除临时目录，清理失败不覆盖原失败。

门闩仅控制测试拥有的外部 CLI，进入预算 5 秒、阻塞预算 10 秒，释放后创建完成预算 20 秒；生产沿用 Chat 就绪／身份获取预算，具体身份失败不新增命令重试。专项没有真实模型请求，受控 Herdr 只证明协议／故障及持久事实，不代替 12 的 OMP／Pi 原生批准、GPT-6-luna 路由或实际模型业务完成前返回的 S3 证据；报告资格完整负例与跨进程恢复分别由 14／19／20 维护。本票完成检查使用本 worktree 构建静态页面及独立绝对 `CARGO_TARGET_DIR`，运行 `node scripts/test-backend.mjs` 和 `npm --prefix frontend run test:browser:build`。

## Chief Task 双宿主原生验收

`python scripts/accept-chief-task.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"` 拥有 P8、P9(Task)、P10(Task)、P11、P13。Python 3.11+，从显式 Herdr worktree 根运行；路径均绝对、证据目录不存在。复用入口专项的 paths、实际模型配置、Cargo provenance 与 installed-host 检查；不安装宿主、不读用户 Chat、不发现凭据、不回退或重试业务决策。两仓 frozen 依赖须已存在，toolkit 的既有隔离 OMP 18.5.0 dock 安装也须已准备好。

独占 Herdr 使用 `headless_cols=200`、`headless_rows=220`，使完整事实与 Yes/No 同时可见；这只改变测试拥有的虚拟终端尺寸，不改变产品 20 秒确认预算。终轮按原生 assistant `stopReason=stop` 观察，再独立断言业务行为，不把模型复述测试标记当作验收条件。含糊需求可由真实原生 Ask 表单或普通回答澄清：先保留实际目标／要求问题，再由用户 Esc 放弃尚未知晓字段的表单并转向另一需求；不代填未知业务信息，也不把等待表单误判为模型未响应。后续用户输入先以 10 秒预算观察 chief 原生 ready，再只提交一次并用 `prompt --wait --until working --until blocked --timeout 10000` 证明本次活动；Herdr 写入成功不是模型已接纳，stalled 不补发或补 Enter，失败前保留屏幕。一次方向键只发送一次，再以 5 秒观察预算等实际选项重绘，不把写入按键成功当作已选中；不延长产品确认时限。启动结果按句检查是否存在未经否定的业务完成声明，保留“不表示业务已完成”等正确边界表述。真实 GPT-6-luna 自行决定工具：独立字面 Bot.when 与名称误导候选区分按适用范围选择；含糊、无匹配、已知匹配三轮均保留原生历史及工具结果。先提出初稿，用户修正 zone 与内部审计/禁止发布要求，再经实际原生 UI 核对最终完整预览。拒绝、Esc 取消、真实 20 秒超时、批准期间 Bot 改目标及 zone 改绑分别观察没有委派/新 Tab；每次工具结束后的 61 秒窗口覆盖 20 秒确认＋30 秒 HTTP 预算、10 秒在线＋30 秒保留＋5 秒维护扫描。用户明确请求新操作后才再预览，不自动重试。批准案例核对单次 preview ID 提交、公开委派事实、原生独立 Chat/cwd/完整输入、真实 Task 来源及无新增 Run；执行者未收到 RELEASE，chief 已返回启动结果并处理下一输入。业务哨兵逐阶段保持不变。

不可用确认复用 `tests/harness-dock/chief-delegation.test.ts` 的两个既有 Task headless case，明确标注为受控 provider 的原生协议证据，不重复认领模型职责；设置 `WORKFLOW_PROMPT_EVIDENCE_DIR` 保留原生输出和历史。CLI 安全检查：`CHIEF_TASK_TOOLKIT="$TOOLKIT_WT" python scripts/accept-chief-task.test.py`，仅证明缺依赖/错误模型 fail-closed、绝对路径与旧证据保护，不算专项业务通过。

明确授权某个 auth 文件时，可用现有 `scripts/accept-workflow-codex.py --scenario chief-task --hosts omp,pi --backend "$HERDR_BOT_BINARY" --toolkit "$TOOLKIT_WT" --paths-file "$ISOLATED_PATHS" --pi-package "$PI_PACKAGE" --auth-file "$AUTHORIZED_AUTH_FILE" --evidence-dir "$NEW_EVIDENCE_DIR" --node "$NODE" --bun "$BUN"`。它只读指定凭据，不刷新/复制/写入证据；实际返回 `serverModel` 必须为 `gpt-6-luna`。独立 `--smoke` 先核对身份。传输保留真实模型决策与原生工具，不证明宿主直接使用 Codex wire。

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

从显式 Herdr worktree 根运行 `python scripts/accept-chief-map.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。所有路径必须绝对，证据目录必须不存在。模型配置、只读授权 Codex adapter／`NativeCompletionTransport` 和返回身份要求沿用上文入口专项；只允许 GPT-6-luna，不自动发现认证或重试模型决策。后端旁置当前 worktree 的 `herdr-paths`，页面与后端均须由参与源码构建。

`chief_map_native_acceptance.py` 独占 P9 的 Map 修正、P10 的 Map 原生确认、P12 的 manager／node 位置与内部管理关系、P14/P15 的完整创建能力选择与否决后授权、P16 的创建阶段批准和结果后无新业务。每个宿主／场景独占 Herdr、HOME／APPDATA／XDG、定义、存储、动态回环端口和原生会话；复用进程句柄退出确认与原始错误／清理错误分别保留。创建目标使用交付的完整 Task＋Bot／Map 创建定义，保留节点 Task 的真实 Suite／Skill／宿主和原有审批，只将物理位置、模型传输和存储移至独占根；原生 MCP 必须支持既有隔离 selectors，不能接入账户 daemon。

测试独占部署还加载 `scripts/chief-ui-observer.mjs` 的副本：仅透传观察安装宿主公开 `ui.confirm` 的原始参数、原始返回／异常与真实 abort 时刻，不选择工具、不修改确认内容或结果、不代用户批准。每条观察原子发布；专项把实际原生消息内的完整 JSON 与 HTTP 预览及提交载荷分别比较，观察缺失不能通过。观察器按公开 UI 自有方法 descriptor 保留真实函数，避免 OMP scoped Proxy 的转发闭包递归；`node --test scripts/chief-ui-observer.test.mjs` 只证明透传机制，不代替原生验收。专项通过公开 `terminal session control --cols 2048 --rows 2000` 调整本例独占 terminal，让完整 Map 原生确认与 Yes/No 按钮可见，不裁剪实际载荷。原生键选择和真实历史仍独立保留。

P15 的四种输入均先取得真实匹配复用建议，再分别仅否决、明确创建 Task、明确创建 Map、放弃；不能用本来没有复用候选的场景冒充否决边界。P16 分别保留实际阶段候选的拒绝／受阻返回，以及真实成功创建后无新业务的场景。成功案例只走所选既有创建流程需要的真实逐阶段正文／保存批准、独立原生报告批准和交接；核对实际新 Task＋完整 Bot 或多节点 Map，不追加创建产品的完整质量验收。创建观察预算为 `20 × --timeout` 秒（默认 3600 秒），与生产等待策略无关；拒绝／受阻不能冲抵成功创建后的授权边界。

原生确认预算 20 秒、请求预算 30 秒；拒绝／取消／超时／陈旧批准及创建结果后观察 46 秒，超过实际 10 秒在线＋30 秒保留＋5 秒维护上界。实际生产预算不改写为测试重试策略。`tests/harness-dock/chief-map-unavailable.test.ts` 是明确标注的受控 provider／peer、真实安装宿主无 UI 机械故障用例，不证明真实模型业务选择；其原生 2 秒预算使用真实进程时钟。其它业务场景须有 GPT-6-luna 返回身份、实际历史、完整工具／HTTP 及公开 Run 事实，不能用 dock 或全局套件结果冲抵。

`CHIEF_MAP_TOOLKIT="$TOOLKIT_WT" python scripts/accept-chief-map.test.py` 仅验证 CLI 的缺依赖／错误模型 fail-closed、拒绝相对路径与不覆盖旧证据。过滤单宿主不能通过整体双宿主覆盖。两仓仍分别完成票据要求的完整构建／后端／前端／浏览器及 Bun typecheck／test／check:integrations，配对 dock 必须使用本次安全关机壳与 worktree 元数据，不能用生产后端运行关机回归。

## Chief 报告持久化与资格回归

`cargo test --manifest-path backend/Cargo.toml --test chief_delegation reports:: -- --nocapture` 通过 S2 HTTP 提交／列表／单份读取、真实独占后端进程和生产 Herdr CLI adapter 验证 P17／P21。报告夹具复用真实 Task Chat／Map Run 的公开委派入口；合法独立 Chat、直接 manager 与无关联身份、同 Task 另一实际 Chat、普通 node 和伪造归属／收件人参数作对比，不通过内部 helper 授权。外部 prompt 门闩进入后先从公开接口读取报告，确认保存先于通知；无 chief 与明确通知失败后均实际终止并重启后端，核对稳定 ID 和完整内容。真实只读 journal 制造保存失败，保存失败无外部通知；接受输入后返回无效回执的 CLI 验证 `uncertain`，不冒充未提交。

所有定义、原生引用、报告 journal、HOME／APPDATA／XDG、动态数值回环端口及 Herdr 副本由当前用例独占。后端 stdout 持续消费、stderr 落在独占目录；Drop 终止并 wait 所属进程，确认退出后才删除临时目录，清理错误不覆盖原始错误。启动预算 30 秒，Chat 完成预算 20 秒，外部门闩进入 5 秒、阻塞 10 秒；实际生产在线 10 秒＋保留 30 秒、维护扫描 5 秒。失败后实际重启并重新登记 chief，再观察 46 秒（超过完整 45 秒维护上界），无新增通知或报告；该有界初始化回归不替代 19 的全部恢复／人工重发矩阵。受控 CLI 仅证明协议／故障，无真实模型请求，不认领 S3／GPT-6-luna 职责证据或 16／19 的独占验收。

完成检查使用当前 worktree 构建的静态页面和独立绝对 `CARGO_TARGET_DIR`，执行 `node scripts/test-backend.mjs`；真实 HTTP／进程检查另运行 `npm --prefix frontend run test:browser:build`。报告持久化仍在后端 `files` authority；chief 和 node 共享 `report_delivery` 的单次保存后通知，但各自持有身份、正文和收件策略。`submitted`／`failed`／`uncertain`／`not-attempted` 是通知事实，不是可靠接收、已呈现或用户验收。

## Chief 持久接收与人工恢复回归

`cargo test --manifest-path backend/Cargo.toml --test chief_delegation receipts:: -- --nocapture` 经批准的 S2 HTTP seam、实际独占后端进程及生产 Herdr CLI adapter 覆盖 17 的开发回归：并发 `receive` 只有一个 `process:true`、接收记录实际重启后读回；人工重发不改变报告 ID／正文、不另建报告，通知只到替换后的当前 chief；重复／并发重发已接收报告不再通知；丢失接收响应并在呈现前终止后端后不重播，当前 chief 可明确 `view` 原报告；只读接收 journal 的真实保存失败不授予处理；外部 inventory 门闩期间接收／撤销会使旧通知授权失效，而已进入外部输入提交的效果不被虚构撤回；独立执行者与直接 Map manager 实际 Resume 保留原报告／委派关联且没有新增 Run 或初始输入重放，同 Task 的另一真实 Chat 仍被拒绝。重复认领临时变异为 `process:true` 时并发用例必须在“两个处理胜者”断言失败，正确实现恢复后确认通过。

每例复用本文件前述独占定义／历史／journal／HOME／APPDATA／XDG、数值回环动态端口、持续消费日志和终止后 wait 所属进程规则；不访问用户 Chat 或凭据。后端启动预算 30 秒、Chat 完成 20 秒、门闩进入／接收事实观察 5 秒、外部阻塞 10 秒、释放后操作完成 15 秒。接收后崩溃专项在实际后端重启／重新登记后观察 46 秒，超过实际 10 秒在线＋30 秒保留＋5 秒扫描完整上界，核对没有新通知／业务报告；没有增加产品等待／重试策略。真实 TCP caller 只在公开 receipt 确认提交后丢弃未读取的响应，避免把尚未被 HTTP 接受的请求取消当成回执丢失。受控 CLI 仅证明传输、持久化、竞态和故障，不证明原生模型已呈现或用户已读；没有实际模型调用，不替代 19 的双宿主恢复矩阵或 20 的原生交接验收。

完成检查仍使用本 worktree 构建的静态页面、独立绝对 `CARGO_TARGET_DIR`，运行 `node scripts/test-backend.mjs`；真实 HTTP／进程检查另执行 `npm --prefix frontend run test:browser:build`。新增公开 API 的精确请求／响应和接收取舍见[使用说明](../usage.md#持久接收人工重发与查看)。

## Chief 双宿主报告专项

从参与的 Herdr worktree 运行 `python scripts/accept-chief-reports.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。全部依赖使用显式绝对路径；证据目录必须不存在。先 frozen-lock 安装两仓依赖，以独立绝对 `CARGO_TARGET_DIR` 构建当前页面、生产后端、`herdr-fixture`、`herdr-paths` 与 `safe_shutdown_shell`。Bun 为 1.4.4；已有 baseline 依赖可用 `OMP_BASELINE_RECORD_ROOT` 定位，Pi 可显式设置 `PI_CLI`／`PI_NODE`。入口不安装宿主、不发现用户认证、不回退模型或重试业务决策。经单独授权的只读 Codex 传输可复用 `accept-workflow-codex.py --scenario reports`，沿用其显式 `--auth-file` 安全边界；先 `--smoke` 核对实际 `serverModel=gpt-6-luna`，不得把回环传输称为两宿主直接 Codex wire 合规。

P18 在当前 chief 存在时分别运行普通／实际 chief 委派的嵌套 Map，真实双角色工具继续向父 manager 报告，内部报告继续向自己的 manager 报告。P20 引用并实际执行既有 `native-chief-report` 的双宿主批准／拒绝／取消／超时／不可用及完整预览用例。新增 `chief-report-backend-faults` 用真实隔离 journal 目的地目录造成保存失败、受控外部 Herdr 明确拒绝，以及代理在实际后端响应后断开 TCP，核对原生结果及公开不可变事实；不是伪造后端成功响应。P36 执行既有 node 原生确认与公开报告门禁，双宿主真实通知拒绝后保存不撤销，观察46秒不新增报告／通知。P37 执行现有集成声明和 import-boundary 检查，并保留源码／锁文件／产物／安装宿主身份；源码审查仍需确认后端 files/report_delivery 唯一持久权威、扩展 common 共享确认与请求，不新增接线快照测试。

`chief_reports_native_acceptance.py` 的 P19/P28 使用真实 GPT-6-luna：独占子进程成功产物、退出7的部分产物、缺输入受阻及公开保存读回的能力定义作为独立业务证据，发送者实际读取并选择原生报告工具，驱动只提供用户输入／原生批准。批准期间替换为无原始对话的新 chief，逐字核对预览、实际请求、保存及读取事实；另对实际发送者进行新进程恢复后提交新的失败结果（不是重发原报告）。真实模型按预先固定的背景／执行对象、结果／限制、交付物／判断项、仅询问满足要求并等待、各类交付事实区分五项 rubric 评价真实呈现；背景与执行对象还必须提供实际 chief 答复中的非空逐字引文，驱动核对引文确实存在，不能引用原报告冒充已呈现；引文由评价器从原始 Markdown 字符串抽取，必须保持连续原文及其中格式符、标点和空白，不能以渲染后的去格式文本冒充逐字引文；程序仍逐字检查子串，不要求 chief 以引文格式回答。独立事实还保留实际产物读回；chief 准确转述权威报告即可，不要求重新执行业务或独立证明结果。缺项、虚构引文或混合结果失败，不重试。每种结果停轮后观察46秒，核对没有新增业务对话消息／工具、报告或 Chat；OMP 自身的标题／model-usage 元数据不是下一项业务。报告专项仅在独占 OMP `agent/config.yml` 中设置既有 `recap.enabled: false`，避免与报告无关的 idle recap 模型请求在清理时被进程退出取消；不改变业务确认、静默窗口或正常安装配置。窗口覆盖45秒隔离 Herdr／后端就绪观察、30秒原生启动／请求、20秒确认、10秒在线＋30秒保留＋5秒扫描预算，不以即时断言或新产品等待策略证明无自动工作。

所有原生历史、确认、工具结果、HTTP、公开读回、模型请求及实际身份、配置预算、原始失败与清理失败均归本次独占证据。HOME／APPDATA／XDG、原生目录、定义、持久化、数值回环端口和 Herdr 实例隔离；退出并确认所属进程结束后才清理。受控用例仅证明机制／故障；缺任何宿主、真实模型、能力或清理证据均非零，不能把 CLI fail-closed 或全局回归当作专项通过。完成检查仍分别运行票据要求的两仓全局入口；配对 dock 必须指向本次安全壳，真实专项 `--backend` 指向生产后端。

## Chief 双宿主人工恢复专项

从参与的 Herdr worktree 根运行 `python scripts/accept-chief-recovery.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。路径必须绝对，证据目录必须不存在；依赖、页面、生产后端与 Cargo depfile 均核对参与 worktree。另以同一独立绝对 `CARGO_TARGET_DIR` 构建 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell --example recovery_herdr_proxy`。后者是验收专用外部 CLI 记录／故障代理，正常时透传到显式安装的 Herdr，仅由独占标记文件制造通知明确拒绝；不进入产品或正常部署。

`chief_recovery_native_acceptance.py` 独占 P23/P24/P25/P26/P27/P34/P35：分别观察通知失败、chief 创建／新进程恢复／重新登记、真实后端重启和时间流逝；真实 GPT-6-luna 在原发送方明确请求后选择原报告重发，完整原生预览、再次批准／拒绝和原 ID／正文逐项核对。故障代理只在实际后端提交之后持有／丢弃原响应，不伪造保存或接收成功；持久接收已可公开读回后、模型 admission 前终止所属原生进程。新 chief 不迁移旧上下文、不自动重播；用户明确查看才实际读取和呈现。重复／并发重发、丢失回执、更换 chief 和真实后端重启后核对原接收记录、最多一次处理授权及无新通知。独立 Chat 与直接 Map manager 分别新进程恢复原关联，恢复前后核对历史／模型请求，观察无初始输入重放、新委派或 Run；同 Task 的另一实际 Chat 恢复后仍无资格。改内容的原 ID 重发须拒绝，新内容的独立原生确认被拒绝时不保存／发送，旧记录保持原样。

每个新 chief 先接收一次无业务内容的用户就绪请求，等待原生历史实际持久化后才开始生命周期观察；新会话仍不包含旧报告 ID、正文或原委派上下文。无自动行为观察通过公开历史枚举实际 Session，逐个以 `GET /api/chief/delegations?sessionId=...` 读取关联，同时比较 Chat 身份集合与 Run；不使用不存在的无身份全局委派列表。原生 OMP 的字符串 `i` 仅作为宿主 intent 元数据记录，重发业务参数仍严格只含原 `id`，实际 HTTP 另行严格核对；已终止进程的 unregister 允许明确 `404 unknown-session`（后端重启后的内存登记可已不存在），其它错误仍失败。原报告预览按完整 JSON 对象比较，不把键顺序当内容差异；恢复前后和静默窗口的无业务模型调用观察使用同一计数边界，仅排除安装 OMP 无工具的独立标题／标题卡请求（其系统指引分别以 `You name coding-agent sessions.`／`You head coding-agent session titles with a small card.` 开始）；带业务工具、普通系统指引或 Pi 请求仍全部计入，仍保存并检查所有请求的实际模型身份；正常终止所属原生进程前，以既有30秒请求预算等待已在途请求完成，避免测试自身取消独立标题请求，不重试或忽略失败，所有用户／assistant／tool 业务消息必须保持不变。每个无自动行为窗口实际观察46秒，覆盖45秒隔离服务启动、30秒原生启动／请求、20秒确认，以及10秒在线＋30秒离线保留＋5秒维护完整上界；这些是测试观察预算，不是产品等待／重试策略。所有原生历史、确认原始参数／决定、HTTP、外部 CLI 调用、真实模型返回身份、源码／锁文件／产物／安装宿主身份及原始错误／清理错误保留在本次独占证据。HOME／APPDATA／XDG、定义、存储、原生会话、数值回环端口和 Herdr 均独占；持有进程句柄确认结束后才交还资源，删除目录不能替代退出。

模型配置沿用上文公开真实模型传输。用户单独授权的只读 Codex provider 可使用 `python scripts/accept-workflow-codex.py --scenario recovery --hosts omp,pi ... --auth-file "$AUTHORIZED_AUTH"`，先 `--smoke` 核对实际 `serverModel=gpt-6-luna`；不发现、复制、改写或自动刷新用户认证，不回退或重试业务决策。它是实际工具决策的传输适配，不是两宿主直接 Codex wire 合规证据。入口实际执行依赖拥有的四份原生协议 dock，但受控 provider／HTTP／Herdr 只证明机制，不能冲抵本专项真实模型及双宿主矩阵。单宿主诊断、缺任一依赖／能力／证据或清理失败均不能整体通过。

可用两个独占证据目录分别运行 `--hosts omp` 与 `--hosts pi` 并行诊断；每次仍保持单宿主内的重启／接收因果顺序，单宿主入口退出非零且不能称整体通过。聚合必须核对两份宿主 `evidence.json` 全部七项 verified、各自 passed/cleanupErrors、实际模型身份与机械检查退出状态及同一源码／产物身份；不能仅看过滤后的进程退出码。`business-stderr.log` 按时间、host、阶段 started/completed 与已验证 coverage 实时 flush，最终 JSON 仍独立输出；宿主根证据记录准确失败。外部 CLI 记录在 `external-cli/calls/*.json` 以独占临时文件写完后 rename 发布，避免并发进程交错 JSONL 或读到半条记录。

CLI 安全与故障代理定向回归：`CHIEF_RECOVERY_TOOLKIT="$TOOLKIT_WT" python scripts/accept-chief-recovery.test.py`、`python scripts/chief_recovery_faults.test.py`；前者只证明 fail-closed／绝对路径／不覆盖旧证据，后者只证明原响应提交后的门闩和丢失行为。另设置 `CHIEF_RECOVERY_PROXY` 为本次 `recovery_herdr_proxy.exe`、`HERDR_FIXTURE` 为本次 `herdr-fixture.exe`，运行 `python scripts/chief_recovery_cli.test.py`，检查初始业务输入逐字透传、只拒绝 chief 报告通知、解除显式故障后原通知逐字透传。完成仍分别运行两仓要求的完整构建、后端／前端／浏览器及 Bun 1.4.4 typecheck／test／check:integrations；配对 dock 只使用本次安全关机壳与本次元数据，生产 `--backend` 不替代安全壳执行关机回归。全局套件不替代专项。
## Chief 存活旧进程与交接专项

从参与的 Herdr worktree 根运行 `python scripts/accept-chief-authority.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。显式绝对路径、独占新证据目录、当前源码构建产物及隔离 roots 的要求与入口专项相同。模型严格使用 GPT-6-luna 并核对实际返回身份；授权只读 Codex provider 可沿用 `accept-workflow-codex.py --scenario authority`（先单独 `--smoke`），不发现用户凭据、不刷新认证、不回退或重试业务决策。

`chief_authority_native_acceptance.py` 独占 P31：两宿主分别保留旧 chief 真实进程与持有的 Windows 句柄；旧原生进程实际收到已提交报告通知并发起 `/receive` 时，在透明 HTTP 传输边界将该真实请求暂挂（最多18秒，短于原生20秒 admission预算），移除原登记并发起新 chief 后释放。真实后端必须拒绝旧原生接收，公开 receipt 仍为空；已提交的外部输入仍保留 `notification.status=submitted`，不能解释成已撤回或可靠接收。之后显式要求原发送者报告另一份新的独立结果，真实新 chief 必须获得持久 receipt 并呈现。此场景不重发同ID，不重复认领恢复专项的去重矩阵。

旧进程迟到登记、Task／Map 的旧预览提交及实际原生派发均须拒绝；当前权威始终属于新 chief。新 chief 再分别经实际原生完整预览与批准执行有效 Task／Map 派发，公开读回真实 Chat／根 Run；只读 Map 夹具显式使用 `report_policy: manual`，允许 manager 按正常流程管理节点并保留阶段审批，但不自动向 chief 报告；不把“禁止全部节点执行”与执行 Map 的有效请求混用。UI 观察复用只透传的 `chief-ui-observer.mjs`，不替模型选择工具或伪造结果；观察目录和环境变量须在独占 Herdr 服务启动前设置，确保其原生子进程实际继承。证据保留竞争单调时序、旧进程持续存活、原生历史、完整确认输入/结果、HTTP、来源/锁文件/产物/安装宿主身份及报告/接收事实。

静默观察为46秒，覆盖实际10秒在线＋30秒保留＋5秒维护上界；配置、启动/请求/确认预算与原始失败、清理失败分别记录。所有 HOME／APPDATA／XDG、定义、持久化、动态数值回环端口、Herdr 和原生会话均独占；结束时关闭所属服务并以持有进程句柄确认退出，不以删目录代替。任何宿主/能力/模型/清理缺证据均非零，单宿主或 `CHIEF_AUTHORITY_TOOLKIT="$TOOLKIT_WT" python scripts/accept-chief-authority.test.py` 的CLI安全检查不算P31通过。

两仓完成检查仍分别执行票据要求的完整 frontend build、当前后端/fixture/paths/安全壳构建、`node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`，以及Bun 1.4.4的 `bun run typecheck`、`bun run test`、`bun run check:integrations` 和受影响 `native-chief-receipt-backend`／`native-chief-reception` dock。独立绝对 `CARGO_TARGET_DIR`；dock 的 `HERDR_BOT_BINARY` 只能指向本次 `safe_shutdown_shell.exe`、元数据指向参与源码，真实专项则使用生产后端。完整回归或受控dock仅证明其实际范围，不替代双宿主真实模型专项。

## 前端与浏览器

通过 npm 脚本运行，保留网络护栏；直接调用 Vitest／Playwright 会绕过 npm hook。脚本定义以 [package.json](../../frontend/package.json) 为准。

- Vitest 的环境和并发见 [vitest.config.ts](../../frontend/vitest.config.ts)。组件使用 jsdom，纯逻辑测试可使用 Node；保持测试文件 VM 隔离，调整 worker 数而非关闭隔离。
- Suite 技能说明的页面断言须区分最终技能白名单与原生目录发现：排除未选中技能不等于不扫描默认目录；空清单仍表示禁用技能。
- 浏览器调度见 [playwright.config.ts](../../frontend/playwright.config.ts)。文件内并行场景保持每个用例独占服务、可变存储和 Herdr 副本；浏览器由 Playwright 按 worker 复用。
- 资源受限或诊断并发问题时，给浏览器入口追加 `-- --workers=1`；Vitest 可追加 `-- --maxWorkers=4` 等覆盖参数。
- 用 `elementFromPoint` 核对 SVG 连线等真实绘制时，先将目标滚入视口；`toBeVisible` 不保证元素位于视口内，不能直接作为命中测试的前提。
- scheduler 的目录回归保留 `groups` 响应与分组标题；空过滤核对条目而非全部按钮（清空过滤仍可用）。Map 保存／删除检查保留标题、说明、节点集合与公开读回／落盘一致性，不固定空 `input_requirements` 是否序列化；最后节点删除须单独断言节点集合为空。前置节点按导航可访问名称断言，不固定装饰分隔符；窄屏从预览返回目录后再选择下一项。
- Chat 预览应在重新选择同一 Task 时保留已生成身份；模型／思考级别失败夹具须使实际提交的单次选择失效，不能仅损坏已被合法覆盖的 Task 默认值。当前／历史 Chat 的切换同时保留可见状态与不同背景的浏览器断言。
- 工作流状态浏览器夹具中的跨节点报告须同时使用所属节点的真实 execution ID 与 Chat 来源，不仅改 `nodeId`；否则违反报告归属校验，不能证明输出隔离。若专门检查依赖阻塞／可派发状态，须显式声明已保存的 `inputsReady = true`；未准备输入的真实 Run 仍应显示准备中。节点报告策略回归分别核对自动报告与原生逐次确认，不恢复已移除的自然语言同意替代门禁。
- 常驻但隐藏的 Run 列表不读取 Map 分组目录；仅在列表可见时刷新，保留筛选状态。全局前端回归中的关机／恢复用例同时约束默认 Chat 页不发出这类无关读取；应用品牌保留可访问标题语义。
- 完整浏览器入口不加快速过滤。分层开发反馈使用 `test:browser:fast`／`test:browser:time-contract`；修改选择或标签后运行 `npm --prefix frontend run test:browser:partition`，核对完整集合是两分区的不相交并集。
- `@time-contract` 标记真实预算、确认超时、迟到结果及无自主刷新／重试／重放的观察窗口，不按文件名或断言 timeout 分类。新增真实时间承诺时同步审查标签。
- OMP 历史身份超时验收检查创建关联、agent 类型及 `identity-timeout` 类别，并验证创建成功、页面不泄露内部诊断、身份恢复后不重读／补写历史及初始化仅提交一次；诊断原因的自然语言措辞不作为契约。
- Chat 的纯临时输入场景（空固定 prompt、无 Skill、无词典引用）应独立核对完整投递文本与输入逐字相等，不假定末尾空段或无条件追加 agent 指引。

## 修改测试或夹具

- **所有权**：每个用例独占可变临时数据与动态回环端口；同一竞争／重启场景保留必要共享。临时夹具目录须原子创建并跳过已有目录，不能因 Windows 重用 PID 而接管旧运行残留的数据。浏览器夹具探测端口后到子进程绑定前仍存在竞争；仅动态端口的已确认地址占用错误可重新分配（最多 32 次），固定地址及其他启动失败直接报错，不重试业务操作。自动发现目标不等于自动消除资源冲突。
- **路径**：真实子进程使用测试拥有的临时目录。普通相对路径按声明配置文件的父目录解析；固定扩展入口相对元数据文件，Suite 扩展入口相对 Suite 文件解析；cwd 可以与配置目录不同。需要根引用时生成隔离根文件，不读取开发者的 `paths.local.toml`。启动期 `data_directory` 与迁移标记也要归测试所有。
- **环境**：`HERDR_FIXTURE_DIR`、`HERDR_BOT_FIXTURE_RECORD_DIR` 会覆盖默认夹具位置；继承环境时保证它们不把不同用例导向同一可变目录。
- **进程**：取得真实子进程端口后持续消费 stdout／stderr，直到进程退出。初始化失败与正常结束都执行所属资源清理；同时保留原始错误和清理错误。
- **并发历史持久化诊断**：跨来源并发 register 验收的 `historySaved` 断言保留完整回执及所属后端 diagnostics，区分事件接受与历史保存失败；后续通过不能抹去先前失败，也不能据此宣称已修复无法定位的间歇故障。`crossSourceConcurrency.spec.ts` 的独立客户端并发登记与进程重启同时回归原子替换期间的文件 scope 身份：协调资格取得前的 canonicalize 观察可能已过期，必须在持有资格后重核，不能把旧临时文件身份当作最终文件的授权边界。
- **Git 超时夹具**：`zone_sync`／`zone_commit` 的派生进程终止场景使用 5 秒命令预算和 30 秒阻塞的 pre-push／pre-commit hook（父 shell 等待唯一后台子任务，避免两个 sleep 同时自然退出与 taskkill 竞争；超时后观察 31 秒确认无迟到 marker），给 Windows 本地 fetch／pull 留出正常执行时间；仍须核对 push 超时、已完成 pull 不回滚、派生进程终止及后续同步恢复，不能将更早的 fetch 超时视为该场景通过。
- **Windows 进程身份**：结束后的 PID 可能被系统复用，不能以迟到的 `tasklist` 命中证明原进程仍存活。`next_operation_after_finish` 的连续启动回归先在测试独占夹具门闩中取得原进程句柄，再释放门闩，收到 ended 响应后直接核对该句柄已 signaled；不移除进程结束断言。
- **Windows 清理**：原生可见性 Chrome 夹具在浏览器操作前持有原进程的 Windows handle，并在清理时以该 handle 的退出确认区分 Node 延迟事件与仍存活的进程；不以迟到 PID 查询或 `taskkill` 部分子进程错误推断原进程存活。`taskkill` 完成后再开始服务退出事件的有界确认窗口，然后解释 `taskkill` 失败状态，避免终止命令在全局负载下耗时较长时，把已退出但尚未被观察到的进程误报为存活；确认窗口结束仍存活时报告错误。临时目录删除不代替进程结束。
- **目录初始化**：Map 与 Bot 目录可能由并发请求创建共同的缺失父目录；浏览器目录验收同时请求两个 catalog，确保创建竞争不产生 500，同时保留文件阻塞与链接拒绝检查。
- **Map 文件接口**：创建 Bot／修订指南的发布回归固定核对 `input_files` 的数量、顺序、ID 与 required，不以整个对象相等禁止合法的材料 `description`；实际保存仍传递完整声明，保留文件预检行为断言。
- **元数据**：修改 `meta-data/task-options.toml` 的显示标签时，同步检查摘要的独立预期值及动态元数据测试的替换样例，具体标签从元数据读取，不在本指引复制。交付 chief 预设的模型变更须同步更新 `backend/tests/tasks.rs` 的独立字面预期；预设重启回归验证当前交付定义，不等同于要求固定模型的原生专项验收。
- **Bot 保存与导航**：保存后的 Map／目录刷新仍属于本次维护操作；刷新结束后才发布退出编辑的成功预览，避免用户看到成功后立即切换页签却被尚在途的操作静默拒绝。`schedulerBotEditing.spec.ts` 的共享能力回归保留保存后立即切换和原节点选中态检查。
- **关机与会话**：关机自动验收使用记录型安全壳或替身；真实关机单独人工验收。会话探针使用隔离数据，不操作用户正在运行的对话。

## Suite 配置与 Task API 格式

Suite 定义管理与 Task 引用完整性的隔离验收入口见[使用说明](../usage.md#task-单-suite-选择与引用保护)。配置通过不代表原生宿主已发现或消费资源，实际加载仍归下列宿主专项验收。历史离线迁移工具及专属测试已归档，不再随仓库提供；正常启动不会自动迁移旧格式。测试和发布工单均不授权修改用户部署数据。

Task API 的旧 `extensions` 字段（包括空数组）必须拒绝；固定扩展在 Suite 页全局只读展示名称、解析后的入口及执行角色，Task 编辑页不再显示该列表。未选择 Suite 仍加载固定扩展；两宿主 auto connect-bot 属于固定扩展，不能再作为 Suite 可选引用。资源装配并发／故障夹具及真实 Pi 模式／MCP runner 的 Task 请求同样不发送该旧字段；Suite 扩展只使用 `extensions: {pi: [路径]}`（OMP 同理），相对 Suite 文件解析；不再提供可选目录或 ID 解析，旧 `extensionOptions` 字段应拒绝。每次启动／新进程恢复使用当前 Suite 路径；新增入口无需修改 TOML。固定入口仍按元数据及角色规则加载。Pi 必须指向具体文件；OMP 仍可传原生支持的文件或目录；相同目标仅传一次。浏览器的精确 Pi argv 预期同时保留 `builtin:mcp`、`builtin:tool-search`、无 Suite 时的 `builtin:codemode` 以及元数据声明的固定 `pi-suite`；过滤恢复仍逐项断言文件扩展，不把内建扩展误当作失效文件。Suite `extensions` 只接受宿主键对象，不接受旧数组；现有浏览器 Suite 管理、Task 引用、三步编辑和窄视口检查使用相同新形状，原先只测试旧 Task 可选扩展勾选的场景已退役，不保留旧业务选择兼容分支。

## 首批 Suite 原生发现与实现命令

`python scripts/accept-initial-suites.py` 拥有 P11 的探索 OMP 集合目录／探索 Pi 逐技能／实现 OMP 集合目录、P42 实际 `hyper-auto-coding` 注册及无任务调用，以及 P44 两仓完整交付清单和原生公开入口一致性。独立字面名称与来源清单不从装配器重算；真实 `pi.getCommands()` 包含手动调用的翻译技能，不能用模型提示中的较窄集合替代。入口、隔离依赖根、明确 Git 基线、构建命令、进程清理和证据含义见[首批 Suite 原生验收](../initial-suite-native-acceptance.md)。`python scripts/accept-initial-suites.test.py` 检查 S3 外来后端拒绝，以及原生验收独立来源 oracle 对 Wayfinder 自有目录和场景副本的直接子技能发现、动态新增与场景隔离；不固定配置纯文字快照，也不等于 S4 通过。两个目标 OMP Suite 只声明 Wayfinder 自有技能目录及各自 `dependent-skills/<suite-id>`；探索 Pi 的九个外部技能逐项引用探索副本，保留原 18 个名称。原生验收不再需要原 Matt Pocock／翻译仓库根。缺宿主能力或实际发现均保留非零和未验证；不安装、不提权、不读取用户凭据，不重跑其他票的资源/MCP/生命周期矩阵来冒充本票成功。

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

`backend/tests/suite_assembly.rs` 拥有 P34/P36 和 P35 的装配发布部分，入口仅为既有公开 Task／Chat HTTP 与独占部署目录（S2/S3），不增加测试专用产品 API。常规 `cargo test --manifest-path backend/Cargo.toml --test suite_assembly` 检查资源消失、Windows 独占读锁造成的真实不可读失败、未拥有文件保留，以及同 Suite／不同 Suite 的两套独立 LSP 与 MCP 配置哨兵；核对 MCP 原生资源包与技能配置、LSP 共属本次装配树，技能源路径保持在原位置，返回的 `--extension` 指向该包。外部 Herdr 夹具在启动效果处受控阻塞；第二个启动必须仍能取得完整产物，随后重读两套最终清单、释放门闩并等待公开完成状态，确保文件协调不跨外部效果。观察失败也先释放门闩并等待已接受的所属命令，原始错误与清理错误分开保留。这些夹具不是原生 agent 消费证据；P35 并发真实启动消费由独立专项负责。

非空技能的原路径成功发布属于默认回归，不依赖 `HERDR_BOT_MANUAL_SYMLINKS`、第二盘或 Windows 链接权限，也不能标为 ignored 后宣称完整通过。验证同 Suite／不同 Suite 并发启动中的独立源技能正文、相对资产和配置，失败只清理本次拥有的配置产物，保留源资源、旧产物及未拥有文件。

受控启动夹具不证明原生 agent 消费；完整交付仍需下述真实 OMP／Pi 场景，并记录实际已安装宿主及 OS 进程身份。OMP 使用现有全局安装，不部署新宿主构建；原生最终技能列表与真实消费不可由配置文件存在推断。完整后端回归使用 `node scripts/test-backend.mjs`。

## OMP Suite 技能真实消费验收

`python scripts/test-real-herdr-suite-skills.py` 专项覆盖 P5/P6/P7/P8/P10/P15 的 OMP 技能场景，必须先构建当前 worktree 的 herdr-bot 后端。它经公开 Task／Chat HTTP → 真实 Herdr → **现有全局安装的 OMP** 正常交付入口运行，不直接调用源码 CLI、SDK 或替代 MCP manager。依赖均须预先具备，runner 不改全局安装或 PATH、不提权、不修改开发者模式；不修改 OMP 源码、不构建或部署新的宿主。`--omp-package` 只声明已有安装包，不替换实际入口。Windows PTY 会从注册表重建 PATH，单独给父进程添加 PATH 前缀或放置另一份构建产物不能可靠指定宿主。runner 保留 OS 实际进程的 ExecutablePath 与 CommandLine，结合原有安装入口／包身份及实际技能消费核对运行身份；版本或文件哈希本身不能证明 Herdr 实际运行了所声明的安装。

验收检查现有 `--config` 对 `skills` 的高优先级局部覆盖：每次启动从所选集合重新取得有效直接子技能，生成最终名称 `includeSkills` 白名单，七个已有 `enable*` 来源开关均为 `false`、`ignoredSkills` 为 `[]`，有效名称为空时 `enabled: false`。核对原生最终可用列表及真实源路径，而不是要求默认目录完全未被扫描；不得把扫描行为与最终技能列表隔离混为一谈。原生 MCP、LSP、agent、扩展和项目指令不因技能覆盖而关闭。

```powershell
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot
python scripts/test-real-herdr-suite-skills.py `
  --herdr 'C:/installed/herdr/herdr.exe' `
  --omp-package 'D:/node_modules/@oh-my-pi/pi-coding-agent' `
  --backend 'backend/target/debug/herdr-bot.exe'
```

默认 `--scenario selected` 创建独立根引用／声明相对路径／含空格的技能集合及名称含 `*` 的技能，在两个不同 Chat cwd 中实际 `read skill://...` 读取技能正文、scripts、references 与资产；第二次发起前向显式来源集合新增技能，验证后续发现将其纳入；未声明目录中的技能不参与。用户／项目／managed 默认技能、已加载插件的同名错误正文及可被错误通配符匹配的技能均为独立哨兵；原生最终列表必须排除它们。第二次启动还核对第一次的配置未被改写。受控回环模型检查原生最终技能列表，只要求真实 read／write 工具执行预定动作；通过实际原生 `xd://mcp__native_probe` 调用取得字面结果，并核对 MCP 服务器记录的 `tools/call`。同次还观察 Suite 本地模板未注入、原生 `AGENTS.md` 指令仍在。这里只证明协议与资源消费机制，不证明真实模型自主遵循业务职责。

runner 隔离 HOME／APPDATA／XDG、agent 配置与会话、命名 Herdr server、动态回环模型／后端和源／生成目录，不读取用户凭据或已有 Chat；静态页面使用测试拥有的占位文件，因为本入口不承担 S1 页面验收。正常及失败路径持续消费服务日志、停止所属进程，持有真实 MCP 进程句柄确认退出；证据 JSON 分开保留原始错误和清理错误。输出指向独占 Temp 的 `evidence.json`，保留实际请求／响应、公开入口结果、MCP wire、来源 revision／diff／哈希、源路径、配置产物及原始日志，不在仓库生成验收报告。

技能直接引用源路径，不需要链接权限或手动 opt-in。必须在不能创建符号链接的普通 Windows 环境实际消费非空技能及相对资源，参数检查不能代替成功证据。`--scenario empty` 是独立的较窄场景：无需建链接，验证明确空技能 Suite 的技能过滤、原生 MCP 实际调用、模板省略及原生指令保留，**不能**替代 selected 场景或证明非空技能／资产消费。缺任何真实依赖或必要宿主能力，完整专项仍是阻塞／未验证。

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
python scripts/test-real-herdr-suite-extensions.py `
  --herdr 'C:/installed/herdr/herdr.exe' `
  --omp-package 'D:/node_modules/@oh-my-pi/pi-coding-agent' `
  --backend 'backend/target/debug/herdr-bot.exe'
```

入口复用技能专项的独占配置、动态回环模型／HTTP 端口、持续消费日志和所属进程清理。一个 Suite 声明相对入口文件与集中根引用的具体非 index 源入口，均包含空格；直接传给原生宿主，不生成扩展链接或入口清单，也不自行展开 package.json。两个不同 Chat cwd 均放置错误相对入口哨兵。真实工厂 import 同包 sibling TS 和 `node_modules` 模块，真实工具读取 `import.meta.url` 相对 JSON 资产并计算，独立字面预期为 `RELATIVE-ASSET-412:64` 与 `ROOT-ASSET-905:80`。受控模型通过 OMP 原生 `write xd://...` 调用工具，不伪造工厂或工具执行；此证据只证明机制，不证明真实模型自主遵循。

验收核对没有生成扩展包链接或 `omp-extensions.json`，来源文件字节不变且固定扩展仍加载。输出的独占 Temp `evidence.json` 保留源入口、公开 HTTP 结果、原生模型请求／工具结果、源码与可执行文件哈希、原始日志和分开的清理错误。此空技能专项不需要符号链接权限；任何真实依赖缺失或工具消费失败均非零退出并记为未验证，配置列表存在或普通套件通过不能替代原生直接加载的 S4 成功证据。

P46 的扩展负例入口为 `cargo test --manifest-path backend/Cargo.toml --test suite_omp_extensions`：真实 Windows 独占共享锁阻止实际源入口读取，经公开预览／启动 HTTP 检查明确拒绝且 Herdr 无调用。装配器只检查入口可读性，不遍历包内资产或 node_modules；模块／资产由原生宿主从源目录读取。负例不需要链接权限，不代替上述真实成功场景。

## Pi Suite 启动开发烟测

`07` 的开发烟测为公开 Task／Chat HTTP → 真实 Herdr → PATH 上安装版 Pi；完整 Pi 契约矩阵仍由资源／MCP和模式／模板验收拥有。先构建当前 worktree 后端，预先安装依赖，toolkit 必须提供公开工厂期 `--pi-suite-config`。不安装、不提权、不修改 PATH，不用 SDK 或替代 launcher：

```powershell
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot
python scripts/test-real-herdr-suite-pi.py `
  --herdr 'C:/installed/herdr/herdr.exe' `
  --pi-package 'D:/node_modules/@earendil-works/pi-coding-agent' `
  --toolkit 'D:/installed/my-agent-toolkit' `
  --backend 'backend/target/debug/herdr-bot.exe'
```

独占临时目录隔离 HOME／APPDATA／XDG、Pi 状态、后端配置、命名 Herdr server 和动态回环端口；不读取用户凭据或会话。共享声明相对路径含空格，扩展包 manifest 指定非 index 入口。受控回环模型请求 Pi 真实 read 消费技能正文，并调用真实包工具读取相对 JSON 资产，独立字面结果分别为 `PI-SKILL-BODY-731`、`PI-PACKAGE-ASSET-482:64`（19×3+7）。这只证明启动接线与消费机制，不证明真实模型自主遵循。

持续消费进程输出，正常及失败均停止所属 Herdr/server/子进程，原始错误与清理错误分别写独占 Temp 的 `evidence.json`；记录源码 revision／diff／哈希、二进制和安装入口哈希、实际 HTTP／模型请求及 terminal。依赖／flag 未交付、实际资源未消费或清理失败均非零，不 skip、不退化为直读源码加载，不把产物／参数检查算作 S4 通过。

## Pi Suite 资源与原生 MCP 跨仓验收

完整 S2/S3/S4 矩阵入口为 `python scripts/accept-suite-pi-resources.py`，显式接收两个 worktree、当前后端、隔离 paths 文件、场景和独占证据目录。覆盖集中根／相对路径／空格／不同 cwd、真实技能与扩展依赖、builtin 与 Suite MCP 同次调用、文件优先／等价／禁用／空清单，以及配置／装配拒绝和原生不可达状态。参见 [Pi 资源验收](../pi-suite-resource-acceptance.md) 的依赖、独立预期和完整复跑命令。缺真实依赖、宿主能力或清理失败非零；受控模型只验证消费机制，不证明模型自主质量，全局套件不能替代逐场景证据。

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

## 嵌套流程真实文件消费验收

独立入口 `python scripts/accept-workflow-nesting-files.py` 经本 worktree 构建的真实页面发起 Run，加载指定 toolkit worktree 的真实角色入口。toolkit 使用 Bun 1.4.4 和 frozen lock；若 typecheck 明确报告缺少已锁定的 `@typescript/typescript-win32-x64`，可在该 toolkit worktree 执行 `bun install --frozen-lockfile --linker isolated --force` 修复本地依赖链接，不替换编译器、不改锁定版本或借用其他 worktree 源码。不要用旧的普通文件交接场景、受控 provider 或最终产物替代它。入口的隔离业务定义显式声明 Pi；`--hosts declared` 保留并执行这些 Task 声明，证据只记 Pi，不外推 OMP 或双角色生命周期通过。现有真实宿主设施不提供 OMP 文件消费能力，显式 `--hosts omp` / `--hosts omp,pi` 非零并保持未验证，不能以 Pi 成功抵消。

```powershell
$env:CARGO_TARGET_DIR = Join-Path $HERDR_BOT_WT 'backend/target'
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --example safe_shutdown_shell
python scripts/accept-workflow-nesting-files.py `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --hosts declared `
  --paths-file 'D:/isolated-acceptance/paths.toml' `
  --model-config 'D:/isolated-acceptance/model.json' `
  --evidence-dir 'D:/isolated-acceptance/new-exclusive-run' `
  --shell-path 'D:/Program Files/Git/bin/bash.exe'
```

依赖声明是验收专属 TOML，使用 `[roots]` 下已安装的 `herdr`、`pi_package`、`pi_agent`（只读 `extensions/herdr-agent-state.ts`）及 `chrome`（包含 `chrome.exe` 的目录）。`--chrome <绝对chrome.exe路径>` 可显式选择已经安装的独立 Chromium；不自动发现或下载替代浏览器。Google Chrome 在完全隔离 Windows known-folder 环境中可能将显式独占 profile 误判为默认目录而拒绝调试，此时保留失败，不添加绕过调试限制的 feature flag、不借用用户 profile。已安装 Chromium 的成功只记录该实际浏览器身份，不覆盖 Chrome 失败。不安装宿主、不改 PATH、不读取用户认证。真实模型配置沿用下节的专用 `provider/model/baseUrl/apiKeyEnv` 声明或公开 `CompletionTransport`，不允许确定响应替代。后端必须来自同一独立绝对 `CARGO_TARGET_DIR`；两仓源码根、revision/diff、锁文件、后端身份、已安装依赖及真实 OS 进程均保留。缺依赖非零，不能以准备完成作为通过。

场景两次经真实浏览器分别提交独立业务文字和不同文件字节，运行 root → middle → leaf 命名重映射，再让子交付分别进入普通执行者及另一子流程。实际 native read 调用与对应 tool result、原生 Session/Run/执行身份、输入绑定和交付字节共同作为证据，不只比地址。根空输入与嵌套空输入执行者不得读取或猜测材料路径，可选 appendix 明确缺席。首次 leaf 执行者实际停轮等待业务 release 后，driver 原地改写已派发文件；另一次实际派发后删除输入，再明确 release，真实执行者必须保留读取错误、报告不可完成，1.25 秒窗口内不修复或重试。release 是记录在原生历史的显式业务输入，不编辑运行记录，也不替模型选择工具。

证据目录必须不存在，由 runner 独占创建；HOME/APPDATA/XDG/原生存储、命名 Herdr、定义和运行目录全部隔离，页面使用独立 Chrome profile，后端只绑定动态数值回环并关闭 mDNS。持续消费 stdout/stderr，正常与失败均停止并等待所属进程；原始错误与清理错误分别保留，清理失败仍非零。保留 browser 请求与确认截图、原始 HTTP/模型请求结果、原生历史、文件原字节、只读运行事实和逐项结论；删除带凭据的临时 models.json。专用 fail-closed CLI 回归：设置 `WORKFLOW_HANDOFF_TOOLKIT=$TOOLKIT_WT` 后运行 `python scripts/accept-workflow-nesting-files.test.py`；它不证明真实消费。

## 文件交接真实角色自动验收

`python scripts/test-workflow-file-handoff.py --scenario roles` 是独立的跨仓入口，加载指定 toolkit worktree 的实际角色源码及当前 herdr-bot worktree 构建的后端。它不使用确定响应服务替代真实业务模型，也不修改用户部署。当前真实模型链路仅支持 Windows / Herdr / **Pi** 和 OpenAI-compatible chat-completions 服务；结果不外推为 OMP 真实模型通过。参数保真、严格 report 校验和真实后端落盘另由入口内调用的 `tests/harness-dock/node-report-contract.test.ts` 同时覆盖 OMP/Pi；该 dock 使用受控 provider/Herdr，不能充当模型职责遵从证据。

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
$env:CARGO_TARGET_DIR = Join-Path $HERDR_BOT_WT 'backend/target'
$env:OMP_BASELINE_RECORD_ROOT = 'D:/original-toolkit'
python scripts/test-workflow-file-handoff.py `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --scenario roles --paths-file 'D:/isolated-acceptance/isolated-paths.toml'
```

`--scenario plan-workflow` 的独立能力缺口阶段会通过真实 Chat 提交 `P28-GAP-SCOPE` 用户澄清：只确定税务申报表这一业务对象，系统、接口、材料、凭据及生产授权仍为未知且阻塞；要求保存并读回能力建设需求，不补齐缺失能力、不批准概要方案或 manual 报告、不授权创建或执行。原始输入及回答保留在 interventions 和原生历史中。

Windows 验收可显式追加 `--shell-path 'D:/Program Files/Git/bin/bash.exe'`，将实际安装的 Git Bash 写入本次隔离 Pi settings，不修改用户配置。不要让原生 Bash 工具落到 WSL：其 Linux 回环与 Windows 后端不同，`curl 127.0.0.1` 会报告连接失败，不能把该环境阻塞当业务能力缺口或强行推进。toolkit 全量跨仓检查还应显式设置 `HERDR_BOT_BINARY` 为本 worktree 的 `backend/target/debug/examples/safe_shutdown_shell.exe`，`HERDR_BOT_METADATA` 为本 worktree 的 `meta-data/task-options.toml`；`HERDR_BOT_REPO` 不替代这两个 interop 参数，禁止使用生产可执行文件冒充安全壳。

验收重定位 `extensions/workflow-chat/config.yaml` 时须保留全部必填配置，包括 `reportConfirmationTimeoutMs`；遗漏会使角色扩展无法加载。模型开始请求不等于角色已就绪，须核对实际模型请求中的 `run`／`collect`／`report` 与当前角色匹配，再归因于模型职责理解。

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

已安装 Pi 的 Codex 网页授权可通过独立入口 `python scripts/accept-workflow-codex.py --scenario revise --hosts declared --backend <当前后端> --toolkit <toolkit根> --paths-file <验收依赖TOML> --pi-package <已安装coding-agent包> --auth-file <明确授权使用的auth.json> --evidence-dir <不存在的目录> --shell-path <已安装Git Bash>` 接入既有真实业务 runner；`--scenario roles` 对应双角色，`--hosts omp,pi` 对应两个宿主分别运行。入口固定 `openai-codex/gpt-6-luna`，不继承本机默认模型、不回退其他模型。先追加 `--smoke` 单独检查一次真实文本调用，再为业务运行使用新的证据目录；烟测通过也不是业务通过。修订入口可用 `--cases task-no-materials` 等逗号分隔的顶层 case 清单做定向反馈，默认 `all` 执行完整矩阵；子集成功仅记录 `subsetPassed`，`passed` 和完整 promise 覆盖仍保持未验证，不能据此关闭票据。仅双角色入口另支持 `--hosts omp`／`--hosts pi`，用于独立运行未进入的宿主覆盖，不重试另一个宿主的失败决策；单宿主通过不能替代 `omp,pi` 完整通过，证据保留实际 `requestedHosts`。原生证据归档使用 Windows 扩展路径保全超长 Session 文件名，并拒绝合并既有目标目录；`python scripts/workflow_evidence.test.py` 检查完整字节与旧证据不被覆盖。

此入口把原始 Chat 历史与工具 schema 映射给安装版 Pi 的原生 Codex provider，由模型直接返回 SDK `toolCall`，再由真实业务宿主执行。不再把整段会话塞入文本 prompt，也不要求模型生成包装 JSON；模型正文不会被解析或截取成工具决策。此映射仍不等于 OMP／Pi 本身直接配置了 Codex provider。每次保留纯 body 的原生请求、SDK 实际响应、服务端模型身份及工具调用 ID；历史工具结果按原调用 ID 绑定，不编造缺失结果。OAuth 只在独立子进程内读取，不进入业务 models 配置、命令行或证据；不刷新、改写用户登录文件。该子进程复用安装版 Pi 的 HTTP 调度器及已有 `PI_PROXY_OPENAI_CODEX`／HTTP 代理设置，不改全局代理或 PATH；SDK 单独调用并不会自动安装 CLI 的代理调度器。过期或距到期不足本次预算／原生刷新窗口的授权非零失败，须由用户自行恢复登录后再明确发起新一轮，不自动重试。模型响应必须具有服务端 `gpt-6-luna` 身份；Codex 不支持调用方服务端输出上限，`max_completion_tokens` 仅作为本地响应交付上限，超过时拒绝交付，不把它称为服务端计费预算。未知字段、非文本输入或缺失工具结果的历史明确拒绝，不静默丢弃。原生错误只保留固定安全类别、HTTP 状态与请求次数，不保存认证头或原始依赖错误。SDK 禁止重试，使用一次 SSE 请求；传输将模型调用、JSON 解析与失败登记串行化，模型或序列化失败后，排队请求也不再调用模型。退出传输时等待所属处理线程及其模型调用结束，清理异常保持非零；不同业务 case／宿主仍是独立覆盖项，不把继续执行另一覆盖项称为重试同一模型决策。业务证据仍需逐项满足所属票据，缺依赖、调用失败或清理失败均保持未验证。设置 `WORKFLOW_PI_PACKAGE` 为明确的安装包后运行 `python scripts/workflow-codex-completion.test.py`，验证过期授权拒绝、无凭据输出、认证文件不变，以及真实 SDK 在受控 HTTP 响应下的原生工具 schema／历史错误／调用 ID 关联；该受控外部 API 不证明有效授权或真实模型业务成功。

CLI 的缺依赖 fail-closed 回归可在设置 `WORKFLOW_HANDOFF_TOOLKIT` 为参与 toolkit 根后运行 `python scripts/test-workflow-file-handoff.test.py`；它只核对非零、原始错误、无虚构 Run 与无服务启动日志，不宣称真实角色链路通过。

通过必须同时具备两宿主 dock、真实模型链路与对应 worktree 常规检查证据。入口未取得模型、宿主缺失、行为不合预期、等待超时或清理失败均非零；缺依赖即明确未验证，不以准备完成、提示词存在或最终 completed 推断通过。正常/失败均停止所属服务与进程，分别保存原始与清理错误，不调用真实关机、不连接/关闭用户服务，不启用 LAN/mDNS。

## 嵌套原生新进程恢复验收

`python scripts/accept-workflow-nesting-recovery.py` 是 P29／P32／P33 的跨仓原生专项；不重复服务端旧记录／碰撞矩阵。先按上文构建当前 worktree 静态页面、`herdr-bot`、`herdr-fixture` 与 `safe_shutdown_shell`，使用独立绝对 `CARGO_TARGET_DIR`。toolkit 使用 Bun 1.4.4、当前 worktree 依赖与角色源码；已有安装版 OMP、Pi、Herdr 和两宿主只读官方状态扩展必须可用，不安装、不修改 PATH、不读取用户认证。授权模型声明及公开 CompletionTransport 的格式、能力和证据限制沿用“文件交接真实角色自动验收”；不能用受控 provider 代替真实模型。公开 completion 传输可用结构化 schema 固定 `content: string` 与 `tool_calls: [{name: string, arguments: object}]`，只约束序列化格式、不预选业务动作；原始请求／结构化结果仍须保留，错误不重试。原生 OMP 的 SessionManager.open 需要 await，且 JSONL 可以先有 title 元数据再有 session 头；读取真实身份时不能假定第一行就是 Session。原生关闭可以追加真实 `session_exit` 元数据，因此同时保留关闭前历史和确认进程退出后的恢复基线；Resume 前后完整 branch 仍须逐项相等，不能过滤业务消息掩盖重放。

```powershell
python scripts/accept-workflow-nesting-recovery.py `
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

`WORKFLOW_HANDOFF_TOOLKIT=<实际 toolkit worktree> python scripts/accept-workflow-nesting-recovery.test.py` 只验证 CLI 缺依赖时保留错误、双宿主未验证且不启动服务；不替代原生专项。常规检查仍按票据分别执行 workflow_dispatch、完整后端、前端、toolkit workflow-chat／相关原生 dock、typecheck 和全量入口。

## 嵌套 manager 双角色真实验收

`python scripts/accept-workflow-nesting-roles.py` 经本 worktree 后端、指定 toolkit 的实际角色入口、已安装 Herdr 和真实授权模型，观察嵌套 manager 的内部 `run`／`collect` 与外层 `report`。入口不安装宿主、不读用户认证、不改全局 PATH，不启用 LAN／mDNS，也不执行关机。

先按上文构建本 worktree 静态页和后端，在 toolkit 安装缺失的 frozen 依赖。依赖根文件必须显式指定，提供 `herdr`、`pi_package`、`omp_package`、`pi_agent`、`omp_agent`；后两者分别仅只读使用 `extensions/herdr-agent-state.ts` 和 `extensions/herdr-omp-agent-state.ts`，不读取同目录的认证或配置。模型声明沿用本页的独立 `model.json`（`provider`、`model`、`baseUrl`、`apiKeyEnv`），密钥仅从显式环境名获取。公开 CompletionTransport 也可作为真实模型的文本传输，但不证明原生 provider wire 合规性。HOME／APPDATA／XDG、原生历史、业务和运行目录、命名 Herdr server、动态数值回环端口均为本次独占。

```powershell
python scripts/accept-workflow-nesting-roles.py `
  --backend "$env:CARGO_TARGET_DIR/debug/herdr-bot.exe" `
  --toolkit $TOOLKIT_WT --hosts omp,pi `
  --paths-file 'D:/isolated-acceptance/paths.toml' `
  --model-config 'D:/isolated-acceptance/model.json' `
  --evidence-dir 'D:/isolated-acceptance/new-exclusive-run'
```

`--hosts omp,pi` 对两个宿主分别运行两套独立 manager／zone／原始业务字面样例；每套重复调用同一 Map，并经真实 manager 的 fail／retry 保留旧 execution 干扰。`--hosts declared` 是额外的混合宿主业务夹具：第一套 parent OMP／child Pi／worker OMP，第二套相反，严格保留入口中 `DECLARED_ROLES` 的实际 Task 声明；不是把用户业务 Task 统一改成一个宿主，也不是对部署业务的验收。该模式不替代双宿主完整矩阵。

每次 manual 外报由原生 UI 展示完整载荷，driver 核对选中 Yes 后提交 Enter，并记录确认前无该次报告 HTTP、确认后请求与预览载荷一致；聊天文字和业务批准不作发送批准。旧执行的确认框明确取消后，须没有该会话的 report HTTP。入口在内部未完成、内部全完成但未 finish、finish 后、进展 false、最终 true、父 manager 显式验收后分别检查公开事实。检查实际文件字节、原生历史／三工具／双指引，以及真实 Herdr 的目标 Tab 精确关闭、内部保留 Tab 不级联关闭；不凭最终 completed 或 observer 身份授予授权。

`--evidence-dir` 必须不存在。原生运行状态使用原子创建的独占短 Temp 根，避免 Windows 已安装宿主的命名管道／存储路径长度限制；退出后完整复制到证据目录的逐宿主子目录，原始短根保留且记录为 `nativeRoot`，不把临时目录删除当作进程退出。成功或失败均保留 `evidence.json`、逐宿主独占子目录、两仓源码 revision／修改身份、锁文件／二进制／已安装宿主身份、模型／HTTP 原始请求结果、用户输入、原生历史、文件字节和分开的清理错误。模型配置在退出时删除。缺模型、宿主、能力、清理失败或行为不符均非零且未验证，不 skip，不重试业务或模型决策。`python scripts/accept-workflow-nesting-roles.test.py` 仅检查缺授权模型时的 CLI fail-closed 行为，不能代替真实专项通过。

## 共享 Bot 指南材料的启用检查

真实模型修订专项为 `python scripts/accept-workflow-nesting-revise.py --backend <本 worktree 后端> --toolkit <toolkit worktree> --hosts declared --paths-file <独占依赖配置> --model-config <专用授权模型声明> --evidence-dir <不存在的独占目录>`；依赖声明、混合宿主／双宿主行为、四次 Task／Map 有／无材料矩阵、专属 Task 配对、修订负例及证据边界见[独立修订验收运行说明](../revise-bot-guides.md#真实模型跨仓修订验收)。设置 `WORKFLOW_HANDOFF_TOOLKIT` 后运行 `python scripts/accept-workflow-nesting-revise.test.py` 仅验证缺后端／模型时非零、覆盖保持未验证且不启动宿主，不替代真实业务通过。

独立修订入口为 `revise-bot-guides`，公开定义／五节点材料接线检查为 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch revise_bot_guides`，安全进程烟测为 `python scripts/check-revise-bot-guides.py --target-dir <本 worktree 独立 target>`。调用接口、稳定专属 Task `task-revise-bot-baseline-v1`（22 配对 owner）、原位切换及证据边界见[独立修订运行说明](../revise-bot-guides.md)；不强制修改目标 Task.prompt，旧独立入口及无关 Map 不变。

`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch shared_business` 经公开定义保存／读取、预检和实际受控宿主派发，使用真实临时 baseline／text／handoff，核对材料直接绑定、不注入正文或三份能力指南、Task／Map 两类目标三字段一次保存、完整读回及真实 stale revision 拒绝。此入口不以提示词快照证明模型遵循职责；P40–P45 的真实创建／修订与六个既有 Task 的独立 Chat／workflow 配对验收仍分别归 21／22，外层修改归 24。材料协议、稳定 Task ID 与所有权见[共同业务材料契约](../business-materials.md)。完成前仍执行一次 `node scripts/test-backend.mjs`，使用当前 worktree 静态页面及独立 CARGO_TARGET_DIR，不运行用户业务或生产迁移。

创建链真实业务矩阵入口为 `python scripts/accept-workflow-nesting-create.py`，必须显式提供 `--backend`、`--toolkit`、`--hosts declared`（或完整 `omp,pi`）、`--paths-file`、`--model-config` 和独占 `--evidence-dir`。安装依赖、独立业务技能根、完整复跑命令及证据边界见[真实创建与配对验收](../create-bot-wrapper.md#真实创建与配对验收入口)。缺构建与重复证据目录的 CLI 回归为 `WORKFLOW_HANDOFF_TOOLKIT=$TOOLKIT_WT python scripts/accept-workflow-nesting-create.test.py`；该回归不认领真实模型、混合宿主或保存安全通过。

最小原生烟测可针对 Pi 的 `task-18e31d34797f1464-3`：将当前 Task.prompt 与明确的独立 Chat 委托组合为测试拥有的 `request.md`，提供字面 `source.txt`，只授权当前隔离目录的新 `text.txt`、`handoff.md`。安装版入口为 `node <pi-package>/dist/bundle/cli.js --offline --no-extensions --no-mcp --no-skills --skill <writing-for-agents/SKILL.md> --no-context-files --no-prompt-templates --no-themes --model <专用provider/model> --thinking off --mode json --print @request.md`。使用本节上文的专用模型配置或公开 CompletionTransport，隔离 HOME／APPDATA／XDG／PI_CODING_AGENT_DIR 与原生 sessions；传输的公开 completion 上下文必须覆盖整个子进程等待期，不能先退出宿主调用再留后台线程访问已失效桥接。HTTP handler 线程不继承宿主 completion 调用上下文；将请求排队交给拥有该上下文的主线程执行，再把结果返回 handler，避免 `Missing session/run/name`。测试不读取用户凭据，不复用用户 Chat，也不启动 daemon。

该烟测观察原生真实 read／write、两份材料完整落盘读回、原文与未授权文件保持，以及 handoff 不含重复正文；保留完整模型请求、native JSONL、错误及真实材料。进程退出零不是充分条件，原生 `stopReason: error`、缺文件或未完整读回均失败。工具必须真实执行，不可由模型或传输伪造结果；传输用完关闭并等待，移除隔离凭据配置。它只证明一个独立原生调用能处理其实际材料，不是六 Task 配对覆盖、公开 Chat 发起或两案例完整 P40–P45 证据。

以下是迁移前历史烟测边界，不再是当前可运行入口：原 Map 目标最小创建烟测曾使用已删除 `task-18e33fdc55177d1c-1` 的 Pi prompt。当前请用上文发布创建图迁移回归和真实原生创建入口，不恢复旧 Task 或加别名。历史流程中，当前 worktree 的 `debug/examples/safe_shutdown_shell.exe --config <隔离backend.toml>` 提供同一生产 HTTP interface；只配置数值回环动态端口、独占定义／运行／zone 文件并关闭 mDNS。先经公开接口准备 normal Task 和已有工作 Bot、放置测试拥有的真实 Map，再授权只新建指定 Map 型 Bot 和 baseline／handoff 文件。原生模型自行读取公开完整定义与真实 edit.revision、新建空白 Bot、写材料并完整读回；独立 HTTP 读回比较 Map 绑定、三空字段、真实 revision、完整共同基线、原有 Task／Map／Bot 不变及无 Run。保留原始 HTTP、native JSONL 和文件，失败及清理错误分开记录；受控的未加载 catalogue 资源只满足后端夹具结构，不作为目标工具或角色原生加载证据。完成后终止并等待所属安全后端与原生进程、关闭模型传输、移除隔离凭据。该调用是轻量启用证据，不认领 21／22 的完整真实案例或六 Task 配对覆盖。

## 创建 Task＋Bot 外层真实兼容验收

`python scripts/accept-workflow-nesting-outer.py --backend <本 worktree herdr-bot.exe> --toolkit <toolkit worktree> --hosts declared --paths-file <专用已安装依赖声明> --model-config <明确授权模型声明> --evidence-dir <不存在的独占目录>` 拥有 Task 创建者／既有 Task.prompt 优化者的独立 Chat 与实际外层 workflow 配对，以及旧两个独立创建入口兼容和未迁移 Map 范围保持。运行方式、完整 Suite/MCP 隔离、独立业务全文确认与报告发送确认、父子真实交接及证据边界见[包装能力运行说明](../create-bot-wrapper.md#真实外层与旧独立入口验收)。必须保留声明宿主，不能将原混合宿主改成单一 Pi 迁就 runner；`--hosts omp,pi` 要求两套完整场景各自通过。它不认领六共享 Task 独立配对或规划选择，也不能由组件、受控 provider、最终 completed 或全局回归代替。CLI fail-closed 回归设置 `OUTER_ACCEPTANCE_TOOLKIT` 后执行 `python scripts/accept-workflow-nesting-outer.test.py`；它仅验证拒绝边界，不是专项真实业务通过。

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
