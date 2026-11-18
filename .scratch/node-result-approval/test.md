# Ticket 15 测试取舍

## 本轮范围

真实模型验收以用户明确指令触发宿主原生工具并得到可核对结果为止。保留真实原生确认及工具回执，不追跑依赖模型自主编排的材料生成、报告、manager 操作和父子 Run 交接。不代调工具、不伪造前置状态，也不把部分结果计成完整通过。

不在 `native_yes()` 重复添加 `approved` 前后读回断言。后端 HTTP 集成测试 [`approval_without_report_or_artifact_only_changes_the_approval_fact`](../../backend/tests/workflow/reports/approval.rs) 已覆盖人工及自动模式下同一执行的 `approved: false → true` 和批准来源；两宿主的真实工具调用、确认和公开状态读回由 [Ticket 10](implementation/10-native-approval-report-acceptance.md) 的 [`accept-node-approval-report.py`](../../scripts/accept-node-approval-report.py) 专项负责。

## 决策依据

- **OMP**：`create_task` 请求一次确认 Q1–Q8，未提交报告；驱动未代做合并授权，Run 超时。[运行证据](evidence/ticket15-followup-omp-20261118T132733454575Z/business/omp/evidence.json)
- **Pi**：原生 Yes 返回 `true`，同一执行取得人工批准回执；但没有交付 `request`／`handoff`，最新报告仍为 `canComplete=false`，manager 的 `complete` 被拒绝。[保留的传输记录](evidence/ticket15-followup-pi-20261118T132733454575Z/completion-wire/)

这些是本轮 `gpt-6-luna` 的实际结果，不代表该模型永远无法推进，也不代表批准失败。

## 已验证的简单场景

| 场景 | OMP | Pi |
| --- | --- | --- |
| 发布配置页、发起真实 V2 Run | 通过 | 通过 |
| 原生 MCP `list_projects` | 成功返回空列表；未建索引 | 成功返回空列表；未建索引 |
| 原生人工批准 | 独立用例 `bc-90b8be180a92`：Yes 后公开读回同一执行 `bc-712e6971fac6` 为 `approved=true`、`approvalSource=manual`，没有生成报告 | 真实 V2 Run `bc-595af5004f0a`：原生 Yes 返回 `true`，执行 `bc-965a87a9bf5e` 的批准回执为 HTTP 200、`approved=true`、`changed=true`、来源 `manual` |
| 原生进度报告 | 独立用例 `bc-b5a2eed16e01`：保存报告 `bc-5facfb9ac3b4`，`canComplete=false`，执行仍未批准 | HTTP 200、`saved=true`、`canComplete=false`，未提交输出 |

OMP 两个独立用例复用 Ticket 10 驱动，使用受控模型响应、实际安装的 OMP、真实原生界面和生产后端；两项通过，清理无错误。这只证明工具行为，不证明真实模型判断或 Ticket 10 整套验收。原始证据保留在票据 worktree 的 `evidence/ticket15-focused-omp-20261118T150412Z/omp/evidence.json`，不随源码合并复制。

## 本轮未覆盖的原定用例

| 原定范围 | 本轮为何停止 | 后续补验入口 |
| --- | --- | --- |
| 双宿主按发布 V2 创建、读回 Task/Bot，并校验 `request`、`handoff`、`saved` 及下游实际读取（P35/V2/S2） | OMP 未通过创建授权；Pi 已创建 Task，但没有生成两份第一阶段交付材料。缺少真实材料，不能独立验证下游消费。 | 在同一个真实 Run 中逐步明确指令；先保存并读回前置材料，再核对下游原生读取和实际保存。 |
| 四个发布节点按实际职责批准、报告与显式交接（P34/V2/S2、P35/V2/S2） | Pi 仅观察到一个人工节点；没有同一 Run 的其余两次人工批准、子 manager `finish`、父层独立自动批准、完整 `complete`／`finish`。单工具结果不能补齐这些前后关系。 | 按真实回执逐步推进，不等模型自动衔接；保留各节点的批准来源、最新可交接报告及父子执行 ID。 |
| 打包页面核对完成后四节点的执行、批准、报告和累计输出（P34/V2/S3） | 两宿主配置页和发起页已通过，但没有完整 Run 可供最终页面与公开 HTTP 对照。 | 完成真实父子 Run 后复用既有 `facts` 浏览器检查。 |
| V2 闭环中确认 V1 未被调用、替换或修改（P35/V2/S2） | 未完成 V2，因此未完成全程副作用与定义读回的核对；不能用静态检查替代。 | 完整 Run 后核对 V2 原生调用、V1 原定义和隔离运行事实。 |

上述场景本轮停止追跑，不是通过，也不从原契约删除。保留 [Ticket 15](implementation/15-published-v2-native-acceptance.md) 的完整专项及原始证据；部分证据可随脚本改进交付，票据未关闭。
