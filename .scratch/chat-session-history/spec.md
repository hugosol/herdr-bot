# chat 历史会话列表与原生会话存储回归

Status: ready-for-agent
Labels: ready-for-agent

日期：2026-10-22。

本文综合本次对话中已确认的产品决定，不规定实施顺序或内部构造。已批准的承诺、seam 和验收责任见 [contract](contract.md)；下一步为对该 contract 执行 `/to-tickets`。

## Problem Statement

herdr-bot 的 chat 当前呈现的是 connect-bot 的当前会话登记，而不是长期保留的对话目录。登记会因注销、过期清理或后端重启消失；从 zone 创建的 Bot 也没有独立于 connect-bot 的持久历史记录。用户无法在一个稳定列表中找回曾经创建或接入的对话，也无法以此作为后续会话管理的基础。

用户需要的“历史”不是“已经结束的会话”：只要对话成功创建或接入，就应收录，与运行状态无关。同一原生对话可能既由 zone 创建，又通过 connect-bot 上报，不能形成两条历史记录。

此外，当前 Bot 创建会覆盖 agent 的原生会话存储目录。这个方案原本用于避免 OMP 的隐式 autoResume，但用户当前没有启用该配置；覆盖目录已经影响从正常启动的 agent 中通过原生 `/resume` 发现这些对话，并可能影响依赖正常存储位置的其他工具。历史目录不应以接管原生会话文件存放位置为代价。

## Solution

在现有 chat 入口内提供“当前／历史”两个互斥视图。默认仍为当前会话登记；通过右上角三角按钮切换到不同底色、信息密度更高的历史会话列表。

历史会话列表是由服务端持久保存、供本机与局域网设备共享的对话元数据目录。zone 成功创建 Bot 或 connect-bot 成功注册后收录，以可信的完整 sessionId 去重。它不显示或维护运行状态，不承载聊天正文，不因当前登记移除而消失。

本次只展示显示名称、agent 类型、cwd 和首次收录时间。按首次收录时间倒序，每页 50 条，提供上一页／下一页。没有名称时显示 sessionId，其余缺失展示值显示 `—`。不做搜索、筛选、原生对话阅读、恢复或删除。

新建 Bot 不再覆盖原生会话存储目录，遵循 agent 自身的正常存储配置；herdr-bot 只保存历史元数据及可获得的原生引用，不复制或管理聊天正文。zone 创建成功后，历史收录在独立任务中执行，身份获取最多等待 30 秒，不延迟创建成功响应，也不在创建结果中展示历史状态；失败只记服务端警告日志。connect-bot 仍向操作者区分接入成功与历史保存失败，用户以后再次成功注册时可以重新尝试收录。

## User Stories

