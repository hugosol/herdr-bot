# 为既有目标包装完整 Bot

`create-bot-wrapper` Bot 调用 [`create-bot-wrapper.json`](../meta-data/workflow-maps/create-bot-wrapper.json)。从 scheduler 预检和发起，或由外层 Map 的节点引用。只为既有 normal Task 或 Map 创建 Bot，不新建、修改或执行该目标。旧独立 `create-bot-task` 和 `create-bot` 保持各自职责，不自动切换为包装流程。

## 创建分支表示的交付边界

材料使用[创建 V1 契约](business-materials.md#创建分支-v1可校验表示与空白创建)。最终保存只读共同 baseline 和四份命名 text，不消费字段 handoff 或批准凭据。[V2](contracts/create-bot-v2-materials.md) 和[修订](revise-bot-guides.md) 使用各自协议，不可替代。自定义旧部署按[脚本更新说明](usage.md#脚本空白-bot-创建)显式更新，不能恢复历史脚本执行或自动重写 Run。

## 调用接口

输入要求的唯一权威来源为 [`create-bot-wrapper.json`](../meta-data/workflow-maps/create-bot-wrapper.json) 的 `map_input_rule.description` 与各项 `map_input_rule.files[].description`；scheduler 发起页分别展示非空总体要求与对应文件标签，Bot 能力摘要与调用者不另存完整要求。文件接口仍为必需 `request` 与可选 `materials`。

启动管理者时可暂不提供 request，用户可先在本次要求或管理者对话中说明委托、提供文件。管理者读取共同输入要求和已有材料，澄清语义与授权，必要时在获准范围内整理为 request 文件，以 `prepare_inputs` 全量绑定并确认后才能派发任何业务节点；确认时 request 必须齐全、可读且语义满足要求，不能以文件存在代替核实。嵌套子管理者同样先完成输入准备；父节点通过 `input_rule` 将已有 request／materials 以同名输入显式绑定，作为子 Run 的准备初值，并填写本调用的 `manager_task_id`、zone 和自包含业务 input；发起前通过公开接口核实所选 Task 真实存在且为 manager 类型、zone 真实可用，不默认继承父层配置。管理者配置不是 Bot 属性。子 Map 公开 `saved` 和 `handoff`。子管理者以 `save_bot` 的同名交付登记本次 `finish` 的公开文件；父节点按需声明同名输出子集，正式报告由系统从已完成子 Run 取文件地址，不由报告者重复登记或由父层从内部节点寻址。地址登记不是业务验收，父管理者仍须显式接受交接。

最终 `saved` 是 v1 JSON envelope，包含公开接口完整读回的真实 `bot`、新 `revision` 和完整 `target.configuration`，不是旧的裸 Bot JSON；`handoff` 是 v1 saved 阶段 JSON，只记真实身份、normal Task／Map 唯一绑定、共同 baseline 与四份正文来源、保存和非目标保持结论及未验证项，不携带批准凭据。父层必须核对真实身份和绑定，不根据“完成”文字推断新能力已验证。必要时显式要求额外交付 baseline，但不通过 handoff 隐式寻址。

## 执行与授权

编写者和优化者只负责自己的字段，按 [共同材料契约](business-materials.md)消费同一基线并交付 text 与 handoff。四个字段的完整正文分别由用户确认；表达优化只整理已确认的意思。创建和保存由 TypeScript 脚本执行，没有 Chat，不使用修订的通用替换者。结构化 Map 决定具体节点、命名接线与批准模式。Task.prompt 负责稳定步骤，node.input 只说明本次分支、材料用途、字段和权限；Bot 能力摘要只供规划者使用。

成果批准与合法业务保存、报告和管理者显式完成独立。批准或 `canComplete:true` 不能证明基线、revision 或目标正确。材料写入只限于本次授权；不创建或执行业务目标，不安装工具、不扩权。配置交付不证明实际资源加载、模型遵从或业务效果。

## 创建 Task＋Bot 的选定外层入口

[`create-bot-task-workflow--bc-61ce48b80a4c.json`](../meta-data/workflow-maps/create-bot-task-workflow--bc-61ce48b80a4c.json) 依次交接 `w1`、`w2`、`wrap_bot` 和 `optimize_bot`。各节点绑定、manager Task 与 zone 以发布定义及实际部署为准。

1. `w1` → `create-task`：从本次 `Run.userInput` 和可选命名 `materials` 创建 normal Task。保留全文和执行配置确认，公开保存／完整读回；`handoff` 自包含地记录真实新 Task ID、需求、确认来源、权限及后续 Bot 创建委托。未明确的授权不得自行补齐。
2. `w2` → `optimize-prompt`：直接消费 `w1.handoff` 中的完整 prompt、已确认语义与授权，仅整理表达并保存 prompt，保留其他配置，不重做需求访谈。W1 记录后续保义优化及保存授权；授权不足、语义变化或冲突须先解决。`saved` 单独承载完整最新 Task 配置，`handoff` 作为子流程 `request`，必须是 v1 严格 JSON：真实 Task 定位、需求及三个约束数组和可选 Bot 命名；不加 Schema 外的核对总结，不复制配置或正文，不生产 authorization、confirmation 或任何改名批准凭据。request／共同基线／目标字段归属见共同业务材料契约；不将执行层成果批准嵌入业务材料。报告可自由发送，成果按各节点 auto_approved 独立批准；外层管理者核实最新报告与交付后显式 complete。
3. `wrap_bot` → Map 型 `create-bot-wrapper`：部署前从该节点读取并核实明确配置的 manager Task 和 zone；需更换时修改本次调用，不改 Bot 属性。`request` 直接绑定 `w2.handoff`，`materials` 直接绑定外层同名可选输入；不依赖 `blocked_by`、父 Chat 或 handoff 的隐式地址传材料。子管理者核实并逐项 complete 子节点，以 `save_bot.saved`／`save_bot.handoff` 作为公开文件 finish 子 Run；正式 node report 由系统取得已登记的同名文件。父管理者另行核实真实 Bot ID、唯一 Task 绑定、完整保存读回与执行层批准事实，显式 complete 本调用后交给 `optimize_bot`，此时不能 finish 外层；子 Run 完成、报告文字或 `can_complete` 都不是这个边界。

4. `optimize_bot` → `prompt优化者——Bot维度`：依赖 `wrap_bot`，显式接收其 `saved`／`handoff` 为 `bot_saved`／`bot_handoff`，并接收外层可选 `materials`。沿用有效需求与确认，查询服务最新配置，联合优化刚创建的 Task.prompt 与 Bot 四字段摘要；展示整体方案、共享影响和完整候选，取得最终文本与保存授权。Skill／文档只读，保持业务语义、身份、绑定和执行配置，不运行目标能力。`saved` 交付完整读回的最终 Bot 与 Task 配置，`handoff` 交付修改说明、确认依据、保存核对结果及未验证项。采用人工成果批准且不自动关闭；报告发送不要求批准，已批准仍须管理者显式完成交接。父管理者验收并 complete 此节点后才可 finish 外层；阻塞或部分保存不算整体完成，保留对象，不自动删除或回滚。包装产物仅是阶段记录，最终注明“文本／定义优化完成，行为未验证”。

根 Run 由调用者选择自己的 manager Task、zone 和本次输入。`materials` 缺席合法，提供却不可读须拒绝。W2 仅获准改 prompt 时遵守[仅修改 Task.prompt](usage.md#仅修改-taskprompt)，不得丢失 Suite 或其他非目标。包装子图只负责 Bot 创建，不负责父层 Task 创建或最终联合优化。

## 真实外层与旧独立入口验收

按[测试指引](agents/testing.md)显式设置 `$TESTER`、`$BOT`。`accept-workflow-nesting-outer` 覆盖独立 Task 创建与 prompt 保存、真实外层调用和旧独立入口，不替代创建包装内部的真实编写者配对或规划验收。

先按[测试指引](agents/testing.md)构建当前 worktree 的页面、`herdr-bot`、`herdr-fixture`、`herdr-paths` 和 `safe_shutdown_shell`，使用独立绝对 `CARGO_TARGET_DIR`；toolkit 使用其 worktree 的 Bun 1.4.4/frozen lock。依赖必须已安装。依赖文件是专用 `[roots]` TOML：`herdr`、`pi_package`、`omp_package`、`pi_agent`、`omp_agent`、`wayfinder`、`wayfinder_skills`、`pi_extensions`、`codebase_memory_mcp`；agent 根只读取明确的 Herdr 状态扩展，不读取用户认证或 settings。`native_shell` 可显式指向已安装的 Git Bash。不要使用用户业务 paths 或默认同级 checkout。源 Suite／固定扩展保持选择与职责，仅在独占部署中重定位其资源、connect-bot/workflow-chat 的回环端点及 model-cost 存储；授权验收模型另追加为 `probe`，不会覆盖原模型映射。

```powershell
$TOOLKIT_WT = 'D:/worktrees/toolkit'
$env:CARGO_TARGET_DIR = 'D:/isolated-build/outer-acceptance-target'
Set-Location $BOT
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-nesting-outer `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --hosts declared `
  --paths-file 'D:/isolated-acceptance/outer-paths.toml' `
  --model-config 'D:/isolated-acceptance/model.json' `
  --evidence-dir 'D:/isolated-acceptance/new-exclusive-outer-run'
```

模型声明沿用[真实角色自动验收](agents/testing.md#文件交接真实角色自动验收)的专用 `provider/model/baseUrl/apiKeyEnv`，或同页公开 CompletionTransport；不读全局 auth，不用受控 provider 代替模型决定。`--hosts declared` 保留每个实际源 Task 的宿主（包含包装内原有 OMP/Pi 混用），逐项记录源码稳定 ID、隔离真实 ID、实际角色和 OS 身份。`--hosts omp,pi` 分别在两宿主执行完整矩阵，任一失败均非零；不能拿单宿主成功抵消另一个。隔离 Herdr terminal 使用已安装的 `powershell.exe`，避免完整 Suite 启动命令超过 `cmd.exe` 行长上限，不裁剪技能或扩展。该入口不安装、不提权、不改全局 PATH、不重试业务或模型决定。

专项分别运行真实独立 Task 创建、Task.prompt 保义保存、发布外层及其包装子 Run，以及旧独立 `create-bot-task` 和 `create-bot` 入口。外层须验收 `optimize_bot` 的完整候选与最终保存，不能在 `wrap_bot` 后结束。预先确定 CSV 只读职责及完整配置预期；目标使用非空 Suite，逐项比较完整前后配置。独立语义评价明确区分 Task.prompt、四字段摘要和完整配置，不能代替原生全文／保存确认或人工成果批准。模型自主使用真实工具，验收者不代填运行记录、不伪造身份、不替管理者推进。

验收真实 Task、Bot、Run、execution 与 Session 关联，核对原生读取了命名 request、共同 baseline 和实际产物，父子 finish、report、complete 顺序正确。包装期间目标全配置不变；最终优化只允许获准的 Task.prompt 与 Bot 四字段摘要变更。无关 Map、共享执行 Task 和所有非目标字段须保持原样，且不运行新目标业务。OMP 可将显式初始 Skill 的用户输入保存在 `attribution: user` 的 `custom_message`，应与 `role: user` 一并核对，不把 agent 消息算成用户输入。

运行前按[测试隔离规则](agents/testing.md)准备独占宿主和存储、数值回环与无关机配置。保留完整 Suite，并用隔离的 MCP runtime、cache 和 allowed-root，不连接用户 daemon。原始失败与清理错误分开保存；缺原生能力、授权或清理证据不得判通过。

## 真实创建与配对验收入口

按[测试指引](agents/testing.md)设置 `$TESTER` 与 `$BOT`，另将 `$TOOLKIT_WT` 指向参与的 Toolkit worktree。使用真实授权模型和安装版 Herdr、OMP／Pi。`--hosts declared` 保留当前发布 Task 的混合宿主；`--hosts omp,pi` 分别验收两宿主，一端成功不能抵消另一端失败。不读取用户认证、已有 Chat 或 daemon。

```powershell
$env:CARGO_TARGET_DIR = 'D:/isolated-build/create-acceptance-target'
Set-Location $BOT
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --example safe_shutdown_shell
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-nesting-create `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --hosts declared `
  --paths-file 'D:/isolated-dependencies/create-paths.toml' `
  --model-config 'D:/isolated-dependencies/authorized-model.json' `
  --evidence-dir 'D:/isolated-acceptance/new-exclusive-create-run' `
  --shell-path 'D:/Program Files/Git/bin/bash.exe'
```

`--paths-file` 使用外层专项所列的同一组 `[roots]`，只用于指定安装与只读资源；服务配置必须实际引用隔离根文件。`--toolkit` 显式指定本次 Toolkit 源码。保留完整 Suite、Task.suite、初始 Skill 和固定扩展，只在隔离部署重定位服务地址与写入位置。使用 `powershell.exe` 作为 Herdr terminal 避免 `cmd.exe` 命令行长度限制，不通过删资源来启动。原生 codebase-memory-mcp 必须支持隔离 `CBM_RUNTIME_DIR`、`CBM_CACHE_DIR` 和 `CBM_ALLOWED_ROOT`，runtime 设为所有者私有；缺隔离能力不启动，不连接用户 daemon。

模型文件使用[测试指引](agents/testing.md#文件交接真实角色自动验收)中的专用 `provider/model/baseUrl/apiKeyEnv` 声明；可使用同处说明的公开 CompletionTransport，其职责只传输真实模型决策，不生成业务步骤或伪造工具结果。密钥仅取显式环境变量，不写请求头日志；结束时删除所有验收拥有的 credential-bearing models 文件。`--timeout 1..900` 是每阶段观察窗口，不是重试次数。

保留 normal Task 与 Map 两类目标、有与无参考材料、实际独立 Chat 与 workflow 消费的区别。独立 Chat 使用非 V1 协议；包装图使用 V1，两个脚本节点无 Chat。四字段正文分别呈现全文、取得原生成果确认并独立发送报告。逐字节比较最终 text 和完整 Bot 读回，并核实所有非目标字段及目标配置不变。独立质量评价按事先确定的目标真实性、字段职责、语义保持、授权边界和正文独立性逐项判定；不授予保存权限。通用替换者的独立与修订材料仍按非 V1 协议检查。

独立保存安全场景经同一保存 Task 的真实 Chat 检查未授权、基线／目标冲突、实际运行使用、真实竞争 revision，以及保存已生效／未生效／未知效果。HTTP 故障只在实际传输边界断开观察或制造真实竞争编辑，不替换宿主、模型或产品结果；保留每次实际请求、原始错误与后续核实，不盲重试或自动回滚。普通文件使用真实 BOM／CRLF／非目标字节样例与真实隔离 junction／越界请求；没有链接能力仍为未验证，不能 skip 后当通过。

`--evidence-dir` 必须尚不存在。原生存储用独占短路径，保留真实确认、原生读取、HTTP、原始字节和逐项结果，不保存凭据。确认所属进程退出，分开保留原始失败和清理错误。缺安装、模型授权、实际消费或清理证据，不得判通过。

## 验证入口与证据边界

[共享材料启用检查](agents/testing.md#共享-bot-指南材料的启用检查)覆盖定义读取、输入预检、真实文件和保存冲突。预检拒绝后须读回原定义确认没有副作用；仅看到 HTTP 错误不足以证明。受控宿主不证明原生批准或模型消费。真实验收须核对文件实际路径字符、原始字节、配置完整读回及未经授权的文件不变；进程退出零、模型自述或 `completed` 都不替代这些证据。
