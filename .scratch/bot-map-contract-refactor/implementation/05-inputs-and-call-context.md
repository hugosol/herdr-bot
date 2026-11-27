# 05: 输入准备、实际上下文与真实角色恢复

**What to build:** 用户可不经 Bot 直接发起 Task 或 Map 管理者；管理者启动后补齐材料，节点取得完整但分层的业务信息，不从 Bot 摘要或专属管理者指令补漏。

**Delivers:** P1, P2, P4, P8, P9, P10, P29, P33, P34, P35, P36

**Blocked by:** 01 集中既有角色说明与消息装配; 04 Run 冻结定义、历史展示与 Map 删除

**Repositories:** bot

**Status:** in-progress

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [x] 独立 Task 取得固定设定、初始 Skill、环境和自由用户输入，不被赋予普通节点报告角色；用独立标记区分 Bot 摘要、实现资料、用户输入和实际提交内容。Bot 文案和 Map.description 不自动注入。
- [x] 直接、嵌套及 chief Map 发起均允许必需材料未齐时先启动管理者；已提供的非法/不可读材料仍拒绝。chief 确认入口在 07 对齐此规则。
- [x] 公开与持久实际值为 mapInput.runInput/files，inputsReady 独立保留；按既有适用约定处理 camelCase/snake_case，不机械改名 prepare_inputs 既有操作参数，不新增原始说明编辑入口。
- [x] prepare_inputs 文件绑定整批替换：遗漏删除、空列表清空；confirmed=false 可保存未齐绑定，确认就绪及业务派发检查必需项；已派发 execution 输入冻结不随后续替换改变。
- [x] 普通节点分段取得 Run 背景、node.input、真实文件地址及输出要求，定义前后字节不变。节点按条件取得业务资料入口；管理者仅收通用编排说明，不加载 Map 专属业务验收。
- [x] 根/子管理者、独立 chief 委派路径复用批准正文，子管理者保留两层职责；OMP/Pi 产品装配及恢复沿用真实 execution/Run 关联，不新建执行或重放 trigger，结束节点仍无普通节点报告角色。
- [x] 每条消息共用一份词典快照，只替换既定类型字段；覆盖转义/未知标签/初始 Skill，不替换自由输入、路径或报告，不在后端展开业务文档。
- [x] P29 本票拥有输入拒绝和有真实外部启动/提交部分效果的 case，分别记录持久和外部事实，不将错误视为完全未发生。后续 06/07 消费本票输入机制而不重新维护政策；补齐 04 的冻结反例。

## 检查与证据

从本票 bot worktree 定向运行 `chat_creation`、`workflow_start`、`workflow_dispatch` 的初始提交/输入准备/roles/end_node case；恢复使用测试指引中 `p42` 及 chief Map 关联 case。页面发起用 `npm --prefix frontend run test:browser:build -- workflowStart.spec.ts --workers=1`。记录实际消息、公开身份/输入和定义字节；受控宿主结果不计为 tester 原生加载通过。

### 本票产品证据

- P1/P4/P10/P33/P36：`roles::same_normal_task_uses_standalone_node_and_persisted_resume_roles_for_both_hosts` 和 `dispatch_receives_background_direct_addresses_and_run_scoped_material_location` 对照受控终端已提交原文、独立 Task/节点角色、业务资料地址与正文、Bot/Map 描述及定义字节；`node_dictionary_reads_latest_and_does_not_expand_run_user_input` 与 `chat_creation` 的词典/Skill 用例核对标签、转义、自由文字、路径及报告。
- P2/P8/P9/P12：`input_preparation::`、`missing_required_child_material_does_not_block_manager_but_blocks_work`、`map_accepts_missing_required_materials_but_rejects_invalid_supplied_materials` 及 `frozen_map::` 核对三种启动、公开及持久 `mapInput`、整批替换和派发/重试门禁。保留启动/预检的既有请求参数；`prepare_inputs` 仍用 `fileInputs`，没有修改原始要求的入口。
- P29/P34/P35：`manager_trigger_and_status_follow_committed_identity`、`nested_dispatch_uses_node_manager_business_zone_and_independent_runs` 和 chief Map/Task 委派分别核对实际初始消息；`p42`、`app_delegated_map_binds_manager_before_approved_input`、`end_node::` 核对原角色与关联。`partial_child_submission_distinguishes_rejected_and_unknown_without_replaying` 和 `map_startup_failure_retains_run_provenance_and_real_submission_outcome` 将已经保存的身份、实际启动、明确拒绝和已写入但回执未知分开验证。
- 定向 Cargo 主轮为 38 项通过；准备、冻结、原角色恢复和初始提交的补测轮为 12 项通过；最终独立 Task/普通节点实际提交对照 1 项通过（有重叠）。浏览器构建命令选 `workflowStart.spec.ts workflowCurrent.spec.ts --workers=1`，13 项通过；冻结图/刷新失败恢复另通过 2 项，嵌套、历史详情、子 Run 入口和 Map 删除另通过 7 项。Vitest 的 `runDetails`、`runExecutionZone`、`runLaunchForm` 共 15 项通过。首次浏览器构建因 Bun 链接安装缺 `picocolors` 未进入测试，改用同一冻结锁文件的本地 hoisted/copyfile 安装后通过。
- 集成 worktree 验收：前端生产构建、后端及安全关机壳构建、Vitest（308 项）通过。后端全量首轮 74 个目标通过（含 `chat_creation`、`workflow_start`、`chief_delegation`）；`workflow_dispatch` 210 项通过、13 项失败，runner 因此未派发 `zone_watch`、未运行 doctest。13 项涉及冻结副本、重试规则、审批夹具配置时机、实际节点消息和三项发布定义夹具；按新契约修正后，定向首轮 13 项通过、独立 Map 信封的材料声明用例修正后另 1 项通过。正向公开往返使用合法测试夹具，旧字段拒绝由独立字面对象验证，未改真实发布定义；「创建map」发布资产的迁移及验证归 08。
- 浏览器全量首轮 387 项通过、1 项跳过、7 项失败（chief 空用户输入时提交角色说明，以及自动关闭结果中的 `mapInput`）；按已批准契约修正断言后，chief、自动关闭与本票启动/详情定向 23 项通过，最终 chief 断言再定向 1 项通过。不把分轮结果称作全量命令重跑通过。
- 本票仅列 bot 仓，无另列跨仓联调；受控宿主和页面结果只证明产品装配、实际受控发送和公开/文件事实，不计为 tester 原生扩展加载、真实模型遵从或现场切换验收。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 05 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。

本票属于当前自动开发队列，完成自身适用实现与自动检查后即可交付；不等待 11–14，也不代做它们。保留真实失败/部分效果，更新本票范围文档及证据。