1. As a herdr-bot user, I want successful zone-created Bots to appear in history, so that I can find conversations created through the main interface.
2. As a herdr-bot user, I want externally started conversations to enter history after a successful connect-bot registration, so that history is not limited to Herdr-created Bots.
3. As a user creating a Bot without selecting connect-bot, I want that successful creation to be recorded, so that optional connection settings do not determine whether history exists.
4. As a herdr-bot user, I want the same native conversation to have one history entry, so that creation and connection do not produce duplicates.
5. As a herdr-bot user, I want repeated registrations of the same sessionId to update the existing entry, so that reconnecting does not grow duplicate history.
6. As a herdr-bot user, I want history to include active conversations, so that I do not have to wait for a conversation to end before it is recorded.
7. As a herdr-bot user, I want one conversation to be allowed in both current chat and history, so that the two views answer different questions without conflicting membership rules.
8. As a herdr-bot user, I want history to survive backend restarts, so that it remains useful across service lifecycles.
9. As a herdr-bot user, I want browser refreshes and browser closure not to erase history, so that records are not tied to one frontend session.
10. As a LAN user, I want the same persisted history on my computer and phone, so that both devices reflect the same service instance.
11. As a herdr-bot user, I want unregister and current-registration expiry not to remove history, so that disconnecting does not erase my conversation directory.
12. As a herdr-bot user, I want removing a zone registration not to remove its conversation history, so that directory registration and conversation retention remain separate.
13. As a herdr-bot user, I want history retained indefinitely without automatic count or age eviction, so that older records remain available.
14. As a user creating a Bot, I want recording to follow confirmed creation success, so that an allocated directory or partially started operation is not mistaken for a successful Bot creation.
15. As a user creating a Bot, I want initial-input submission confirmation rather than task completion to determine creation success, so that history does not depend on how long the model works.
16. As a connect-bot user, I want history recording to follow an accepted registration, so that rejected registrations do not create history.
17. As a user with incomplete connection metadata, I want a valid sessionId to establish a history entry, so that missing optional details do not prevent recording.
18. As a herdr-bot user, I want later valid metadata to fill or update the same history entry, so that records improve without changing identity.
19. As a herdr-bot user, I want omitted metadata not to erase known values, so that an incomplete reconnect does not degrade history.
20. As a user creating named Bots, I want the Bot display name to take precedence over a connect-bot session name, so that the name I chose remains recognizable.
21. As a user of externally connected conversations, I want the latest valid reported session name to appear, so that native session renaming is reflected in history.
22. As a herdr-bot user, I want an unnamed conversation to display its sessionId, so that it remains identifiable.
23. As a herdr-bot user, I want to see the agent type, so that I can distinguish Pi, OMP, or another reported agent without relying on model names.
24. As a herdr-bot user, I want to see cwd, so that I can recognize the project or directory associated with a conversation.
25. As a herdr-bot user, I want to see the first collection time, so that I can understand when a conversation entered this directory.
26. As a herdr-bot user, I want the first collection time to remain unchanged on reconnect, so that history order is not driven by connection churn.
27. As a herdr-bot user, I want the newest collected records first, so that recent additions are easy to find.
28. As a herdr-bot user, I want pages of 50 records with previous and next navigation, so that long-lived history remains usable.
29. As a desktop user, I want a compact table, so that more history fits on the screen than current chat cards.
30. As a mobile user, I want compact readable entries, so that the same four fields remain usable on a narrow screen.
31. As a herdr-bot user, I want a top-right triangle button to switch current and history views, so that history is available inside chat without another top-level tab.
32. As a herdr-bot user, I want different panel backgrounds and explicit current/history labels, so that I can distinguish views without relying only on color or triangle direction.
33. As a herdr-bot user, I want opening or fully refreshing the page to default to current chat, so that the existing entry behavior is preserved.
34. As a herdr-bot user, I want switching into history to read the latest persisted records, so that I do not mistake an old browser-local list for current history.
35. As a user creating a Bot, I want history collection to run independently without delaying creation success, changing the creation result, terminating the agent, or repeating my initial input, so that collecting metadata does not disrupt real work.
36. As a connect-bot user, I want registration to remain successful when only history saving fails, so that history storage does not silently disarm an otherwise accepted connection.
37. As a herdr-bot operator, I want zone history-collection failures recorded in server warning logs and connect-bot history-save failures reported to the connecting user, so that each failure is observable through its agreed feedback channel.
38. As a connect-bot user, I want a later successful registration to try history recording again, so that a previous save failure can be recovered without duplicate entries.
39. As an agent user, I want new Bot conversations stored according to the agent's normal configuration, so that ordinary native session discovery is not bypassed by herdr-bot.
40. As an agent user, I want existing native files left untouched by this feature, so that adding a metadata directory does not risk deleting or rewriting my conversation content.
41. As a herdr-bot user, I want existing keepAlive and shutdown-monitoring behavior unchanged, so that history does not create a second source of runtime state.
42. As a herdr-bot user, I want this delivery limited to display rather than resume, so that future execution controls are not introduced before their behavior is agreed.

## Implementation Decisions

### 1. 范围与领域模型

- 历史会话列表是持久元数据目录，不是离线会话列表、已结束 Bot 列表、当前登记的备份或原生对话文件。
- 当前 chat 继续显示所有当前已登记会话，包括现有离线保留窗口中的登记。不能把这个视图改成“确认仍有进程运行的 Bot 列表”。
- 历史与当前登记的成员关系独立；收录历史不建立当前登记，不修改 keepAlive、agent 状态、完成后关机监听集合或关机倒计时。
- 历史不维护在线、离线或结束状态，不把原生文件是否仍存在当作列表成员条件。

### 2. 收录时机与成功语义

- zone 创建沿用当前成功定义：目标 agent 可交互，且合成初始指示的提交已经得到确认，创建进入 succeeded。无需等待模型回答、Skill 执行结束或任务完成。
- 只有进入上述成功阶段，才以 zone 创建来源收录；部分创建、目录分配或仅生成文件不是 zone 收录的成功凭据。
- zone 收录不依赖选择 connect-bot。必须取得与本次实际创建对应的真实原生身份；不能拿创建标识、Herdr Tab/Pane 标识、控制别名或 cwd 冒充 sessionId，也不能按“最新文件”猜关联。
- connect-bot 在服务端接受 register 后尝试收录。用户明确接受其 sessionId 就是正确的原生对话身份，不额外要求提供 agent 类型或文件路径才能建立历史。
- 两种来源可能先后或并发到达，最终仍是一条历史记录。若 zone 创建失败，但该原生对话独立成功完成 connect 注册，它仍可依 connect 收录规则进入历史。

