# 为既有目标包装完整 Bot

公开能力为 `create-bot-wrapper` Bot，唯一目标是 `create-bot-wrapper.json` Map。使用现有 scheduler 的定义读取、Run 预检与发起入口，或由外层 Map 的节点引用这个 Bot；不增加产品入口。它与旧的“创建 Task＋Bot”流程不同：目标必须已经存在，不能由本流程新建、修改或执行。仅选定外层 `create-bot-task-workflow--bc-61ce48b80a4c.json` 已改为调用此能力，其他 Map 不自动替换；独立 `create-bot-task` 和 `create-bot` Bot 的职责及 Task 绑定保持原样。本文件不表示用户部署已迁移。

## 调用接口

| Map 命名文件输入 | 必填 | 内容 |
| --- | --- | --- |
| `request` | 是 | UTF-8 业务委托：真实 normal Task ID 或 Map 文件名（二选一）、明确创建要求、业务需求、资源／权限／非目标、材料写入范围，以及可选的 Bot 名称／ID 和正式交付位置 |
| `materials` | 否 | 可读参考材料及其用途；没有参考材料时不提供此绑定，不制造空路径。提供的文件不可读则失败，不视为未提供 |

`request` 无须含完整 Task／Map 配置，核实者自行经真实服务查询。`userInput` 提供本次运行背景，不替代 `request`。嵌套调用时，父节点通过 `input_rule` 以同名输入显式绑定 request／materials，并填写自己的 `manager_task_id`、zone 和自包含业务 input；管理者配置不是 Bot 属性。父节点还应声明 `saved` 和 `handoff` 输出，要求子管理者在完成整个子 Run 后将 `save_bot` 的同名交付直接报告给父层；地址登记不是业务验收，父管理者仍须显式接受交接。Map 不声明不存在的全图输出接口。

最终 `saved` 是公开接口完整读回的真实 Bot 对象，`handoff` 只记真实身份、normal Task／Map 唯一绑定、确认／授权依据、三字段来源、保存和非目标保持结论、已发生效果及未验证项。父层必须核对真实身份和绑定，不根据“完成”文字推断新能力已验证。必要时显式要求额外交付 baseline，但不通过 handoff 隐式寻址。

## 执行与授权

材料角色与安全保存只遵循[共同业务材料契约](business-materials.md)，不另立协议：

1. `create_blank` 直接消费 request 和可选 materials，自行核实真实目标及 Bot 目录。创建委托授权新建唯一绑定的空白 Bot；三字段严格为空，名称／ID 冲突就停止，不复用、覆盖或改名绕过。完整读回并确认目标未变，交付单份 baseline 和必要 handoff。
2. 三个 `write_*` 分支各自直接消费同一 baseline，独立编写、确认所属全文并交付唯一 text＋必要 handoff；不写 Bot。每个分支的 `report_policy: manual` 要求另取得报告发送确认，不能用全文确认、保存授权或 `can_complete=true` 替代。
3. 三个 `optimize_*` 分支各自直接消费同一 baseline、所属 text＋handoff，只优化表达。语义变化导致确认失效时重新确认受影响语义，不自行扩权；不写 Bot。
4. `save_bot` 直接消费 baseline 与三组最终 text＋handoff，依真实批准来源核实整个替换批次的授权，不因流程走到此处或 `can_complete=true` 推断用户批准。重读 Bot、真实 revision 及完整目标；缺材料、未确认、冲突或运行禁写时整批不开始。合法时一次保存三个字段，保持身份、绑定及全部非目标，公开完整读回 Bot 并重读目标确认未变。失败或不明先核实效果，不盲目重试、回滚或吞掉错误。

仅使用六个既有共享 Task，稳定 ID 和独立／workflow 配对归属见共同契约。本能力没有新增专属 Task。不会自动运行新能力、安装工具、补建目标、扩权或改写自身运行配置。配置交付不证明目标工具／Skill／扩展加载、模型遵从或业务效果。

## 创建 Task＋Bot 的选定外层入口

`create-bot-task-workflow--bc-61ce48b80a4c.json` 现在只有三个工作单元：

