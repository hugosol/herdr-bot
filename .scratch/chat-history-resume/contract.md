# Contract — 历史 Chat Resume

Source: [Spec](spec.md)
Acceptance mode: Rapid iteration
Approval: approved — seam 选择、六面审阅和验收模式均已获得用户明确批准。

本合同记录可观察承诺，不规定实现顺序。Spec 的 Implementation Decisions 及相关 ADR 是继承事实；交付切片由 /to-tickets 在各承诺下分配，不在这里指定。承诺完成与否由交付工作、所需检查和人工确认推导，不维护状态列。

## Promises

Coverage 中“自动”包含 agent 负责的 TDD、行为检查及证据准备，不等于只验证受控 adapter。真实浏览器交互使用交付页面；持久化使用独立预写样例、真实临时文件和进程重启；预期值来自承诺和独立字面样例，不调用被测实现重新计算。

|#|Promise (one observable result)|Coverage|Source|Seam|
|---|---|---|---|---|
|P1|每个历史 Chat 都可发起 Resume，不以当前会话登记或原有 Herdr 绑定为资格。|自动：浏览器操作；独立请求者对旧记录、connect-bot 来源记录、登记已移除及重启后的记录发起操作。恢复信息不足时允许明确失败，不隐藏入口。|[stories 1–3,27](spec.md#user-stories)|S1,S2|
|P2|已有目标按原生对话身份匹配，包括同一身份的路径／ID 两种表达，不按显示名或 cwd 相同判定。|自动：独立原生文件头、ID／路径相等和不相等样例，以及同名同 cwd 的不同对话反例；真实身份验证并入 P6 的 H-Pi／H-OMP。|[story 4；身份决策](spec.md#implementation-decisions)|S2,S3|
|P3|找到唯一已有目标时，切到其实际 Workspace、Tab、Pane，不另开恢复副本。|自动：受控目标在不同 Workspace 的场景，观察 focus 目标且无 Tab／agent 新建；真实 Herdr 观察并入 P6 的 H-Pi／H-OMP。|[stories 4,5](spec.md#user-stories)|S2,S3|
|P4|定位已有对话不向其提交输入。|自动：已有目标处于工作或等待输入时，观察没有 prompt／输入提交；真实输入观察并入 P6 的 H-Pi／H-OMP。|[story 6](spec.md#user-stories)|S2,S3|
|P5|多个匹配目标时，页面提示后按稳定顺序自动聚焦其中一个，不增加选择交互或再启动。|自动：多个独立预置目标、改变返回枚举顺序的场景；真实浏览器观察提示与无需选择，受控记录核对所选目标及没有新增启动。|[stories 7,8；多目标决策](spec.md#user-stories)|S1,S2,S3|
|P6|成功查找且无匹配时，新建目标加载指定原生对话，并在确认后聚焦该目标。|自动：真实请求驱动受控 Herdr 的查找、创建、启动和聚焦，观察独立身份样例和阶段结果。**人工 H-Pi**：专用 Pi 旧对话的已有目标定位及新进程恢复；**人工 H-OMP**：专用 OMP 旧对话的相同检查，另确认未自动继续中断任务。agent 为两项分别准备原生 ID／文件身份、工作目录、恢复前后 Herdr 目标、实际参数、已知历史内容、页面结果及输入／任务活动观察，并准备可重复验收步骤；用户确认真实对话与焦点。两项均不得改动用户现有 Chat。|[stories 9,10](spec.md#user-stories)|S1,S2,S3|
|P7|已收录的原 Workspace 信息跨重启保留，并在可用时作为新进程恢复位置。|自动：通过正常收录途径保存已知原 Workspace，重启后恢复，观察实际创建目标。定位或恢复本身不改写该信息；回退后再次读取仍是原值。|[story 11；Workspace 决策](spec.md#implementation-decisions)；用户审阅修正：Resume 不更新原 Workspace|S2,S3|
|P8|缺少原 Workspace 信息或原目标已删除时，在配置默认 Workspace 恢复并显示回退说明。|自动：旧格式记录、已删除原 Workspace 两种场景，独立预置默认目标，观察其真实选择和页面说明；记录缺失可保持缺失，不要求 Resume 补写。|[stories 12,13,27](spec.md#user-stories)|S1,S2,S3|
|P9|默认 Workspace 不存在、重名或查询失败时明确报错，不任意放置或自动创建替代 Workspace。|自动：缺失、多个同名、查询失败各场景，观察错误与没有后续创建；原 Workspace 查询失败也不得伪装成已删除而回退。|[story 14；Workspace 决策](spec.md#implementation-decisions)|S2,S3|
|P10|加载目标优先使用可用会话文件；文件不可用时仅允许以 ID 加载同一原生对话。|自动：文件和 ID 同时存在、仅 ID、文件不可用等独立样例，观察明确目标与参数；真实原生加载证据并入 P6 的 H-Pi／H-OMP，不能由受控参数记录替代。|[stories 15,16](spec.md#user-stories)|S2,S3|
|P11|新进程恢复使用历史记录的工作目录，不静默替换目录。|自动：包含空格等合法路径的真实临时目录，观察传入和实际目标 cwd；缺失目录观察明确失败。|[stories 17,18](spec.md#user-stories)|S2,S3|
|P12|agent 类型缺失／未知／不受支持或原生对话不可用时，明确失败而不以空白新对话冒充恢复。|自动：独立无效记录和不存在的原生目标，观察可见原因及禁止替代行为；实际 Pi／OMP 对旧身份的加载核实并入 P6 的人工项。|[story 18；新进程恢复决策](spec.md#implementation-decisions)|S1,S2,S3|
|P13|Resume 不重发 Bot 固定初始指示。|自动：以带可辨认初始指示的 Bot 为反例背景，观察没有 prompt／初始输入；P6 的 H-Pi／H-OMP 确认原生对话未出现新提交输入。|[story 19](spec.md#user-stories)|S2,S3|
|P14|Resume 不重新应用 Bot 预设的模型或思考级别。|自动：Bot 预设与原生对话设置不同的独立样例，观察启动未施加 Bot 预设覆盖；不把 agent 自身恢复规则误写为本项目保证。|[story 20](spec.md#user-stories)|S2,S3|
|P15|恢复加载历史，不自动继续中断任务。|自动：观察 Pi 使用原生 session 加载、OMP 使用 resume 而非 restore，且无新输入提交；实际运行结果由 P6 的 H-Pi／H-OMP 验收，H-OMP 明确检查没有自动继续中断任务。|[story 21](spec.md#user-stories)|S3|
|P16|同一实例同一 Chat 同时最多执行一次 Resume，不排队执行重复启动。|自动：来自独立请求者的并发操作与受控延迟，观察最多一次创建／启动；不能仅凭单页按钮禁用证明。|[story 22](spec.md#user-stories)|S2,S3|
|P17|启动等待超时在页面及操作结果中表示“启动结果未确认”，不表示成功或确定未启动。|自动：可控启动超时，观察浏览器及 HTTP 结果均保留不确定性，并能识别已取得的目标。|[story 23](spec.md#user-stories)|S1,S2,S3|
|P18|超时后保留已创建目标供检查，不自动销毁或另开副本。|自动：受控超时，观察目标仍在、没有关闭／终止／再次创建或启动。|[stories 23,24](spec.md#user-stories)|S2,S3|
|P19|后续用户重试仍从查找开始，而非盲目重放启动。|自动：首次超时后已有目标变得可识别，下一次操作观察查找及聚焦，且无第二次启动。|[story 25](spec.md#user-stories)|S2,S3|
|P20|查找或聚焦失败明确返回相应失败，不作为未匹配而启动副本。|自动：不可达、错误响应、无法解析和聚焦失败样例，观察对应失败与无后续启动。|[story 26；身份决策](spec.md#implementation-decisions)|S1,S2,S3|
|P21|旧历史仍可读取，同一原生对话恢复后不增加重复历史条目。|自动：正常接入与历史读取，加载前后及重启后比较独立预写的身份集合；不依赖当前登记存在。|[story 27；历史兼容决策](spec.md#implementation-decisions)|S2|
|P22|成功反馈只表示已确认的 Herdr 内部定位或正确对话恢复，不声称 Windows 窗口已置前。|自动：页面／HTTP 结果与实际控制结果相符，创建 Tab 或提交命令不足以成为恢复成功；真实焦点与身份确认并入 P6 的人工项。接受同一 Herdr 会话其他客户端同步导航。|[story 28；焦点与结果决策](spec.md#implementation-decisions)|S1,S2,S3|
|P23|Resume 不让客户端越过历史选择去指定任意执行内容，既有会话事件来源限制不变。|自动：伪造客户端可执行文件、参数、原生路径或输入等额外内容，观察拒绝或不能覆盖后端记录与配置；独立会话事件来源拒绝场景保持成立。|inferred — 落实 [不增加任意命令执行入口及来源限制](spec.md#implementation-decisions) 所必需，已通过审阅|S2,S3|
|P24|可能影响同一对话判定的身份核实失败明确报错，不静默降级为无匹配后启动。|自动：路径／ID 对照依赖的身份资料暂不可读等场景，观察核实失败与没有启动。仅适用于影响所选 Chat 匹配的证据，不要求所有无关 agent 的身份都必须可读。|inferred — 落实 P2、P20 及 [先定位再恢复](spec.md#implementation-decisions) 所必需，已通过审阅|S2,S3|

**Complete implementation and all assigned automated checks before the human-led acceptance round. Planned human-led checks do not pause otherwise executable work. At the final handoff, report the remaining checks together with their prepared evidence; they remain unverified until the user confirms them. Report genuine blockers when encountered.**

人工例外只分配 P6 中 H-Pi／H-OMP 所描述的真实环境确认；其他 Coverage 引用它们时复用同一证据，不额外增加人工检查轮次。真实浏览器、HTTP、持久化、失败及并发检查仍归 agent；生产实现和有用的自动验证不因人工例外而缩减。尚未得到用户确认的人工项不得标为通过。

## Seam decisions

|Seam|Exposes|Hides|Alternatives considered|Reversibility|
|---|---|---|---|---|
|S1：交付页面的历史会话交互 interface|用户可操作入口、等待与结果、多目标及回退提示、失败与未确认状态|页面状态组织、渲染结构和交互内部辅助逻辑|沿用现有真实浏览器先例；不以内部状态或浅层渲染代替用户交互。只观察页面不足以证明原生身份与外部副作用，因此与 S2／S3 配合。|页面布局可变；承诺描述行为而非列数、按钮文案或内部结构，视觉细节变化无需重定合同。|
|S2：herdr-bot 对调用者公开的 HTTP interface|按历史身份发起 Resume、权威操作结果、并发约束、历史兼容及跨重启可读性|请求后的流程组织、锁与任务管理、存储布局及进程内匹配算法|复用现有真实请求、临时文件和重启观察方式；新增 Resume 操作及必要结果读取能力，不另加只供测试调用的恢复 interface。只有页面检查不足以区分实例级并发与单页按钮禁用。|具体地址、字段形态与结果传递方式在首次交付前可确定；交付后改变调用者约定需迁移。合同不固定轮询或其他传递机制。|
|S3：Herdr 公开控制 interface|查找、身份引用、Workspace 选择、目标创建、原生启动、焦点、输入与超时产生的可见结果|CLI adapter 的进程调用和解析细节、外部项目内部实现、agent 内部恢复机制|扩展既有 CLI adapter 的查询／聚焦能力；生产 adapter 与受控 adapter 已有先例，不为 Resume 新造抽象层。仅用受控调用记录被否决，因为它不能证明真实原生对话已加载；真实环境检查分配为 H-Pi／H-OMP。|Herdr 或 agent 的公开控制语义变化可能要求 adapter 与受控样例更新，并重新做真实验证；上层承诺仍适用，除非能力变化迫使重议合同。|

依赖分类：匹配、稳定目标选择及执行资格为 **In-process**；文件与目录使用真实临时对象，按 **Local-substitutable** 验证；Herdr 与 Pi／OMP 按 **True external** 对待，受控 adapter 不代替真实验证。本功能没有必须新增的 **Remote but owned** 依赖。上述分类不要求把内部 seams 暴露给调用者。

批准记录与取舍：

- 用户明确批准 S1／S2／S3，未要求额外 module-interface 重设计。
- 用户通过六面审阅；P23／P24 是已有承诺的必要推论，不是新增产品能力。
- 用户纠正并否决“每次 Resume 更新原 Workspace”的假设：记录代表原位置，临时回退或已有目标的实际位置不应覆盖它。因此不增加恢复后的 Workspace 写入，也不引入相应部分成功分支。
- 用户在完整自动化与快速迭代之间选择 **Rapid iteration**，批准两项专用原生对话真实环境确认由人工完成。完整自动化未被选中是验证责任分配，不是降低恢复正确性要求。
- 没有形成同时满足难以逆转、缺少上下文会令人意外、且来自实质取舍的新架构决策；本轮不新增 ADR。

## Not yet specified

- **阻止批准的未决事项：无。** 三个决策门均已完成。
- **延后但不改变承诺的 interface 细节：** Resume 请求及结果的具体表示、原 Workspace 的持久表示、稳定选取多个目标的排序键。后续交付必须满足本合同；这些细节不是新增 seam，也不在此预切工单。
- **交付分配：** /to-tickets 为各 P 编号指派交付工作，并将 Coverage 转入相应验收条件。
- **验收证据：** 本轮仅批准合同，未执行功能验证。H-Pi／H-OMP 的专用样例与证据在实现期间准备，用户实际确认前始终未验收。
- 实现若推翻 seam 或发现无法兑现的承诺，先修订合同并重新取得受影响决定的批准，再重开相应工单；不能静默降低保证。

## Out of scope

完整继承 [Spec — Out of Scope](spec.md#out-of-scope)，不新增范围外条目。该节把 seam 选择留给本阶段的程序性安排已由本合同履行；不改变其中任何功能范围。

## Coverage review

|检查|结果|
|---|---|
|每个承诺至少对应一个 seam|P1–P24 全部覆盖，无例外。|
|每个 seam 至少承载一个承诺|S1、S2、S3 均承载承诺，无空 seam。|
|Entry|已检视 S1 浏览器和 S2 请求。没有新增用户 CLI 或库入口，故不增加此类 seam。|
|Data and state|已检视历史兼容、原 Workspace 收录与不被 Resume 改写、去重、真实文件及重启。|
|External dependencies|已检视 Herdr、Pi／OMP 与文件系统；不要求新增远端自有依赖。|
|Errors and failure|已检视查询与身份核实失败、目标缺失、Workspace 歧义、启动超时、无副本及无伪成功。|
|Output and observability|已检视页面与请求结果同实际控制结果一致；没有新增日志或指标产品承诺。|
|Resources and concurrency|已检视实例内同 Chat 执行资格、无重复排队、超时保留目标；附在 S2／S3，不新增 seam。|
|Not yet specified|无批准阻塞；仅保留 interface 细节、交付分配及后续验收证据。|
|验收模式与人工覆盖|Rapid iteration；仅 H-Pi、H-OMP，定义及证据要求见 P6 Coverage，相关承诺在 Coverage 中引用。|