### 3. 唯一身份与元数据合并

- 直接使用完整 sessionId 作为历史唯一身份；不使用 ID 前缀，不把 agent 类型作为必填复合键，不按名称或 cwd 合并不同 ID。
- 同一原生对话的重复接入或不同收录来源均更新同一记录。用户接受本需求中的 sessionId 身份假设，而不是要求系统实现跨 agent 的全局唯一性证明。
- 有效的后续元数据可以填空或更新已有值；字段未提供或无有效值时，不清空历史中的已有值。
- zone 创建产生的 Bot 显示名优先于 connect-bot 会话名；仅通过 connect 接入的记录使用最近一次有效上报的会话名。跨来源到达顺序不改变这项优先级。
- 可获得的原生会话文件绝对路径作为内部引用保存并允许更新，但本次不展示，不读取或复制对话正文，不据此执行恢复。
- 首次收录时间是历史条目首次成功持久化的服务端时间，后续重复接入、名称更新、cwd 或路径更新均不改写。此前尚未成功保存的尝试不能被当作已有持久记录。
- 历史的合并规则不要求顺带修改当前会话登记原有的字段覆盖或注册排序语义。

### 4. 持久化、错误与再次接入

- 历史由服务端持久保存，供同一实例的本机与局域网设备共享；不是浏览器本地收藏或前端自建事实来源。
- 成功收录必须代表已经保存；保存失败不能只在内存中放一条记录后向用户宣称已持久化。
- zone 创建成功立即可见；原生身份获取与历史记录由本次独立任务执行，不处于创建响应的关键路径，不为等待身份持有当前登记或关机状态等共享锁，也不阻塞其他创建、connect-bot 或当前观察请求。任务与页面生命周期独立，关闭弹窗不取消。
- 原生身份获取总预算最多 30 秒。取得精确身份后尝试保存一次；身份超时或保存失败即结束本次任务，不猜身份，不增加无期限等待或后台写入重试。具体线程和任务组织不属于产品约束。
- 历史写入失败时，不撤销已经成功的 Bot 创建或 connect 注册，不关闭 Tab、不停止 agent、不重发初始指示。
- zone 创建结果只表达 Bot 创建本身，不增加历史保存状态或历史专用轮询；身份获取失败、超时或历史保存失败仅记服务端警告日志，不增加创建页面提示。日志需能关联到对应创建及失败原因。用户接受创建者不会立即知道该对话尚未收录的取舍。
- connect-bot 反馈仍必须区分接入成功和历史保存失败，向操作者明确提示；不能只增加一个现有 caller 会忽略的响应字段。
- 一次历史更新失败不能损坏此前已经保存的记录，也不能把无法读取的历史伪装为空列表再覆盖。
- 后续再次成功 register 时，重新尝试历史写入：已存在则更新，不存在则新建，不生成重复项。
- 本次不增加后台自动重试、自动重新 connect 或新的失败预算。现有接入和重试策略不因历史功能被隐式改写。
- 历史读取失败应与真正空列表区分，不能把“读取失败”显示成“没有历史”。

### 5. 保留与展示

- 历史永久保留，本次没有过期、数量淘汰、自动清理或删除入口。
- 注销、当前登记过期、后端重启、zone 取消登记，不删除历史。
- 仅展示显示名称、agent 类型、cwd、首次收录时间；不展示模型、来源、运行状态、keepAlive 或关机监听操作。
- 无名称时用 sessionId 代替；其余缺失展示值使用 `—`。
- 默认按首次收录时间倒序，重复接入和心跳不影响排序；每页 50 条，上一页／下一页导航。
- 当前与历史在 chat 内通过右上角三角按钮切换，使用不同底色和明确文字标签；桌面为紧凑表格，手机为紧凑条目。
- 打开或整页刷新默认回当前 chat；进入历史或点击上一页／下一页时读取当时最新的持久结果。停留在当前页时不自动刷新或实时重排，不承诺多次翻页间的冻结快照；并发新增时记录可能跨页移动，出现相邻页重看或本轮翻页漏看的情况，但不等于持久数据重复或丢失。用户接受普通分页的取舍；以后若要求连续新增期间严格不重不漏，需重新审视分页 interface。具体传输与内部实现不在此固定。

