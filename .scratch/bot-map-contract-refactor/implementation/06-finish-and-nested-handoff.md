# 06: 公开成果 finish 与父子 Run 显式交接

**What to build:** 管理者一次提交全部公开成果并显式结束；父节点按同名公开 ID 取得所需子集，中途报告、成果批准、报告通知和节点完成仍是独立动作。

**Delivers:** P7, P15, P16, P17, P18, P19, P20, P21, P27, P29

**Blocked by:** 05 输入准备、实际上下文与真实角色恢复

**Repositories:** bot

**Status:** ready-for-agent

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [ ] 通过公开 finish 参数 map_output.files、HTTP/持久 mapOutput.files 完成全链路，身份来自真实 Session/Run。无公开文件声明可省略列表；有声明必须一次交齐。
- [ ] 重复/未声明/缺失/非法/不可读文件逐一构造反例，拒绝不完成 Run、不伪造成功。再次 finish 不改写首次完成事实和成果。持久化部分效果如实保留，不新增事务或回滚。
- [ ] P7 运行时 case 使用同名公开输入和输出子集；内部节点输出名/地址与公开成果不同。子 Run 保存全部公开成果，父 execution 只取得冻结 outputRule 的同名子集，不重复维护或接受报告覆盖地址。
- [ ] 逐动作读回证明子 finish 不自动批准父成果、不报告、不 complete 父节点、不派发下游。子 Run 未 finish 与普通节点均可无文件 can_complete=false 报告；普通 Task 正式成果仍按原协议登记并批准。
- [ ] 结束节点成功发起只表示交给用户继续，不等待其普通节点报告；管理者仍显式 finish，不能欠交公开必交文件或依赖未来用户对话补交。
- [ ] P27 本票拥有节点报告保存/通知受控失败 case：保存保留、通知单独反馈，有限窗口内无重投或业务重做。P29 本票拥有 finish 校验/持久故障、父交接失败的独立效果 case。
- [ ] 所有规则从本 Run 副本读取，补齐 04 已拥有的 P12 finish/准备反例；通过真实公开接口和独立文件字节观察，不调用私有算法生成期望。同步更新运行协议说明。

## 检查与证据

从本票 bot worktree 运行 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch` 中 `nested_completion::`、`nesting::`、`nested_retry::`、`end_node::`、报告通知及新增 finish case；补齐并复跑 04 的冻结选择器。分别保留状态、HTTP、实际文件和外部调用证据，不把最终 completed 当作全部交接通过。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 06 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。

本票属于当前自动开发队列，完成自身适用实现与自动检查后即可交付；不等待 11–14，也不代做它们。保留真实失败/部分效果，更新本票范围文档及证据。
