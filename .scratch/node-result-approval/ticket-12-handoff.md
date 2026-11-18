# Ticket 12：原生批准生命周期验收交接

日期：2026-11-18。用户要求本轮收尾，后续会话继续。**未完成，不认证成功，不集成、不关闭，未成功执行 hyper_auto_coding finish（工具因三票未集成而拒绝结束）。** 停止新增测试、模型请求和修复；保留原分支、worktree、原始失败与交付证据。

工作流最终已 collect：无 active／integrating，12为带未完成error的 `ready`，不是验收通过；总体 `finished=false`。见 `evidence/handoff-workflow-state.json`。下次获准继续时用原工作流 `status`／`dispatch` 恢复实现，不用 `retry_integration`，不重建票据。

## 恢复位置

- 原票：`implementation/12-native-approval-lifecycle-acceptance.md`。
- herdr-bot worktree：`C:/Users/hugos/AppData/Local/Temp/hyper-ticket-5s1ANw`。
- toolkit worktree：`C:/Users/hugos/AppData/Local/Temp/hyper-ticket-icIkYS`。
- 原分支分别为 `hyper/node-result-approval/12-native-approval-lifecycle-acc-e82a78-349645ddb346`、`hyper/node-result-approval/12-native-approval-lifecycle-acc-e82a78-3a3cc4cfa470`。
- 已报告提交：herdr-bot `b39ac70`；toolkit `8b0625a`。以归档 `lifecycle12-handoff/evidence.json` 和收尾记录为准；不要将这些验收源码误认为已合入 dev。
- 关键 ledger：`lifecycle12-handoff/evidence.json`，其中 `ticketComplete=false`、`certifiesIntegration=false`；最新停止状态由 `lifecycle12-stop.json` 补充。
- 最终源码状态来自最后真实运行的sourceStatus：herdr-bot干净；toolkit有未提交 `framework/docs/pi-acceptance.json`（自动生成验收元数据，已保留且未纳入提交）。协调仓另存 `evidence/ticket-12-handoff/uncommitted/toolkit-pi-acceptance.json`。

## 最后在途单分区的收尾结果

停止指令前已经启动的 `lifecycle12-omp-lifecycle-only` 自然结束：45次实际模型请求、transportFailed=false，但exit1。实际working、旧原生Yes=true及query/report/独立分支complete时序已有证据；无效果观察窗口仅45.141秒、88样本，未达到要求51秒，因此不能认证P11或整体lifecycle。

最早错误为 `Native scheduling phases exhausted before the required observation finished`。原始gate事件显示agent_start与context持有事件约相差4.5ms，**不能假设公开hooks全部串行、把各hook预算简单相加**。下一会话最先读 `lifecycle12-omp-lifecycle-only/business/omp/retry-gate/events.jsonl`，不延长产品／hook超时、不缩51秒判据，不用已观察到Yes替代完整因果窗口。10个进程已确认退出、cleanupErrors=[]；目前无在途自有模型、测试或运行。

## 真实验收状态

| 分区 | OMP | Pi | 边界 |
|---|---|---|---|
| P14 执行恢复／后端重启 | 有真实通过证据 | 有真实通过证据 | 不需因其他分区失败重跑 |
| P16 成果实际改写及自由报告 | 有真实通过证据 | 有真实通过证据 | 不代表自动重试或自动推进已通过 |
| P3 执行模式冻结／retry | 未完整验证 | 未完整验证 | 部分公开事实不能认证完整生命周期 |
| P10 角色／旧执行／双身份隔离 | 未完整验证 | 未完整验证 | 受控额外身份参数拒绝另列机械证据 |
| P11 旧 Yes 与新 execution 隔离 | 未完整验证 | 未完整验证 | 51秒 predecision gate、释放后继续的完整真路径未通过 |
| P24 确认在途仍可操作 | 未完整验证 | 未完整验证 | 已有 query/report/独立分支完成的局部时序；整例未通过 |
| P28 父子 Run 双模式交接 | 未完整验证 | 未完整验证 | 子 manager 整体批准、实际 Yes、父 manager 独立 complete 均须继续证明 |

真实有效原件：OMP `lifecycle12-native-partitions/business/omp/evidence.json`；Pi `lifecycle12-pi-isolated/business/pi/evidence.json`。逐承诺映射和历史轮次见最终 ledger，不将支持性事实提升为整票成功。

## 最早阻碍与已完成的最小修复

**已证明是本票驱动接入错误，不是已证明的应用死锁。**

