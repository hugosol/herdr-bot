# 历史 Chat Resume：按来源 Bot 加载扩展与缺失提示

Status: ready-for-agent

Contract: [已批准合同 — Rapid iteration](contract.md)

## Problem Statement

用户从历史会话列表 Resume Chat 后，发现发起时使用的可选扩展没有加载，恢复后的使用环境与预期不同。

当前恢复流程关闭扩展自动发现，只加载当前 Bot 选项目录中该 agent 的固定扩展，不读取来源 Bot。历史会话列表没有保存来源 Bot 标识，因此无法使用来源 Bot 的扩展选择。通过 connect-bot 接入的外部 Chat 也被同样处理，其原生默认扩展被关闭。

项目目前没有 Bot 版本管理。用户不要求冻结发起时的扩展版本或配置，而是希望：由 Bot 发起的 Chat 恢复时使用该 Bot 的最新扩展配置；没有来源 Bot 信息或 Bot 已删除时，按照 agent 原生默认配置加载扩展；个别扩展失效时继续恢复并明确提示，而不是让整个扩展集合失效。

文档还保留了移除 `--session-dir` 时留下的 autoResume 说明，容易让人误以为 herdr-bot 实现了自动选择旧对话续接的功能，需要一起整理。

## Solution

在历史 Chat 初始化收录时记录可选的来源 `botId`，后续不更新该关联。Resume 需要启动新进程时，若关联的 Bot 仍存在，就读取其最新扩展选择，并结合当前 Bot 选项目录中的固定扩展形成加载集合。逐项检查扩展，跳过不可用项，加载其余项；前端明确展示缺失扩展的警告，同时如实展示对话恢复结果。

历史没有来源 Bot 信息，或来源 Bot 已删除时，不关闭 agent 扩展自动发现，也不显式追加 Bot 扩展，使用 agent 的原生默认配置。旧历史无需人工迁移或补绑。

恢复仍然加载同一原生对话，不重新执行发起 Chat 的初始动作。已有 Herdr 目标仍然只做定位，不重启进程或修改其扩展。

同步整理文档：明确原生会话文件由 agent 管理、发起 Chat 不传 `--session-dir`；不再将 autoResume 的历史说明描述成本项目功能或此次恢复规则的依据。

## User Stories

1. As a herdr-bot user, I want a Chat launched from a Bot to remember its source Bot, so that Resume can find the intended extension selection.
2. As a herdr-bot user, I want Resume to use the source Bot's latest extension configuration, so that editing a Bot affects future restorations without requiring version management.
3. As a herdr-bot user, I want the source Bot association to remain unchanged after initialization, so that later activity does not rewrite the Chat's recorded origin.
4. As a herdr-bot user, I want the source Bot association to survive service restarts, so that extension recovery does not depend on an in-memory creation task.
5. As a herdr-bot user, I want connect-bot reports to preserve an existing source Bot association, so that routine registration does not erase provenance.
6. As a herdr-bot user, I want Resume itself to leave the source Bot association unchanged, so that recovery does not rewrite history.
7. As a herdr-bot user, I want restoration from an available source Bot to load its selected optional extensions, so that the restored Chat has the tools selected in that Bot.
8. As a herdr-bot user, I want restoration from an available source Bot to include the current fixed extensions for the Chat's agent, so that fixed integrations remain part of the managed extension set.
9. As a herdr-bot user, I want the managed extension set to exclude unselected ambient extensions, so that restoring from a known Bot does not silently add unrelated extensions.
10. As a herdr-bot user, I want one unavailable extension to be skipped independently, so that other usable extensions still load.
11. As a herdr-bot user, I want missing extension mappings to be reported without blocking the conversation solely for that reason, so that catalog changes do not unnecessarily prevent access to history.
12. As a herdr-bot user, I want missing or unreadable extension entrypoints to be skipped with a warning, so that one broken installation does not discard the remaining extension set.
13. As a herdr-bot user, I want the interface to identify unavailable extensions and known reasons, so that I can understand and repair the degraded environment.
14. As a herdr-bot user, I want extension warnings to be visible as part of the Resume feedback, so that a degraded startup is not presented as silent, complete extension recovery.
15. As a herdr-bot user, I want the same per-extension filtering rule to apply when no extension remains usable, so that a rare empty result does not introduce a different loading policy.
16. As a herdr-bot user, I want a Chat without source Bot information to use native default extension discovery, so that externally started conversations retain the agent's normal loading behavior.
17. As a herdr-bot user, I want a deleted source Bot to fall back to native default extension discovery, so that deleting a preset does not by itself prevent conversation recovery.
18. As a herdr-bot user, I want default-discovery recovery to avoid explicitly appending Bot extensions, so that it does not mix the default environment with a managed override set.
19. As a herdr-bot user, I want old history without a source Bot to remain readable and resumable, so that I do not need to edit existing records.
20. As a herdr-bot user, I want a missing source Bot to be filled only from explicit launch provenance, so that the system never guesses its origin from names, directories or unrelated later activity.
21. As a herdr-bot user, I want a matching Chat already hosted in Herdr to be focused without restarting it, so that Resume does not replace a running environment merely to change extensions.
22. As a herdr-bot user, I want new-process recovery to load the exact original native conversation, so that extension changes never substitute another or an empty conversation.
23. As a herdr-bot user, I want model and thinking restoration to remain under native session rules, so that reading the latest Bot does not overwrite conversation settings.
24. As a herdr-bot user, I want Resume to avoid resending initial instructions or invoking the initial Skill, so that returning to history does not repeat the original task.
25. As a herdr-bot user, I want Resume not to automatically continue an interrupted task, so that I decide when work resumes.
26. As a herdr-bot user, I want real startup, identity-confirmation and focus failures to remain visible even when extensions are missing, so that a warning does not conceal an unsuccessful recovery.
27. As a herdr-bot user, I want documentation to distinguish native session storage from herdr-bot recovery behavior, so that I am not misled into expecting a separately implemented autoResume feature.

