# 10: OMP/Pi chief 委派与报告工具适配

**What to build:** OMP/Pi chief 和实际受委派执行者沿用原生预览确认，以同一协议提交输出要求、读取或报告结构化成果，并区分保存和通知结果。

**Delivers:** enabling: unblocks P22, P23, P24, P25, P26, P27, P28, P34, P35, P37

**Blocked by:** None (can start immediately)

**Repositories:** toolkit

**Status:** ready-for-agent

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [ ] chief_delegate 支持本次 output_rule，保持 Task/Map 范围、预览、变化核对、原生确认/取消与异步返回，不新增 script 分支。
- [ ] chief_report 支持结构化 outputs、保留原说明和结果分类；Task 正式文件、Map 完成后取公开子集、中途失败/受阻无文件分开表达。不新增 can_complete、进度枚举或所有 success 的统一 finish 门禁。
- [ ] manager-chat 与独立 chief-report 路径共享同一 chief 报告指引/动态回执来源，OMP/Pi 消费同一业务政策；保留独立报告/重发的原生确认，普通节点 report 不获得发送确认。
- [ ] chief_read_report 支持按完整报告及真实文件呈现陌生成果；不硬编码能力建设入口，不按 ID 猜内容，不自主扫描积压或追加委派。身份/接收确认仍由系统负责。
- [ ] 统一 schema、工具/参数说明、初始化和动态 guidance；保存失败/不确定与通知失败分别反馈，不建议重做业务或自动重投。工具包本地回归与跨仓/真实原生证据分开。同步工具包文档。

## 检查与证据

在本票 toolkit worktree 运行 `bun run typecheck`、`bun run check:integrations` 及受影响 workflow-chat chief/共享报告测试；涉及原生确认的 dock 按仓库约定绑定本次安全壳并保存具名结果。实际请求进入产品的契约 case 归手动 11，tester 真实安装格归手动 13。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 10 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。

本票属于当前自动开发队列，完成自身适用实现与自动检查后即可交付；不等待 11–14，也不代做它们。保留真实失败/部分效果，更新本票范围文档及证据。