1. `w1` → `create-task`：从本次 `Run.userInput` 和可选命名 `materials` 创建 normal Task。保留全文和执行配置确认，公开保存／完整读回；`handoff` 自包含地记录真实新 Task ID、需求、确认来源、权限及后续 Bot 创建委托。未明确的授权不得自行补齐。
2. `w2` → `existing-task-prompt-optimizer`：直接消费 `w1.handoff`，在当前 Chat 交互确认并优化已创建 Task 的 prompt；只保存 prompt，完整保留其他配置。其 `handoff` 包含真实最新目标、业务要求、确认及授权、材料用途和写入范围，同时作为子流程的 `request`。两步均为 manual 报告，业务确认与发送报告确认分别成立；外层管理者核实真实报告后显式 complete 才可推进。
3. `wrap_bot` → Map 型 `create-bot-wrapper`：本次明确使用 manager Task `task-18e2c2827c484128-0` 和 zone `hugosol/herdr-bot`（部署须先公开核实或显式修改本调用配置，不改 Bot 属性）。`request` 直接绑定 `w2.handoff`，`materials` 直接绑定外层同名可选输入；不依赖 `blocked_by`、父 Chat 或 handoff 的隐式地址传材料。子管理者核实并逐项 complete 子节点，完成整个子 Run 后通过真实 node report 将 `save_bot.saved`／`save_bot.handoff` 直接登记为父节点同名输出。父管理者另行核实真实 Bot ID、唯一 Task 绑定、完整保存读回与授权证据，显式 complete 本调用再 finish 外层；子 Run 完成、报告文字或 `can_complete` 都不是这个边界。

根 Run 仍由调用者经 scheduler 选择自己的 manager Task、zone 和本次业务输入；`materials` 缺席合法，提供却不可读必须拒绝。仅移除了本外层原先内联的空白 Bot、三路指南、三路优化及最终保存分支；包装 Map 的八节点职责保持不变，不将 Task 创建移入包装。旧独立 `create-bot-task` 仍绑定 `task-18e315be1cf0ef10-0`，`create-bot` 仍绑定 `task-18e31d347ccbeb4c-4`，不改变其独立职责，不迁移其他 Map 或用户部署。

### 外层 Task 兼容与独立／workflow 配对清单

| 真实稳定 Task ID | 本链职责与接口修正 | 配对验收唯一 owner |
| --- | --- | --- |
| `task-18e31d30fe69a51c-2` | Task 创建者；POST 当前字段含至多一个 `suite`，不提交旧 `extensions`；按 tasks/options 列表取得真实 ID，再从 GET 的 task 完整读回；来源为 `backend/src/tasks/mod.rs` | 24（P45） |
| `task-18e3454a33cecffc-6` | 既有 Task.prompt 交互优化者；PUT 只改变 prompt，完整保留 suite 和全部其他配置；不发送旧字段，确认、共享影响及运行禁写职责不因独立／node 身份改变 | 24（P45） |

上述两项不属于 21 拥有的共享六 Task 清单，不重复认领。源码修订只纠正当前公共契约，不增加旧字段别名、兜底重试或失败吞没。完整真实外层调用与旧入口保留的 contract 验收由 24 拥有（P46）；本票公开定义接线和最小共享烟测不是这两项通过证据。

## 真实创建与配对验收入口

`python scripts/accept-workflow-nesting-create.py` 使用实际授权模型、已安装 Herdr 与 OMP／Pi，经公开独立 Chat、Run 和定义 HTTP 入口验收。`--hosts declared` 保留六个共享 Task 的实际混合宿主：空白创建者和表达优化者为 Pi，三个指南编写者和保存者为 OMP；不得为适配 runner 全改 Pi。`--hosts omp,pi` 另对两宿主各执行完整矩阵，任一失败不可由另一成功抵消。此入口不安装宿主、不改全局 PATH、不读取用户认证，不连接现有 Chat 或 daemon。

```powershell
$env:CARGO_TARGET_DIR = 'D:/isolated-build/create-acceptance-target'
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --example safe_shutdown_shell
python scripts/accept-workflow-nesting-create.py `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --hosts declared `
  --paths-file 'D:/isolated-dependencies/create-paths.toml' `
  --model-config 'D:/isolated-dependencies/authorized-model.json' `
  --evidence-dir (Join-Path $HERDR_BOT_WT '.scratch/workflow-nesting/evidence/create-exclusive-run') `
  --shell-path 'D:/Program Files/Git/bin/bash.exe'
