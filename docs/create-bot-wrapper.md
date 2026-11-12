# 为既有目标包装完整 Bot

公开能力为 `create-bot-wrapper` Bot，唯一目标是 `create-bot-wrapper.json` Map。使用现有 scheduler 的定义读取、Run 预检与发起入口，或由外层 Map 的节点引用这个 Bot；不增加产品入口。它与旧的“创建 Task＋Bot”流程不同：目标必须已经存在，不能由本流程新建、修改或执行。仅选定外层 `create-bot-task-workflow--bc-61ce48b80a4c.json` 已改为调用此能力，其他 Map 不自动替换；独立 `create-bot-task` 和 `create-bot` Bot 的职责及 Task 绑定保持原样。本文件不表示用户部署已迁移。

## 创建分支表示的交付边界

固定 Rust 创建／三路合并的精确 request、baseline、handoff、saved 字段与独立字面实例见[创建分支 v1](business-materials.md#创建分支-v1可校验表示与空白创建)及[可机读 Schema](contracts/rust-script-bots.schema.json)。命名 request／materials、baseline／text／handoff、最终 saved／handoff 与原接线不变；纯正文不包装，来源索引不负责寻址。发布定义已将原 `create-blank-bot` 身份切换到真实 `create_blank_bot`，仅将 `save_bot` 改为新共享 Bot `save-created-bot-guides`（`save_bot_guides`，三指南严格为空）。两脚本节点均为 auto 报告、auto_close=false。三个共享编写者及本图优化 input 明确区分创建 v1 与原修订格式；外层 W2.handoff 同步交付严格 request JSON，最终优化者读取 saved.bot／target.configuration。

删除范围仅原 Task `task-18e33fdc55177d1c-1`，不保留兼容别名，不保证该 Task 的旧来源恢复；不删除无关 Task、Chat 或 Run。原通用 replace-text Bot、Task 全部配置与修订 Map 保持原样，不按 Bot ID 全量替换消费者，也不建立 Map 私有 Bot。迁移前完整发布定义归档在 `fixtures/rust-create-migration/before.json`；这不是自动部署工具，用户服务与正式历史不受仓库更新影响。修订及独立非 v1 调用仍按原协议，无须补创建授权／digest。实际验证边界见末节。

## 调用接口

输入要求的唯一权威来源为 [`create-bot-wrapper.json`](../meta-data/workflow-maps/create-bot-wrapper.json) 的 `input_requirements` 与各项 `input_files[].description`；scheduler 发起页分别展示非空总体要求与对应文件标签，Bot 指南与调用者不另存完整要求。文件接口仍为必需 `request` 与可选 `materials`。

启动管理者时可暂不提供 request，用户可先在本次要求或管理者对话中说明委托、提供文件。管理者读取共同输入要求和已有材料，澄清语义与授权，必要时在获准范围内整理为 request 文件，以 `prepare_inputs` 全量绑定并确认后才能派发任何业务节点；确认时 request 必须齐全、可读且语义满足要求，不能以文件存在代替核实。嵌套子管理者同样先完成输入准备；父节点通过 `input_rule` 将已有 request／materials 以同名输入显式绑定，作为子 Run 的准备初值，并填写本调用的 `manager_task_id`、zone 和自包含业务 input；发起前通过公开接口核实所选 Task 真实存在且为 manager 类型、zone 真实可用，不默认继承父层配置。管理者配置不是 Bot 属性。父节点还应声明 `saved` 和 `handoff` 输出，要求子管理者在完成整个子 Run 后将 `save_bot` 的同名交付直接报告给父层；地址登记不是业务验收，父管理者仍须显式接受交接。Map 不声明不存在的全图输出接口。

最终 `saved` 是 v1 JSON envelope，包含公开接口完整读回的真实 `bot`、新 `revision` 和完整 `target.configuration`，不是旧的裸 Bot JSON；`handoff` 是 v1 saved 阶段 JSON，只记真实身份、normal Task／Map 唯一绑定、确认／授权依据、三字段来源、保存和非目标保持结论及未验证项。父层必须核对真实身份和绑定，不根据“完成”文字推断新能力已验证。必要时显式要求额外交付 baseline，但不通过 handoff 隐式寻址。

## 执行与授权

各节点按职责读取[共同业务材料契约](business-materials.md)，只执行所属阶段：

| 本图职责 | 必读章节与用途 |
| --- | --- |
| 全部材料生产者／消费者 | 「三种材料，直接交给消费者」定义 baseline／text／handoff；要求文件时另读「文件与独立 Chat」，核实材料写入授权并安全交付 |
| `create_blank` | 「目标核实与基线生产」承接目标调查、空白创建与单份 baseline 交付 |
| 三个 `write_*` | 「共同基线消费」「三个编写者与表达优化者」承接真实配置比较、所属全文编写确认与 text／handoff 交付 |
| 三个 `optimize_*` | 「共同基线消费」「表达优化」承接真实配置比较、保义与有效确认，仅交付新 text／handoff |
| `save_bot` | 「一次合法保存与失败核实」承接整批授权、精确基线比较、一次合法保存与完整读回及失败效果核实 |

固定创建与最终保存由两个编入后端的 Rust 能力执行，其间三个编写者和通用优化者仍是真实共享 Task；没有新增专属 Task。原通用替换 Task 继续服务修订和普通文件，不参与本图最终保存。稳定 ID 与格式边界见共同契约。通用表达优化者与保存者不默认携带 Bot 业务，故各自 node.input 必须显式提供适用章节、真实服务、字段、材料用途和权限；空白创建与三个编写者的 input 只选择本次分支、材料用途及文件交付。包装 Bot 指南供规划者选用、安排与准备业务输入，不注入执行 Chat，也不重抄执行流程或框架路由语法；父子交接由上节「调用接口」承接。

材料写入、字段全文确认、最终 Bot 保存授权分别核实；报告发送依实际 report_policy 和注入角色执行，manual 时另获原生 UI 对本次 payload 的批准。业务确认与报告批准互不替代，流程到达或 can_complete 不授予保存权限。创建目标、业务试运行、安装工具或扩权均不在本能力范围；配置交付不证明工具／Skill／扩展加载、模型遵从或业务效果。

## 创建 Task＋Bot 的选定外层入口

`create-bot-task-workflow--bc-61ce48b80a4c.json` 现在有四个工作单元：

1. `w1` → `create-task`：从本次 `Run.userInput` 和可选命名 `materials` 创建 normal Task。保留全文和执行配置确认，公开保存／完整读回；`handoff` 自包含地记录真实新 Task ID、需求、确认来源、权限及后续 Bot 创建委托。未明确的授权不得自行补齐。
2. `w2` → `optimize-prompt`：直接消费 `w1.handoff` 中的完整 prompt、已确认语义与授权，仅整理表达并保存 prompt，保留其他配置，不重做需求访谈。W1 记录后续保义优化及保存授权；授权不足、语义变化或冲突须先解决。`saved` 单独承载完整最新 Task 配置，`handoff` 作为子流程 `request`，必须是 v1 严格 JSON：真实 Task 定位、需求及三个约束数组、明确 authorization 布尔值、confirmation.evidence／scope 和可选 Bot 命名；不加 Schema 外的核对总结，不复制配置或正文。缺创建／材料授权阻塞，不推断自然语言，保存授权未知可明确 false。报告按各节点实际 report_policy 执行，manual 时由原生 UI 单独批准发送 payload；外层管理者核实真实报告后显式 complete。
3. `wrap_bot` → Map 型 `create-bot-wrapper`：本次明确使用 manager Task `task-18e2c2827c484128-0` 和 zone `hugosol/herdr-bot`（部署须先公开核实或显式修改本调用配置，不改 Bot 属性）。`request` 直接绑定 `w2.handoff`，`materials` 直接绑定外层同名可选输入；不依赖 `blocked_by`、父 Chat 或 handoff 的隐式地址传材料。子管理者核实并逐项 complete 子节点，完成整个子 Run 后通过真实 node report 将 `save_bot.saved`／`save_bot.handoff` 直接登记为父节点同名输出。父管理者另行核实真实 Bot ID、唯一 Task 绑定、完整保存读回与授权证据，显式 complete 本调用后交给 `optimize_bot`，此时不能 finish 外层；子 Run 完成、报告文字或 `can_complete` 都不是这个边界。

4. `optimize_bot` → `prompt优化者——Bot维度`：依赖 `wrap_bot`，显式接收其 `saved`／`handoff` 为 `bot_saved`／`bot_handoff`，并接收外层可选 `materials`。沿用有效需求与确认，查询服务最新配置，联合优化刚创建的 Task.prompt 与 Bot 三指南；展示整体方案、共享影响和完整候选，取得最终文本与保存授权。Skill／文档只读，保持业务语义、身份、绑定和执行配置，不运行目标能力。`saved` 交付完整读回的最终 Bot 与 Task 配置，`handoff` 交付修改说明、确认依据、保存核对结果及未验证项。采用 manual 报告且不自动关闭；保存确认不替代报告发送确认。父管理者验收并 complete 此节点后才可 finish 外层；阻塞或部分保存不算整体完成，保留对象，不自动删除或回滚。包装产物仅是阶段记录，最终注明“文本／定义优化完成，行为未验证”。

本次只更新仓库定义，不自动部署服务配置；其他创建 Bot 的入口不增加该节点。四节点定义的公开保存／读取及可选材料预检由 `workflow_dispatch::outer_create` 回归覆盖；真实外层验收入口已纳入最后的候选确认、交接及完成边界，但接线回归通过不代表新增优化节点的真实模型执行已验收。

根 Run 仍由调用者经 scheduler 选择自己的 manager Task、zone 和本次业务输入；`materials` 缺席合法，提供却不可读必须拒绝。仅移除了本外层原先内联的空白 Bot、三路指南、三路优化及最终保存分支；包装 Map 的八节点职责保持不变，不将 Task 创建移入包装。旧独立 `create-bot-task` 仍绑定 `task-18e315be1cf0ef10-0`，`create-bot` 仍绑定 `task-18e31d347ccbeb4c-4`，不改变其独立职责，不迁移其他 Map 或用户部署。

### 外层提示词归属

W1 负责业务定稿、执行配置确认与创建；W2 使用通用优化者的直接保存分支，只负责保义表达整理与授权保存。完整语义由 W1 确认，W2 沿用有效确认与授权；无法保持语义、确认失效或缺保存授权时才提出具体阻塞。报告发送按注入策略执行；manual 的原生 UI 确认与业务批准分开。

通用 Task 只承担保义优化、授权保存、安全写入和完整读回，不内置 Bot、三指南、baseline/text/handoff 或特定服务接口。仅修改 Task.prompt 前读取[Task 保存规则](usage.md#仅修改-taskprompt)；W2.input 只选择原文、目标、授权来源与交付用途。包装三路的目标／共同基线核实和材料分工由其已有 node.input 与共同材料契约承接；独立调用需显式提供适用的业务协议及材料。Bot 指南面向规划者，不承担执行规则注入。wrap_bot 保留子 finish → 父节点 report → 父 manager complete 的边界，不复制八节点执行手册。

W1.handoff 是完整 Task 创建交接；W2 的 saved 是完整最新配置，handoff 则是无需复制配置的包装 request。包装自行经服务核实目标，不沿 handoff 寻找未声明文件。包装三路表达优化仍使用共同 baseline／text／handoff，只交付文本而不保存 Bot；通用 Task 的直接保存分支不会改变这些调用的权限。

### 外层 Task 兼容与独立／workflow 配对清单

| 真实稳定 Task ID | 本链职责与接口修正 | 配对验收唯一 owner |
| --- | --- | --- |
| `task-18e31d30fe69a51c-2` | Task 创建者；POST 当前字段含至多一个 `suite`，不提交旧 `extensions`；按 tasks/options 列表取得真实 ID，再从 GET 的 task 完整读回；来源为 `backend/src/tasks/mod.rs` | 24（P45） |
| `task-18e31d34797f1464-3` | 通用提示词优化者的 Task.prompt 直接保存分支；仅表达整理，保留 suite 和全部非目标配置，独立 Chat 显式提供原文、服务与 Task 保存规则 | 24（P45，仅 Task 直接保存分支）；Bot 材料优化配对仍归 21 |

旧独立 `create-bot-task` 的 Task `task-18e315be1cf0ef10-0` 同样使用当前 `suite` 字段，不提交已退役的 `extensions`（包括空数组），并从 `backend/src/tasks/mod.rs` 核实接口；它仍只在当前 Chat 创建／复用 Task＋Bot，不转为 Map 包装或自动执行新能力。该旧入口兼容场景归 24（P46），不增加 P45 配对清单。

Task 创建者归 24；通用优化者按业务分支划分覆盖，24 负责 Task.prompt 直接保存，21 负责 Bot 指南材料优化，不重复认领。源码修订只纠正当前公共契约，不增加旧字段别名、兜底重试或失败吞没。外层真实业务发现的共享 `task-18e340f5287462ec-4` 职责修正仅禁止生成指南正文复制框架文件路由语法，保留真实材料、命名输入与授权要求；其独立／workflow 配对唯一 owner 仍为 21，不在本清单重复认领。完整真实外层调用与旧入口保留的 contract 验收由 24 拥有（P46）；本票公开定义接线和最小共享烟测不是这两项通过证据。

## 真实外层与旧独立入口验收

`python scripts/accept-workflow-nesting-outer.py` 是外层兼容专项，唯一拥有 Task 创建者／通用优化者的 Task.prompt 直接保存分支的独立 Chat 与实际外层 workflow 配对，以及旧 `create-bot-task`／`create-bot` 独立入口。它不重跑包装能力六个共享 Task 的独立配对，不认领规划选择，也不把定义接线回归当业务通过。

先按[测试指引](agents/testing.md)构建当前 worktree 的页面、`herdr-bot`、`herdr-fixture`、`herdr-paths` 和 `safe_shutdown_shell`，使用独立绝对 `CARGO_TARGET_DIR`；toolkit 使用其 worktree 的 Bun 1.4.4/frozen lock。依赖必须已安装。依赖文件是专用 `[roots]` TOML：`herdr`、`pi_package`、`omp_package`、`pi_agent`、`omp_agent`、`wayfinder`、`wayfinder_skills`、`pi_extensions`、`codebase_memory_mcp`；agent 根只读取明确的 Herdr 状态扩展，不读取用户认证或 settings。`native_shell` 可显式指向已安装的 Git Bash。不要使用用户业务 paths 或默认同级 checkout。源 Suite／固定扩展保持选择与职责，仅在独占部署中重定位其资源、connect-bot/workflow-chat 的回环端点及 model-cost 存储；授权验收模型另追加为 `probe`，不会覆盖原模型映射。

```powershell
python scripts/accept-workflow-nesting-outer.py `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --hosts declared `
  --paths-file 'D:/isolated-acceptance/outer-paths.toml' `
  --model-config 'D:/isolated-acceptance/model.json' `
  --evidence-dir 'D:/isolated-acceptance/new-exclusive-outer-run'
```

模型声明沿用[真实角色自动验收](agents/testing.md#文件交接真实角色自动验收)的专用 `provider/model/baseUrl/apiKeyEnv`，或同页公开 CompletionTransport；不读全局 auth，不用受控 provider 代替模型决定。`--hosts declared` 保留每个实际源 Task 的宿主（包含包装内原有 OMP/Pi 混用），逐项记录源码稳定 ID、隔离真实 ID、实际角色和 OS 身份。`--hosts omp,pi` 分别在两宿主执行完整矩阵，任一失败均非零；不能拿单宿主成功抵消另一个。隔离 Herdr terminal 使用已安装的 `powershell.exe`，避免完整 Suite 启动命令超过 `cmd.exe` 行长上限，不裁剪技能或扩展。该入口不安装、不提权、不改全局 PATH、不重试业务或模型决定。

专项先由真实独立 Chat 创建、优化一个仅配置交付的隔离 normal Task，再执行发布的三节点外层和八节点包装子 Run；外层创建者／通用优化者保持实际源 prompt，只使用明确业务要求与当前公开 HTTP。独立优化场景直接提供完整原配置、实际服务和 `docs/usage.md` 的“仅修改 Task.prompt”指针。目标使用非空 Suite，以完整前后字段比较检查优化者不丢失该选择。候选全文先写入业务授权的 proposal 文件并真实读回；自动业务核对使用预先固定的 CSV 只读职责、完整配置字面预期与真实模型独立语义评价，评价输入显式标注当前候选的 Task.prompt／指南／独立完整配置角色，不能靠场景名称猜职责；成功后在原生 Chat 发送明确全文／保存确认。manual 节点保存读回后再发送另一条报告发送确认，两种授权分开留痕；评价本身不是用户批准。本专项显式要求候选全文再批准，属于本次更严格验收条件，不证明默认已有授权时无需重复澄清的行为。模型自主选择和执行真实工具，driver 不手改运行记录、不伪造 Session，也不替管理者 dispatch／complete／finish。

证据必须同时具有真实 Task/Bot/Run/execution/Session 关联、原生工具及请求结果、命名 request／共同 baseline／实际输出的完整消费、真实文件字节、父子 finish/report/complete 顺序、目标全配置不变和无新目标业务运行。OMP 显式初始 Skill 的原始用户输入可能保存为 `attribution: user` 的原生 `custom_message`；核对完整输入时与普通 `role: user` 消息一起检查，绝不把 agent 归属的自定义消息当作用户输入。最终 `completed` 或 wrapper 文件存在均不充分。旧两个独立入口各自另外运行隔离场景；未迁移 Map 与共享执行 Task 逐项保持不变。保存前后、自动确认、报告确认、源码／锁文件／构建产物／安装宿主身份和原始错误均在本次独占目录保存，不将旧成功证据合并进新调用。

独占 HOME/APPDATA/XDG/原生存储、命名 Herdr、业务定义和运行目录；只用动态数值回环、关闭 mDNS，不执行真实关机。Suite 的真实 codebase-memory-mcp 保留，但可选 UI 关闭；已安装二进制必须支持公开 `CBM_RUNTIME_DIR`，并使用测试拥有的 owner-private runtime、cache 和 allowed-root，绝不连接用户 daemon。正常与失败均停止、等待并确认所有所属进程退出（包含脱离 stdio 客户端的所属 MCP daemon），原始验证错误和清理错误分开记录；隔离 credential-bearing models 文件会移除。回环 HTTP observer 对真实 Task `PUT` 与读取／创建请求一样原样转发方法、正文和状态，不自行代替服务返回；本地传输回归只证明透明转发，不充当优化业务通过。原生 turn 失败时另用公开 `herdr agent read` 保存所属 pane 的真实近期终端，原错误与捕获错误分别保留；不补发 prompt、Enter 或改变业务状态。缺任何宿主、授权模型、必要 Suite 能力、隔离或清理条件均保持未验证且非零。

PowerShell 中先设置 `$env:OUTER_ACCEPTANCE_TOOLKIT = $TOOLKIT_WT`，再执行 `python scripts/accept-workflow-nesting-outer.test.py`；这是 CLI fail-closed 回归，仅证明缺后端不宣称通过、已有证据不可覆盖；不证明上述真实业务已通过。运行说明描述可复跑入口及其边界，不是当前部署的成功声明；结果仅以实际本次逐覆盖项证据为准。
## 真实创建与配对验收入口

`python scripts/accept-workflow-nesting-create.py` 使用实际授权模型、已安装 Herdr 与 OMP／Pi，经公开独立 Chat、Run 和定义 HTTP 入口验收。`--hosts declared` 保留共享 Task 的实际混合宿主：表达优化者为 Pi，三个指南编写者和独立通用替换者为 OMP；创建图两脚本节点没有 Chat。不得为适配 runner 全改 Pi。`--hosts omp,pi` 另对两宿主各执行完整矩阵，任一失败不可由另一成功抵消。此入口不安装宿主、不改全局 PATH、不读取用户认证，不连接现有 Chat 或 daemon。

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

隔离后端通过显式 `paths_file` 加载本次 `paths.local.toml`，包括固定扩展使用的 `omp_agent`／`pi_agent` 根；只在文件中声明根而不接入后端配置不能证明解析成功。隔离 Herdr 的 terminal 使用已安装的 `powershell.exe`，避免完整 Suite 的真实技能与扩展命令超过 `cmd.exe` 输入行上限；不裁剪 Suite 规避长度限制。Chat 启动失败保留原始状态及 pane 原始文本，额外捕获失败单独记录，不覆盖原始错误。

完整固定扩展仍使用实际源码。仅在部署副本中将 connect-bot 指向本次动态后端，将 model-cost 存储指向本次独占目录。原生 codebase-memory-mcp 使用实际安装程序与完整工具能力，仅关闭无关可视化（`--ui=false`）；分别设置独占 `CBM_RUNTIME_DIR`、`CBM_CACHE_DIR` 和 `CBM_ALLOWED_ROOT`，runtime 目录限定为当前 Windows 所有者私有 ACL。缺这些已安装原生隔离能力则拒绝启动，不接入用户 account daemon。实际进程观察包含拥有进程的全部后代而非仅 OMP／Pi 白名单，持有并等待 MCP daemon 的进程句柄，退出失败仍为验收失败。

模型文件使用[测试指引](agents/testing.md#文件交接真实角色自动验收)中的专用 `provider/model/baseUrl/apiKeyEnv` 声明；可使用同处说明的公开 CompletionTransport，其职责只传输真实模型决策，不生成业务步骤或伪造工具结果。密钥仅取显式环境变量，不写请求头日志；结束时删除所有验收拥有的 credential-bearing models 文件。`--timeout 1..900` 是每阶段观察窗口，不是重试次数。

独立 normal Task 案例不提供可选材料，Map workflow 案例提供明确参考材料。独立 Chat 按原非 v1 显式材料协议验证共享 Task；创建图按 v1 验证。三个分支分别展示完整全文并取得真实确认，manual 报告另行通过真实原生 UI 发送同意。三编写者和表达优化者保留独立／workflow 实际消费记录；通用替换者参与原非 v1 独立材料与文件回归；创建验收最后复用修订入口的真实 task-no-materials 场景，核对共享编写者旧格式与原通用保存，再标 P28。修订入口仍可独立复跑。删除的空白创建 Task 不再配对，也不构造假 Chat 代替 Rust 执行；创建 Map 没有额外专属 Task。保存检查比较最终唯一 text 字节、真实 Bot 完整读回及全部非目标与目标配置；不以创建成功推断业务能力已经执行或验证。表达质量另由同一明确授权服务的独立实际请求按预先固定的目标真实性、字段职责、语义保持、授权边界和正文独立性 rubric 逐项评价，保留原始输入、回答与逐项理由；任一混合／缺证结果失败，评价结果不授予业务确认或保存授权。

独立保存安全场景经同一保存 Task 的真实 Chat 检查未授权、基线／目标冲突、实际运行使用、真实竞争 revision，以及保存已生效／未生效／未知效果。HTTP 故障只在实际传输边界断开观察或制造真实竞争编辑，不替换宿主、模型或产品结果；保留每次实际请求、原始错误与后续核实，不盲重试或自动回滚。普通文件使用真实 BOM／CRLF／非目标字节样例与真实隔离 junction／越界请求；没有链接能力仍为未验证，不能 skip 后当通过。

证据目录必须不存在，由 runner 独占创建。HOME／APPDATA／XDG、原生存储、临时目录、命名 Herdr、业务定义和运行目录均隔离，动态数值回环后端关闭 mDNS。原生存储使用额外独占短路径避免 Windows 历史文件长路径，结束后将不含凭据的副本归档到证据目录。持续消费 stdout／stderr，成功与失败均停止并等待所属进程，分别保存原错误和清理错误。证据包括两仓根／revision／修改身份、锁文件／产物身份、已安装宿主／OS 进程、原始输入／批准、模型与工具结果、HTTP、原生历史、实际文件字节、只读运行事实和逐覆盖项结论。缺真实依赖、模型授权、实际消费或清理成功时非零，所有未完成项保持未验证。

CLI fail-closed 回归：设置 `WORKFLOW_HANDOFF_TOOLKIT=$TOOLKIT_WT`，运行 `python scripts/accept-workflow-nesting-create.test.py`。它只检查缺构建／重复证据目录不会变成成功或启动服务，不证明实际业务矩阵通过。真实矩阵结果以运行保存的原始证据为准，本使用说明本身不宣称通过。

## 验证入口与证据边界

使用本 worktree 独立 `CARGO_TARGET_DIR`，先执行 `bun install --cwd=frontend --frozen-lockfile`、`npm --prefix frontend run build`；定向入口为：

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