### 6. 原生存储行为修正

- 新 Bot 不再由 herdr-bot 指定或预先分配专属原生会话目录；移除为该目录服务的创建前置条件，不因历史功能而搬迁新对话。
- 遵循 Pi／OMP 自身正常的存储配置，不把用户配置覆盖成一个由 herdr-bot 控制的新位置。
- 用户当前没有启用 OMP autoResume。本次不修改其配置，也不为防范未来可能启用的配置引入替代性强制新会话机制。
- 不将“新建 Herdr Tab”描述成 agent 一定产生新对话的保证。用户以后更改 agent 配置时，原生行为由 agent 决定，不能伪称 herdr-bot 已独立隔离。
- 既有原生会话文件、附属文件和已经迁移的文件均不是这次历史功能的删除或重写对象。
- 实现时需同步修正此前以独立会话目录规避 autoResume 的运行说明，避免继续要求用户依赖已撤销的存储策略。

### 7. 权威、来源规则及未决定的技术事项

- 沿用现有同机写入、局域网读取的权限语义，不扩大 connect 会话写入来源，不新增认证方案。
- 沿用当前会话登记和关机功能的后端权威模型；持久历史不使这些临时状态变为持久状态。
- 本 spec 不固定 seam placement 或内部构造；已批准的 seam 与可观察承诺以 contract 为准。存储格式、具体编码及内部任务组织等未被 contract 冻结的事项仍保持实现自由，不在 spec 中补做技术选择。
- 必须在后续 contract 中闭合 zone 原生身份采集、跨来源名称优先级以及保存失败警告实际可见的承诺；不能以仅实现 connect 来源或仅返回不可见警告来缩减本 spec。

## Testing Decisions

已有可参照的覆盖包括：当前会话注册、重复注册、注销、过期清理、SSE 快照、写入来源约束，Bot 创建的阶段与失败处理、断开 HTTP 后继续创建、模板与 zone 持久化，以及桌面／手机浏览器的 Bot 操作验收。现有模式是事实来源，不要求原样复制其断言或增加只检查 wiring 的测试。

本次对话及项目规则确定的验证约束：

- 具体 seam、在各 seam 上承诺什么、承诺分别如何覆盖，由 `/to-contract` 决定；这里不指定测试层次分配或 mock 接口。
- 不以编译成功、参数回显或测试替身记录，代替真实原生身份、原生存储和页面展示的运行证据。
- 改变原生存储行为后，需在隔离会话中验证真实存放位置及相应 agent 的原生发现能力；不能向用户已有对话提交测试输入，或把 CLI 帮助视为恢复实测。
- 页面改动需实际打开桌面／手机尺寸页面，观察切换、不同样式、四项字段、缺失值及分页，不只验证内部数据结构。
- 验证跨重启保留、同 ID 归并、来源先后顺序、名称优先级、缺失不清空、首次时间不变、保存失败和再次注册恢复等用户可见行为；永久测试仅保留能捕捉实际行为错误的内容。
- 若使用受控时间或故障注入，必须确定性、隔离且不影响完整测试套件；不得用真实关机、停止用户 agent、删除真实会话或改写用户全局设置作为验证手段。
- 本次发布 spec 没有运行新的实现测试，也没有重新执行文件迁移或恢复会话。已有迁移验证不能代替本功能尚未实施的验收。

## Out of Scope

- 搜索、筛选、模型列、来源列、正文展示或全文检索。
- 从历史列表执行 resume、restore、打开 Herdr Tab 或任何会话控制操作；会话恢复曾是最初目标，但已明确从本次交付移除。
- 展示或推断历史会话的当前运行状态、存活性、退出原因或完成状态。
- 历史删除、定期过期、容量淘汰、归档清理或原生文件存活性扫描。
- 自动扫描、导入上线前的旧对话；旧对话在上线后重新成功 connect 时可按普通收录规则入库，不属于批量旧数据迁移。
- 原生会话文件复制、迁移、格式转换、删除、重写，以及自动修复 Herdr 或 extension 保存的旧路径引用。
- 对上一次迁移遗留的运行中会话或类型不明文件进行后续搬移；这是独立维护工作。
- 启用、关闭或覆盖用户的 autoResume、原生会话目录及其他 agent 全局配置；新增强制新会话替代策略。
- 改变 connect-bot 的可选属性、自动启用 connect-bot、重新注册策略、心跳节奏或失败预算。
- 扩大到跨机 agent 接入、多实例路由、认证系统或新的会话身份生成服务。
- 将当前登记、完成后关机监听集合或关机倒计时持久化。
- 在 spec 阶段实现功能、选定 seam、发布实施 ticket 或确定完整接口 contract。

