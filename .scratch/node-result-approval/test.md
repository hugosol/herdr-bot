# Node 结果批准：测试交接

## Ticket 15：测试取舍

### 本轮范围

真实模型验收以用户明确指令触发宿主原生工具并得到可核对结果为止。保留真实原生确认及工具回执，不追跑依赖模型自主编排的材料生成、报告、manager 操作和父子 Run 交接。不代调工具、不伪造前置状态，也不把部分结果计成完整通过。

不在 `native_yes()` 重复添加 `approved` 前后读回断言。后端 HTTP 集成测试 [`approval_without_report_or_artifact_only_changes_the_approval_fact`](../../backend/tests/workflow/reports/approval.rs) 已覆盖人工及自动模式下同一执行的 `approved: false → true` 和批准来源；两宿主的真实工具调用、确认和公开状态读回由 [Ticket 10](implementation/10-native-approval-report-acceptance.md) 的 [`accept-node-approval-report.py`](../../scripts/accept-node-approval-report.py) 专项负责。

### 决策依据

- **OMP**：`create_task` 请求一次确认 Q1–Q8，未提交报告；驱动未代做合并授权，Run 超时。[运行证据](evidence/ticket15-followup-omp-20261118T132733454575Z/business/omp/evidence.json)
- **Pi**：原生 Yes 返回 `true`，同一执行取得人工批准回执；但没有交付 `request`／`handoff`，最新报告仍为 `canComplete=false`，manager 的 `complete` 被拒绝。[保留的传输记录](evidence/ticket15-followup-pi-20261118T132733454575Z/completion-wire/)

这些是本轮 `gpt-6-luna` 的实际结果，不代表该模型永远无法推进，也不代表批准失败。

### 已验证的简单场景

| 场景 | OMP | Pi |
| --- | --- | --- |
| 发布配置页、发起真实 V2 Run | 通过 | 通过 |
| 原生 MCP `list_projects` | 成功返回空列表；未建索引 | 成功返回空列表；未建索引 |
| 原生人工批准 | 独立用例 `bc-90b8be180a92`：Yes 后公开读回同一执行 `bc-712e6971fac6` 为 `approved=true`、`approvalSource=manual`，没有生成报告 | 真实 V2 Run `bc-595af5004f0a`：原生 Yes 返回 `true`，执行 `bc-965a87a9bf5e` 的批准回执为 HTTP 200、`approved=true`、`changed=true`、来源 `manual` |
| 原生进度报告 | 独立用例 `bc-b5a2eed16e01`：保存报告 `bc-5facfb9ac3b4`，`canComplete=false`，执行仍未批准 | HTTP 200、`saved=true`、`canComplete=false`，未提交输出 |

OMP 两个独立用例复用 Ticket 10 驱动，使用受控模型响应、实际安装的 OMP、真实原生界面和生产后端；两项通过，清理无错误。这只证明工具行为，不证明真实模型判断或 Ticket 10 整套验收。原始证据保留在票据 worktree 的 `evidence/ticket15-focused-omp-20261118T150412Z/omp/evidence.json`，不随源码合并复制。

### 本轮未覆盖的原定用例

| 原定范围 | 本轮为何停止 | 后续补验入口 |
| --- | --- | --- |
| 双宿主按发布 V2 创建、读回 Task/Bot，并校验 `request`、`handoff`、`saved` 及下游实际读取（P35/V2/S2） | OMP 未通过创建授权；Pi 已创建 Task，但没有生成两份第一阶段交付材料。缺少真实材料，不能独立验证下游消费。 | 在同一个真实 Run 中逐步明确指令；先保存并读回前置材料，再核对下游原生读取和实际保存。 |
| 四个发布节点按实际职责批准、报告与显式交接（P34/V2/S2、P35/V2/S2） | Pi 仅观察到一个人工节点；没有同一 Run 的其余两次人工批准、子 manager `finish`、父层独立自动批准、完整 `complete`／`finish`。单工具结果不能补齐这些前后关系。 | 按真实回执逐步推进，不等模型自动衔接；保留各节点的批准来源、最新可交接报告及父子执行 ID。 |
| 打包页面核对完成后四节点的执行、批准、报告和累计输出（P34/V2/S3） | 两宿主配置页和发起页已通过，但没有完整 Run 可供最终页面与公开 HTTP 对照。 | 完成真实父子 Run 后复用既有 `facts` 浏览器检查。 |
| V2 闭环中确认 V1 未被调用、替换或修改（P35/V2/S2） | 未完成 V2，因此未完成全程副作用与定义读回的核对；不能用静态检查替代。 | 完整 Run 后核对 V2 原生调用、V1 原定义和隔离运行事实。 |

上述场景本轮停止追跑，不是通过，也不从原契约删除。保留 [Ticket 15](implementation/15-published-v2-native-acceptance.md) 的完整专项及原始证据；部分证据可随脚本改进交付，票据未关闭。

---

## Ticket 14：单格验收收尾交接

**验收未完成，不关闭 Ticket 14。** 十格均有过独立运行；截至本次收尾，只有两格 `investigation` 完整通过。已实现单格入口、隔离夹具及证据采集；以下八格不是未写入口，而是运行失败或尚未复验。未运行两仓完整回归，不能把单格、历史记录或部分 S3 当成整票通过。

