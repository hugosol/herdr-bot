# Bot 与 Map 全面重构及选择性流程迁移

Label: wayfinder:map

## Destination

确定 Bot、Map/node 的完整新数据结构、系统代码设计、关联提示词和可复用迁移规则，形成可交给 /to-tickets 与 TDD 的确定输入，并提供供用户逐项选择迁移或废弃的流程列表。独立转换试验已确认不再需要；全部 ticket 完成后等待用户手动启动 spec.md 生成流程。

## Notes

- 参考基线：[重构设计基石](E:/Developer/course/agent-course/homework/herdr-bot-layering-design-foundation.md)。遵循其知识归属与独立 Task、chief、嵌套 Map 场景；概念名称不是强制新增实体。
- 每次咨询 grilling 与 domain-modeling，遵循 docs/agents/issue-tracker.md、docs/agents/domain.md；选择验证前读取 docs/agents/testing.md。决策发布按 Lighthouse，发布后按 Backtracer 追踪。
- 用户在本次建图对话明确：Wayfinder 确认系统代码设计；另用专门 grilling 票共同完成提示词。有逻辑关联的提示词必须在同一票完成，不按字段拆票。
- 顺序：完整结构与代码契约 → 提示词联合设计／迁移规则及剩余切换交接决策 → 全部 ticket 完成后由用户手动启动 spec.md → /to-tickets → TDD，不自动进入下一阶段。独立转换试验不再作为交接门槛，确认处置见 Out of scope。
- 已有初步转换样例只保留参考，不继续扩充或请求确认；不提前实现生产系统，不要求试验适配程序或开发前运行流程。设计与静态检查不等于运行验收。
- 用户确认复用适用既有自动化检查，不另建通用自动跑流程设施；开发后全流程业务验收由用户执行。文档已记录创建、修订、外层真实模型验收入口，但本次未运行，不推定覆盖新格式。
- TDD 直接使用已确认提示词。开发后发现遗漏报告用户，由用户补充；不授权开发 Agent 临时生成或自决语义。
- 用户按迁移列表逐项选择迁移或废弃；未迁移数据不保证在新版运行。不静默全量迁移、不默认废弃、不默认新旧兼容。共享对象的其他消费者必须显式列出。
- 决策票已全部完成；本次止于规划发布，等待用户手动启动后续流程，不生成 spec.md、不执行 /to-tickets。实际状态和 claim 以各决策票字段为准。
- Traverse 后续 grilling 的两项确认修订已发布回原权威：节点承担专属业务检查，管理者仅承担通用编排交接；中途报告不要求附文件。恢复与追踪记录见[关联提示词的联合设计与确认](decision/04-coherent-prompts.md#handoff)。
- 一次停机切换的定义 backup、旧 Run 归档、chief 清理和 Chat 保留边界见切换交接权威；不等待全部流程迁移，第一条迁移完成后即可测试。

### 决策导航

- [流程迁移清单与共享影响范围](decision/01-flow-inventory.md)
- [Bot 与 Map node 的完整数据结构和知识归属](decision/02-definition-schema.md)
- [调用上下文与系统代码改造契约](decision/03-call-context-design.md)
- [关联提示词的联合设计与确认](decision/04-coherent-prompts.md)
- [可复用迁移方法与转换规则](decision/05-reusable-migration.md)
- [单条流程全要素初步转换试验](decision/06-conversion-trial.md)
- [选择性迁移的系统切换与开发交接门槛](decision/07-cutover-handoff.md)

## Decisions so far

- [流程迁移清单与共享影响范围](lighthouse/01-flow-inventory.md)：Map 入口三状态清单；旧 Map／Bot 离线逐项迁入，确认完成后移除旧副本，未变格式资源原位维护。

- [Bot 与 Map node 的完整数据结构和知识归属](lighthouse/02-definition-schema.md)：Bot 为四字段能力索引、不注入执行；Map 归整输入输出规则，node 承接本次差异和条件式业务指引，专属业务检查归节点而非管理者。

- [调用上下文与系统代码改造契约](lighthouse/03-call-context-design.md)：Run 冻结自身 Map，finish 登记公开成果；中途报告可不附文件，正式交付按声明检查；后端 prompts 只供给通用协议与运行事实，删除 Map 不连带删除 Run 历史。

- [关联提示词的联合设计与确认](lighthouse/04-coherent-prompts.md)：确认「创建map」全组正文；业务文档随 Map，由各节点按条件读取，description 不承载指令；后端负责通用流程说明，extension 聚焦工具调用，不代表生产已迁移。

- [可复用迁移方法与转换规则](lighthouse/05-reusable-migration.md)：复用联合提示词票的起手问题，以单流程调查与 grilling 展开边界；[通用会话指引](../../docs/agents/workflow-migration.md)供后续 Agent 使用，不预先穷举范围或另建迁移机制。

- [选择性迁移的系统切换与开发交接门槛](lighthouse/07-cutover-handoff.md)：一次停机切换，旧 Run 归档、chief 运行记录清理而 Chat 保留；首条迁移后可测试，后续 spec 由用户手动启动。

## Not yet specified

无阻塞本地图交接的未决问题。其他存量流程的独特语义、共享影响和废弃选择按已确认迁移方法，在用户选定流程后逐项调查与确认，不预判答案、不作为全量迁移门槛；后续发现的具体设计缺口回到原权威处理。

## Out of scope

- [单条流程全要素初步转换试验](lighthouse/06-conversion-trial.md)：重复已完成的设计与正文决策，用户确认无需独立试验门槛；已有样例仅供参考，不代表验收通过。后续切换交接消费原设计权威，其余未决范围不变。
- 本地图内的生产代码重构、全量数据迁移和执行 /to-tickets。
- 重写执行引擎、强制所有 Chat 经过 Bot、统一各实现宿主。
- 为本轮新建通用流程自动化验证平台；开发前真实运行新格式流程。
- 无关 Task 重构、历史业务状态重写、未经用户决定删除旧流程。