1. dual manager 已合法调用 collect，内层节点 running 且曾报告 `canComplete=false`。生产 collect 合法等待后续状态／通知。
2. 本票新 `track` 在身份登记之后额外等 assistant stop；`role` 又向忙 manager 塞“列工具”输入并等 stop。驱动因此不推进能释放 collect 的子工作，形成测试自身等待循环。
3. 另一次本票用 PID 差集找新进程，但 manager 已先自主 dispatch；驱动再次 dispatch 被应用正确拒绝，却误判没有新进程。应按 SDK Session/PID、实际 argv 和活句柄关联。

既有正确模式已经存在：

- `scripts/acceptance/host/native.py` 的 `native(chat)`、`observe_processes` 只观察身份／进程，不等待 assistant stop。
- 前票10 `scripts/node_approval_report_acceptance.py` 的 `track` 只登记，`settled` 独立调用。
- toolkit `scripts/node-approval-concurrent.ts` 在身份核实后先发布 ready，再等并发释放。

本票不是缺少新 seam，而是组合时破坏了已有正确粒度。不要再新建身份／collect 框架。

### 最小反馈回路结果

`lifecycle12-collect-corrected/native/pi/evidence.json`：

- 1 test，18.220秒，exit0。
- 真实安装 SDK／Session + 隔离生产 HTTP；本地机械 provider 仅选择 collect，**不是真实模型业务验收**。
- collect 已在途时，track/catalog 在0.531秒返回，collect 仍未结束。
- 随后真实保存 child 进度报告，原 collect 返回 `reason=notification`。
- 4个进程退出确认，`cleanupErrors=[]`。
- 初轮 `lifecycle12-collect-minimal/console.log` 的失败保留：前置未建 running child，误抓到合法 `no-running` 短返回；不能把它当长等待。

复跑入口为原 worktree 的 `python scripts/accept-node-approval-lifecycle.test.py CollectRegistrationFixture`，须使用测试声明的显式 toolkit/backend/paths/evidence 环境。完整环境名／命令见收尾 ledger 与原日志；不可拿此单测替代 P3/P10/P11/P24/P28。

## 外部失败与证据限制

- OMP `lifecycle12-native-partitions/codex-completions/0054-error.json` 为真实 `provider-error/httpStatus=400/requests=1`。
- 既有脱敏记录没有具体原因，不能认定为产品缺陷、上下文过长或 tool-call 不匹配。
- 共享 terminal 曾使尚未真实尝试的 Pi 只得到502；这不等于 Pi 业务决定失败。
- 不重发原失败请求、不重置 terminal 刷绿，不改产品 timeout/retry。

## 实际改动及检查

改动限验收入口／场景、显式 Chat 的共享确认 driver 参数、验收 JS 调度 observer、toolkit 验收 TS 身份故障 caller 和测试文档；未改 backend/frontend/toolkit 生产业务逻辑或锁文件。

既有九项检查已实际执行成功，后续未重跑：76个后端 binary 零未派发；前端274；浏览器375通过／1个既有 symlink skip；toolkit1932／244文件 JUnit；typecheck、integrations、3个具名 dock 共10例。全仓中含离线／受控测试，不能称全部使用真实外部服务。

窄检查：CLI/路由/gate9例、共享兼容15例、TS caller typecheck及上述 collect 回路。原命令、退出码、日志／JUnit映射在 `lifecycle12-handoff/evidence.json`；这些不认证缺失 S2。

## 下一会话最小步骤

1. 先读原票、contract、本交接、最终 ledger；核对 worktree 当前内容和提交，保留未提交工作。
2. 复用已通过 collect 回路，不再重建或做大重构。只选一个尚未有效验证的 lifecycle 分区，使用现有 selector／入口逐步推进。
3. 真实记录身份／旧 Yes／新 execution 的输入→应用返回→公开状态；若生产链确实违反契约，再修对应生产代码，而不是改期望或扩大等待预算。
4. 另行完成 P28 两种模式的真实父子交接；不能用受控 provider、CLI安全、observer机械回归认证业务成功。
5. 对已有 P14/P16、全仓检查按源码／依赖身份明确复用；不因局部分区失败重跑整矩阵。

## 保存和清理

协调仓副本：`evidence/ticket-12-handoff/`。原证据根为本 worktree `.scratch/node-result-approval/evidence/`；副本保持相对结构，原 JSON 的绝对路径不改写。复制范围／完整性见副本 `preservation.json`，未复制文件仍在保留 worktree，不能假定整个原树都已归档。

收尾前已报告无在途自有运行；各保存原生轮次清理错误为空，最小回路有退出确认。既有 stdio `ResourceWarning` 保留，未隐去，也未扩到另修基础设施。保留源码、Git 元数据、worktree、模型／原生记录和检查日志；不得自行删除归属不明目录、用户会话、共享缓存或认证。