```

paths 文件为验收专属 TOML，`[roots]` 明确声明已安装的 `herdr`、`pi_package`、`omp_package`、`pi_agent`、`omp_agent`，以及原 Suite 所需的 `wayfinder`、`wayfinder_skills`、`pi_extensions`、`codebase_memory_mcp`。两个 agent 根**仅**用于只读加载 `extensions/herdr-agent-state.ts`／`extensions/herdr-omp-agent-state.ts`，不读取其 settings、models、auth、sessions 或技能目录。保留源码完整 Suite 声明、Task.suite 和初始 Skill，不把业务 Suite 缩成四个技能或删除 MCP／扩展／LSP。`toolkit` 根由本票指定源码复制到独占部署，不从默认同级旧仓取后端或角色源码；其他依赖可用同名 CLI 参数显式覆盖。

完整固定扩展仍使用实际源码。仅在部署副本中将 connect-bot 指向本次动态后端，将 model-cost 存储指向本次独占目录。原生 codebase-memory-mcp 使用实际安装程序与完整工具能力，仅关闭无关可视化（`--ui=false`）；分别设置独占 `CBM_RUNTIME_DIR`、`CBM_CACHE_DIR` 和 `CBM_ALLOWED_ROOT`，runtime 目录限定为当前 Windows 所有者私有 ACL。缺这些已安装原生隔离能力则拒绝启动，不接入用户 account daemon。实际进程观察包含拥有进程的全部后代而非仅 OMP／Pi 白名单，持有并等待 MCP daemon 的进程句柄，退出失败仍为验收失败。

模型文件使用[测试指引](agents/testing.md#文件交接真实角色自动验收)中的专用 `provider/model/baseUrl/apiKeyEnv` 声明；可使用同处说明的公开 CompletionTransport，其职责只传输真实模型决策，不生成业务步骤或伪造工具结果。密钥仅取显式环境变量，不写请求头日志；结束时删除所有验收拥有的 credential-bearing models 文件。`--timeout 1..900` 是每阶段观察窗口，不是重试次数。

独立 normal Task 案例不提供可选材料，Map workflow 案例提供明确参考材料。独立 Chat 的必要日期口径歧义由真实用户消息澄清；三个分支分别展示完整全文并取得真实确认，manual 报告另行取得发送同意。六个稳定 Task 逐个保留独立／workflow 配对记录，创建 Map 当前没有额外专属 Task。保存检查比较最终唯一 text 字节、真实 Bot 完整读回及全部非目标与目标配置；不以创建成功推断业务能力已经执行或验证。表达质量另由同一明确授权服务的独立实际请求按预先固定的目标真实性、字段职责、语义保持、授权边界和正文独立性 rubric 逐项评价，保留原始输入、回答与逐项理由；任一混合／缺证结果失败，评价结果不授予业务确认或保存授权。

独立保存安全场景经同一保存 Task 的真实 Chat 检查未授权、基线／目标冲突、实际运行使用、真实竞争 revision，以及保存已生效／未生效／未知效果。HTTP 故障只在实际传输边界断开观察或制造真实竞争编辑，不替换宿主、模型或产品结果；保留每次实际请求、原始错误与后续核实，不盲重试或自动回滚。普通文件使用真实 BOM／CRLF／非目标字节样例与真实隔离 junction／越界请求；没有链接能力仍为未验证，不能 skip 后当通过。

证据目录必须不存在，由 runner 独占创建。HOME／APPDATA／XDG、原生存储、临时目录、命名 Herdr、业务定义和运行目录均隔离，动态数值回环后端关闭 mDNS。原生存储使用额外独占短路径避免 Windows 历史文件长路径，结束后将不含凭据的副本归档到证据目录。持续消费 stdout／stderr，成功与失败均停止并等待所属进程，分别保存原错误和清理错误。证据包括两仓根／revision／修改身份、锁文件／产物身份、已安装宿主／OS 进程、原始输入／批准、模型与工具结果、HTTP、原生历史、实际文件字节、只读运行事实和逐覆盖项结论。缺真实依赖、模型授权、实际消费或清理成功时非零，所有未完成项保持未验证。

CLI fail-closed 回归：设置 `WORKFLOW_HANDOFF_TOOLKIT=$TOOLKIT_WT`，运行 `python scripts/accept-workflow-nesting-create.test.py`。它只检查缺构建／重复证据目录不会变成成功或启动服务，不证明实际业务矩阵通过。真实矩阵结果以运行保存的原始证据为准，本使用说明本身不宣称通过。

## 验证入口与证据边界

使用本 worktree 独立 `CARGO_TARGET_DIR`，先执行 `npm --prefix frontend ci`、`npm --prefix frontend run build`；定向入口为：

```powershell
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch outer_create -- --nocapture
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch create_bot_wrapper -- --nocapture
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch shared_business -- --nocapture
node scripts/test-backend.mjs
```

外层 `outer_create` 回归先用独立字面预期核对发布 Map 仅有 `w1`／`w2`／`wrap_bot`、当前稳定 Bot／Task 绑定、子 manager、zone、request／materials 和 saved／handoff 接口，然后经公开保存／读取及预检实际消费接线；旧内联十节点定义已观察到此用例失败。受控 host 环境使用测试拥有的 normal／manager Task 与 zone，保留包装子图完整拓扑，不证明部署 Suite 或原生角色加载。外层发布烟测实际运行本 worktree 构建的安全后端壳，复用 `scripts/check-workflow-file-consumption.py` 的隔离 `Smoke.prepare`、真实 Windows `deny_read` 和进程日志消费／清理设施，通过公开输入与节点保存／完整读取、预检检查可选材料缺席／可读成功、真实独占读锁和派发前删除输入拒绝、拒绝后定义完整不变且没有外部宿主调用。原始 HTTP、服务日志和 evidence.json 保留在执行输出的独占目录；它没有发起业务 Run，不替代 24 的真实外层调用。

外层切换同时另行实际执行过上述安装版 Pi 最小共享 Task 烟测：使用当前 `task-18e31d34797f1464-3` 原始 prompt、原生 writing-for-agents 与公开 CompletionTransport，首次调用真实 read 源文件、真实写入唯一 text／handoff 并分别完整 read 回。独立读回确认来源路径字符准确、无重复正文、源文件及未授权哨兵不变；优化稿保留“用户提供且明确授权读取、仅差异报告、不改账单、不付款”四项已确认语义。原生 JSONL、真实模型 wire、request、源与交付文件保留在独占目录，隔离模型配置已移除，进程与传输已停止／等待。它只证明这一次独立共享 Task 的材料与授权边界，不证明创建者／Task.prompt 优化者配对、公开 Chat 启动、OMP 或完整子流程／外层执行。

wrapper 回归通过既有公开 Bot 保存／读取、Map 输入与节点保存／读取、真实文件预检检查八节点完整接线、可选材料缺席合法和错误材料拒绝，不固定提示词措辞。发布检查还实际运行本 worktree 构建的 `debug/examples/safe_shutdown_shell.exe --config panel.toml`，复用 `scripts/check-workflow-file-consumption.py` 的隔离 Smoke.prepare 配置和日志消费设施，随后经 HTTP 保存／读取本 Map 八节点与包装 Bot；真实 request 文件、参考材料缺席／可读、Windows 独占读锁拒绝均已观察。语义上未声明的 Run 输入被现有接口拒绝为 `map-read-failed`／HTTP 500（不是本烟测最初预期的 400）；保留原错误后重启同一隔离后端，公开完整读回核实原定义未变。拒绝不以特定 4xx 为充分条件，必须核对无定义副作用；这不是新增的错误状态契约。全程无 Run 或外部宿主调用，成功失败均终止并等待所属安全后端与日志线程。测试先核对发布的六个共享 Bot／Task 稳定绑定；受控宿主预检使用测试拥有的 Task 环境和 zone，不能作为部署 Suite 或原生角色加载证据。shared_business 回归独立检查 Task／Map 两种绑定三字段一次保存、真实 stale revision 拒绝及完整非目标保持。

最小真实共享 Task 调用使用[测试指引](agents/testing.md#共享-bot-指南材料的启用检查)中的隔离 Pi 入口，保留原生材料 read／write、授权结果、实际文件及完整读回。本能力发布时实际使用安装版 Pi、`task-18e31d34797f1464-3` 的原始 prompt 和公开 CompletionTransport 运行独立 Chat：真实读取源文件和技能、写入唯一 text／handoff 并完整读回，保留源文件与未授权哨兵。首次 handoff 的来源路径被原生生成的 Python 字符串转义为 TAB；保留该失败材料，在同一原生 Chat 中明确授权仅新建修正 handoff，再实际读回核对准确来源和旧文件不变。这是带显式错误修正的窄烟测，不是首次自主交付完全成功或六 Task 配对通过的证据。路径必须核对实际字符，进程零退出与模型自称完成均不足以证明材料准确。该烟测不是完整八节点自主业务执行；P39–P45 的两类目标、有／无材料、真实批准来源和六 Task 配对由 21 唯一负责，外层真实调用归 24。不能以配置存在或受控回归通过宣称这些 contract 验收通过。缺真实宿主、模型授权或能力时必须如实保留未验证。
