# Ticket 14：双宿主十场景真实验收交接（2026-11-18）

**结论：测试已结束，票未通过，不应关闭或集成。** `bg_8` 退出码 1，耗时 1 小时 46 分 49 秒。10/10 格均已执行并退出，0/10 格端到端通过；92 项验收要求中 5 项取得证据，均为 S3 页面项。另 87 项未认证，不是 87 个独立断言都执行后报错。两仓前置检查 9/9 退出 0，汇总 `cleanupErrors=[]`。本轮没有恢复 hyper-auto-coding 工作流。

## 从哪里接手

- 需求：[原票](implementation/14-published-v1-native-acceptance.md)、[合同 P31/P34/P35](contract.md)。[上一份交接](ticket-14-handoff.md)是更早一轮 48 项口径；其 Run、HTTP 400、`409 output-missing` 和覆盖结果保留作历史证据，**不与本轮合并计数**。
- `WT`（实际脚本）：`C:/Users/hugos/AppData/Local/Temp/hyper-ticket-KirY7B`，HEAD `2c910b98421508828cf7844f0c130fe8f98285fb`。配对 toolkit：`C:/Users/hugos/AppData/Local/Temp/hyper-ticket-gEpwuE`，HEAD `15b11cfe6827398e597606efa508120a6e1e3458`。**WT 有未提交改造**，不能用干净 HEAD 或协调仓 `dev` 代替；勿清理两个 worktree、未跟踪脚本或证据。
- 下文 `R` = `WT/.scratch/node-result-approval/evidence/t14-real-20261118-142434`。本文件在协调仓；**R 在临时 worktree，不是本文档旁的 `evidence/`**。先读 `R/business/evidence.json` 的 `cells`、`coverage`、`commands`，再读 `R/evidence.json` 的 `transportByCell`、`businessExit`、`modelRequests`。逐格判定见 `R/business/cells/<host>/<case>/native/worker.json`；其旁 `<host>/evidence.json` 的 `originalErrors` 保留具体失败，`<host>/data/runs/<Run ID>/` 有 `run.json`、`history.jsonl`、`messages.jsonl`。
- 本轮执行入口为 `WT/scripts/accept-workflow-codex.py --scenario node-approval-v1 --hosts omp,pi --cases all --jobs 1`（非 `--native-only`）。完整业务命令、资源路径及源码身份见两份汇总；wrapper 临时模型配置不是可持久复用的配置。模型为 `openai-codex/gpt-6-luna`，正式运行 332 次模型请求；另有 1 次独立身份预检成功，证据在同级 `t14-real-20261118-142434-smoke/evidence.json`。凭据仅通过既有显式授权只读使用；分析不需读取、复制或刷新凭据。

## 已经改了什么（均指票 14 worktree）

| 范围 | 实现与边界 |
|---|---|
| 矩阵与准入 | `scripts/accept-node-approval-v1.py`、新 `scripts/node_approval_v1_matrix.py`：五个已发布 Map 分别由 OMP、Pi 担任 manager；worker 保留定义声明的宿主和 Suite。每格独立子进程、Run、后端、存储和证据目录。22 个节点 × 2 宿主 × S2/S3，加两个宿主各自的 P31、P35，共 92 项。outer 中嵌套的 V1 不代替顶层 V1。只接收本格、当前源码与运行的证据；过滤或 `--native-only` 不认证整票。 |
| 并发与故障范围 | `scripts/accept-workflow-codex.py`、`scripts/acceptance/host/{completion_transport,model,wire}.py`：默认串行；并发 2 需显式模型容量，不能按 CPU 推断。每格独立传输终止状态；普通单格失败不污染下一格；共享授权丢失或清理不安全时停止派发新格。 |
| 业务判据 | `scripts/node_approval_v1_acceptance.py`：分开记录原生业务 S2 和页面 S3；关联本次 Run、execution、当前报告及 manager `complete` 的真实 HTTP 响应。区分成功完成与 409 拒绝，驱动不补写缺失的 `handoff`。核对 V1 命名材料及字面业务约束，Ask 的答案须有已给事实或已审核候选支撑；修正 Windows 上 planning 误认已有 Map 为新增的路径比较；investigation 不再继承人工批准节点的完整候选／proposal 要求。 |
| 页面与清理 | `scripts/node-approval-v1-browser.mjs` 与场景驱动：一个页面节点失败不抹掉另一节点的独立证据。孤立 USERPROFILE 准备 Windows Local AppData 目录；只将当前节点精确的 `POST .../nodes/edit`、纯 `{id}` 识别为读取草稿，不把保存视为观察；必须有 Chrome 退出确认。本地用真实 Chrome、打包页和隔离旧 Run 副本验证过正例及局部失败，不将其计作本轮原生通过。 |
| 传输诊断与离线验证 | `scripts/workflow-codex-completion.mjs`、wrapper：非成功 HTTP 记受限的错误分类／请求结构；HTTP 200 后失败另记流首帧、终止帧、计数和 SDK 结果。37 个本地定向测试通过：CLI 10、矩阵 6、业务判据 7、transport 5、SDK 9。入口为 `scripts/accept-node-approval-v1.test.py`、`scripts/node_approval_v1_matrix.test.py`、`scripts/node-approval-v1-behavior.test.py`、`scripts/workflow_completion_transport.test.py`、`scripts/workflow-codex-completion.test.py`。使用说明在 `WT/docs/agents/testing.md` 的“发布 V1 与非 V2 节点职责专项”。这些改造未改产品 backend、frontend/src 或 toolkit 业务逻辑。 |

## 没有通过的十个脚本场景

