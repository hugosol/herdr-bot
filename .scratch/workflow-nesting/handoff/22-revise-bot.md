# 22票交接：既有 Bot 指南修订业务验收

## 结论

代码已合入两仓本地 `dev`，但本票完整验收未通过。真实 declared 修订场景失败，后续 OMP／Pi 双宿主运行因长时间不返回、持续消耗而由主代理按用户要求中止；中止不是成功退出。P40／P42／P43／P44／P45 不因代码合入或冲突解决而认领通过。

本次交接只归档历史记录，没有重跑验收、发起模型请求或修改编排内部状态。

## 从哪里继续

- [原票及完整验收条件](../implementation/22-cross-repo-revise-bot-acceptance.md)
- [批准契约](../contract.md)
- [最后一次回归及未关闭结论](evidence/22-before-final-sync.md)
- [之后仅解决同步冲突的报告](evidence/22-final-sync.md)
- 普通 Git 收尾后的源码基线（不含后续交接文档提交）：herdr-bot `0393c41df31bfb17b19bc299234671c521607d42`；toolkit `367cc4c5cbb7479b7d2878e63620350bae9f042c`。

## 逐 case 结果：成功、失败、未执行与未知

逐项清单及状态依据见 [机器可读 case 表](evidence/22-case-status.json)、[declared 原始终端结果](evidence/22-declared-run.json)及[双宿主中断记录](evidence/22-interruption-and-cleanup.json)。这些是旧运行的归档和结果重建，不是本次新执行。

### declared：14个顶层 case

已直接记录失败7项、未执行5项、待核实2项。没有找回可直接核实的成功详情，**不等于可以断言没有任何 case 成功**。计数以14次顶层 `attempt` 调用为单位；`revision-baseline-files` 内含 link／traversal 两个子项，不重复计数。

| Case | 场景 | 状态 | 结果／依据 |
|---|---|---|---|
| `dedicated-chat` | 独立修订基线 Chat | 失败 | `Baseline omitted complete real Task/Map target` |
| `task-no-materials` | Task型，无可选材料 | 失败 | `Real revision workflow did not finish` |
| `task-materials` | Task型，有可选材料 | 失败 | `Timed out: real Chat reaches user boundary` |
| `map-no-materials` | Map型，无可选材料 | 失败 | `Incomplete guide offered for approval`，字段 `when` |
| `map-materials` | Map型，有可选材料 | 失败 | `Real revision workflow did not finish` |
| `revise-unauthorized` | 未授权保存拒绝 | 失败 | 触发 `Independent Chat incorrectly injected node role` 断言；不能仅由该断言判定根因 |
| `revise-baseline-conflict` | 共同基线冲突拒绝 | 待核实 | [INFERENCE] 按完整错误表与执行控制流推断通过，但原始成功详情缺失 |
| `revise-target-conflict` | 真实目标变更冲突拒绝 | 待核实 | [INFERENCE] 按完整错误表与执行控制流推断通过，但原始成功详情缺失 |
| `revise-running` | 使用中的定义禁止保存 | 失败 | `Actual model transport failed; no retry or later response can erase it` |
| `revise-revision` | revision冲突 | 未执行 | 原始记录明确 `attempted=false`，此前传输或清理前提已失败 |
| `revise-applied` | 保存已生效但报错 | 未执行 | 同上 |
| `revise-not-applied` | 保存未生效 | 未执行 | 同上 |
| `revise-unknown` | 保存结果未知 | 未执行 | 同上；“未知结果”是case主题，不是说本次是否执行未知 |
| `revision-baseline-files` | 基线文件link／traversal越界拒绝 | 未执行 | 顶层 `attempted=false`，两个子项均未进入执行 |

错误表来自原始终端 JSON 的 `verificationError` 中完整 `scenarioErrors`，通过安全的 `ast.literal_eval` 解析其 Python 字面量，不执行错误文本。两个待核实项均不在12条错误记录中；核对 runner 后，失败与未执行都会记录错误，而成功返回的 `rejected_save` 会设置 `verified=true`，因此可以作上述推断。相关三份 runner 源码与历史源码提交 `ec60bbe304d9f55dfd0979e5758b30a9ea6980fe` 无差异；**推断仍不等于找回了完整成功证据，不能据此关闭票据或promise**。

### 强制双宿主：不能与declared混算

