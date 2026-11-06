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

## 真实外层与旧独立入口验收

`python scripts/accept-workflow-nesting-outer.py` 是外层兼容专项，唯一拥有 Task 创建者／既有 Task.prompt 优化者的独立 Chat 与实际外层 workflow 配对，以及旧 `create-bot-task`／`create-bot` 独立入口。它不重跑包装能力六个共享 Task 的独立配对，不认领规划选择，也不把定义接线回归当业务通过。

先按[测试指引](agents/testing.md)构建当前 worktree 的页面、`herdr-bot`、`herdr-fixture`、`herdr-paths` 和 `safe_shutdown_shell`，使用独立绝对 `CARGO_TARGET_DIR`；toolkit 使用其 worktree 的 Bun 1.4.4/frozen lock。依赖必须已安装。依赖文件是专用 `[roots]` TOML：`herdr`、`pi_package`、`omp_package`、`pi_agent`、`omp_agent`、`wayfinder`、`wayfinder_skills`、`pi_extensions`、`codebase_memory_mcp`；agent 根只读取明确的 Herdr 状态扩展，不读取用户认证或 settings。`native_shell` 可显式指向已安装的 Git Bash。不要使用用户业务 paths 或默认同级 checkout。源 Suite／固定扩展保持选择与职责，仅在独占部署中重定位其资源、connect-bot/workflow-chat 的回环端点及 model-cost 存储；授权验收模型另追加为 `probe`，不会覆盖原模型映射。

```powershell
python scripts/accept-workflow-nesting-outer.py `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --hosts declared `
  --paths-file 'D:/isolated-acceptance/outer-paths.toml' `
  --model-config 'D:/isolated-acceptance/model.json' `
  --evidence-dir 'D:/isolated-acceptance/new-exclusive-outer-run'
```

模型声明沿用[真实角色自动验收](agents/testing.md#文件交接真实角色自动验收)的专用 `provider/model/baseUrl/apiKeyEnv`，或同页公开 CompletionTransport；不读全局 auth，不用受控 provider 代替模型决定。`--hosts declared` 保留每个实际源 Task 的宿主（包含包装内原有 OMP/Pi 混用），逐项记录源码稳定 ID、隔离真实 ID、实际角色和 OS 身份。`--hosts omp,pi` 分别在两宿主执行完整矩阵，任一失败均非零；不能拿单宿主成功抵消另一个。该入口不安装、不提权、不改全局 PATH、不重试业务或模型决定。

专项先由真实独立 Chat 创建、优化一个仅配置交付的隔离 normal Task，再执行发布的三节点外层和八节点包装子 Run；外层创建者／优化者保持原 prompt，只使用明确业务要求与当前公开 HTTP。目标使用非空 Suite，以完整前后字段比较检查优化者不丢失该选择。候选全文先写入业务授权的 proposal 文件并真实读回；自动业务核对使用预先固定的 CSV 只读职责、完整配置字面预期与真实模型独立语义评价，成功后在原生 Chat 发送明确全文／保存确认。manual 节点保存读回后再发送另一条报告发送确认，两种授权分开留痕；评价本身不是用户批准。模型自主选择和执行真实工具，driver 不手改运行记录、不伪造 Session，也不替管理者 dispatch／complete／finish。

证据必须同时具有真实 Task/Bot/Run/execution/Session 关联、原生工具及请求结果、命名 request／共同 baseline／实际输出的完整消费、真实文件字节、父子 finish/report/complete 顺序、目标全配置不变和无新目标业务运行。最终 `completed` 或 wrapper 文件存在均不充分。旧两个独立入口各自另外运行隔离场景；未迁移 Map 与共享执行 Task 逐项保持不变。保存前后、自动确认、报告确认、源码／锁文件／构建产物／安装宿主身份和原始错误均在本次独占目录保存，不将旧成功证据合并进新调用。

独占 HOME/APPDATA/XDG/原生存储、命名 Herdr、业务定义和运行目录；只用动态数值回环、关闭 mDNS，不执行真实关机。Suite 的真实 codebase-memory-mcp 保留，但可选 UI 关闭；已安装二进制必须支持公开 `CBM_RUNTIME_DIR`，并使用测试拥有的 owner-private runtime、cache 和 allowed-root，绝不连接用户 daemon。正常与失败均停止、等待并确认所有所属进程退出（包含脱离 stdio 客户端的所属 MCP daemon），原始验证错误和清理错误分开记录；隔离 credential-bearing models 文件会移除。缺任何宿主、授权模型、必要 Suite 能力、隔离或清理条件均保持未验证且非零。

PowerShell 中先设置 `$env:OUTER_ACCEPTANCE_TOOLKIT = $TOOLKIT_WT`，再执行 `python scripts/accept-workflow-nesting-outer.test.py`；这是 CLI fail-closed 回归，仅证明缺后端不宣称通过、已有证据不可覆盖；不证明上述真实业务已通过。运行说明描述可复跑入口及其边界，不是当前部署的成功声明；结果仅以实际本次逐覆盖项证据为准。

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
