# 04: Run 冻结定义、历史展示与 Map 删除

**What to build:** 每个新 Run 在管理者执行前拥有完整 Map 副本，后续操作与 workflow 都读取它；修改或删除原 Map 不篡改该 Run，旧 Run 原始归档也不被补造为新版历史。

**Delivers:** P11, P12, P13, P14, P30, P32, P55

**Blocked by:** 03 Map 公开规则、node 校验与编辑界面

**Repositories:** bot

**Status:** ready-for-agent

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [ ] 启动前完成校验和副本落盘，再允许管理者执行；管理者取得副本绝对地址。保存/启动失败如实反馈，不冒充可执行。副本不可编辑，不建立同步或版本系统。
- [ ] 本票拥有 P12 全部产品侧冻结一致性 case：原 Map 修改后，派发、retry、prepare_inputs、finish 使用原副本，新 Run 使用新定义。采用独立旧/新图及规则；05/06 后续增加的新规则必须接入同一读取入口，不另复制这组测试。
- [ ] 副本缺失/损坏时 HTTP 和真实 workflow 页面明确失败，即使原 Map 仍合法也不回退；workflow 显示冻结图、实际输入和公开成果，刷新/查看不增加启动或派发次数。05/06 负责新增实际值的装配，沿用本票页面契约测试补齐显示断言。
- [ ] 子 Run 只在自己启动时冻结子 Map；父启动后修改子定义应影响后来启动的子 Run。不冻结 Bot、Task、script、Skill、文档、文件内容或环境。
- [ ] 删除 Map 的页面、请求和后端分支不再支持 deleteHistory/连带删除历史；真实删除后 Run、副本、列表及详情保留，独立 Run 删除能力不取消。
- [ ] P55 本票拥有隔离归档 case：旧 Run 全目录原始文件逐项/逐字节保留在在线范围外，在线列表/读取排除，不补造副本、成果或跨版本管理关联。操作只作用于隔离数据，不新增自动迁移工具；现场归档证据归手动 14。

## 检查与证据

产品：`cargo test --manifest-path backend/Cargo.toml --test workflow_start --test workflow_dispatch` 中冻结、retry、嵌套及相应故障 case，发布精确选择器。页面：`npm --prefix frontend run test:browser:build -- workflowCurrent.spec.ts workflowRefresh.spec.ts workflowHistory.spec.ts --workers=1`，并运行 scheduler 删除对应定向 case。P12 的后续规则反例须在 05/06 最后改动后复跑，不能沿用旧通过记录。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 04 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。

本票属于当前自动开发队列，完成自身适用实现与自动检查后即可交付；不等待 11–14，也不代做它们。保留真实失败/部分效果，更新本票范围文档及证据。
