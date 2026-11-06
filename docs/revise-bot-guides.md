# 独立修订既有 Bot 三指南

公开能力为 `revise-bot-guides` Bot，唯一绑定现有文件 `enhance-existing-bot--bc-301b25d9f8d6.json`。该 Map 已原位切换为“只读核实 → 三路修订确认 → 一次保存”，不再先优化 Task.prompt。没有旧输入字段别名或自动兼容分支；已保存的用户部署、运行中定义与历史记录不会自动迁移。旧独立 Task.prompt 优化者、create-bot／create-bot-task 及无关 Map 不变；确需优化目标 Task 时，可在适用外层工作中单独安排，并在修改完成后重新核实修订基线。

## 调用与交付

| 命名输入 | 必填 | 内容 |
| --- | --- | --- |
| `request` | 是 | 可读 UTF-8 委托：真实既有 Bot ID、业务需求、三字段修订范围及保存授权、资源权限、非目标、材料用途与写入范围；可附正式交付位置 |
| `materials` | 否 | 本次参考材料及用途；缺席合法，提供但不可读则拒绝，不制造空地址 |

只需真实 Bot ID，不要求用户抄完整 Bot、Task 或 Map。`userInput` 是本次背景，不替代 request。通过现有 scheduler 定义、预检及发起入口使用，或由父 Map 的一个节点引用此 Bot：填写本次 `manager_task_id` 和管理者 zone，以同名 `input_rule` 显式绑定文件。子管理者和业务文件不保存到 Bot，也不从父 Chat 隐式继承；调用 zone 不覆盖子 Map 内节点 zone。

父节点声明 `saved` 与 `handoff` 输出，要求子管理者在整个子 Run 完成后把 `w5` 的真实同名结果报告父层；父管理者仍须显式接受交接。Map 不声明不存在的全图输出接口。`saved` 是公开接口完整读回的真实 Bot 对象；handoff 只记录身份、绑定、批准依据、字段来源、保存／读回／非目标核对结果、已发生效果及未验证项，不复制正文、基线或 saved，也不承担隐式寻址。若父层确需 baseline，应额外显式要求该交付。

## 五项工作与安全边界

全部业务材料、安全文件写入、授权、真实 revision 冲突、运行禁写与部分效果规则只遵循[共同业务材料契约](business-materials.md)，不引入另一套协议。

1. `w1` 使用 `prepare-bot-revision`：直接读取 request 与实际提供的 materials，自行经公开接口读取目录、完整 Bot 和真实 revision，核实唯一绑定的 normal Task 或 Map 及完整目标配置。未知事实自行调查，未决业务取舍用 grill-with-docs 在当前 Chat 真实澄清；身份、职责、授权或绑定冲突未解决则阻塞。仅保存单份 baseline 和必要 handoff，完整读回；不创建或修改目标或 Bot，不优化 Task.prompt。
2. `w2`／`w3`／`w4` 分别使用既有 when／plan_guide／node_guide 编写者，直接绑定同一 `w1.baseline`，独立修订、整理并确认所属全文，不读取兄弟定稿。修订案例在同一 Chat 整理表达，不新增独立优化阶段。各自产出唯一 text 与必要 handoff，不写 Bot。
3. `w5` 使用既有文本替换者，直接绑定共同 baseline 与三组 text／handoff。真实批准来源必须覆盖整个保存批次；流程到达、报告发送确认或 `can_complete` 不授予保存权。重读完整 Bot、真实 revision 和完整目标，与 baseline 精确比较。任何缺项、冲突或禁写条件都使整批不开始，不自动合并、重试、回滚或替换对象。
4. 合法时一次 edit/revision/save 保存三个字段，原样使用各 text，保持 id、name、唯一绑定及全部非目标；公开完整读回后逐字段核对，再重读完整目标确认未变。失败或结果不明先核实现场并保留原错误与已生效／未生效／无法确认效果，不吞掉失败。

`w1` 和三编写者为 manual 报告：业务决定、全文确认、保存授权及报告发送确认分别成立，管理者仍负责验收。配置交付不授权业务运行、目标补建、安装、扩权或自身运行配置修改，也不证明目标工具／Skill／扩展加载和真实业务质量。

## 专属 Task 与验收归属

| 稳定 Task ID | Bot | 用途 | 独立 Chat／workflow 配对 owner |
| --- | --- | --- | --- |
| `task-revise-bot-baseline-v1` | `prepare-bot-revision` | 只读核实既有 Bot、真实目标、需求与授权，交付共同基线 | 22 |

