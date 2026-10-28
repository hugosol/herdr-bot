# 17: workflow 列表、只读图与当前节点详情

**What to build:** 用户在 workflow 查看全部 Run、只读状态图和当前节点工作/报告/失败原因；旧 Run 明确未接管，当前 Map 不可对应时保留事实而不伪造历史图。

**Delivers:** P50, P51, P52, P57

**Test owner for:** P52.01, P57.01（仅本票所列分区；整个 promise 须全部分区完成）。

**Blocked by:** 07 — 显式交接完成、解除依赖与 finish；12 — 失败核实、原执行 recover 与人工交接；16 — 后端重启后旧 Run 仅可读取

**Status:** in-progress

## 契约与范围

依据：[approved contract](../contract.md) 与其继承的 [spec](../spec.md)。P 编号与 S1–S4 的含义以 contract 为准；本票不重新定义 promise 或新增观察 seam。

复用既有图布局和依赖方向，manager 不作为 node。状态文字配合标记/边框/填充，选中态独立，不以 Chat 在线状态替代。提供基础手动读取；不新增流程操作按钮、搜索、分组、筛选或历史定义快照。当前/旧 Chat 的实际 Resume 和折叠执行历史由后续票完成。

仅交付本票的覆盖分区；共享 promise 不因某一分区通过就算整体完成。实现前沿是所有列出的阻塞票均完成的票，不按编号强制全串行。

## 验收标准

- [x] **P50.01**（本票编写并执行的契约检查）：独立多 Map、多 Run、不同时间/状态数据在实际 workflow 倒序完整展示，含完成数量和未接管提示；不新增筛选、分组或操作按钮。
- [x] **P51.01**（本票编写并执行的契约检查）：实际图用状态文字、标记、边框/填充及独立选中态表达状态；故意令 Chat 在线状态不同于 node 状态，依赖方向正确、manager 不作为 node，不只依赖颜色。
- [x] **P52.01**（本票编写并执行的契约检查）：当前定义/执行详情展示 Bot/Task、zone/input、依赖、阻塞/失败、完成说明与适用报告正文/时间；无关联明确显示且不隐式创建执行。
- [x] **P57.01**（本票编写并执行的契约检查）：图注明来自当前 Map；定义丢失、不可解析或 node 无法可靠对应时不补造历史图，保留可读事实/user_input/报告/历史关联，缺名显示保存标识。
- [x] 完成上述能力所需调用者、错误反馈和角色/用户指引；不留占位实现、假成功、运行时旧格式兼容或未接线入口。
- [x] 保留本票真实入口的可重复验证证据及检查结果；相关长期说明随能力更新，不只更新临时票据。

## 验证与证据

在交付浏览器页面查看多 Map、多状态、Chat 状态故意不同的样例；删除/损坏或改为无法对应的当前定义，观察事实仍可读、图不伪造且缺名显示保存标识。

遵守仓库测试指引：复用真实 HTTP TestServer、现有前端/npm 浏览器入口及 toolkit 公共 interface；页面能力观察实际交付页面，宿主能力需要真实 Herdr/omp 证据。受控 adapter 只证明确定性故障/时序，不证明模型或原生宿主行为。

普通检查只用数值回环、独占临时数据/cwd/动态端口与必要 Herdr 副本，持续消费并清理所属子进程；不操作用户对话或产物，不绕过 npm 网络护栏。LAN/mDNS、真实符号链接和关机仍遵守显式人工授权规则。

自动化预期取自契约、独立字面样例或预先算定结果，不用被测实现重算预期。负向自动行为检查注明有界观察窗口并保留外部调用证据；取消/错误不得伪造快照或自动重试。真实环境缺失如实报告阻塞，不以替身或跳过宣称通过。

## 共享 promise 的 coverage partition

下面列出本票涉及的共享 promise 的完整分区。每个分区只有一个交付票和测试 owner；其他票的行是依赖说明，不是本票重复认领的验收项。各分区最终均通过后 promise 才完成。

| Promise / coverage item | Coverage | Ticket | Test owner |
| --- | --- | --- | --- |
| P52.01 | 当前定义/执行详情展示 Bot/Task、zone/input、依赖、阻塞/失败、完成说明与适用报告正文/时间；无关联明确显示且不隐式创建执行。 | 17 | 17 |
| P52.02 | 独立新旧执行样例区分当前适用报告与旧执行历史，折叠历史及 manager/当前/旧 Chat 入口均指向正确关联。 | 18 | 18 |
| P57.01 | 图注明来自当前 Map；定义丢失、不可解析或 node 无法可靠对应时不补造历史图，保留可读事实/user_input/报告/历史关联，缺名显示保存标识。 | 17 | 17 |
| P57.02 | 当前定义不可用时，仍可查看已保存执行历史并进入已知 manager/当前/旧 Chat 的实际 Resume，不以补造历史图作为前提。 | 18 | 18 |

## Comments

2026-10-29：本票实现及所属验收完成，等待独立集成；按仓库规则在合入前保持 `in-progress`，不自行标记 merged/closed。P52/P57 只认领本票分区，不代表后续 Resume/折叠历史或轮询票已完成。

- P50.01：`workflowCurrent.spec.ts` 的真实列表用例；两个 Map、三个不同创建时间/状态 Run，逐行字面核对顺序、完成数量、未接管及刷新链接。桌面/390px 截图与页面无外溢检查。
- P51.01/P52.01：同文件 current Map 用例；实际 Chat 在线/工作中而节点 completed，另一个无在线 Chat 的节点保持 running；检查 SVG 实际绘制端点、标记/边框/文字/选中态、Bot/Task 与当前定义和已保存执行差异、阻塞/失败、交接说明和报告时间/正文、无关联提示。750ms 外部调用及三份记录前后不变。
- P57.01：missing/invalid/mismatch 三个真实文件变更用例；从正常图开始，手动刷新后无图、无误配新名称，保留 user_input/失败/当前与旧执行 Task/zone/Chat/报告。截图和 JSON 保存于 `frontend/test-results/workflowCurrent-*/`（可用命令重建）。
- TDD 实际 red：列表测试找不到全部 Run 表；图详情测试找不到当前 Map 来源说明；missing/invalid 测试缺少旧报告，mismatch 测试仍画图。实现后所属五例全部 green；在线状态检查使用已有 aria 图片语义，重启说明断言限定到详情段落避免新增列表行歧义，未改变验收预期。
- 最终交付页面：`npm --prefix frontend run test:browser:build -- workflowCurrent.spec.ts workflowStart.spec.ts schedulerGraph.spec.ts --workers=1`，12/12 通过（其中本票5例）；构建包含 TypeScript、Vite 与后端二进制，网络护栏通过。
- 前端回归：`npm --prefix frontend test -- --maxWorkers=4`，27文件/205测试通过。
- 浏览器分区：`npm --prefix frontend run test:browser:partition`，311 = 278 fast + 33 time-contract，集合互斥且完整，通过。
- 完整后端：`node scripts/test-backend.mjs`，所有已发现二进制与 doctest 通过，0未派发；日志及 summary 位于 `C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-MQKpKg/`。
- 长期用户/角色说明已更新 `docs/workflow-runtime.md`。本票不需要真实模型/宿主执行质量评价，浏览器通过的是交付页面和真实 HTTP/文件行为；受控 Herdr 记录仅支持有界无隐式启动结论，不冒称实际 Resume 或 toolkit 原生能力验收。未运行全量311例浏览器套件；本票所属验收无未完成项。
