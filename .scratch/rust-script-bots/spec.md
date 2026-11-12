# 脚本型 Bot 与创建 Bot 流程的固定 Rust 执行

Status: ready-for-agent

Approved contract: [contract.md](contract.md) — Rapid iteration；S1/S2/S3 与约束审查已批准，真实 manager 会话闭环保留 H1/H2 人工验收。

## Problem Statement

当前 Bot 的执行目标为普通 Task 或 Map，节点工作通过 Chat 执行。创建 Bot 流程中，创建空白 Bot、汇合三份定稿并保存等相对固定的操作仍要启动 agent 会话，承担不必要的会话与模型成本，也使本可由代码严格判断的操作依赖自然语言执行。

用户希望增加不创建 Chat 的固定代码执行类型，并用当前创建 Bot 流程实际验证。发布产品时不应要求用户额外安装脚本语言环境。与此同时，现有“通用文本替换者”仍承担其他流程的通用能力，不能为实现固定三路合并而收窄其语义或破坏原 Task 绑定。

## Solution

增加与 Task 型、Map 型并列且执行目标互斥的脚本型 Bot。脚本业务使用编入后端的固定 Rust 实现，不接受临时生成的代码，也不依赖独立解释器或额外业务 exe。

manager 沿用既有派发工具，派发后立即返回；后端异步执行 Rust 逻辑，处理共享报告并通知 manager。输入输出沿用命名文件交接，节点不创建 Chat。Rust 严格校验结构化材料、执行真实写入并完整读回，成功后提交可完成报告；manager 继续按现有流程显式 complete，再安排后续工作，无需重复语义验收。

本次交付两个固定业务实现：将现有“空白 Bot 创建者”切换为 Rust 执行并删除原 Task；另建三份指南为空的创建流程专用“三指南合并保存者”Bot，替换创建流程末尾的保存节点。原“通用文本替换者”及其 Task、修订流程保持原样。仅收紧创建流程材料格式，保留命名输入输出和既有接线。

## User Stories

