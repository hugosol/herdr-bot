# 02: Bot 新格式、编辑与定义消费者整体切换

**What to build:** 用户能通过现有 Bot 界面保存、修改并重新读取四字段能力索引；在线创建/修订消费者和可信 Bun 脚本使用同一新格式，不依赖旧指南或兼容别名。

**Delivers:** P3, P29, P31, P38, P39, P53

**Blocked by:** None (can start immediately)

**Repositories:** bot

**Status:** ready-for-agent

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [ ] 贯通新四字段、稳定身份/分组及既定 Task/script/Map 互斥目标；允许全部目标缺省的不可运行草稿和空字符串。保留目标检查、相对 TypeScript 路径边界及未知字段拒绝，不解析 Markdown 标题作为保存门禁。
- [ ] 真实页面编辑、重新加载、公开 HTTP 与独立持久文件交叉核对；旧字段、未知字段、非法类型/路径、冲突目标和过期 revision 拒绝后不伪造成功或误写。
- [ ] 迁移直接受新 Bot 格式影响的创建、修订、材料 schema、说明和读写消费者；有价值的旧指南知识须有明确新归属，不整段改名或直接丢弃。不得替未选择流程决定新业务语义。
- [ ] 通过隔离 Map 派发真实 Bun，核实创建/保存、完整读回、revision/并发保护、命名文件、异常与部分效果；不以直接调用脚本函数替代执行链，不伪造 Chat，也不要求脚本解析 Run 背景。
- [ ] P29 本票拥有 Bot/脚本输入拒绝、保存已生效而读回/材料或通知失败的公开效果 case；保留真实文件和已保存报告，不承诺事务、回滚或自动重投。
- [ ] P53 本票拥有 backup 定义不会进入在线列表/读取且不会回退的隔离 case；移出在线范围的旧格式不需要兼容。实际数据移动由手动 14 授权执行。
- [ ] 同步更新本票范围的产品 schema、示例、系统说明和词汇；Map 规则形状归 03，若共享材料描述涉及它，遵循批准契约，不要求 03 先落地才能提交独立 Bot 工作。

## 检查与证据

产品 HTTP/文件：`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch typescript_`，以及同 target 的 `rust_create_`、`rust_save_`、`revise_bot_guides`、`shared_business` 对应 case。页面：`npm --prefix frontend run test:browser:build -- schedulerBotEditing.spec.ts --workers=1`。脚本表示：`python scripts/test-script-contracts.py`。新 Bot/backup case 纳入对应公开接口 target 并记录可精确复跑的选择器；覆盖表中的不同 case 不重复编写。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 02 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。

本票属于当前自动开发队列，完成自身适用实现与自动检查后即可交付；不等待 11–14，也不代做它们。保留真实失败/部分效果，更新本票范围文档及证据。
