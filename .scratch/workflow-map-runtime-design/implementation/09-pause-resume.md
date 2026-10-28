# 09: 暂停新执行与恢复派发资格

**What to build:** 管理者 pause 后不再允许新 dispatch，但已有执行可报告和完成；resume 只恢复资格，不自动发起工作。

**Delivers:** P18, P20, P35, P36, P37, P68

**Test owner for:** P18.03, P20.03, P35.02, P36.01, P68.04（仅本票所列分区；整个 promise 须全部分区完成）。

**Blocked by:** 07 — 显式交接完成、解除依赖与 finish

**Status:** ready-for-agent

## 契约与范围

依据：[approved contract](../contract.md) 与其继承的 [spec](../spec.md)。P 编号与 S1–S4 的含义以 contract 为准；本票不重新定义 promise 或新增观察 seam。

状态动作及重复调用随能力交付；暂停不要求 Chat 退出。paused 可在全部完成后 finish。collect 对暂停的返回由 collect 票覆盖，paused retry 由 retry 票覆盖，批次与 pause 的并发先后由串行票覆盖。

仅交付本票的覆盖分区；共享 promise 不因某一分区通过就算整体完成。实现前沿是所有列出的阻塞票均完成的票，不按编号强制全串行。

## 验收标准

- [ ] **P18.03**（本票编写并执行的契约检查）：Run paused 时 dispatch 拒绝且无外部启动。
- [ ] **P20.03**（本票编写并执行的契约检查）：重复 pause/resume 不重复生效或覆盖首次事实；对比外部调用及历史。
- [ ] **P35.02**（本票编写并执行的契约检查）：Run paused 时全部 completed 同样可 finish，不需先恢复派发资格或等待 Chat 退出；终态拒绝 pause/resume，重复 finish 保留原事实。
- [ ] **P36.01**（本票编写并执行的契约检查）：pause 生效后 dispatch 拒绝，但既有 Chat 可 report/complete，不要求退出。
- [ ] **P37.01**（本票编写并执行的契约检查）：resume 只恢复派发资格；状态与有界外部调用观察证明不自动派发，后续显式 dispatch 才启动。
- [ ] **P68.04**（本票编写并执行的契约检查）：resume 后无自主 dispatch，直到显式请求；pause 不自主终止既有 Chat。
- [ ] 完成上述能力所需调用者、错误反馈和角色/用户指引；不留占位实现、假成功、运行时旧格式兼容或未接线入口。
- [ ] 保留本票真实入口的可重复验证证据及检查结果；相关长期说明随能力更新，不只更新临时票据。

## 验证与证据

实际 pause 后尝试 dispatch、报告与 complete，再 resume；观察恢复后直到显式 dispatch 前没有新增启动调用。

遵守仓库测试指引：复用真实 HTTP TestServer、现有前端/npm 浏览器入口及 toolkit 公共 interface；页面能力观察实际交付页面，宿主能力需要真实 Herdr/omp 证据。受控 adapter 只证明确定性故障/时序，不证明模型或原生宿主行为。

普通检查只用数值回环、独占临时数据/cwd/动态端口与必要 Herdr 副本，持续消费并清理所属子进程；不操作用户对话或产物，不绕过 npm 网络护栏。LAN/mDNS、真实符号链接和关机仍遵守显式人工授权规则。

自动化预期取自契约、独立字面样例或预先算定结果，不用被测实现重算预期。负向自动行为检查注明有界观察窗口并保留外部调用证据；取消/错误不得伪造快照或自动重试。真实环境缺失如实报告阻塞，不以替身或跳过宣称通过。

## 共享 promise 的 coverage partition

下面列出本票涉及的共享 promise 的完整分区。每个分区只有一个交付票和测试 owner；其他票的行是依赖说明，不是本票重复认领的验收项。各分区最终均通过后 promise 才完成。

| Promise / coverage item | Coverage | Ticket | Test owner |
| --- | --- | --- | --- |
| P18.01 | 目标非 pending（已派发 running 或派发失败 failed）时 dispatch 拒绝；观察外部启动调用缺席，而非只断言报错。 | 05 | 05 |
| P18.02 | A/B→C 仅 A completed 时 C 拒绝；Run completed 时 dispatch 拒绝；联合观察外部启动调用缺席。 | 07 | 07 |
| P18.03 | Run paused 时 dispatch 拒绝且无外部启动。 | 09 | 09 |
| P20.01 | 正常已记录派发的重复请求不增加外部启动，保留首次事实；明确崩溃/关联失败窗口不承诺完整幂等。 | 05 | 05 |
| P20.02 | 重复 complete 和 finish 不重复生效或覆盖首次说明、成功历史及终态事实。 | 07 | 07 |
| P20.03 | 重复 pause/resume 不重复生效或覆盖首次事实；对比外部调用及历史。 | 09 | 09 |
| P20.04 | 重复 fail 保留首次失败说明和历史，不重复产生状态效果或外部动作。 | 12 | 12 |
| P35.01 | Run running 时全部 completed 可 finish，未完成拒绝；重复 finish 返回原事实，终态拒绝变更且可查询；Chat 活跃不阻止成功。 | 07 | 07 |
| P35.02 | Run paused 时全部 completed 同样可 finish，不需先恢复派发资格或等待 Chat 退出；终态拒绝 pause/resume，重复 finish 保留原事实。 | 09 | 09 |
| P35.03 | Run 终态拒绝 fail、reconcile_dispatch、recover 和人工 complete，保留首次终态事实与外部调用不变。 | 12 | 12 |
| P35.04 | Run 终态拒绝 retry，不创建新 Chat 或改写旧执行事实。 | 13 | 13 |
| P36.01 | pause 生效后 dispatch 拒绝，但既有 Chat 可 report/complete，不要求退出。 | 09 | 09 |
| P36.02 | pause 排在完整在途批次后生效，不中途撤回已启动目标。 | 11 | 11 |
| P36.03 | paused 时 retry 明确拒绝且无新 Chat；必须先显式 resume。 | 13 | 13 |
| P68.01 | 页面刷新/重新进入的有界窗口无重复 Run/manager 启动，保存外部调用证据。 | 04 | 04 |
| P68.02 | 通知失败/诊断失败的有界窗口无自主重投或仅因投递失败自动重报。 | 06 | 06 |
| P68.03 | report/停轮/退出不自动完成，complete 后不自主派发下游，保留观察窗口与外部调用证据。 | 07 | 07 |
| P68.04 | resume 后无自主 dispatch，直到显式请求；pause 不自主终止既有 Chat。 | 09 | 09 |
| P68.05 | collect 网络失败/取消/到期不自动重试、改变状态或伪造正常快照，窗口与调用证据明确。 | 10 | 10 |
| P68.06 | 派发/执行失败不自主重试、暂停或终止 Chat；核实/recover/人工 complete 不启动新执行或补发材料。 | 12 | 12 |
| P68.07 | retry 不撤销旧执行副作用、不自动结束旧 Chat，不借旧报告自动完成新执行。 | 13 | 13 |
| P68.08 | 后端重启不自动接管、改写旧状态或发起执行，保留有界外部调用观察。 | 16 | 16 |
| P68.09 | Resume 不接管 Run、不重放 trigger config，不宣称有限观察证明无限期不存在自动行为。 | 18 | 18 |

## Comments

尚未实施或验收；本票发布不代表其 promise 已通过。
