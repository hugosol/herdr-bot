# 01: 集中既有角色说明与消息装配

**What to build:** 先把现有 Task、Run 和 chief 路径的通用角色正文、运行事实格式化、词典及消息组合集中到既有 prompts 模块，保持调用者可观察行为，为后续协议切换提供单一维护入口。

**Delivers:** enabling: unblocks P1, P10, P33, P34, P36

**Blocked by:** None (can start immediately)

**Repositories:** bot

**Status:** in-progress

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [x] 迁移实际拼接逻辑及其调用者，不保留散落正文、仅转发的第二入口或重复角色政策；复用批准的既有模块组织决定。
- [x] 调用者仍拥有身份判定、状态准入、材料解析与 Chat 启动；不把这些职责搬入提示词模块。
- [x] 保持独立 Task 的初始 Skill、固定设定、用户输入、根/子管理者差异、chief 路径、结束节点例外与真实恢复关联。
- [x] 词典仍仅作用于既定类型字段，每条消息采用同一快照；不新建模板引擎、角色框架或运行时正文部署依赖。
- [x] 这是保行为 prefactor，不提前实现新业务政策；后续 05 负责新语义的公开提交内容测试。

## 检查与证据

- 集中既有词典、消息组合、运行事实格式及角色正文。根与子管理者共用事实与结束节点协议的装配，保留两条路径的既有措辞与子 Run 向父层交接差异；chief Map 复用根管理者装配，chief Task 保留委派上下文及经确认的预览文本。权威判定、材料解析、启动和恢复仍由原调用方处理，不启用新协议。
- 本票 worktree 执行 `cargo check --manifest-path backend/Cargo.toml --all-targets`，退出 0。改动后原始结果：``Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.07s``。
- 同一 worktree 执行 `cargo test --manifest-path backend/Cargo.toml --lib prompts::`：原组合和词典回归 5 通过、0 失败，包括字面输入、单次 Skill 调用、转义、缺键和快照重读。
- 复用 S1 公开 HTTP 及受控宿主提交记录，既有断言不改；实际命令如下，11 通过、0 失败：

  ```sh
  cargo test --manifest-path backend/Cargo.toml --test chat_creation --test workflow_start --test workflow_dispatch --test chief_delegation -- \
    dictionary_ both_skills_submit_one_native_skill_invocation_with_the_full_body a_skill_only_task_submits_the_bare_skill_call \
    app_root_run_waits_for_native_readiness_and_persists_before_business_input app_nested_manager_and_end_chat_keep_separate_run_roles \
    end_node_start_and_persisted_resume_do_not_require_node_extension_for_either_host app_delegated_map_binds_manager_before_approved_input \
    task_bot_launches_independent_chat_with_previewed_input_and_durable_sender_identity
  ```

  `chat_creation` 5 项核对 Skill、固定设定、原样输入和词典错误停止发送；`workflow_dispatch` 2 项核对节点词典与原样 Run 输入、结束节点启动及真实执行关联恢复；`workflow_start` 3 项核对根／子管理者与 chief Map 启动控制及结束节点例外；`chief_delegation` 1 项核对独立 Task 的已预览原文、材料及委派关联。
- 这些是保行为证据，不证明实际安装的原生宿主、工具包接缝或模型行为。覆盖分区没有分配 01 新语义用例；不认领 promise 完成。合并由独立集成代理执行，票据状态待合并后关闭。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 01 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。

本票属于当前自动开发队列，完成自身适用实现与自动检查后即可交付；不等待 11–14，也不代做它们。保留真实失败/部分效果，更新本票范围文档及证据。