原票：[14-published-v1-native-acceptance.md](implementation/14-published-v1-native-acceptance.md)。原 worktree 中的原始证据不归档。下表目录名仅是历史运行标识，不是可用的证据路径。复跑须重新准备 `paths.toml` 并使用新证据目录；先读 `<目录>/business/evidence.json` 的 `cells`，再读 `business/cells/<host>/<case>/native/<host>/evidence.json` 的 `originalErrors`、`publicFacts`、`coverage` 和 `cleanupErrors`；模型或代理问题对照 `completion-wire` 和格内的 `model-wire/index.json`。

| 宿主 / Case | 本次证据目录 | 已观察 / 下一步 |
|---|---|---|
| OMP / `investigation` | `oi-1794997421` | **4/4 通过**；51.344 秒静默观察，10 条合法 heartbeat 未误判成业务推进。 |
| Pi / `investigation` | `gi-1795008712` | **4/4 通过**；51.172 秒、10 条 heartbeat、无清理错误。只给出清理建议不是执行清理，判据已对齐发布 Map。 |
| OMP / `revision` | `xr-1794997990` | 隔离索引就绪、`w1` 已派发；模型流无终帧超时。用新版基线职责限定单跑；核验 `baseline`、`handoff`、三个修订者分别原生批准、真实替换保存和新 revision。 |
| Pi / `revision` | `ir-1795009561`、`jr-1795011418` | 已验证 `collect` HTTP 200 完整响应。此前 `w1` 误等候选确认；已限定候选要求只适用于编写者，但复验在派发前网络超时。先复验只读基线、再验证完整修订闭环。 |
| OMP / `outer` | `to-1795013054`、`oo-1795001719` | 命名 `materials` 已实证让 manager 确认输入并派发 `w1`。较早原生 Ask 内有完整 Task 提案，提取已用录制表单验证；最近 worker 只在 Chat 问范围后报告阻塞。新版委托明确既定范围并要求实际 Ask，尚未复验。仍缺真实 Task 创建/读回、W2 新 request、八节点子 Run、父子批准与最终 Task/Bot 保存。 |
| Pi / `outer` | `qo-1795002198` | 索引可查，`w1` 无报告、候选或实际 Ask，最终超时。新版命名材料、明确委托与原生 Ask 处理尚未在 Pi 复验；后续完整度同 OMP。 |
| OMP / `planning` | `xp-1795005415` | 两个隔离 Bot 公开读回成功；模型流在 `outline` 派发前无终帧超时。复验候选及原生决定、Map 保存/读回和 `inspect → summarize` 的 `findings` 绑定；不运行生成的 Map。 |
| Pi / `planning` | `rp-1795012862`、`fp-1795016334` | 已越过索引及错误的 Bot 详情路由；一轮把「先展示、之后再确认」误判为「批准当前候选」。已修正判据，不批准缺失候选。`fp` 已结束且失败：worker 最终真实读回两 Bot、revision 与 Task；仍将 Suite 实际资源/zone 视为未验证，未交付概要，manager 将 `outline` 判失败。下一次先提供获准的只读资源核验路径，再走候选、批准和保存；不得执行生成 Map 或宣称未核验的运行能力。 |
| OMP / `v1` | `ov-1795005870`、`sv-1795011473` | 早期真实 Bun 创建完成，三编写者并行冷启动时代理排队后断连；逐个派发版本在 `create_blank` 前遇网络超时。隔离索引、正确 Bot 路由、原生 Ask 修复未在该宿主复验；差三字段保存及完整原生/UI/manager 证据。 |
| Pi / `v1` | `pv-1795013304`、`lv-1795013819` | 已实证真实 manager 逐个派发，Bun 创建及部分 S3 已取证；Ask 值/标签比较已用录制表单验证。最近 `write_when` 因 Bot 查询 404、索引和业务确认停住；新版预检与委托未复验，最终保存未发生。 |

### 恢复顺序

1. 有授权模型和独占依赖时，从复验 Pi `planning`/`revision` 开始，验证已修正的材料和批准边界；随后逐格验两宿主 `outer` 与 `v1`，补 OMP `planning`/`revision`。每次一个宿主、一个 case、一个新证据目录；不重放原失败请求或旧 Run 副作用。
2. 区分三类失败：上游未给 HTTP 响应、HTTP 200 流无终帧、宿主在单槽排队时断连。单格内按真实 manager 逐个派发 V1 节点；不擅自增加模型容量，或修改产品超时/发布 Map/批准顺序。只看到节点报告或 UI 确认不等于保存和 `complete`/`finish`。
3. 全部单格恢复后，再按[原票](implementation/14-published-v1-native-acceptance.md)运行双仓检查及完整 92 项覆盖；不拼接跨版本证据认证整票。

```sh
python scripts/accept-workflow-codex.py --scenario node-approval-v1 \
  --hosts "$HOST" --cases "$CASE" --jobs 1 --native-only \
  --backend "$HERDR_BOT_BINARY" --toolkit "$TOOLKIT_WT" \
  --paths-file "$ISOLATED_PATHS" --pi-package "$PI_PACKAGE" \
  --auth-file "$AUTHORIZED_AUTH_FILE" --evidence-dir "$NEW_EVIDENCE_DIR"
```

参数用绝对路径，证据目录必须不存在，Windows 路径尽量短。可选 shell 用 `--shell-path` 显式指定。单格 `--native-only` 按设计不认证整票；看所选 `cells[].state`、coverage 和清理证据，不仅看外层退出码。不得提交凭据、模型原文或宿主运行目录。Toolkit 票分支 `9e337e4` 只含旧宿主验收快照，与其 `dev` 较新的快照冲突；不应以旧快照覆盖新记录。