## Implementation Decisions

- **领域身份**：Bot 是可复用预设，Chat 是实际对话，历史仍按完整原生 `sessionId` 识别 Chat。本次记录来源使用 `botId`；不以名称规则使用的短 `chatId` 作为配置索引，不建立短标识到启动配置的查询链。
- **持久来源关联**：历史记录新增可选来源 Bot 标识。发起流程提供明确来源时，允许补入尚为空的字段；首次取得有效值后不再覆盖。普通 connect-bot 上报缺少该字段时不擦除已有值，Resume 不改写关联。不主动扫描或猜测旧历史、外部历史的来源，不改变原生会话身份。这里的初始化是来源字段首次取得有效值，不要求某一条历史写入必须先执行。
- **最新配置，不做快照**：恢复时使用关联 Bot 当前的扩展选择及当前 Bot 选项目录的映射。Bot 的编辑可以改变以后 Resume 的扩展集合；不保存发起时扩展快照，不提供版本回放，也不冻结扩展文件内容。
- **有 Bot 的加载策略**：读取当前固定扩展和 Bot 选择的可选扩展，关闭自动发现，只显式加载逐项检查后可用的扩展。该策略适用于现有支持的 Pi 与 OMP。
- **逐项降级**：扩展选择的映射缺失、入口不存在或不可读时，跳过对应项并积累可见警告；不让单项失败废弃其他有效项。固定与可选扩展都按逐项原则处理，不为某一个扩展设置全有或全无策略。
- **空集合**：不新增专门回退逻辑。若全部扩展都被过滤，则有 Bot 分支得到空的显式加载集合，仍保持关闭自动发现。该分支仅尝试恢复，不承诺缺少必要集成时身份确认等后续步骤一定成功。
- **无 Bot 的加载策略**：没有来源关联或关联 Bot 已删除时，不传 `--no-extensions`，也不显式追加 Bot 固定或可选扩展，让 agent 按原生有效配置及恢复工作目录加载。系统默认在本 spec 中指 agent 原生默认配置，不是 Bot 固定扩展集合。
- **警告呈现**：恢复操作产生的扩展缺失反馈必须呈现在前端，尽可能标明扩展名称或标识及已知原因；不能只有服务端日志，也不能把跳过缺失项描述为完整恢复了全部扩展。警告与定位、恢复、失败、启动结果未确认等实际结果应可区分。具体响应字段及展示结构尚未决定。
- **恢复配置边界**：读取 Bot 仅为恢复扩展，不重新应用其模型、thinking、固定初始指示或初始 Skill。无关的模型或 Skill 选择失效不应成为扩展恢复的阻碍。原生模型与 thinking 的恢复遵循既有规则，不扩大本项目保证。
- **保留既有恢复行为**：先查找已有 Herdr 目标，找到则只定位；仅在成功查明无匹配后走新进程恢复。原生身份、工作目录、Workspace、无额外输入、执行资格和未确认启动结果的既有语义不因本次扩展降级而放宽。
- **文档整理**：将此前因移除 `--session-dir` 留下的说明整理为当前明确行为：发起 Chat 不覆盖原生会话存储目录，原生文件由 agent 管理。移除将 autoResume 历史说明当成本项目功能或本次来源关联设计理由的措辞，不改变 agent 配置或新增续接功能。
- **实现涉及的职责**：历史元数据持久化与合并、发起 Chat 后的原生身份收录、Resume 的扩展解析与启动参数、恢复反馈及前端呈现、相关现行文档。普通发起 Chat 的扩展校验策略不因 Resume 的容错要求而一并放宽。
- **trigger config 与 env config**：Bot 在结构上区分 trigger config（发起配置）和 env config（会话环境配置）。trigger config 包含初始消息、初始 Skill 调用、model、thinking；Resume 不重放它。env config 当前包含扩展加载配置，发起与恢复均使用；未来可包含加载型 skills 与系统提示词注入，但这两项能力不在本次实现范围。初始 Skill 调用不同于加载 Skill，初始消息不同于系统提示词注入。
- **集中恢复降级**：所有恢复回退规则集中在同一个文件的 module 中管理，包括扩展逐项过滤、无来源 Bot 时的默认扩展发现、Workspace 回退及精确原生 ID 回退；抽取共同的降级反馈逻辑，统一输出降级要素、原因和实际采用的策略。多个降级应同时保留。该 module 不执行自动修复，也不改变原有回退条件。
- **统一信息补齐入口**：历史收录与 register 的持久信息合并使用统一函数管理字段规则。同一原生身份去重，缺省值不擦除已有值，重复提交相同信息不改变结果；对于同次收录提供的一致且互补的信息，结果不依赖写入先后。写入必须安全串行化，不要求只写一次。来源字段首次有效后不覆盖，首次记录时间与备忘不受重复收录影响。
- **查询只读**：历史查询不触发初始化、持久信息补齐或修复。原 Workspace 名称按已保存 ID 临时解析以供显示，不落盘，不并入持久信息补齐入口。
- **降级结果呈现**：已确认恢复但发生降级时，前端必须明确表示存在降级并列出具体要素，不能仅显示普通成功；失败或结果未确认时同样保留已发生的降级说明。Bot 存储或配置读取损坏不得伪装成 Bot 已删除，也不触发自动修复。
- **合同归属**：验收 seams 与覆盖责任由关联合同记录；内部 module 的构造和具体响应字段不能替代用户可观察承诺。

