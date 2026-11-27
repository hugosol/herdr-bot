# 独立修订既有 Bot 四字段摘要

`revise-bot-guides` Bot 调用 [`enhance-existing-bot--bc-301b25d9f8d6.json`](../meta-data/workflow-maps/enhance-existing-bot--bc-301b25d9f8d6.json)。它只读核实既有 Bot，分别确认四份修订，再保存一次。不先优化 Task.prompt；如需修改目标 Task，应另行授权并在修改后重新核实基线。旧输入没有兼容别名，用户部署和历史不自动更新。

## 调用与交付

| 命名输入 | 必填 | 内容 |
| --- | --- | --- |
| `request` | 是 | 可读 UTF-8 委托：真实既有 Bot ID、业务需求、四字段修订范围及保存授权、资源权限、非目标、材料用途与写入范围；可附正式交付位置 |
| `materials` | 否 | 本次参考材料及用途；缺席合法，提供但不可读则拒绝，不制造空地址 |

只需真实 Bot ID，不要求用户抄完整 Bot、Task 或 Map。`userInput` 是本次背景，不替代 request。通过现有 scheduler 定义、预检及发起入口使用，或由父 Map 的一个节点引用此 Bot：填写本次 `manager_task_id` 和管理者 zone，以同名 `input_rule` 显式绑定文件。子管理者和业务文件不保存到 Bot，也不从父 Chat 隐式继承；调用 zone 不覆盖子 Map 内节点 zone。

父节点声明 `saved` 与 `handoff` 输出，要求子管理者在整个子 Run 完成后把 `w5` 的真实同名结果报告父层；父管理者仍须显式接受交接。子 Run 对父层的命名文件交付以实际声明的 Map 公开输出为准，不能依据内部节点名字推断。`saved` 是公开接口完整读回的裸 Bot 对象，不用 V1 或 V2 的 envelope。`handoff` 只记录身份、绑定、批准依据、字段来源、保存核对、部分效果和未验证项，不复制正文、基线或 saved，也不承担隐式寻址。若父层确需 baseline，应额外显式要求该交付。

## 修订接线与安全边界

全部业务材料、安全文件写入、授权、真实 revision 冲突、运行禁写与部分效果规则只遵循[共同业务材料契约](business-materials.md)，不引入另一套协议。

1. `w1` 只读核实委托、已有 Bot 与唯一目标，交付单份共同 baseline 和 handoff。不起草指南，不等待之后的全文确认或保存。未决业务取舍在该 Chat 内澄清；身份、权限或绑定冲突未解决时阻塞。
2. `w2`（when）、`w3`（description）、`w4`（input_rule）、`w6`（output_rule）各自直接取得同一 `w1.baseline`，在同一个 Chat 中独立修订、整理并确认所属全文。不读取其他字段定稿、不修改 Bot。各自交付唯一 text 和对应 handoff。
3. `w5` 直接读取共同 baseline 与四组最终 text 和 handoff，验证确认与授权覆盖整批，再按[一次合法保存与失败核实](business-materials.md#一次合法保存与失败核实)保存并完整读回。不要套用创建 V1 的五输入保存。

四个字段节点需要原生人工成果批准。只读基线和授权保存的节点按自动模式批准。业务确认、保存授权、成果批准、报告和管理者显式 complete 彼此独立。配置交付不授权执行业务、补建目标、安装或扩权，也不证明原生加载或业务质量。

## 专属 Task 与独立使用

`prepare-bot-revision` 使用 `task-revise-bot-baseline-v1`，只读核实既有 Bot、目标、需求和授权，交付共同基线。独立 Chat 可使用同一 Task，显式提供委托和材料用途；没有 workflow 注入时仍遵守[文件与独立 Chat](business-materials.md#文件与独立-chat)。

## 运行与验证

[共享材料启用检查](agents/testing.md#共享-bot-指南材料的启用检查)包含产品接线与 `check-revise-bot-guides` 隔离烟测。核对同一份直接 baseline、四组最终 text 与 handoff、一份完整保存以及缺失或不可读输入拒绝后定义不变。受控宿主不能证明原生批准或真实模型的材料消费。读取实际路径字符和完整原字节，核对原件与未获授权文件不变；退出零或最终状态不能代替证据。

## 真实模型跨仓修订验收

按[外部验收的配置和授权](agents/testing.md)设置 `$TESTER`、`$BOT`。此入口以实际宿主和授权模型运行修订流程，不覆盖创建场景或通用文件替换。使用当前产品 worktree 的产物及独占绝对构建目录。

```powershell
$TOOLKIT_WT = 'D:/worktrees/toolkit'
$env:CARGO_TARGET_DIR = 'D:/isolated-build/revise-acceptance-target'
Set-Location $BOT
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --example safe_shutdown_shell
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-workflow-nesting-revise `
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

修订入口分别为 Task／Map 目标执行有／无可选材料的四次完整修订；专属 `task-revise-bot-baseline-v1` 另有独立 Chat 核实，与 workflow 消费配对。用户真实澄清“扫描”范围，四路逐字全文批准分别在 Chat 输入、记录；四个人工编写节点须通过真实原生 UI 的 Yes 确认成果，记录实际批准与 report HTTP 的独立事实，不把聊天文字或报告当作成果批准；旧报告发送确认不再适用，S1 不替代对应原生验收。每个直接消费者须有成功原生 read 和完整工具结果；共同基线、唯一正文、必要 handoff 逐个核对，不从 completed 推断过程。保存前后完整目标和全部非目标保持，一次真实四字段保存、真实 revision、逐字段实际批准版本及完整 saved 读回共同构成成功证据。固定业务质量量表逐项评价目标真实性、字段职责、语义保持、授权边界和来源完整性；混合或缺证据不算通过。

最小诊断可在同一入口增加 `--cases task-no-materials`，或用逗号选择 `dedicated-chat`、`task-materials`、`map-no-materials`、`map-materials` 及 `--help` 列出的独立安全 case；默认 `--cases all` 才运行完整矩阵。空项、重复项及未知 case 在启动前拒绝。过滤执行只运行所选 case，不自动补跑；成功时退出码为 0、`subsetPassed=true`，但 `passed=false`、`verifiedHosts=[]`，全部 promise coverage 仍未验证。`requestedCases` 与 `filtered` 标明证据范围；完整验收不得以子集退出码代替。真实模型的职责遵循只能由所选真实业务运行核实，CLI 离线回归不证明提示有效。

负例分别核对未授权、基线或目标冲突、运行禁写、实际竞争 revision，以及保存响应丢失后的已生效、未生效和未知三种事实。故障只作用于真实 HTTP 边界，不替代后端或模型结果；不盲目重试或回滚。文件负例使用本例自有的 junction 和越界路径，正例逐个原生读回真实材料。失败不能由其他场景成功抵消；模型传输或清理失败后不补跑，剩余项标为未验证。

证据目录须尚不存在。保留本次原始确认、实际工具与 HTTP、文件字节和逐场景结果；区分原始失败与清理错误，确认所属进程退出并移除含凭据的临时模型配置。缺依赖、授权、真实证据或清理失败不判通过。
