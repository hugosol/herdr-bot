# 创建map：转换参考样例（不设验收门槛）

本目录是开发前的参考资产，不是已部署配置，不可指向当前服务运行。用户在本轮 Q1 确认先放 `.scratch`，理由为更稳妥；未授权覆盖服务定义。随后用户确认独立转换试验不再需要，决策票已按该处置 resolved；本目录不继续扩充、不再请求确认，也不声称已验收。见[确认处置](../../lighthouse/06-conversion-trial.md)。

## 权威与来源

- [结构权威](../../lighthouse/02-definition-schema.md)、[调用设计](../../lighthouse/03-call-context-design.md)、[全部已批准正文](../../lighthouse/04-coherent-prompts.md)。本目录只是这些正文与真实对象的实例化，不另立政策；正文修订须回原权威。
- [来源清单](manifest.json)记录原文件 SHA-256 和 Git 提交 `43f957cdf823d5b514953e39fbfb85054e609144`。生成前 Git 查询显示 Task、Bot、选定 Map 和原业务文档相对该提交无修改。原始输入通过该提交恢复，不另建备份。
- 目录中的 `meta-data/` 表示未来仓库目标路径，不是让当前服务读取本目录。

## 完整对象及差异

| 资产 | 变更 |
| --- | --- |
| [创建map Bot](meta-data/bots/bot-1794388558375394800-31524-0.json) | 保留身份、名称、Map 绑定；移除旧两指南，使用已批准四字段正文 |
| [概要规划 Bot](meta-data/bots/bot-1793304246857557100-58200-0.json) | 保留身份、名称、分组、Task 绑定；同上 |
| [概要方案转 Map Bot](meta-data/bots/bot-1793305512701853300-58200-1.json) | 保留身份、名称、分组、Task 绑定；同上 |
| [整图优化 Bot](meta-data/bots/bot-1794222010819446100-10104-0.json) | 保留身份、名称、分组、Task 绑定；同上 |
| [五个完整 Task 对象](meta-data/tasks.json) | 三个业务 Task 仅替换 prompt；manager、chief 原样列入，固定 prompt 仍为空 |
| [创建map Map](meta-data/workflow-maps/plan_workflow.json) | 替换 description，删除 input_requirements；不增加公开输入输出规则；两个 input 置空、优化节点采用已批准差异委托 |
| [流程业务文档](meta-data/workflow-maps/plan_workflow/planning-deliverables.md) | 从正文权威提取完整业务章节，章节标题降为文档二级标题；不搬入部署或测试命令 |

**Task 文件只含本样例关联的五个完整对象。实施时按 ID 更新三个业务 Task，保留其他 Task；禁止以此子集整体覆盖服务 tasks.json。** manager/chief 的列入不等于给本流程新增固定管理者绑定。模型、Suite、初始 Skill、Chat 命名等非目标字段不变。

原 Map 的 node ID、Bot 绑定、zone、blocked_by、input_rule、output_rule、auto_approved、auto_close、end_node 全部保留。内部材料仍为 outline → map → map/handoff；不因无公开文件输出而删除内部交接。

## 引用与执行侧入口

- 创建map Bot → `plan_workflow.json`。
- outline → outline-planning → task-18e318b13b71a8bc-0；初始 Skill 为 grill-with-docs。
- generate_map → plan-to-map → task-18e319d73777fc6c-1；无初始 Skill。
- optimize_map → bot-task-18e65aed740825f8-0 → task-18e65aed740825f8-0；初始 Skill 为 writing-for-agents。
- 三个业务 Task 保留 `codebase-exploration-omp` Suite；当前 Suite 声明 wayfinder 技能集合及 dependent-skills/codebase-exploration，不复制 Suite 或 Skill 定义。
- 三个 Task 通过显式文档路径取得业务规则；Map description 指向同一文档的「创建 Map 的整图交接」，供管理者读取。执行者不依赖 Bot 摘要补足知识。
- 独立 Task 使用自身正文、用户输入和引用资料；节点再取得运行背景、实际材料和公共角色说明。公共说明由后续后端/extension 接入，不塞进 Task 或 Bot。

## 共享影响与必须接入的位置

本次扫描当前磁盘全部 Map/Bot 定义：三个内部 Bot 仅被本 Map 引用，未发现其他 Map 引用外层创建map Bot；三个业务 Task 未发现其他 Bot 绑定。独立 Task、chief 动态选用及未来消费者仍存在，不由上述静态扫描排除。

公共正文不在本样例另存副本：

- 后端管理者、节点、chief、chief 委派说明及 extension 工具指引：采用[正文权威第八节](../../lighthouse/04-coherent-prompts.md#8-后端基本流程说明与-extension-工具提示的最终正文)。后端集中 prompts 模块的组织见调用设计。
- 工具描述、参数和动态回执：采用[正文权威第九节](../../lighthouse/04-coherent-prompts.md#9-工具描述参数说明与回执正文)。OMP/Pi 与 manager/chief-report 两条加载路径均在开发范围，不由本次 JSON 样例代替实现。
- 系统文档 scheduler/workflow-runtime/workflow-contract/usage 和术语仍需随系统开发更新。当前 GLOSSARY 中旧 Bot 指南与 input_requirements 描述不是目标格式权威。
- `docs/workflow-runtime.md` 目前仍有 `planning-deliverables.md` 的相对链接。正式文档迁移必须改为 `../meta-data/workflow-maps/plan_workflow/planning-deliverables.md`；旧文档中的系统部署/测试说明回归系统文档，不能删除后丢失。
- 其余当前生产引用位于两个 Bot 旧指南、两个 Task prompt 和本 Map description，均由样例替换内容消除。正式迁移仍须再查新增消费者；不能因本样例保存而提前删除旧文档。

公共角色变化影响所有相应角色的流程，不代表其他流程已经迁移或验收。新 schema、Run 快照、文件公开交付、前端及工具协议仍按已批准设计开发；本目录不提供兼容层或适配程序。

## 交付边界

本样例仅供后续整理时参考，不再作为独立交付或用户确认门槛。未启动服务、Chat、Run 或真实模型，未证明新格式被当前程序接受、角色实际注入或业务运行成功。新旧切换、历史与活动执行边界仍由「选择性迁移的系统切换与开发交接门槛」决定。