1. As a 用户, I want a 与 Task 型和 Map 型并列的脚本型 Bot, so that 固定工作可以不依赖 agent 会话执行。
2. As a 部署者, I want a 编译进后端的 Rust 业务实现, so that 发布后无需另装 Rust、Python、JavaScript 或 TypeScript 运行环境。
3. As a 能力维护者, I want a 固定能力标识选择已实现业务, so that 不必在每次调用时生成或传入代码。
4. As a 能力维护者, I want a 明确互斥的执行目标, so that 同一个 Bot 不会同时被解释为 Task、Map 和脚本执行。
5. As a manager, I want a 复用现有工具的脚本节点派发方式, so that 不必学习另一套调度流程。
6. As a manager, I want a 派发后立即返回的执行方式, so that 无需等待 Rust 工作完成才能继续处理其他工作。
7. As a manager, I want a 后台执行完成后自动保存并通知的报告, so that 不必额外调用接收脚本结果的专用工具。
8. As a manager, I want a 与现有节点一致的等待和收取报告方式, so that 可以统一管理混合类型的 Map。
9. As a 用户, I want a 不创建节点 Chat 或 Herdr Tab 的脚本执行, so that 固定任务不会产生无用途会话。
10. As a 用户, I want a 不伪造 agent session 身份的执行记录, so that 工作事实能准确反映真正的执行方式。
11. As a Map 维护者, I want a 复用 input_rule 和 output_rule 的脚本输入输出, so that 既有材料依赖和下游引用不需要另一套协议。
12. As a 脚本调用者, I want a 通过命名文件传递的严格结构化请求, so that 固定代码可以明确判断参数是否合法。
13. As a 脚本调用者, I want a 不依赖 node.input 自然语言的执行契约, so that 业务行为不会因自然语言解释产生歧义。
14. As a 用户, I want a 缺项、冲突或定位歧义时明确拒绝的执行, so that 程序不会猜测业务意图或扩大写入范围。
15. As a 用户, I want a 真实保存并完整读回的业务结果, so that 成功报告不只是待执行计划或生成的候选文本。
16. As a manager, I want a 仅在确定性业务检查全部通过后出现的 canComplete=true 报告, so that 无需重复由模型进行语义验收。
17. As a manager, I want a 仍由现有 complete 操作确认交接的流程, so that 脚本节点不引入不同的完成与依赖放行规则。
18. As a manager, I want a 不自动派发下游的脚本结果处理, so that 工作安排仍由现有管理流程负责。
19. As a 用户, I want a 明确呈现错误和实际写入效果的异常反馈, so that 用户或 agent 能修复后决定是否重跑。
20. As a 用户, I want a 不自动重试或回滚的执行方式, so that 不会因为结果不明而重复产生副作用。
21. As a 创建 Bot 流程使用者, I want a Rust 实现的空白 Bot 创建者, so that 核实目标、创建唯一绑定及三个空指南可以由代码完成。
22. As a 创建 Bot 流程使用者, I want a 保留原创建 Bot 身份和 Map 引用的迁移, so that 既有流程接线保持稳定。
23. As a 创建 Bot 流程使用者, I want a 创建后真实配置及 revision 形成的 baseline, so that 三路编写和最终保存使用同一份共同基线。
24. As a 维护者, I want a 替代完成后删除的旧空白创建 Task, so that 不保留已经废弃的双重执行方式。
25. As a 创建 Bot 流程使用者, I want a 独立的三路合并保存脚本 Bot, so that 三份定稿能按固定对应关系一次保存到目标 Bot。
26. As a 创建 Bot 流程使用者, I want a 保留非目标字段和执行目标配置的保存操作, so that 更新三指南不会改动其他配置。
27. As a 创建 Bot 流程使用者, I want a 独立交付的 saved 和 handoff, so that 完整保存结果与交接说明不混为一份材料。
28. As a Map 维护者, I want a 三指南允许为空的内部固定能力 Bot, so that 不必为仅由既定流程使用的执行能力编写无用途的规划说明。
29. As a 通用文本替换者的调用者, I want a 保持原 Bot 与 Task 绑定及能力语义的现有入口, so that 创建流程优化不会影响其他用途。
30. As a 修订 Bot 流程使用者, I want a 不受创建流程专用材料格式变化影响的既有流程, so that 本次改造不会迫使无关流程迁移。
31. As a 创建 Bot 流程维护者, I want a 保持输入输出 ID 但明确程序可读格式的材料契约, so that Rust 可以严格处理而无需重建接线。
32. As a 用户, I want a 真实 Map 节点运行产生的验收证据, so that 不会把函数调用成功误当成完整流程交付。
33. As a 用户, I want a 隔离数据上的写入和冲突验证, so that 功能验收不破坏正式配置或已有 Chat。
34. As a manager, I want a 不因 Rust 业务长期执行而占住 Run 操作锁的派发, so that 已有工作管理不会被固定任务阻塞。

## Implementation Decisions

