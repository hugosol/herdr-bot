# Ticket 13：失败与未覆盖场景交接

日期：2026-11-18。本文只整理后续修复入口；本次未修改产品、验收逻辑、票据状态或运行新的模型请求。

## 状态与阅读边界

- 工作流已登记 `13-model-handoff-end-node-acceptance.md` 为 `integrated`。这不是“所有场景全绿”。
- 最终 `final-contract-ledger.json` 声明 P20 四格已验证，P30-S2 由 OMP/manual、Pi/auto 的真实结束节点与 manager finish 证据覆盖。
- 原四格完整运行仍为 **8/16 覆盖标记通过、2/4 完整场景通过**，退出非零。不能把这次运行改写成通过。
- 后续独立 OMP/auto 运行补齐该格四项覆盖；Pi/manual 的批准和报告可证明 P20，但其完整生命周期仍失败。
- `integrated`、按承诺分区补证完成、单次完整矩阵成功是三个不同结论。本文保留差异，不重新裁定或修改已登记状态。

## 最终覆盖矩阵

| 场景 | P20 成果顺序／进度可先报／批准来源 | P30 结束节点与 finish | 后续处理 |
|---|---|---|---|
| OMP/auto | 后续独立运行通过 | 后续独立运行四项 coverage 通过，原矩阵未到达 | 原 Ask 表单分支仍缺真实闭环证据，见 H13-02 |
| Pi/auto | 原矩阵通过 | 原矩阵通过；结束节点 running、未批准、无报告，manager 显式 finish | 无已知剩余失败 |
| OMP/manual | 原矩阵通过；真实原生 Yes、manual 来源 | 原矩阵通过，同上 | 无已知剩余失败 |
| Pi/manual | 原始调用、材料、公开状态可证明；整格原 coverage 仍全 false | **失败／未验证**：work 未 complete、end 未派发、manager 未 finish | H13-01 |

原矩阵每格有四个内部标记：`P20/order`、`P20/progress-before-approval`、`P20/approval-source`、`P30/end-node-finish`。`8/16` 是这些标记的数量，不是 16 个独立业务场景。Pi/manual 的部分事实未被原聚合器计为成功；最终 ledger 单独列出其 P20 证据，没有修改原结果。

## H13-01：Pi/manual 在成果报告后停留 collect，未完成交接

**状态：仍失败，根因未确定。** 不应直接归因于模型、产品或网络。

已观察：

- Run `bc-db5460a0d07d`；work execution `bc-6d8971b12029`。
- 原生 Yes 返回成功；公开执行为 `autoApproved=false`、`approved=true`、`approvalSource=manual`。
- `quote.json` 为 quantity=7、unitPriceCents=1200、discountPercent=25、totalCents=6300；文件存在且已读回。
- 原生 approve 先于正常成果 report。最新同 execution 报告 `bc-8a92407330d5` 的 `canComplete=true`，输出 ID 为 `quote`。
- manager 收到报告后调用 collect；在原预算内未调用 complete、未派发 end、未调用 finish。最终 Run/work 仍 running，end pending。
- 原错误：`Timed out: actual native manager explicitly finishes user handoff`。
- preconditions 中最后一次 collect 有 HTTP 200，但代理读取 chunked response 的异常及空响应；这是调查线索，不足以判定根因或因果方向。

后续定位顺序：先核对报告通知、collect 请求／流结束、宿主工具返回及 manager 后续调度的同一时序；确定问题属于报告消费、collect 生命周期、宿主恢复还是模型实际选择。禁止用人工 complete/finish、提示模型固定工具顺序或延长预算代替修复。

验收：新的独占 Pi/manual Run 中，真实批准→成果报告后，manager 自主 complete 普通节点、派发结束 Chat、显式 finish；结束 Chat 仍不需要批准、报告或完成用户后续业务。保存真实 HTTP／原生调用、模型身份、状态及无自动行为观察窗口。

证据：`ticket-13-authorized-corrected-fixture/{cell-summary.json,failed-cell-boundaries.json,pi-manual-completion-preconditions.json,pi-manual-P20-audit.json}`；原格 `business/manual/pi/evidence.json`、`backend-wire/`、`histories/`。

## H13-02：OMP/auto 的真实 Ask 折扣表单闭环尚未验证

**状态：旧运行失败；驱动已修复并有离线实装组件回归，但真实表单分支尚未补证。**

- 原 Run `bc-fbf65d890a79`：先保存缺折扣问题报告，work 未批准；模型随后合法调用 `ask`，表单字段 `discountPercent` 为 number。
- 当时驱动等待 assistant 完整结束，而未处理仍 pending 的业务表单，错误为 `Timed out: actual native completed assistant turn`。
- 清理时 Ask 返回 `Ask input was cancelled`、assistant aborted；这不是已证明的 provider 错误，也不能当作用户拒绝成果批准。
- 后续驱动新增 Ask-first 识别和独占终端表单输入能力；安装 parser/实际组件/observer 离线检查为 3 tests、28 assertions，通过且 0 模型请求。
- 后续 OMP/auto Run `bc-97382a08bb1d` 实际走普通业务回复，31 次真实请求、serverModel=`gpt-6-luna`、transportFailed=false，四项覆盖成功；其 `nativeFormUsed=false`。因此它补齐正常交接覆盖，**没有证明真实 Ask 表单已可成功提交**。

