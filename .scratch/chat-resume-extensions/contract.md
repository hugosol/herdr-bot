# Contract — 历史 Chat Resume：env config 与降级反馈

Source: [Spec](spec.md)
Acceptance mode: Rapid iteration
Approval: approved — 用户已批准 S1／S2／S3、经修正的六面审阅方向，以及 Rapid iteration 和两项真实 agent 扩展加载人工确认。

本合同记录可观察承诺及其 external seams，不规定实现顺序。交付工作由 `/to-tickets` 填入相应票据并引用 P 编号；不维护承诺状态列。完成情况由交付工作、所需检查及人工确认共同推导。

## Promises

Coverage 中“自动”由 agent 负责实现、TDD、行为验证与证据准备；独立字面样例和本合同决定预期结果，不调用被测实现重算期望。真实浏览器使用交付页面，持久化使用真实隔离文件及进程重启。受控 adapter 的参数记录不等于真实 agent 已加载扩展。

|#|Promise (one observable result)|Coverage|Source|Seam|
|---|---|---|---|---|
|P1|明确由 Bot 发起的 Chat 能在恢复时找到其来源 Bot，且该关联跨重启保留。|自动：正常发起并确认原生身份后重启，修改该来源 Bot 的扩展选择再恢复，以独立预写的扩展 A／B 区分来源是否正确且已持久保存；不要求新增公开来源查询 interface。|[stories 1,4](spec.md#user-stories)|S2,S3|
|P2|来源 Bot 首次取得有效值后，不被后续收录、普通 register 或 Resume 覆盖或擦除。|自动：已知来源 A，随后收录来源 B、无来源 register 和 Resume；之后编辑 A／B 为可区分扩展集合，恢复结果仍来自 A。对当前缺失字段，只有发起流程提供的明确来源可以填入。|[stories 3,5,6,20；持久来源关联](spec.md#implementation-decisions)|S2,S3|
|P3|同次收录的一致、互补信息按相同原生身份安全合并，结果不依赖发起收录与 register 的先后，重复提交不产生重复历史或重置首次时间与备忘。|自动：分别制造 register 先到、发起收录先到及并发写入，包含缺省来源和明确来源；重启后核对独立身份集合、已知首次时间和备忘，并通过恢复集合核对来源。重复相同信息结果不变；本承诺不规定互相冲突的新旧可更新字段必须满足交换律，也不允许覆盖已确定来源。|[stories 1,3–6,20；统一信息补齐入口](spec.md#implementation-decisions)|S2,S3|
|P4|来源 Bot 存在时，恢复使用该 Bot 最新 env config 中可用的固定／可选扩展集合，排除未选择的 ambient 扩展。|自动：创建后把选择从 A 改为 B，改变当前映射，核对受控 adapter 收到最新固定项和 B，关闭发现且不含 A／ambient。**人工 H-Pi／H-OMP**：分别确认真实 agent 的实际显式加载集合及 ambient 排除；证据要求见 P7 Coverage，复用同一轮检查。|[stories 2,7–9；最新配置与 env config](spec.md#implementation-decisions)|S2,S3|
|P5|单项扩展映射缺失、入口不存在或不可读时，仅跳过该项，其余可用固定／可选扩展继续参与恢复。|自动：在独立样例中分别删除映射、删除入口、制造不可读入口，保留另一有效项，核对实际提交集合与缺失原因；固定与可选项均适用。**人工 H-Pi／H-OMP**：各确认一个有效项与一个缺失项并存时的真实加载结果，复用 P7 的证据和检查轮次。|[stories 10–12](spec.md#user-stories)|S1,S2,S3|
|P6|所有扩展均被过滤时，仍沿用有 Bot 分支的空显式集合，不自动切换到默认发现。|自动：全部入口不可用的独立样例，核对显式集合为空、自动发现仍关闭，页面保留逐项降级信息；后续身份确认不足时只能返回相应失败或未确认，不能假定恢复成功。无需把极端空集合另列人工验收项。|[story 15；空集合](spec.md#implementation-decisions)|S1,S2,S3|
|P7|来源关联缺失或关联 Bot 已删除时，恢复使用 agent 原生默认扩展发现，不显式追加 Bot 固定／可选扩展。|自动：旧记录、外部接入记录、已删除 Bot 三类独立样例，核对既无 `--no-extensions` 也无 Bot 扩展追加；保留同一原生对话选择。**人工 H-Pi**、**人工 H-OMP**：各自在专用隔离环境中确认 P4 的显式集合、P5 的部分可用集合，以及本行的默认发现集合。agent 为每项准备原生 ID／文件身份、实际 cwd、来源 Bot 及原生默认配置、可区分的扩展实际加载证据（不能仅给参数）、ambient 对照、Herdr 目标、页面降级反馈与可重复步骤。用户只确认真实 Pi／OMP 加载结果；不得改动用户现有 Chat。|[stories 16–19](spec.md#user-stories)|S1,S2,S3|
|P8|历史查询不改变持久记录、不猜测来源；原 Workspace 名称只根据已保存 ID 临时解析显示。|自动：预写缺少来源的历史，多次读取与重启不产生来源或其他持久修改；Workspace 改名后展示新名称而原 ID 不变，名称查询失败仍可读。明确来源由正常发起收录写入属于 P2／P3，不属于读取补绑。|[stories 19,20；查询只读](spec.md#implementation-decisions)|S1,S2|
|P9|Resume 不重新应用 trigger config。|自动：来源 Bot 设置可辨认的初始消息、Skill、model、thinking；观察不提交初始消息或 Skill，不传 Bot 的 model／thinking 覆盖。原生会话自身恢复设置不受本项目重新解释；实际输入活动沿用既有恢复验证，不只断言内部字段。|[stories 23,24；trigger config 与 env config](spec.md#implementation-decisions)|S2,S3|
|P10|与扩展无关的 trigger config 选择失效，不阻止按可用 env config 恢复。|自动：先保存有效 Bot，再使其 model 或初始 Skill 目录选择失效，同时保留有效扩展和原生对话；恢复仍使用该扩展，且不重放失效的 trigger config。结构拆分本身不做源码形状或转发测试。|[恢复配置边界](spec.md#implementation-decisions)|S2,S3|
|P11|已有匹配 Herdr 目标时仅定位，不为了更新 env config 而重启、另开目标或提交输入。|自动：已有目标和 Bot 已编辑并存，观察正确聚焦及无新启动／输入；即使新启动配置不可用，也不为仅定位动作额外要求扩展恢复。|[story 21](spec.md#user-stories)|S1,S2,S3|
|P12|新进程恢复保持原生身份与历史 cwd，不用空白或其他对话替代，也不自动继续中断任务。|自动：沿用既有恢复证据与回归先例，核对同名同 cwd 反例、原生 ID／文件、真实目录、无新输入及所需原生运行观察；受控记录只能证明提交内容，不能独自证明真实对话加载或任务未继续。新增扩展场景中继续核对所选身份，不重新分配本行人工例外。|[stories 22,25；保留既有恢复行为](spec.md#implementation-decisions)|S2,S3|
|P13|原 Workspace 信息缺失或成功查询确认原位置已删除时，回退到已配置的唯一默认 Workspace，保留原记录。|自动：分别覆盖缺失与已删除；观察实际目标及页面回退说明，重读／重启后不把回退位置写为原位置。默认 Workspace 不存在或重名、原位置查询失败时按既有规则报错，不创建替代 Workspace。|[保留既有恢复行为、集中恢复降级](spec.md#implementation-decisions)；[既有合同 P7–P9](../chat-history-resume/contract.md#promises)|S1,S2,S3|
|P14|会话文件不可用时只允许用精确原生 ID 加载同一对话；可读文件身份不一致时拒绝恢复。|自动：独立预写同一 ID、另一 ID、不可用文件及不可核实路径样例，观察实际加载引用或明确错误；不扫描最近文件，不改写或修复原生文件。该回退也纳入 P15 的降级说明。|[保留既有恢复行为、集中恢复降级](spec.md#implementation-decisions)；[既有合同 P10](../chat-history-resume/contract.md#promises)|S2,S3|
|P15|发生恢复降级时，用户能同时看到所有已发生的降级要素、已知原因及实际采用的策略。|自动：通过交付页面触发扩展过滤、默认扩展发现、Workspace 回退、精确原生 ID 回退，包含组合场景；核对每项具体原因与实际替代策略。已确认恢复有降级时明确呈现有降级，不能只给普通成功，也不能只写日志；措辞和视觉布局不固定。|[stories 13,14；集中恢复降级、降级结果呈现](spec.md#implementation-decisions)|S1,S2,S3|
|P16|降级说明不替代权威恢复结果：失败和启动结果未确认仍如实呈现，并保留已经发生的降级说明。|自动：扩展缺失结合启动拒绝、身份不符、确认超时或聚焦失败，观察页面与 HTTP 的结果及降级信息；缺少必要身份集成时不把命令已提交当作恢复成功。尚未作出的选择不虚构为已发生的降级。|[story 26；降级结果呈现](spec.md#implementation-decisions)|S1,S2,S3|
|P17|无法可靠读取 Bot 或配置时不冒充来源缺失／Bot 已删除，也不执行自动修复。|自动：Bot 存储损坏、配置无法解析或不可读等独立失败机制，观察错误，不启动任意默认配置，不改写损坏文件。可明确定位的单项缺失按 P5 处理；真正来源缺失／Bot 删除按 P7 处理。|[降级结果呈现、Out of Scope](spec.md#implementation-decisions)|S1,S2,S3|
|P18|普通发起 Chat 仍拒绝无效扩展选择或不可用入口，不采用 Resume 的逐项容错启动。|自动：相同无效扩展样例分别用于发起和恢复，发起观察原有明确拒绝且无 agent 启动，恢复按 P5 处理；有效发起仍应用 trigger config 与 env config。|[实现涉及的职责](spec.md#implementation-decisions)|S2,S3|
|P19|Resume 调用者只能选择历史 Chat，不能临时替换其来源 Bot、扩展入口、执行参数或输入。|自动：独立请求尝试附加这些内容，观察不能覆盖后端记录与配置或被拒绝，且无非法启动副作用；既有 connect-bot 写入来源限制不变。|inferred — 兑现[历史身份与保留既有行为](spec.md#implementation-decisions)及既有合同 P23 所必需，六面审阅已确认|S2,S3|
|P20|扩展容错和集中降级不破坏既有执行资格、查找失败、超时不确定性与保留目标规则。|自动：复用既有并发、查找／身份核实失败、结构化超时和重试查找先例，在受影响流程核对不排队重复执行、不将查询失败当作空结果、不自动销毁或重启目标；具体既有承诺引用原合同，不在此重建另一套规则。|[保留既有恢复行为](spec.md#implementation-decisions)；[既有合同 P16–P20,P24](../chat-history-resume/contract.md#promises)|S1,S2,S3|
|P21|交付说明明确区分 trigger config 与 env config，并不再把原生存储的历史说明描述为本项目自动续接功能。|agent 审阅交付说明与领域词汇，核对两组配置、最新环境而非历史快照、初始 Skill 调用／加载型 Skill、初始消息／系统提示词注入的区别，以及不传 `--session-dir` 的当前行为。未来加载型 skills 与系统提示词注入不冒充已交付能力；不新增源码或文案逐字匹配测试。|[story 27；文档整理、配置分组](spec.md#implementation-decisions)|S1|

**Complete implementation and all assigned automated checks before the human-led acceptance round. Planned human-led checks do not pause otherwise executable work. At the final handoff, report the remaining checks together with their prepared evidence; they remain unverified until the user confirms them. Report genuine blockers when encountered.**

人工例外只有 P7 Coverage 定义的 H-Pi、H-OMP 两项真实扩展集合确认；P4／P5 引用同一轮证据，不额外增加人工项。实际浏览器操作、持久化／重启、失败与并发验证仍由 agent 执行。Rapid iteration 不允许把真实加载承诺缩减为参数记录，也不允许因人工项尚未确认而停下其他可执行工作。

## Seam decisions

|Seam|Exposes|Hides|Alternatives considered|Reversibility|
|---|---|---|---|---|
|S1：用户交付 interface——历史会话页面及使用说明|用户可见的恢复结果、具体降级与组合反馈，以及对发起、环境、原生存储行为的说明|页面内部状态、渲染组织、反馈的内部构造|复用既有交付页面与真实浏览器先例；仅检查 HTTP 结果不能证明用户看到降级，仅查文案也不能证明真实恢复。说明作为随附交付内容审阅，不另造执行 interface。|布局、文案和提示样式可调整；具体降级事实与结果不能被隐藏。|
|S2：herdr-bot 公开 HTTP interface|正常发起、Bot 修改、历史收录与读取、Resume 的权威结果，以及重启和并发后的可见行为|持久表示、补齐函数、字段合并与内部任务组织、trigger config／env config 的内部构造|复用真实请求、隔离文件和重启先例；通过后续恢复观察来源，不为测试另加来源查询或写历史入口。将补齐函数暴露给测试无法替代正常收录路径，因此不选择新的 internal seam 作为验收 interface。|首次交付前可确定具体表示；改变已交付字段或持久格式须迁移受影响调用者与数据，不保留隐式旧路径。|
|S3：Herdr 公开控制 interface|目标查找、实际启动集合、原生身份、cwd、输入、聚焦，以及真实 Pi／OMP 环境的加载证据|CLI adapter 内部解析、agent 的扩展发现及原生恢复实现|复用生产与受控 adapter；仅受控参数断言不能证明真实加载，故保留 H-Pi／H-OMP。集中降级与统一补齐是内部 locality 要求，不为它们增加新的 external seam 或通用 port。|Herdr 或原生 agent 的公开行为变化可能要求 adapter、样例和真实验证调整；若不能继续兑现承诺，先修订合同。|

依赖分类：配置选择与反馈组合为 **In-process**；文件和目录为 **Local-substitutable**，以真实隔离临时对象验证；Herdr、Pi／OMP 为 **True external**，受控 adapter 不替代真实运行。没有新增 **Remote but owned** 依赖。

批准记录与取舍：

- 用户批准复用 S1／S2／S3。新增功能及更多样例不需要新增测试专用 interface。
- 用户明确要求集中降级、统一持久信息补齐入口，以及 trigger config／env config 的结构区分。这些内部结构约束继承 Spec，不将私有函数或源码形状变成验收 seam。通用逻辑应服务现有规则，不引入未来加载能力的空实现。
- 不强制发起收录先于 register。两者可以重复写入；在统一补齐入口中安全合并一致互补信息，比新增执行顺序协调更简单。来源首次有效后不覆盖，未知值不擦除已知值；不主动猜测旧历史来源。
- 不选择“单项扩展失效就整体切换默认发现”：用户认为扩展可独立使用，要求保留最大可用集合并明确降级。无来源 Bot／Bot 已删除的默认发现是另一条既定策略。
- 不选择“只有统一提示、规则仍散落”：用户要求所有恢复回退规则集中管理，且统一补齐与降级职责分离；临时显示名称的解析不变成持久补齐。
- 不做 Bot 版本或启动快照。用户明确选择最新 env config，因此恢复环境可随 Bot 及目录修改而变化；不宣称恢复历史版本。
- 用户选择 **Rapid iteration**，批准 H-Pi／H-OMP 为人工真实加载确认；其他可执行验证归 agent。未选择 Full automation 是责任分配，不是删除承诺。
- 本次没有新增同时满足难以逆转、缺少上下文会意外、且来自实质架构取舍的决定；不新增 ADR。将来若 seam 被推翻，先修订合同，再重开受影响票据。

## Not yet specified

- **阻止批准的未决事项：无。** seam 选择、修正后的六面审阅及验收模式已经用户确认。
- **延后到实施但不改变承诺的 interface 细节：** 降级信息的具体字段和页面布局、两组配置的具体表示及需要的显式迁移、统一补齐入口的参数形式。不得借此重新引入覆盖来源、静默降级或自动修复。
- **结构约束与验收的区别：** Spec 要求集中降级和统一补齐；它们不是新增 external seam，也不以源码文字检查证明行为。内部选择若影响公开承诺、迁移范围或需要新的重大取舍，应先回到合同审阅，不静默扩展范围。
- **交付分配：** `/to-tickets` 将 P1–P21 分配给交付票据并继承 Coverage。本合同不预切片，也不维护人工状态栏。
- **尚未产生的证据：** 本阶段未运行新功能；H-Pi／H-OMP 的隔离样例、实际加载记录和步骤在实施期间准备，用户明确确认前仍未验收。

## Out of scope

完整继承 [Spec — Out of Scope](spec.md#out-of-scope)，不新增范围外内容。Spec 对当前文档阶段的程序性说明不限制后续合同与实施阶段；未来加载型 skills 与系统提示词注入仍不在本次实现范围。

## Coverage review

|检查|结果|
|---|---|
|每个承诺至少对应一个 seam|P1–P21 全部覆盖，无例外。|
|每个 seam 至少承载一个承诺|S1、S2、S3 均承载承诺，无空 seam。|
|Entry|已走查交付页面及公开 HTTP interface；没有新增用户 CLI 或库入口，此类入口不适用。|
|Data and state|已走查来源字段首次有效后不覆盖、互补合并与安全写入、重复收录、重启、查询只读、配置分组及旧记录。|
|External dependencies|已走查文件系统、Herdr、Pi／OMP 与真实扩展加载；没有新增自有远端依赖。|
|Errors and failure|已走查逐项缺失、空集合、无来源、Bot 删除与损坏的区别、必要信息失败、Workspace 与精确 ID 回退、启动未确认及禁止自动修复。|
|Output and observability|已走查每项降级原因及替代策略、组合降级、页面与权威结果一致、随附说明；没有新增日志或指标产品承诺。|
|Resources and concurrency|已走查写入串行化、收录顺序与幂等、Resume 执行资格及既有超时保留目标规则；附在 S2／S3，不新增 seam，也不新增修复循环或后台重试。|
|Not yet specified|无批准阻塞；保留不改变承诺的表示细节、交付分配和后续验收证据。|
|验收模式与人工覆盖|Rapid iteration；仅 H-Pi、H-OMP，定义于 P7 Coverage，由 P4／P5 复用。其余覆盖由 agent 完成。|
