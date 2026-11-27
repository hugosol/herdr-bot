# 调用上下文与系统代码改造契约

Type: grilling
Status: resolved
Assignee: none
Parent: [Bot 与 Map 全面重构及选择性流程迁移](../map.md)
Blocked by: 02

## Handoff

- 已完成：2026-11-27 Q1–Q21 逐轮确认后，用户明确“同意，请发布”。已发布 [调用上下文与系统代码改造契约](../lighthouse/03-call-context-design.md)，含运行输入层级、finish 公开成果、父节点与 chief 交付、Run Map 副本、删除与界面边界及消费者接入范围。本票无待确认问题。
- 下一就绪决策为 [关联提示词的联合设计与确认](04-coherent-prompts.md)；[可复用迁移方法与转换规则](05-reusable-migration.md)亦已满足决策依赖。本次不认领下一票，不执行生产开发。
- 关键范围收窄：chief 维持仅直接委派 Task／Map；脚本参数规则不变。撤回独立输出登记工具、脚本 Run 背景、chief 展示／确认重做及通用 success→finish 新门禁。Run 只冻结自身 Map，不递归冻结引用实现；当前定义读取候选由副本方案取代。具体政策只维护于 Lighthouse。
- 已批准上游修订：结构权威原“删除边界不变”已在原话题标明 Q19 例外并链接本票，不保留竞争结论。外部设计基石要求 chief 三类直调的范围由用户明确收窄，本次不回写外部文件。
- Backtracer 已按意图、既有模式、依赖和层级追踪：静态规则与输入准备由结构权威提供，共享原件／用户迁移选择由清单权威提供；公共角色、创建修订和上下文正文由联合提示词票覆盖；旧指南、脚本与材料消费者归位由迁移方法和完整转换试验覆盖；历史 Run 缺副本、旧字段／报告和活动执行切换由交接门槛覆盖。未发现需新建票或追加问题的未覆盖决策；未决提供方不代表能力已实施。保留地图现有试验相关 fog，不凭接口设计预判实际转换缺口。下游票及其 claim 未修改。
- 紧凑证据：backend/src/runs.rs 与 runs/{startup,preparation,preflight,retry,completion,reports}.rs；chief_delegation.rs、chief_reports.rs；scheduler/maps/mod.rs 的 deleteHistory 分支；chats/mod.rs、prompts.rs；frontend workflow 图与现有表单；由 paths.local.toml／task-options.toml 追踪的 my-agent-toolkit workflow-chat 角色。当前源码的实时 Map 读取、无公开输出、chief 无 script 分支是改造基线，不是新版已具备能力。
- 证据限制：仅静态调查与规划文档发布，未运行服务、代码测试或业务流程，未迁移数据。Toolkit 公共提示词的 run 输入来源、输出门禁及结束节点说明存在旧文案漂移，按本权威在联合提示词票处理；仓库探索 Task 固定 prompt 为空的知识归位线索由迁移调查承接。已有测试仅列作后续复用入口，不宣称通过。
- 不将初步转换当作运行验收，不在开发阶段临时生成提示词。

## Question

系统各消费者怎样使用新格式，才能保证独立 Task、Map 调用、chief 调用和子 Map 封装符合设计基石？

- 调查实际正常路径，明确持久化、API、编辑界面、工具、脚本宿主、提示词装配及跨仓消费者的改造接口。
- 定义 Task 工作设定、Bot 契约、本次业务委托和公共角色指引的供给方式及冲突处理；不靠提示词顺序隐式覆盖。
- 说明调用关联、结果保存、通知、批准、父节点交接的边界；复用现有机制，不默认重写执行引擎或统一全部状态机。
- 确定字段消费、引用解析、错误语义与兼容性检查的系统代码设计，明确哪些现有测试可复用。
- 本票不实现生产代码，不创作业务提示词正文；缺失事实先调查，不能作为用户设计选择。