三个编写者与保存者沿用[共同契约的既有稳定 Task](business-materials.md#稳定-task-清单与验证边界)，本票不再修改共享 Task 协议。专属 Task 同样可独立使用：显式提供委托和材料用途，缺少必要业务决定就澄清；要求文件但无位置／授权时先澄清，不因缺少 workflow 注入丢失业务安全职责。

## 运行与验证

使用本 worktree 的独立 `CARGO_TARGET_DIR`，先生成当前静态页面并构建安全宿主：

```powershell
npm --prefix frontend ci
npm --prefix frontend run build
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch revise_bot_guides -- --nocapture
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch shared_business -- --nocapture
cargo build --manifest-path backend/Cargo.toml --bin herdr-fixture --example safe_shutdown_shell
python scripts/check-revise-bot-guides.py --target-dir $env:CARGO_TARGET_DIR
node scripts/test-backend.mjs
```

`revise_bot_guides` 是启用接线检查：通过公开定义保存／读取及预检消费五节点、同一直接 baseline、三路独立确认与七材料汇合，验证必填缺失、可选缺席／可读及不可读路径拒绝，不固定提示词措辞。`shared_business` 维护 Task／Map 两种绑定的一次保存、真实 stale revision、完整读回及非目标保持边界，本能力不复制 contract 测试所有权。

独立安全后端烟测 `check-revise-bot-guides.py` 实际经 HTTP 保存、读回专属 Task、Map 输入／五节点与 Map 型包装 Bot，用真实临时 request／materials 验证缺席、存在、缺失及 Windows 独占读锁拒绝；核对拒绝不改变定义，无 Run 或宿主调用。每次保留独占目录下的 evidence.json、HTTP 请求响应及持续消费的进程日志，正常和失败均终止并等待所属安全后端及日志线程。使用受控资源只证明公开接线，不证明部署 Suite 或原生角色加载。

最小共享 Task 原生调用沿用[共享材料启用检查](agents/testing.md#共享-bot-指南材料的启用检查)：安装版 Pi、公开 CompletionTransport、隔离 HOME／APPDATA／XDG／sessions，加载实际共享 Task.prompt，真实读取源材料、写入唯一 text／handoff 并完整读回，核对原文和未授权哨兵不变。保留实际授权、原生 read／write 及准确文件字符；进程零退出、模型自称成功或静态定义存在均不充分。该窄烟测不是完整五节点自主执行。发布时实际用安装版 Pi 与 `task-18e31d34797f1464-3` 原始 prompt 执行独立 Chat：真实读取 writing-for-agents 与源文件，真实执行安全文件写入，再直接完整读取 text／handoff；独立核对来源准确、正文不重复、源文件与未授权哨兵不变。首次传输的真实模型输出混入 JSON 外文字，记录了 HTTP 502 与原生 stopReason:error，因此该次不判通过，即使 Pi 自动继续并生成了材料；保留原始失败材料。第二次在全新隔离目录仅对公开 completion 使用 JSON schema 约束序列化，不给定工具选择或业务答案，五次实际模型响应、原生真实工具与完整读回均成功，无原生或传输错误。两次均终止并等待所属原生进程、关闭传输并移除隔离模型配置。此结果不证明专属基线核实 Task 的自主质量或 22 的配对／完整修订验收。

## 真实模型跨仓修订验收

从 herdr-bot worktree 运行 `python scripts/accept-workflow-nesting-revise.py`。入口复用公开 Chat／Run／HTTP、原生历史与真实宿主隔离设施，但使用本节修订场景和五节点定义，不运行创建场景或普通文件替换兼容用例。

```powershell
$env:CARGO_TARGET_DIR = Join-Path $HERDR_BOT_WT 'backend/target'
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --example safe_shutdown_shell
python scripts/accept-workflow-nesting-revise.py `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --hosts declared `
  --paths-file 'D:/isolated-acceptance/paths.toml' `
  --model-config 'D:/isolated-acceptance/model.json' `
  --evidence-dir 'D:/isolated-acceptance/new-exclusive-revise-run' `
  --shell-path 'D:/Program Files/Git/bin/bash.exe'
```

依赖均须预先安装；不安装、不提权、不修改全局 PATH、不读取用户认证。paths TOML 通过 herdr-paths 解析 `[roots]`，须明确声明 `herdr`、`pi_package`、`omp_package`、`pi_agent`、`omp_agent`、`wayfinder`、`wayfinder_skills`、`pi_extensions`、`codebase_memory_mcp`。agent 根仅用于读取所声明的状态扩展源文件，不复制用户状态。原生 MCP 须支持公开 `CBM_RUNTIME_DIR`／`CBM_CACHE_DIR`／`CBM_ALLOWED_ROOT` 隔离选择；缺能力即失败，不连接用户 daemon。模型 JSON 的 `provider`、`model`、`baseUrl`、`apiKeyEnv` 必填，密钥只来自显式命名的环境变量；可选 `contextWindow`、`maxTokens`、`reasoning`。只支持真实 OpenAI-compatible chat-completions 服务或[公开 CompletionTransport](agents/testing.md#文件交接真实角色自动验收)，不以确定响应选择业务步骤。该模型传输证据不外推为原生 provider wire 合规性。

公开 CompletionTransport 为 stateless：每轮重新发送完整原生历史和可用工具，业务 agent 的自主工具／子代理调用也可能增加请求，不能把一次 CLI 调用当作一次模型调用。`model=default` 仅是公开声明，不暴露实际 resolved provider/model 或 usage／账单，禁止推测它等于某个具体模型。运行时须保留请求数量与原始失败；不要将多个昂贵矩阵包装成没有中间反馈的长批次。用户因时长或额度要求中止时，区分外部终止与自然完成，清理并保持未完成矩阵未验证，不为补齐结果自动重跑。

`--hosts declared` 保留实际发布 Task 的混合宿主和 Suite／初始 Skill；只重定位隔离资源和授权模型。`--hosts omp,pi` 则分别执行两套完整矩阵，两者都通过才通过，单宿主成功不能抵消另一宿主失败。HOME／APPDATA／XDG／原生存储、命名 Herdr、定义与运行目录独占，服务只监听动态数值回环、关闭 mDNS，不连接用户 Chat、不执行真实关机。

修订入口分别为 Task／Map 目标执行有／无可选材料的四次完整修订；专属 `task-revise-bot-baseline-v1` 另有独立 Chat 核实，与 workflow 消费配对。用户真实澄清“扫描”范围，三路逐字全文批准与四个 manual 报告发送批准分别输入、记录。每个直接消费者须有成功原生 read 和完整工具结果；共同基线、唯一正文、必要 handoff 逐个核对，不从 completed 推断过程。保存前后完整目标和全部非目标保持，一次真实三字段保存、真实 revision、逐字段实际批准版本及完整 saved 读回共同构成成功证据。固定业务质量量表逐项评价目标真实性、字段职责、语义保持、授权边界和来源完整性；混合或缺证据不算通过。

修订负例单独命名：未授权、基线／目标冲突、运行禁写、真实并发 revision 冲突、保存响应丢失后的已生效／未生效／未知。传输故障只作用于真实 HTTP 边界，不代替模型、宿主或后端决定；保存错误不盲重试、不回滚。独立业务场景各执行一次，失败逐项保留，不用其他场景成功抵消；一旦模型传输或所属资源清理已失败，余下不再提交并明确保持未验证。文件安全负例让专属基线核实 Task 面对实际 junction 和越界输出；正例实际安全生成 baseline／text／handoff／saved 并逐一原生完整读回，保留文件原字节和来源。这不是共享文本替换 Task 的普通文件兼容用例副本。六个已有共享 Task 的独立／workflow 配对不在此重认领，但本入口核对修订链实际 Task 身份和材料消费。

证据目录必须不存在。保留两仓源码／锁文件／页面／二进制身份、实际安装入口和 OS 进程、模型服务身份（无密钥）、原始用户输入和确认、模型／HTTP／工具请求结果、原生历史、文件字节、只读运行事实和逐覆盖项结论。成功和失败都停止、等待并核实所属进程退出，原始验证错误与清理错误分开保留；移除含密钥的临时模型配置。缺依赖、宿主能力、模型权限或清理失败均非零、保持未验证。CLI 的缺依赖隔离回归为设置 `WORKFLOW_HANDOFF_TOOLKIT=$TOOLKIT_WT` 后运行 `python scripts/accept-workflow-nesting-revise.test.py`，只证明拒绝与证据边界，不证明真实业务成功。

完整 P40／P42／P43／P44／P45 的 Task 型与 Map 型目标、有／无材料、真实澄清、全部负例与专属 Task 配对由修订专项唯一负责；启用检查不认领这些 contract 场景。外层真实调用使用独立专项。入口存在、准备完成、受控响应或单次最终状态均不是完整通过，必须以实际运行证据判断。
