# 02: 成功 register 形成可持久读取的历史视图

**What to build:** connect-bot 的 register 被服务端接受后，将对话元数据保存为历史。用户可在 chat 内切换当前／历史两个互斥视图，在电脑和手机读取同一实例的持久记录；关闭浏览器或重启后端不丢失历史。

**Delivers:** P03、P06、P08、P10、P14、P15、P27、P28；P25 的显式 register 收录分区、P26 的正常注册／更新／重启分区。

**Test owner for:** P25 的不批量导入及旧对话显式 register 收录；P26 的正常 register、元数据更新及重启。两项 promise 均由后续分区共同完成。

**Blocked by:** None (can start immediately).

**Status:** in-progress

- [x] 沿用现有 register 必填字段与来源校验；只有服务端接受的 register 触发收录。分别覆盖有效注册、缺可选元数据、未知非 register 写入和被拒绝来源，观察公开回执及历史结果。
- [x] 使用可信完整 sessionId 标识记录；正常重复 register 更新同一记录，不按名称、cwd 或 ID 前缀误合并。不额外要求 agent 类型或原生文件路径才能收录。
- [x] 后续有效元数据填充或更新已有值；缺失或无有效值不清空已知值。原生会话文件精确绝对路径允许作为内部引用更新，但不在页面展示，不读取正文。
- [x] 提供服务端持久历史的公开只读读取；用已知记录重启隔离后端，再从独立客户端及浏览器上下文读回精确集合。浏览器刷新、关闭和重新打开不改变持久数据。
- [x] 实际点击 chat 右上角三角按钮，验证当前／历史两个方向互斥切换、明确文字标签和不同面板底色，不仅依赖颜色或三角方向。
- [x] 实际桌面和手机尺寸浏览器检查紧凑表格／紧凑条目，只展示名称、agent 类型、cwd、首次收录时间。长名称和长路径可读；无名称回退完整 sessionId，其余缺失展示值为 `—`，不得猜类型或以“离线”替代未知值。
- [x] 历史不展示模型、来源、keepAlive、运行状态或关机监听操作；交付页面和已文档化公开 interface 不新增搜索、筛选、正文、恢复、删除或原生文件存活性扫描能力。
- [x] 保持同机写入、局域网读取边界，不新增认证。通过公开请求分别验证允许与拒绝来源；拒绝写入不建历史，局域网仍可按既有权限读取。
- [x] 不扫描或批量导入旧会话；预放旧对话在没有注册事件时不入历史，之后对其明确成功 register 可普通收录。此项为 P25 的本票覆盖，不重复 01 的存储策略保护或 05／06 的精确引用读取检查。
- [x] P26 本票覆盖正常 register、元数据更新及隔离重启：保持原有 keepAlive、登记生命周期、监听和倒计时语义；重启后历史存在，当前登记及临时关机意图不恢复。复用必要的既有行为覆盖，绝不执行真实关机。
- [x] 通过公开 HTTP 和实际浏览器验证完整 register→持久化→展示链路，更新受影响的使用说明与公开 interface 文档。分页归 03，故障反馈和恢复归 04，不以它们尚未完成代替本票正常路径证明。

## 验证边界

依据已批准 contract 的 S1、S2。独立预写 ID、字段及期望值，不能调用被测合并实现生成期望。基本唯一性与已知数据安全从本票起成立；07 拥有跨来源与并发组合验证，不允许此前明知错误仍合入。

## Comments

本票实施及验收完成，待独立集成代理合入后按项目规则置 `closed`；当前不操作集成分支。

- 批准 seam：S2 交付 HTTP interface、S1 真实浏览器；未把存储内部提升为测试 seam。
- Red→green：历史读取最初为 404；空白更新最初把已知名称清空；浏览器最初找不到切换按钮。随后对应行为均通过。
- P03／P27：`accepted_register_is_readable_as_history_without_optional_metadata` 与 `rejected_sources_missing_required_fields_and_non_register_events_never_collect` 覆盖成功接入、可选字段缺省、必填 cwd 缺失、三类未知非 register、拒绝来源及局域网只读。来源拒绝使用真实路由的受控 ConnectInfo，非物理异机连接。
- 基本唯一性／P08：`full_identity_and_valid_metadata_survive_partial_and_empty_updates` 使用独立完整 ID、同名前缀及 cwd，核对有效覆盖、后续类型填充、空白和缺省不清空。公开读取固定只有五个字段，原生引用绝不下发。
- P25／P26 本票分区：`explicit_register_survives_restart_without_restoring_current_or_shutdown_intent` 预放旧文件及附属记录，确认无 register 时历史为空、之后显式 register 收录、元数据更新不清除当前状态和监听，重建隔离后端仅恢复历史。原生文件字节未改变；关机动作为永不接触系统的测试替身。
- P06：`durable history survives refresh, browser closure and backend restart for independent device contexts` 关闭桌面浏览器上下文、重启真实交付后端，独立 HTTP 客户端及手机浏览器上下文读回精确集合；本机网卡入口实际可达，刷新仍默认当前视图，历史不变。
- P10／P14／P15／P28：`chat current/history are exclusive and readable at 1280px/390px` 实际双向点击三角按钮，核对标题、不同底色、互斥面板、四个展示字段、完整 ID 回退、`—`、长名称路径换行及无越界；没有模型、来源、存活状态、关机操作、搜索或正文等界面。已查看两张实际页面截图（Playwright test-results 下 history-1280.png／history-390.png）。README 与示例 TOML 已更新公开 interface、配置、使用说明及变更记录。
- 实际验证：`cargo test --test session_history` 最终 4 passed；包含 session_events、session_write_boundary、session_expiry、session_report_recovery、shutdown、session_window_config、recovery、shared_observation 及 session_history 的回归命令共 9 suites／74 passed。
- 实际验证：`npm test` 为 27 files／205 passed；`npm run build`、`cargo build` 成功；最终 `npx playwright test sessionHistory.spec.ts` 为 3 passed（真实 Chrome，未替换 HTTP 后端）。
- 未完成事项：本票无实施或验收遗留；尚未合入集成分支。分页、故障反馈／恢复及 zone 收录不属于本票，不宣称已覆盖。未执行真实关机、模型任务、原生配置修改、旧会话批量导入或其他票据操作。
