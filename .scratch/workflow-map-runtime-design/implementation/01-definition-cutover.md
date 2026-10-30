# 01: Map 与 Bot 唯一新格式切换

**What to build:** 规划者通过现有 scheduler 编辑并读取唯一新格式的 Map、node 和共享 Bot；未完成的外部绑定可以保存，非法内部结构不能覆盖原定义。

**Delivers:** P1, P2, P3, P4

**Test owner for:** P2.01（仅本票所列分区；整个 promise 须全部分区完成）。

**Blocked by:** None (can start immediately)

**Status:** in-progress

## 契约与范围

依据：[approved contract](../contract.md) 与其继承的 [spec](../spec.md)。P 编号与 S1–S4 的含义以 contract 为准；本票不重新定义 promise 或新增观察 seam。

同步切换定义读写、现有编辑入口、示例与调用者。Bot 三份文档独立；不复制 Task/Chat，不保存运行事实，不保留旧字段或版本兼容。沿用现有依赖方向和共享 Bot 引用。

仅交付本票的覆盖分区；共享 promise 不因某一分区通过就算整体完成。实现前沿是所有列出的阻塞票均完成的票，不按编号强制全串行。

## 验收标准

- [x] **P1.01**（本票编写并执行的契约检查）：独立完整定义样例经真实读写入口往返；拒绝旧字段和版本；Map/node/Bot 只保存职责内定义，核对 Task/Chat 引用而非复制运行事实。
- [x] **P2.01**（本票编写并执行的契约检查）：三份 Bot 文档使用互异标记分别读取；允许空文本独立保存，不依赖其他文档已读，也不把 Markdown 当校验规则。
- [x] **P3.01**（本票编写并执行的契约检查）：分别保存 Bot 缺 task_id、node 引用不存在 Bot、无效 Task/zone 的内部合法定义，确认没有制造默认绑定。
- [x] **P4.01**（本票编写并执行的契约检查）：未知字段、错误类型、缺必需字段、不存在的同图依赖和环均明确拒绝，联合核对原定义未被错误覆盖。
- [x] 完成上述能力所需调用者、错误反馈和角色/用户指引；不留占位实现、假成功、运行时旧格式兼容或未接线入口。
- [x] 保留本票真实入口的可重复验证证据及检查结果；相关长期说明随能力更新，不只更新临时票据。

## 验证与证据

在真实 scheduler 保存一份合法但尚未绑定 Task 的定义，分别读取三份文档，再提交非法依赖，观察原定义保持不变。

遵守仓库测试指引：复用真实 HTTP TestServer、现有前端/npm 浏览器入口及 toolkit 公共 interface；页面能力观察实际交付页面，宿主能力需要真实 Herdr/omp 证据。受控 adapter 只证明确定性故障/时序，不证明模型或原生宿主行为。

普通检查只用数值回环、独占临时数据/cwd/动态端口与必要 Herdr 副本，持续消费并清理所属子进程；不操作用户对话或产物，不绕过 npm 网络护栏。LAN/mDNS、真实符号链接和关机仍遵守显式人工授权规则。

自动化预期取自契约、独立字面样例或预先算定结果，不用被测实现重算预期。负向自动行为检查注明有界观察窗口并保留外部调用证据；取消/错误不得伪造快照或自动重试。真实环境缺失如实报告阻塞，不以替身或跳过宣称通过。

## 共享 promise 的 coverage partition

下面列出本票涉及的共享 promise 的完整分区。每个分区只有一个交付票和测试 owner；其他票的行是依赖说明，不是本票重复认领的验收项。各分区最终均通过后 promise 才完成。

| Promise / coverage item | Coverage | Ticket | Test owner |
| --- | --- | --- | --- |
| P2.01 | 三份 Bot 文档使用互异标记分别读取；允许空文本独立保存，不依赖其他文档已读，也不把 Markdown 当校验规则。 | 01 | 01 |
| P2.02 | 使用三份空文档的合法绑定定义成功启动；启动不以文档内容作为业务质量门禁。 | 04 | 04 |

## Comments

2026-10-28：本票实现与 P1.01／P2.01／P3.01／P4.01 已通过真实 scheduler／HTTP／文件检查；尚未合入集成分支，按仓库规则保留 in-progress，由独立集成代理接管后关闭。P2.02 属于票据 04，未在本票认领或宣称通过。

- 新增 `frontend/browser-tests/schedulerDefinitionCutover.spec.ts` 三项契约场景：未绑定 Bot 与独立文档读取；非法／旧 Bot 定义拒绝并保留原文件；未绑定 node 保存、非法依赖与环拒绝并保留整个 Map。`schedulerNodeEditing.spec.ts` 另验证 Markdown 指引不校验空 input；既有 scheduler 严格格式、编辑、删除、重载、目录边界及样例用例同步切换。
- TDD：新 Bot 格式最初保存失败、编辑器未关闭（artifact://20）；新 Map 最初无法显示 node（artifact://34）；最小切换后两项通过（artifact://39）。最后定向契约检查 `npm --prefix frontend run test:browser -- schedulerDefinitionCutover.spec.ts --workers=1`：3 passed。
- 完整浏览器 `npm --prefix frontend run test:browser:build -- --workers=4`：289 passed、1 skipped（未授权真实文件符号链接）、1 failed。唯一失败在未改的 `tasks.spec.ts:192`，实际 Skill 选项多出 wayfinder；已交给 Task 票所有者与 Main，未更改其他票拥有的文件。完整结果见 artifact://88，不能宣称整个浏览器回归通过。本票 scheduler 全部用例在该轮通过。
- `node scripts/test-backend.mjs --binary-jobs 2`：passed，含 build、全部已派发测试二进制和 doctest；summary 位于 `C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-8MaBvZ/summary.json`。
- `npm --prefix frontend test`：27 files、205 tests passed；以上 npm 检查均经过既有网络护栏。
- 最后定向场景保留实际截图／JSON：`frontend/test-results/schedulerDefinitionCutover-f12c3-ntly-without-a-Task-binding/` 下的 `unbound-bot-documents.png`、`saved-unbound-bot.json`；`schedulerDefinitionCutover-ddfd2-alid-edits-preserve-the-Map/` 下的 `invalid-dependency-preserves-map.png`、`preserved-map.json`、`external-launch-calls.json`（实际为 []）。外部调用缺席只观察测试启动至最后定义响应的有界窗口，不证明原生 Herdr／omp 或模型业务质量。
- 长期文档更新 `docs/scheduler.md`、`docs/example/README.md`、独立 Map／Bot 样例和 GLOSSARY 中 node／Bot 责任说明。历史离线更名迁移工具保留其原职责，明确不完成本次格式切换；运行时无旧格式兼容。