## Testing Decisions

- 现有测试先例包括：Pi／OMP 各自的历史恢复、创建后的原生身份收录、历史合并与重启持久化、connect-bot 历史写入，以及浏览器中的 Resume 反馈。
- 现有恢复用例将“仅加载固定扩展”作为期望，本次会有意改变这一扩展加载契约；实施时应调整受影响的行为断言，而不是同时保留互相矛盾的恢复规则。
- 既有覆盖的身份确认、cwd、Workspace、已有目标定位、无输入、并发执行资格与启动结果未确认语义仍是回归约束，不应为了扩展容错而删除其行为保障。
- 生产默认配置或用户现有 Chat 不作为可随意更改的验证样例；验证需要避免向用户现有会话提交输入、关闭其 Tab 或修改其原生记录。
- 启动参数记录只能说明要求加载哪些扩展，不能冒充真实 agent 已成功加载对应扩展的证据；入口可读也不等于运行时加载成功。
- 本次对话没有运行恢复场景或测试，没有新增运行证据。本文不宣称现有用例已验证新规则。
- 本节仅记录已有先例与约束，不指定必须采用的测试 seam、替身边界、工具组合或逐条承诺覆盖；这些由 `/to-contract` 决定。

## Out of Scope

- Bot 版本管理、启动配置快照、历史版本选择、扩展源码或依赖版本冻结。
- 覆盖已经取得有效值的来源 Bot 关联；主动扫描旧历史或既有外部历史补绑 Bot，或依据名称、cwd、短 `chatId` 猜测来源。发起流程提供明确来源时补入尚为空的来源字段属于已批准的信息合并，不属于猜测补绑。
- 加载型 skills 与系统提示词注入的功能实现；本次仅为这类未来 env config 内容确定术语与结构职责。
- 新增自动续接功能、选择最近会话、强制新建原生会话，或修改 agent 的原生自动续接配置。
- 处理 Bot 的 agent 类型发生变化；用户明确表示当前没有这种修改手段，本次不新增对应分支。
- 扩展安装、下载、自动修复、导入探测、运行时异常隔离、自动重试或通过重启修复扩展。
- 保证每个可读扩展在 agent 内运行成功，或保证缺失身份集成时仍能完成恢复确认。
- 将普通发起 Chat 的扩展错误改为容错启动。
- 重新提交初始指示或 Skill、覆盖原生模型／thinking、自动继续中断任务。
- 修改现有 Workspace 回退、原生文件管理、connect-bot 接入边界或关机机制。
- 本轮直接实现生产代码、选择 seams、批准承诺或拆分实现票据。

## Further Notes

- 本 spec 是既有[历史 Chat Resume spec](../chat-history-resume/spec.md)的增量：替换其中“恢复只加载当前固定扩展”的规则，不重做整个恢复流程。
- 用户已确认的关键取舍：使用 Bot 最新配置而非历史快照；无来源 Bot 时使用原生默认扩展；扩展独立降级并显式提示；来源关联只初始化、不更新；文档应清理误导性的历史说明。
- 原生 `sessionId` 不会因为收录、恢复或关联 Bot 而变成另一原生会话的 ID。本次不以改变原生身份为任何设计前提。
- 不为极少见的全部扩展不可用情形增加额外策略，以普通逐项过滤结果为准。
- 发布为 `ready-for-agent` 表示可进入后续合同流程，不表示生产功能已实现、seams 已选定或验收已通过。
- seam 选择、六面审阅及 Rapid iteration 验收分配已批准，具体承诺与覆盖责任见关联合同。下一步执行 `/to-tickets .scratch/chat-resume-extensions/contract.md`。
