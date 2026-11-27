# 03: Map 公开规则、node 校验与编辑界面

**What to build:** 作者在现有 scheduler 中声明整图输入输出、维护 node 接线并保存读回；调用者按公开 ID 理解接口，不依赖子 Map 的内部节点布局。

**Delivers:** P5, P6, P7, P29, P31

**Blocked by:** None (can start immediately)

**Repositories:** bot

**Status:** ready-for-agent

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [ ] 贯通严格静态格式、公开保存与页面编辑：规则对象可整体省略，内部 description/files 的既定默认值、输入 required 显式布尔、输出声明即必交；两侧 ID 命名空间独立。拒绝 null、旧字段及未知字段。
- [ ] 保留 node 既定字段、默认值、空 input、固定/动态 zone、file/node/run 三类来源、目标/策略和路径/图校验；材料引用不得自动补 blocked_by。
- [ ] P7 本票拥有定义保存边界的同名公开输入、公开输出子集及未知 ID 拒绝 case；内部输出 ID 与公开 ID 不同也可合法。运行时交付 case 归 06。
- [ ] 真实编辑后重新加载，并将页面、公开读回和独立文件对象对照；省略规则与只写输出说明不能被 UI 变成必需公开文件。
- [ ] P29 本票拥有 Map/node 非法字面对象被拒绝且定义不变的 case；期望来自独立样例，不用被测验证器生成。同步更新相应系统文档、类型、示例及词汇。

## 检查与证据

从本票 bot worktree 运行覆盖 scheduler 定义保存的定向后端 case，并明确记录 target/选择器；页面使用 `npm --prefix frontend run test:browser:build -- schedulerNodeEditing.spec.ts --workers=1`，按需在同一现有业务分区补齐规则编辑/重载 case。不得仅以组件渲染或 schema 自测替代公开保存。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 03 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。

本票属于当前自动开发队列，完成自身适用实现与自动检查后即可交付；不等待 11–14，也不代做它们。保留真实失败/部分效果，更新本票范围文档及证据。