后续验收：在合法业务输入下观察真实 pending Ask；驱动只填写业务折扣 25 并提交，记录真实工具返回、材料 6300、approve→report 顺序及后续交接。不得强制模型选择 Ask，不得把普通聊天回复或按键发送成功当作表单提交；错误／取消应明确 fail-closed。若模型未选 Ask，继续标此分支未覆盖，而不是重复模型请求刷绿。

证据：旧格 `ticket-13-authorized-corrected-fixture/business/auto/omp/evidence.json` 及 `failed-cell-boundaries.json`；后续 `ticket-13-omp-auto-native-form/{evidence.json,single-cell-summary.json}`；最终 ledger 的 `latestNativeCommand.nativeFormScope`。

## H13-03：单次完整四格矩阵全绿尚无证据

**状态：未覆盖的整体验收目标，不是已定位产品缺陷。**

- 原完整运行：99 次真实 SDK 请求，全部返回身份为 gpt-6-luna，transportFailed=false；Pi/auto、OMP/manual 整格通过，其余两格失败。
- 最新 OMP/auto 为 `--hosts omp --cases auto --native-only` 的独立补证，wrapper exit=1、passed=false；这是部分范围的既有退出约定，不表示该格业务失败，也不认证完整矩阵／全仓检查。
- 最终按分区组合的 ledger 通过，不能据此宣称有一次完整入口四格全流程全部成功。
- 若后续目标是“四格所有完整生命周期都成功”，修复 H13-01 并解决 H13-02 的真实验证缺口后，再按授权执行独立完整矩阵；不得覆盖历史失败、修改 coverage 数量或重试同一模型决定。

## 已处理的历史阻碍：不要重复当作当前业务缺陷

| 历史运行 | 当时阻碍 | 后续证据与边界 |
|---|---|---|
| `ticket-13-retry-native` | 后端监听落在 10577 等非 49152–65535 端口；四格准备失败，0 模型请求 | 后续独占动态数值回环监听已进入真实模型场景 |
| `ticket-13-native-dynamic-listeners` | 41 次真实调用后，等待批准前问题报告超时；Map 未声明命名输入 order，prepare_inputs 被 409 拒绝 | 修正 fixture 的 Map input_files；真实 HTTP 回归证明未声明输入被拒绝、声明后新 Run 可准备。不能声称这是 P20 业务通过 |
| corrected-fixture 的 OMP/auto | 合法 Ask 尚 pending，旧驱动等 assistant stop | 后续正常业务路径已通过；真实 Ask 缺口见 H13-02 |

## 证据保存与定位

本文证据根目录为协调仓的 [`evidence/ticket-13-handoff/`](evidence/ticket-13-handoff/)。上文所有 `ticket-13-*` 路径均相对此目录。

优先阅读：

1. `ticket-13-retry-global/final-contract-ledger.json`：最终按承诺分区的覆盖声明。
2. `ticket-13-retry-global/integration-authorized-result.json`：原完整运行明确的失败结论。
3. `ticket-13-authorized-corrected-fixture/cell-summary.json`：四格原结果、8/16、状态及错误。
4. H13-01/H13-02 指出的原格 evidence、HTTP、history、原生确认及业务材料。
5. `ticket-13-retry-global/check-ledger.json`：已有两仓检查，不需要为阅读／诊断重新跑全仓。

原来源：`C:/Users/hugos/AppData/Local/Temp/hyper-ticket-ZZlduZ/.scratch/node-result-approval/evidence/`。本次按原目录结构复制各相关运行顶层 JSON/log、各格 evidence.json、backend-wire、histories、native-confirmations、work-zone；未复制认证目录、依赖、编译产物或完整 model-wire／codex-completions。原证据内绝对路径保持不变，浏览本副本时按上述相对结构映射；不能假定其中指向临时业务目录的路径永久有效。

“保留原失败”指原日志／退出码／coverage 不被改写；后续运行另存，不给旧运行追记成功。证据包含本地路径、业务材料和会话记录，不宜直接公开上传。

## 后续工作约束

- 本次只交接，**不开始上述修复**。
- 相关入口：`scripts/accept-node-approval-handoff.py`、`scripts/node_approval_handoff_acceptance.py`、`scripts/accept-workflow-codex.py`、`scripts/node-approval-observer.mjs`；验证规范见 `docs/agents/testing.md`。
- 先以保留证据诊断，再选择最窄回归；缺模型／宿主时明确未验证，不使用受控成功替代真实业务。
- 沿用独占 Run/Chat/目录/端口/transport，授权认证只读；不改变产品 timeout/retry，不复用用户历史会话。
- 原完整矩阵、后续独立补证、最终 ledger 必须分开报告。清理错误与原业务错误分别记录。