- 已采用脚本型 Bot 作为领域名称，执行目标与普通 Task、Map 互斥。它不是 Task 类型，也不是 Chat 或运行中的 agent。
- 固定业务逻辑使用 Rust 编入后端，由固定能力标识选择。无需独立 exe 或解释器；能力变化需要重建后端，这是已接受的部署取舍。
- 采用异步派发，不采用等待整个脚本批次完成的同步工具返回设计，不新增批次调度机制。manager 的工具调用完成仅表示派发结果，不表示业务已完成。
- 后端执行 Rust 工作后接入共享报告处理，复用报告校验、保存、输出登记、通知和既有完成流程。脚本没有 Chat，不伪造 session 身份。具体内部接口、执行身份表示和代码组织由后续 contract 确定。
- 业务执行期间不长期占用 Run 操作锁。脚本结果不因采用异步方式变成另一套权威运行记录。
- 脚本型节点固定自动报告，不提供依赖节点 Chat 的 manual 报告确认。所谓自动关闭仅表示结束执行并回收资源，没有 Chat 或 Tab 可关闭，不表示自动 complete。
- 保留 manager 显式 complete。代码完成确定性检查和实际操作后报告 canComplete=true，manager 无需再判断代码已验证的业务语义，但仍履行现有交接动作。Task 型、Map 型完成逻辑不变。
- 输入输出复用 input_rule、output_rule、实际文件输入和报告 outputs 的逻辑身份，不把结构化参数塞入 node.input。脚本不消费 node.input，也不为脚本强制提供有意义的自然语言指示。
- 结果沿用既有报告的 body、canComplete 和可选 outputs 语义；输出 ID 与文件绝对地址绑定，交接说明和纯正文分别交付。具体承载接口留待 contract，不引入外部程序 stdin/stdout 协议。
- 缺项、格式错误、目标冲突、revision 冲突或定位歧义明确拒绝，不推断授权，不理解自然语言语义，不自行调查任意平台写入接口。业务所需条件必须成为明确、可代码校验的契约。
- 业务成功要求实际写入与完整读回，不以函数返回、候选对象生成、HTTP 成功或报告成功单独代替业务成功。报告保存与通知继续区分，通知失败不撤回已保存事实。
- 不新增故障恢复系统。保留错误、已发生效果和无法确认的部分，用户或 agent 修复后显式重跑；不自动重试、回滚或强行跳过完成门槛。
- 保留 create-blank-bot 的 Bot 身份，将执行目标从原 Task 切换为固定 Rust 能力。核实已有合法目标，避免身份冲突，创建唯一绑定且三指南严格为空字符串的 Bot，完整读回并形成创建后 baseline 和必要 handoff；不创建、修改或运行其执行目标。
- 替代验证后删除原空白创建 Task。用户明确不要求该 Task 的历史兼容、独立 Chat 发起能力或历史恢复环境来源兼容；这不授权删除历史 Chat 或 Run 数据。
- 原 replace-text Bot 及原 Task 绑定完整保留，不重命名、不清空指南、不收窄原通用能力，也不删除其 Task。
- 新建创建流程专用三路合并保存 Bot，三份指南为空，绑定固定 Rust 实现。仅创建 Bot Map 的末尾保存节点改为引用它；修订 Map 继续使用原通用文本替换者。
- 新 Bot 消费当前 baseline、when_text/when_handoff、plan_guide_text/plan_guide_handoff、node_guide_text/node_guide_handoff，输出 saved 和 handoff；保留这些 ID 和现有材料来源接线。唯一正文原样写入对应字段，通过合法配置保存方式汇总一次保存，并核对非目标及关联执行目标不变。
- 创建流程的 request、baseline 和必要交接材料收紧为程序可读的结构化契约，同步调整该流程上游生成和下游消费要求。无需为了格式统一额外复制一份内容相同的参数材料；具体字段设计留待 contract。
- 创建流程专用是调用安排，不新增 Map 私有 Bot 类型、独占引用约束或禁止复用机制。
- 两个业务实现只覆盖当前固定输入输出，不建设通用文本替换引擎、任意平台适配框架或额外业务抽象。框架已有输入输出与报告处理仍复用。
- 新类型需在受影响的定义维护、校验、预检、派发、运行事实读取和观察路径中正确表达，不要求 Task、manager Task 或 Chat 身份来伪装脚本执行；不另建执行器 UI。具体修改范围和接口承诺由 contract 固化。
- 上述共享报告与现有派发/完成流程的复用方向已在访谈中确定；本 spec 不进一步选择模块 seam、数量或测试覆盖分工。

## Testing Decisions

