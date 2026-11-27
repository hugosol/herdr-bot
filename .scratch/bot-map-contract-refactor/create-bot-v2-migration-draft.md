# 创建bot-V2 迁移正文确认记录

本轮正文审阅已完成，已按确认内容写入正式定义；迁移清单仍为“未开始”，等待用户对转换结果的最终确认。未运行业务、未删除 backup，现场切换事实不由本记录代替。

## 已确认内容与唯一正文

| 对象 | 当前权威 |
| --- | --- |
| 跨对象分层、DRY、读取入口与可达性 | [共享整理基准](../../docs/prompt-information-ownership.md) |
| 三个 Bot 的能力摘要 | [流程入口](../../meta-data/bots/bot-1794855295140784100-12332-6.json)、[需求对齐](../../meta-data/bots/bot-1794846408546638600-12332-0.json)、[设计创建](../../meta-data/bots/bot-1794850150349270100-12332-3.json) |
| W1、W2 与 Map 维度优化者的稳定职责 | [Task 正文](../../meta-data/tasks.json)，ID 分别为 task-18e892cb6456b0d0-0、task-18e89651aa593290-1、task-18e65aed740825f8-0 |
| V2 接线及公开接口 | [创建bot-V2](../../meta-data/workflow-maps/create-bot-v2.json) |
| V2 材料含义与授权 | [材料契约](../../meta-data/workflow-maps/create-bot-v2/materials.md) |
| 创建map身份交付 | [定义](../../meta-data/workflow-maps/plan_workflow.json)、[阶段标准](../../meta-data/workflow-maps/plan_workflow/planning-deliverables.md) |

已批准正文直接由上述权威维护；原审阅稿全文不再作为并行提示词来源。

## 确认来源

本会话用户逐项确认三个 Bot 摘要、W1、共享分层与 DRY 基准、引用该基准后的 W2 及 Map 维度优化者、V2 Map 与节点说明、材料契约及写入授权。保留两阶段、稳定身份、执行配置和人工成果批准；W2 保留候选保存前确认与保存后最终结果确认。

材料设计已确认：request 只定位目标，由 W1、W2 直接复用；W1 只交付 bot_requirements，W2 自行读取目标当前配置；取消 materials、target_materials 及旧 saved/handoff。公开 created_bot 提供实际 Bot ID，创建map公开 created_map 提供实际 Map 文件名；后者 description 采用最终简版“已创建 Map 的定位文件，JSON 字段 map_file 为正式 Map 文件名（含 .json）。”，外层 Bot 成果摘要采用“已保存并完成提示词优化的 Map 配置，以及用于后续引用的身份文件。”

流程调用授权在运行提供的内部材料位置新建必需交接文件，覆盖已有文件仍须明确授权。授权正文只保留在 V2 材料契约，两个节点通过写入前指针读取；不改变独立 Task 的通用文件规则。

## 父流程后续迁移边界

创建bot-V2 迁移会话中，用户明确需要新增嵌套 Run 输入向下传递机制，并决定在上游迁移时解决。当时子 Run 的本次要求仅来自父调用节点 input；该记录描述历史交接，不作为当前运行契约。

创建bot-V2 迁移结束时，创建task-V2 与创建task-script type 均在离线 backup；其旧接口尚不兼容。后续创建task-V2 会话已批准并落地通用逐层输入下传及身份交付，当前状态见[迁移清单](../../docs/workflow-migration-inventory.md#创建task-v2本轮选择性迁移)。创建task-script type 仍未迁入；backup 原件不变。

## 交付与剩余证据

见[迁移清单](../../docs/workflow-migration-inventory.md)及[交付报告](../../docs/bot-map-contract-refactor-delivery.md)。静态/受控检查、用户迁移完成确认、实际模型效果与现场切换分别记录。