## Further Notes

### 已知技术事实，不等于已选实现

1. 当前 zone 创建进度及 connect-bot 登记都存在内存，没有产品级持久历史目录。已有原生文件不等于已有历史记录。
2. 当前 Herdr 的 agent／pane 查询可包含原生会话引用；引用可能是 ID，也可能是绝对路径，而且是可选的。现有 herdr-bot 启动 adapter 未向上保留这些信息。
3. 原调查发现创建显式关闭扩展自动发现且未保留 Herdr 官方集成；随后按用户要求，Pi／OMP 官方集成已加入固定扩展配置。隔离的真实 Pi／OMP TUI 已向本地命名管道发出官方会话引用上报，未提交模型输入；该 smoke 不是对真实 Herdr 保存结果的端到端验证。后续 contract 仍须闭合本次创建身份的精确取得、时序和引用缺失处理，不能把可选查询字段当作必然存在，也不能要求用户必须勾选 connect-bot。
4. connect-bot 当前实际发送原生 sessionId、agent 类型、cwd、可选名称和模型，但没有发送原生会话文件路径。后端能接收路径字段不代表当前 caller 已提供它。
5. connect-bot 当前成功 receipt 只保留接入结果和在线数量，其他成功响应字段会被忽略。历史保存失败若只写进服务端响应，用户不会自动看见。当前 feedback 面向操作者 UI／stderr，不等于模型上下文。
6. Pi／OMP 当前都使用 version 3 的原生会话 header，包含 ID 和 cwd；OMP 文件可能在 session header 之前有 title 元数据行。精确文件路径可用于读取 header，但不能仅取第一物理行，也不能用格式相近推断原始 agent 身份。
7. 两者当前默认生成 UUIDv7，没有产品 namespace 保证；打开或复制原生文件会保留 ID。对话中本机检查的 Pi 默认目录 55 个、OMP 默认目录 1183 个、自定义目录 17 个主文件，组间 sessionId 交集均为 0。这只证明当时所扫描目录无交集，不是未来绝对唯一性的证明；用户据此确认本功能直接以 sessionId 去重。
8. 原生文件目录调整是原有功能的策略，不是 Herdr 创建 Tab 的要求。当前原生发现逻辑不会自动把任意自定义目录并入默认 `/resume` 列表。
9. 此前已独立迁移 10 条可识别且未运行的 OMP 会话，共 42 个文件，字节校验一致；本机 OMP 原生发现逻辑确认 10 条均可发现且符合选择器展示条件。1 条运行中的 OMP 和 6 条未确定类型的主会话当时原位保留。这不是本 spec 的实现状态。

### 与现有领域决定的关系

- [领域词汇表](../../GLOSSARY.md)：沿用 Bot、connect-bot、会话登记、历史会话列表、keepAlive 和会话恢复的区别。
- [ADR-0004：会话写入来源](../../docs/adr/0004-session-write-boundary-same-host.md)：保持同机写入、局域网读取的权限语义。
- [ADR-0006：后端权威会话登记生命周期](../../docs/adr/0006-server-authoritative-session-lifecycle.md)：历史是另一个持久目录，不撤销当前登记的有界保留，也不恢复监听或关机意图。该 ADR 顶部保留历史性的“目标设计”说明，不能据此忽略当前已实现的两窗口清理。
- [原有 Bot 初始化与目录说明](../../docs/bot-template-agent-inputs.md)：本 spec 明确撤销其中以独立原生目录规避 autoResume 的方案；实施时需更新对应说明，不沿用旧目录要求。

### 调查与维护证据

- [Herdr 关闭 Tab 与会话恢复调查](../herdr-tab-close-and-session-resume.md)：理解原生引用与会话恢复的区别；恢复不在本次范围。
- [自定义目录、原生发现与 ID 检查](../agent-resume-custom-session-directory.md)。
- [已执行的一次性 OMP 文件迁移](../omp-session-migration-2026-10-22.md)及其关联清单。

下一步：执行 `/to-tickets .scratch/chat-session-history/contract.md`。获批承诺及 seam 见 [contract](contract.md)，本 spec 只维护需求和范围，不重复承诺表，也不预切实施工作。
