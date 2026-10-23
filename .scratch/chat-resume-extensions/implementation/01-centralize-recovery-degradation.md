# 01: 集中既有恢复降级，保留精确恢复与定位规则

**What to build:** 用户恢复历史 Chat 时，既有 Workspace 与精确原生 ID 回退有统一、具体且可同时保留的页面反馈；已有目标仍只定位，新进程仍加载同一原生对话并保持历史 cwd。将恢复回退规则集中在同一个文件的 module 中管理，统一降级要素、原因及实际策略，同时保持既有恢复条件、权威结果与执行资格。此票独立交付既有路径，不等待新增来源 Bot 功能。

**Delivers:** P11, P12, P13, P14；P15（Workspace／精确原生 ID 回退覆盖）, P20（既有路径覆盖）。P 编号引用本功能已批准合同。

**Test owner for:** P15 的 Workspace／精确原生 ID 单类降级反馈；P20 的既有路径并发资格、查找／身份核实失败、结构化超时、目标保留及重试查找。其他分区分别由 03／04／05 拥有，本票不宣告这两个 promise 整体完成。

**Blocked by:** None (can start immediately).

**Status:** in-progress

- [x] 将现有恢复回退规则集中管理，抽取共同降级反馈逻辑；持久信息补齐与降级职责分离，不新增自动修复、后台重试或测试专用 interface。为后续默认发现与扩展过滤复用同一机制，但不预造未来能力。
- [x] 已有匹配 Herdr 目标时只聚焦正确 Workspace／Tab／Pane，不重启、不另建目标、不提交输入；即使最新启动配置不可用，也不为仅定位动作额外解析或要求扩展恢复。
- [x] 新进程保持原生 ID／文件身份与历史真实 cwd；包含同名同 cwd 的不同对话反例，不用空白或其他对话替代，不提交新输入，不自动继续中断任务。复用既有真实原生运行证据与回归先例；参数记录不能单独证明真实加载或任务未继续，也不得将本项新增为人工例外。
- [x] 原 Workspace 信息缺失或成功查询确认已删除时，仅回退到配置的唯一默认 Workspace。默认目标不存在、重名或原位置查询失败时明确报错，不任意放置、不创建替代 Workspace。
- [x] 回退后重读及进程重启仍保留原 Workspace ID，不将实际回退位置写回原位置。
- [x] 优先使用可用会话文件；文件不可用时仅以精确原生 ID 加载同一对话。用独立预写的同 ID、另一 ID、不可用文件及不可核实路径样例验证，可读文件身份不一致时拒绝恢复；不扫描最近文件、不改写或修复原生文件。
- [x] 通过交付页面分别触发 Workspace 与精确 ID 回退，显示具体已知原因及实际策略；保留权威失败／未确认结果，不以创建目标或提交命令作为成功依据。多类新降级的组合覆盖归 05。
- [x] 复用并维护既有行为回归：同实例同 Chat 并发不排队重复执行；查找／相关身份核实失败不当作无匹配；超时保持未确认并保留目标；后续重试先查找，不自动销毁、重启或创建副本。
- [x] 使用公开 HTTP、Herdr 控制 interface、真实隔离文件／目录及交付页面验证；浏览器由 agent 实际操作。预期身份和目标来自独立样例，不调用被测实现重算期望。不得改动用户现有 Chat。
- [x] 完成受影响行为测试和实际恢复路径 smoke，记录证据并更新相关现行说明；不新增源码形状、内部转发或文案逐字匹配测试。

## Comments

### 2026-10-23 — 实现与本票验收完成，待独立集成

- S1／S2／S3：`resume.rs::degradation` 统一文件／Workspace 降级选择及 `element`／`reason`／`strategy` 反馈；已发生的降级随 failed／unconfirmed／restored 保留。Herdr adapter 只负责严格 ID 查询；不涉及历史补齐或来源 Bot 的后续实现。保留既有 `fallback` 仅表示 Workspace 回退的语义。
- P11：`chatResumePi.spec.ts` 实际页面在启动目录损坏、同名同 cwd 的另一原生身份并存时仍仅定位已匹配目标，无新目标／输入。真实 Pi／OMP smoke 也在损坏启动配置后仅定位。
- P12：既有 Pi／OMP、同名同 cwd 与 cwd 核实回归通过；新增真实运行记录见 `docs/evidence/chat-history-resume/recovery-degradation-01.json`，专用 Herdr session、旧原生样例的独立副本、真实进程与页面。Pi 显示已知旧输入／回复；OMP 显示旧任务及 Read，保持 idle、没有新 user／tool 活动和模型请求。OMP 原生 CLI 追加一条空 assistant／aborted 记录以标记前进程中断，明确不声称 OMP 文件字节不变。未改动用户现有 Chat；专用服务与模型观测端口已关闭。
- P13／P14：`history_workspace.rs`、`chat_resume_pi.rs`、`chat_resume_omp.rs` 与 `historyWorkspace.spec.ts` 保留原 Workspace、缺失／删除／查询失败／重名、文件优先／精确 ID／另一 ID／不可核实路径及重启覆盖。精确 ID 反馈新增 HTTP red→green：初次缺少 degradation facts 而失败，最小实现后通过。
- P15 本票分区：`chatResumePi.spec.ts` 分别核对单类 Workspace 成功降级、单类精确 ID 启动拒绝降级；Workspace 反馈先 red（空数组），实现后 green。`chatResumeTimeout.spec.ts` 核对未确认时仍显示实际默认位置及已知原因。没有宣告扩展分区或组合承诺整体完成。
- P20 既有路径：并发不排队、查询／身份失败、结构化超时、保留目标与重试先查找的既有 HTTP／浏览器回归全部通过。没有宣告新增扩展分区整体完成。
- 实际执行：`cargo test --test chat_resume --test chat_resume_pi --test chat_resume_omp --test chat_resume_concurrency --test chat_resume_timeout --test history_workspace --test session_history`：35 passed；`cargo build --bins` 成功；`npm ci && npm run build` 成功。
- 实际执行：七个受影响浏览器文件（chatResume、Pi、Omp、Contract、Concurrency、Timeout、historyWorkspace）共 13 passed；随后补强未确认反馈断言后单独重跑 Timeout：2 passed。一次同时运行浏览器和 Cargo 的构建遇到 Windows 二进制占用，改为先 Cargo 后浏览器完成；不将该失败记为通过。
- 现行说明已更新 `docs/integration-boundaries.md`。本票没有未完成的自有自动验收或新增人工例外；真实扩展加载验收不在本票。按 triage 规则合入后方可标记 closed，当前保留 in-progress 供独立集成代理接管。