- 必须以真实 Map 节点运行验证两个固定能力，不以直接调用 Rust 函数、配置非空、源码文本检查或模拟业务写入代替。
- 验收包括真实创建空白 Bot、三路定稿合并保存、完整配置读回、baseline/saved/handoff 文件交接、报告保存和 manager 接收、manager 显式 complete 及后续流程使用。
- 必须观察脚本节点未创建 Chat 或 Tab；创建流程中的其他 Task 型节点和 manager 仍可正常使用 Chat。
- 使用真实隔离配置和文件验证目标冲突、旧原文或 revision 不匹配的拒绝，不误写、不报告成功；不引入通用文件替换业务以扩大验收范围。
- 验证原通用文本替换者与 Task 绑定保留、修订流程未被迁移为脚本专用契约。历史兼容不属于旧空白创建 Task 的验收要求。
- 现有 workflow 派发集成测试、shared_business 业务读写验证、节点 report 保存与通知失败验证、nested_completion 完成资格验证可作为 prior art；现有材料契约覆盖真实 revision 冲突和读回，不依赖提示词全文快照证明执行正确。
- 已有报告测试明确区分保存失败和保存后通知失败，并通过真实 TCP 验证通知等待不锁住 Run；沿用这些既有行为约束，不在此决定测试 seam 或逐承诺覆盖层级。
- 所有真实写入与运行验证使用独占临时定义、Run、zone、文件和回环服务，不操作正常服务或用户会话。测试涉及后端进程时须按项目规则终止并确认进程退出后再清理。
- 运行验证遵守项目测试指引，使用当前 worktree 构建产物、独立构建目录及适用的网络和进程安全约束。不得把受控外部响应当成真实 manager/agent 消费证据。
- 本次发布的是 spec，未运行产品测试，未宣称 Rust 能力、迁移或端到端验收已实现。实际测试入口、覆盖承诺和证据责任由后续 contract 确定。

## Out of Scope

- 外部脚本解释器、TypeScript/Python 运行环境、独立业务 exe、运行时编译 Rust 或临时生成代码。
- 沙箱、安全执行平台或新的认证体系。
- 通用替换引擎、任意接口发现、任意配置平台适配或通用业务插件框架。
- 同步等待整批业务结果的新工具、额外批次调度、自动派发下游。
- 自动完成节点、自动重试、自动回滚、新增跳过节点或失败后自动修复。
- Rust 对自然语言授权、需求合理性或正文语义进行判断；也不将已删除的语义判断变成隐含 manager 验收职责。
- 改造原通用文本替换者的能力、绑定或修订 Bot 流程材料契约。
- 保留原空白创建 Task 的历史兼容、为它绑定新的 Bot，或迁移删除无关历史运行数据。
- Map 私有 Bot、强制独占引用机制或新的工作流状态体系。
- 为功能顺带重构无关模块、选择尚未确认的内部 seams 或制定实现票据。

## Further Notes

- 本文按访谈最后确认的方案取代早期“把通用文本替换者直接转为脚本”“保留两个原 Task”“外部解释器/独立 exe”“同步批次等待”“脚本成功自动完成”等已放弃提案。
- 领域术语已记录在 GLOSSARY.md；架构取舍见 [ADR 0019](../../docs/adr/0019-rust-script-bots.md)。相关现有约束见 [workflow 契约](../../docs/workflow-contract.md)、[统一交接与报告规则](../../docs/workflow-runtime.md)及[共同业务材料契约](../../docs/business-materials.md)。这些既有文档仍有仅描述 Task/Map 或节点 Chat 的部分，实现时须同步更新适用范围，不能将文档存在视为脚本执行已经实现。
- 当前创建 Map 的保存节点与修订 Map 的保存节点均引用通用文本替换者；仅迁移前者是明确范围，不可按共享 Bot ID 全量替换所有消费者。
- 当前“空白 Bot 创建者”Task 在已核查的定义数据中仅由同名 Bot 绑定；实现时按现有合法定义修改规则完成切换与删除，不绕过运行期间的禁写约束。
- 需要在下一阶段明确的工程契约包括固定能力标识的表示、无 Chat 执行事实、创建专用材料的具体结构及异常结果到既有流程的映射；它们不改变本文已确定的用户承诺，也不在 spec 阶段擅自定案。
- 下一步：对本 spec 执行 `/to-contract`，选择 seams 并批准具体承诺，然后再拆解实现工作。
