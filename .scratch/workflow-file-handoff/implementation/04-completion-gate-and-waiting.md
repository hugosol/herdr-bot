# 04: 最新报告完成门禁与等待语义

**What to build:** manager 首次 complete 只有在当前 execution 最新报告严格声明 can_complete=true 时才能通过新增门禁，且仍遵守既有身份与状态规则。拒绝给出明确原因及适用报告，不改变运行事实。补充报告不撤销已完成节点；report 不自动推进，collect 和既有 workflow 展示继续使用同一 Run 事实。

**Delivers:** P14, P15, P16, P18, P19, P29；P17, P20 的下列覆盖项。

**Blocked by:** 02: report 完成声明与单一来源输出登记。

**Repositories:** herdr-bot

**Status:** ready-for-agent

## 契约与覆盖归属

依据 [contract](../contract.md) 与 [spec](../spec.md)，经 S1、S3 验证。复用现有同 Run 操作协调和文件持久化；不在 toolkit 做有竞争窗口的替代预检，不新增 force 或人工绕过。

| Promise | 本票覆盖项 | Test owner | 其余覆盖归属 |
|---|---|---|---|
| P14 | 当前最新报告严格 true 门禁的全部边界及原状态规则 | 04 | 无 |
| P15 | 拒绝前后状态／历史不变、原因及相关最新报告 ID，无报告时明确不存在 | 04 | 无 |
| P16 | false 先保存则拒绝；完成先生效则后续 false 不追溯撤销 | 04 | 无 |
| P17 | report(true) 无自动完成／派发；后续显式动作才转换 | 04 | 真实工具顺序：07 |
| P18 | 完成后 false 与重复 complete 不覆盖首次事实、不重复副作用 | 04 | 无 |
| P19 | 既有适用人工完成分支也受门禁，无 force 绕过 | 04 | 无 |
| P20 | 新通知等待、保留待办、超时／取消／状态变化、拒绝不造通知 | 04 | 真实 manager collect 行为：07 |
| P29 | 既有 workflow 回归呈现同一 Run 的完成及报告事实，无新来源／页面动作 | 04 | 无 |

## Acceptance criteria

- [ ] 覆盖 true、false、无报告、历史缺字段、旧执行 true、新执行 false、true 后 false。只取当前 execution 最新报告，不回退较早 true；无文件输出不影响门禁。
- [ ] 保留原身份、终态及节点状态规则，不将 can_complete 当质量证明、用户批准凭据或新状态。既有可适用执行失败人工完成分支在无当前 true 报告时同样拒绝。
- [ ] 门禁拒绝明确原因和相关最新报告信息；没有报告时不伪造引用。比较拒绝前后的当前事实、历史及外部效果，确认无状态转换或新报告通知。
- [ ] 报告保存与首次完成在现有协调边界内具有可解释先后；以受控并发请求、实际提交记录和对外结果验证两种顺序，不断言私有锁实现，也不增加跨文件事务。
- [ ] report(true) 后状态和宿主启动调用不自动变化；manager 显式 complete 后才解除相应依赖，显式 dispatch 后才启动下游。
- [ ] 节点完成后补 false 再重复 complete，保留首次完成说明、身份、时间、历史和既有幂等结果，不重复触发自动关闭等完成副作用。
- [ ] 保留 collect 的 generation／checked 语义：消费既存通知后仍有 running node 时等待，即使同一 pendingReports 仍保留；新报告才触发通知返回。复用并保留超时、取消和状态变化覆盖，不假设现有待办重复唤醒 bug。
- [ ] 复用既有 workflow 展示回归，核对完成与报告来自同一 Run；不增加页面动作或状态来源，不以 rendered completed 替代门禁证据。
- [ ] 同步迁移受影响的后端完成调用场景与长期文档。删除与新业务规则冲突的旧断言，不通过统一伪造 true 或只修改措辞使旧测试通过；toolkit 调用方由 05 迁移。

## Verification

在本票 worktree 根运行 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture` 及受影响的启动／恢复检查，再运行 `node scripts/test-backend.mjs`。执行公开 HTTP 的 false→拒绝→新 true→显式 complete 冒烟并核对持久记录。P29 运行 `npm --prefix frontend test` 及 `npm --prefix frontend run test:browser:build`，复用既有 workflow 场景，不新增浏览器 seam 或文本快照测试。并发和通知场景用事件／提交证据协调，不以任意 sleep 代替先后成立的证明。
