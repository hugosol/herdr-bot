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

## 未解决事项一：真实修订专项

已归档回归报告明确保留既有 declared 失败与双宿主中断，不再自动补跑付费矩阵。该报告的 `businessAcceptance.newModelRequests` 为0。

此前会话已经观察到未完成基线、修订 workflow 未完成、user boundary 超时等失败；这些仅是历史现象，尚未形成根因结论。原完整场景证据未随本次交接归档，不能把摘要中的每种现象视为对当前源码的已复现缺陷。

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

原真实场景根为 `C:/Users/hugos/AppData/Local/Temp/hyper-ticket-ffG0QG/.scratch/workflow-nesting/evidence/revise22-20261106/`；后续回归证据根为同级 `integrate22-20261106/`。这些只是历史定位信息，临时目录可能已被部分删除。本次归档的是两份可读集成报告，不是完整运行目录、原生历史、HTTP日志或进程快照。

编排曾把22记为 integrated，仅表示该流程已正常返回并合入，不能推导本票验收通过。13／14／23的failed状态、报告发布、retro和残留工作树清理是后续独立收尾问题；不得回滚已合入源码或因retro失败重跑实现。
