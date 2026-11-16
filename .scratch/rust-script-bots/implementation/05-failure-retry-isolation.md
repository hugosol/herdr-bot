# 05: 交付脚本失败、显式重跑与旧执行隔离

**What to build:** manager 从真实失败报告获知错误与实际效果，显式 fail 后才能按原规则 retry；重跑创建独立执行，保留旧事实，旧报告不能赋予新执行完成资格。

**Delivers:** P13, P14, P19（自动化分区；真实 manager H2 由 11 准备）

**Blocked by:** 02 — 通过真实 Map 异步创建空白 Bot 并交接报告。

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 在真实 Map 中制造业务失败，自动保存 canComplete=false、可得错误和实际效果；业务结束不自动将 node 改为 failed，不自动 retry、complete 或回滚。
- [ ] 对照执行失败报告前后读回节点状态；未按现有规则显式 fail 或缺少 retry 必要确认/原因时重跑拒绝，拒绝不新增执行或业务效果。
- [ ] manager 合法 fail/retry 产生新 execution 和本次真实材料/报告，previousExecutions 保留旧执行、失败、文件及报告；不创建脚本 Chat 或虚构 session。
- [ ] 已结束的脚本执行不能通过 recover 恢复，不启动新业务、不改成待继续会话；不破坏普通 Task/Map 原有 recover 规则。
- [ ] 构造新旧执行报告交错：旧 true 晚到、新执行无报告、新 false、再新 true 等场景。每份报告和输出保持原 execution 归属，pendingReports/collect 仅适用当前执行，首次 complete 不从旧 true 或较早 true 获资格。
- [ ] 重跑不继承旧输出；相同 output ID 的真实不同文件和独立消费者证明当前材料归属。不从其他 node/Run 或旧报告补齐。
- [ ] 更新现有运行文档，区分执行结束、失败报告、manager fail、recover、retry 和节点交接，不把脚本业务失败解释成自动状态动作。

## Coverage partition and ownership

本票唯一拥有 P13/P14 的 S1 自动化及 P19 全部自动化；P13/P14 的真实 S2 H2 由 11 准备、用户确认。

03 唯一拥有 P15“保存后材料失败并以原 baseline 重跑冲突”的业务用例，本票提供合法 fail/retry 通路但不复制该测试。11 在两票合入后执行该组合用例。故障期页面观察归 09，不以页面测试替代本票状态门禁。

## Verification

沿既有 S1 公开 manager 动作、真实文件、报告和完整执行读回验证，不直接修改运行记录制造可绕过的资格。用受控故障/报告提交边界制造交错，不新增可指定任意脚本 execution 的公共工具。使用独占数据、回环服务和本 worktree 独立绝对 CARGO_TARGET_DIR，先构建静态页，再运行 `node scripts/test-backend.mjs`；相关真实子进程按测试指引确认退出后清理。