下表的场景名就是脚本 `--cases` 的值，宿主对应 `--hosts`。计数为**具名验收项已验证数／本格负责项**，不是节点执行数。表中状态是停止收证时的 Run 快照，不表示进程仍在运行。`*-error.json` 均位于 `R/codex-completions/<host>/<case>/`。

| 宿主 / 场景 | 本次 Run ID | 验证项 | 首要阻碍或相互独立的失败事实 |
|---|---|---:|---|
| OMP / `v1` | `bc-22f459a88de6` | 0/18 | `0008-error.json`：HTTP 200，135 帧后以 `error` 结束，SDK 报错；快照中 `create_blank` 等节点仍 pending，不能认定创建／三字段保存已跑通。 |
| OMP / `outer` | `bc-04eda580b1ca` | 0/8 | `0033-error.json`：HTTP 200，112 帧后以 `error` 结束，SDK 报错；`w1` running，其余 pending。 |
| OMP / `revision` | `bc-f2c810a8992a` | 1/10 | Run 超时，`w1` failed，manager 有 `w1` 待处理报告 `bc-9513c216c597`，其余节点 pending。仅 `w1` 页面 S3 已验证。先看报告中的材料／配置核对，不把它同 Pi 的 running 混为一因。 |
| OMP / `planning` | `bc-0abadfe0738a` | 0/6 | `outline` running；Ask 判定 `Unknown business decision; no answer fabricated`：当前范围未确认、没有已评估的完整候选，不能替用户确认概要方案。未走到生成 Map。 |
| OMP / `investigation` | `bc-d0d3b068850a` | 2/4 | 两节点、Run 均 completed，两个页面 S3 均通过。`tibo-monitor` 最终评判仅 `handoff=false`；`c-drive-inspection` 报告未通过 worktree 来源核对；静默窗口另失败。 |
| Pi / `v1` | `bc-705a76789b02` | 0/18 | `0173-error.json`：HTTP 200，16550 帧，无终止帧，SDK 仍 pending 时 timeout；不是收到流错误终帧。节点仍 pending。 |
| Pi / `outer` | `bc-d6b5a04080b2` | 0/8 | Run 超时，`w1` failed，其余 pending。待处理报告 `bc-b6315f9b601a` 称查询索引得到 `No projects indexed yet` 且无建索引工具；这是 worker 的报告，工具暴露情况尚待核实。 |
| Pi / `revision` | `bc-2e4897a003e2` | 0/10 | 25 分钟 Run 等待预算耗尽；`w1` 仍 running、无报告，`w2`–`w5` pending。不能从超时推断 `w1` 为何停滞。 |
| Pi / `planning` | `bc-83f8e448b07e` | 0/6 | `0296-error.json`：HTTP 200，109 帧后以 `error` 终止，SDK 报错；`outline` running。与 OMP 的 Ask 未获决定是不同阻碍。 |
| Pi / `investigation` | `bc-5a71673053c2` | 2/4 | 两节点、Run 均 completed，两个页面 S3 均通过。`tibo-monitor` 的 `truth=true`，`responsibility/scope/handoff=false`；`c-drive-inspection` 报告未通过 worktree 来源核对；静默窗口另失败。 |

## 下次逐个分析：先读证据，不先重跑

1. **调查场景的静默窗口（双宿主，可直接比对）。** 已按同格 `evidence.json:quietWindow.started/ended` 对齐 `backend-wire/index.json`：两个失败窗口内各有一条 `POST /api/sessions/events`，OMP 记录 `0803.json`，Pi `0647.json`，响应均为 200。脚本 `quiet_window()` 用非 GET/HEAD 总数变化判断“意外业务动作”。先核对这两条请求的事件类型、来源和副作用，区分会话事件与实际自动业务动作；**目前不能判定为产品回归，也不能直接豁免整个 events 路由**。这两个请求与浏览器的草稿读取不是同一个端点。
2. **调查报告与判分边界。** 在同格的 `originalErrors`、`qualitativeEvaluations`、Run 报告、原生工具结果中，将两个节点的原文、所属 execution、独立业务输入、项目／worktree／branch 与 `contract.md` 对照。尤其核实评判是否把另一节点职责或旧候选要求施加给 `tibo-monitor`。记录真实交付缺口与驱动误判的区别；静默判据即使修好也不能自动解决两个节点的业务失败。
3. **节点失败后的长等待。** 对比 OMP/revision、Pi/outer 的 `w1=failed` 与 Run 仍 running。`run_case()` 检查 Run 的 failed/cancelled 及 `execution.lifecycle.status`，不直接按 node.status=failed 终止；默认预算为 300 秒 × max(2, 节点数)。先确认 manager 是否仍有可行的合法处理路径，再决定如何尽早报告／终止明确不可恢复的格。Pi/revision 是 `w1=running`、无报告，应单独查最后一次有业务意义的原生工具调用，而非合并成同一超时根因。
4. **模型传输四例。** 三例 `response.created → error`，一例无终止帧；四份错误回执都只有 1 次请求、HTTP 200，无 401/403 证据。不要拿旧轮 HTTP 400、身份预检成功或本地代理后续 502 解释本轮根因。用回执、对应 completion-wire 及 SDK 版本／诊断边界先离线定位；不要重发原失败请求，不把 HTTP 200 当作成功模型决策。
5. **OMP/planning 的授权边界。** 对照 Ask 原问题、候选内容、caller 输入和 Scope；判断缺的是业务决定还是驱动未从已有材料取证。没有依据时保留阻碍，不把默认选项当用户回答。

后续如需真实复验：先报告要验证的**一个宿主、一个 case、一个失败假设**，用新证据目录，保留原错误与退出码。单格结果用于诊断，不导入旧矩阵、不冒充整票通过；遇到新阻碍立即报告，不等整套结束。此交接仅新增文档，未改脚本、重跑模型或变更 ticket 状态。