| 执行范围 | 结果 | 逐case信息边界 |
|---|---|---|
| OMP强制宿主矩阵 | 已启动、外部中止，未验证 | 没有可用逐项终态；不能将所有case记成成功，也不能一律记为未执行 |
| Pi强制宿主矩阵 | 未启动、未验证 | 中断记录明确 `piMatrix=not started`，该矩阵全部计划case未执行 |

历史两仓回归、runner机制测试与定向dock通过列表见后文及原集成报告，**不折算成上表真实业务case通过**。

## 未解决事项一：真实修订专项

已归档回归报告明确保留既有 declared 失败与双宿主中断，不再自动补跑付费矩阵。该报告的 `businessAcceptance.newModelRequests` 为0。

上表已逐项归档未完成基线、修订 workflow 未完成、user boundary 超时等失败。它们尚未形成根因结论，也不等于在当前源码上重新复现。原完整 `evidence.json` 和成功项的详细 `safety` 结果未找回；已保存的终端结果包含完整错误清单，证据缺口不得自动补成通过。

传输使用的 `public-completion-transport/public-default` 是配置标识；当时没有取得底层实际服务模型名的可核实证据，不要把别名当成已解析的真实模型身份。

继续真实业务前需要重新明确授权与调用边界。不得在本轮用户要求“只提交合并、问题留到新对话”之后自动启动模型、重试业务或追加矩阵。

## 未解决事项二：toolkit最后一次canonical失败

[回归报告](evidence/22-before-final-sync.md)对应 herdr-bot `ec60bbe304d9f55dfd0979e5758b30a9ea6980fe`、toolkit `94cf3cd6d50cb9d043086618c132bc397fe6499b`，最终修改后的 `bun run test`：

```text
exit 1
1818 passed, 1 failed
1819 tests across 230 files
```

失败为 `tests/harness-dock/detail-tree.test.ts:728`：预期的消费记录没有按用例要求发布。报告观察到20秒结束时文档仅有 child session，没有 main session。原错误包含：

```text
no consumption document the case expects was published
```

之后独立串行运行该文件通过，但原因未证实，不能拿串行诊断替代 canonical 全绿，也不能说它已修复。不要直接放宽超时或断言。

报告还记录：前端构建、后端构建、workflow_dispatch 165项、后端完整入口、前端244项、runner测试、toolkit typecheck、workflow-chat 25项和定向dock修复后34项通过。修改前一次1819全绿不是最终修改树的全绿证据；这些历史结果也不等于当前 dev 的新验证。

## 已完成的源码和同步工作

- toolkit `94cf3cd` 修正晚到 register 回执测试的观察边界：区分断开前合法状态与断开后的禁止复活，不改产品逻辑。
- 依赖恢复后 typecheck通过，没有修改锁文件。
- 最后同步提交 `4bec782403ee2c0f3568fe4592777cbb5f2c4b3d` 保留较新的完整 Pi workflow-chat 验收记录，仅解决合并冲突，没有重跑业务或全局验收，也没有改变之前失败结论。
- 普通 Git 收尾提交 `367cc4c5cbb7479b7d2878e63620350bae9f042c` 把13票 `0da53ea` 纳入 toolkit dev 历史；保留已有等效测试修复及较新的验收记录。该合并前后文件树完全一致。

## 下轮建议顺序

1. 先读两份归档报告及原票，区分真实业务失败、被中止运行与非付费 canonical 失败。
2. 非付费排查优先定位 detail-tree main session 消费文档缺失的发布／观察边界，保留失败日志；不要以一次单测通过改判全量结果。
3. 盘点原真实场景证据的可达性，再决定修订业务／传输需要怎样的具体修复。不要将不完整日志或 runner 尚未返回当成业务成功。
4. 只有获得新授权后才运行真实模型矩阵，并按原票分别完成 declared与omp,pi；不改业务定义以迁就宿主，不读取用户认证，不重试模型决策掩盖失败。

## 证据与编排边界

原真实场景根为 `C:/Users/hugos/AppData/Local/Temp/hyper-ticket-ffG0QG/.scratch/workflow-nesting/evidence/revise22-20261106/`；后续回归证据根为同级 `integrate22-20261106/`。这些只是历史定位信息，临时目录可能已被部分删除。本次已归档两份集成报告、declared原始终端JSON、双宿主中断／清理JSON及重建的逐case状态表；不是完整运行目录、完整evidence.json、原生历史、HTTP日志或进程快照。

编排曾把22记为 integrated，仅表示该流程已正常返回并合入，不能推导本票验收通过。13／14／23的failed状态、报告发布、retro和残留工作树清理是后续独立收尾问题；不得回滚已合入源码或因retro失败重跑实现。
