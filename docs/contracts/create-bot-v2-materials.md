# 创建bot-V2 材料契约

本契约定义显式选择创建 V2 时的材料格式，不适用于[创建 V1、修订和独立 Chat](../business-materials.md)。各 Task 承担执行步骤、权限核实、安全保存和完整读回。

## 按角色读取

- 准备或核实根输入时，管理者读取「通用格式与目标定位」和「request 与 materials」。
- W1 消费输入及生成双材料前，读取上述两节和「target_materials」；bot_requirements 按关联 Task 的「交付双材料」生成，尤其遵循其中权限与非目标的阶段标注规则，区分本阶段只读限制与后续创建权限。
- W2 消费双材料及生成最终文件前，读取「通用格式与目标定位」「target_materials」「saved 与 handoff」；核实委托授权含义时另读「request 与 materials」。后续阶段的候选及保存批准不构成 W1 的完成门槛。

## 通用格式与目标定位

保留原版 request／可选 materials 以及 saved／handoff 的接口名称与职责；不承诺旧创建 v1 Schema 兼容，不修改旧格式或消费者。以下是 V2 的业务材料协议，不给 Bot／Map 定义增加 schema_version。

JSON 材料统一为无围栏 UTF-8 单对象；版本为 schema_version=2，拒绝重复键、未知字段、错类型、缺少必填字段或目标歧义，不把参考材料作为隐藏参数。自由说明使用字符串，明确没有条目时使用空数组。

**目标定位 `target_ref`。** 只选择一个既有目标。`type=task` 提供非空 `task_id`，指向 normal Task；`type=map` 提供服务目录内确切 JSON 文件名 `map_file`；`type=script` 提供配置 `scripts` 根下合法的相对 `.ts` 路径 `script`。定位不证明能力已验证。

## request 与 materials

**request** 顶层字段：schema_version、kind=request、target、requirements、constraints、non_goals、resource_limits、authorization、confirmation，以及可选 bot。

- target 为 target_ref；requirements 为非空字符串；三个约束字段为字符串数组。
- bot 可选，包含可选 id／name／group；均遵循平台命名规则。未指定时由节点提出并确认；冲突不随机改名或覆盖。
- authorization 为 write_materials、create_bot、optimize_task_prompt 三个 boolean。write_materials 必须 true 才能进入文件交付；其余 false 表示尚未授权，可在 W2 对具体候选取得。true 只表达本次委托范围，不能替代未生成正文的最终批准。非 Task 目标 optimize_task_prompt 必须 false。
- confirmation 为 evidence 非空字符串和 scope=request。只记录当前委托来源与范围，不表示三指南全文批准。
- 不接受 `create_blank` 或 `scope=create_blank`。V2 不创建中间空白 Bot。

**materials**：保持可选参考文件，不强制改成 JSON；调用者说明内容、格式、用途与读取授权。影响下游的必要参考原文由 W1 纳入 target_materials，不只给隐式地址。

## target_materials

**target_materials** 顶层字段：schema_version、kind=target_materials、target、captured_at、references。

- target 包含 target_ref 的字段及 configuration。Task／Map configuration 是完整原始配置对象，模板保持原文。Script configuration 为 source_path（真实核实绝对地址）、source_text（完整原文）、dependencies（字符串数组，已知依赖事实）；不得 import／执行脚本来调查。
- captured_at 为带时区的读取时间字符串，仅供追踪，不是版本锁。
- references 为对象数组，每项 source、purpose、content 均为字符串，content 是必要完整参考原文；无参考为 []。不可按一份文件掩盖必要材料缺失或不可读。

## saved 与 handoff

**saved** 顶层字段：schema_version、kind=saved、bot、revision、target。bot 为完整读回 Bot；revision 为服务真实 file／original；target 使用上述含 configuration 的目标表示，承载最终读回配置或重新核实的脚本原文。不得用候选、摘要或伪 revision 代替。

**handoff** 顶层字段：schema_version、kind=handoff、stage=saved、bot_id、target、confirmation_evidence、sources、changes、checks、unverified。

- target 为 target_ref；confirmation_evidence 为非空字符串数组，记录意图、最终正文及保存授权的真实依据。
- sources 为对象数组，每项 binding_id、direction（input／output）、role；role 为 requirements／target_materials／saved／handoff。只追踪实际本地输入输出，不负责寻址。
- changes 为字符串数组，记录分层归属和共享影响，不复制完整正文或配置。
- checks 为 bot_read_back、target_read_back、task_prompt_changed、non_target_unchanged、semantics_preserved 五个 boolean。成功要求前两项及后两项为 true；task_prompt_changed 如实记录，Map／Script 必须 false。非目标保持包含 Map／Script 全目标不变，Task 除获准 prompt 外不变。
- unverified 为字符串数组，记录原生加载和业务效果等未验证项。V2 不要求 `target_unchanged`，获准的 Task.prompt 改动必须如实记录。

该 handoff 表示成功交接，不用失败现场填出虚假成功 checks。部分失败通过真实报告和另行明确标记的材料说明，不声明整体完成或冒充成功 saved 阶段交付。
