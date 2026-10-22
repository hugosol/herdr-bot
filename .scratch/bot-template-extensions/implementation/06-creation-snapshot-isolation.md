# 06: 固定每次创建快照并隔离已有 Bot

**What to build:** 每次已接受的 Bot 创建固定使用同一版本模板和完整映射，并发创建独立推进；后续编辑、删除模板或修改元数据只影响后续创建，不混用正在进行的配置，也不改配、关闭或重建已有 Bot。

**Delivers:** P36、P37

**Blocked by:** 04 — 按模板扩展集合检查依赖并创建 Bot。

**Status:** in-progress

## Acceptance criteria

- [x] 用已有受控 Herdr 能力阻塞一条已接受创建，期间编辑模板及元数据并发起另一条创建；第二条无需等待第一条解除阻塞即可独立推进，不引入全局串行创建队列。
- [x] 分别观察两条创建的独立进度和外部记录：旧创建准确使用预写旧模板、完整 provider/model、thinking、固定扩展、可选扩展及初始输入，新创建准确使用预写新配置；不得混合任一新旧映射。
- [x] 被接受创建的后续推进不重新读取模板或元数据而改变本次已接受配置；解除阻塞后仍按原快照完成既有就绪与提交语义。
- [x] 创建 Bot 后修改模板、删除模板并修改元数据，独立外部调用和对象观察均无新增改配、关闭或重建；不以页面列表不变代替外部结果。
- [x] 修改后从仍有效的模板创建使用新配置；删除的模板不能作为新创建来源，但删除不改变已有 Bot。
- [x] 保持既有失败保留现场及输入不自动重发语义，不新增临时创建状态的持久化、恢复或自动重试。记录并发复现场景及实际观察。

## Verification and scope

依据：[批准契约](../contract.md)、[规格](../spec.md)。本票拥有 P36/P37 的全部约定自动检查；通过公开创建、公开模板编辑及真实元数据修改驱动，使用独立外部调用/对象证据和预写新旧字面配置，不仅断言模板名或私有快照对象。

不依赖 05：并发和已有 Bot 隔离使用有效配置即可观察。此处不重新认领 04 的六组合或 03 的模板生命周期测试。

## Comments

### 2026-10-22：实现与本票验收完成，待独立集成

- 生产路径已经在接受时按值捕获 `BotTemplate` 和完整 `ResolvedPreset`，后续阶段没有模板或元数据读取入口。本票保留该实现，不为已满足行为制造 red，也不新增队列、缓存、持久化或重试。
- P36 / 第 1–3 项：`p36_accepted_creation_keeps_full_mapping_while_new_creation_finishes_independently`。复用受控 Herdr 屏障机制增加按 Tab 名称匹配的 `tab_create_gate`，在原生启动参数构造之前阻塞旧创建。公开编辑模板、真实改写元数据后，新创建在旧屏障未释放时先成功。删除模板并移除元数据之后旧创建仍依次完成就绪、提交和成功。两条外部启动/输入分别匹配预写的新旧 provider/model、thinking、Skill、固定/可选扩展及多行正文，Tab/Pane/agent/会话目录各自独立，只有两组各四次外部调用。
- P37 / 第 4–5 项：`p37_template_and_metadata_changes_leave_existing_bots_untouched`。已有 Bot 完成后编辑模板与元数据，独立 Herdr 日志和对象保持原样；有效模板后续创建使用新映射；删除后使用旧预览直接创建得到 404 `template-not-found`。两条已有 Bot 的外部对象与公开状态不变，无改配、关闭、重建或重发。
- 第 6 项：复用并实际运行既有 `bot_creation` 的启动失败、明确拒绝和投递超时保留现场/不重发测试，以及 `runtime_data::real_restart_does_not_restore_registry_shutdown_intent_or_inflight_creation` 的真实进程重启测试。并发复现步骤与观察同步写入 README。
- 实际命令：`cargo test --manifest-path backend/Cargo.toml --test bot_creation_detached p36_ -- --nocapture` → 1 passed；同命令的 `p37_` → 1 passed。
- 最终回归：`cargo test --manifest-path backend/Cargo.toml --test bot_creation --test bot_creation_detached` → 21 + 10 passed，0 failed；`cargo test --manifest-path backend/Cargo.toml --test runtime_data real_restart_does_not_restore_registry_shutdown_intent_or_inflight_creation` → 1 passed，0 failed。两份修改的 Rust 文件已用 `rustfmt --edition 2024 --config skip_children=true` 格式化。
- 未验证项：本票 S2 受控 Herdr 证据不证明真实 OMP/Pi 的扩展加载（P23/H1 不属本票）。集成尚未执行，依仓库规则状态保留 `in-progress`，由独立集成代理完成合入后关闭。
