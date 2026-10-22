# 01: 集中运行数据并完成一次性切换

**What to build:** 使用契约与规格指定的集中运行数据布局启动、保存模板和创建 Bot。只迁移 zone 登记，清除已授权的旧模板和旧会话；切换后不再读取旧位置，正常重启保留新数据。保留显式持久位置配置，不改变 zone 实际目录或临时状态生命周期。

**Delivers:** P40（运行数据布局部分）、P41、P42、P43、P44、P45、P46、P47、P48

**Test owner for:** P40 的运行数据布局覆盖；完整 P40 在 02 的元数据布局覆盖完成后才完成。

**Blocked by:** None (can start immediately).

**Status:** in-progress

## Acceptance criteria

- [x] 默认与随附配置使用规格规定的运行数据位置；在真实隔离磁盘上经公开保存、创建及真实进程重启观察实际结果，不以私有路径函数返回值作为证据。
- [x] 显式指定非默认模板持久位置后，公开保存及重启读取均使用该位置；默认和显式配置均不自动读取或回退旧位置。
- [x] 新目标不存在时，旧 zone 登记完整迁入目标，公开读取保持原登记信息及所指目录；旧源不再被读取，zone 内容通过逐字节或等价外部证据确认未改动。
- [x] 新旧 zone 登记内容不同时保留新目标原样并删除旧源，不合并、不覆盖新内容，也不按冲突停机处理。
- [x] 一次性切换清除授权范围的旧模板和旧会话，不迁移、不恢复它们；旧模板清理后公开列表为空。旁置无关文件、zone 内容保持原样，不发送 agent 关闭指令。
- [x] 切换后新建模板和会话样例，连续两次真实进程重启仍保留新数据；正常启动不会重复执行清空新数据的行为。
- [x] 缺少运行数据目录时可创建；公开创建两个 Bot 后观察独立的新会话位置及无旧上下文的文件证据。目录或写入失败时明确失败，不假报成功。
- [x] 在使用交付忽略规则的隔离真实 Git 场景，运行数据及临时文件不进入候选跟踪集，手工元数据可跟踪；不对用户运行数据执行添加或提交。
- [x] 重启不恢复会话登记、关机意图或创建进度，不因整理数据位置扩大既有生命周期 ADR 的范围。
- [x] 同步随附配置、运行说明及忽略规则；不新增旧模板兼容、双位置回退、数据库服务或 zone 实际目录搬迁。

## Verification and scope

依据：[批准契约](../contract.md)、[规格](../spec.md)。目标布局及授权清理范围以规格和契约 B1/B2 为准。测试预期使用独立预写的登记、文件内容和公开结果；使用真实隔离文件与真实程序启动，不 mock 文件写入结果。

本票拥有 P41–P48 的约定自动检查；P40 只拥有运行数据布局检查，02 拥有独立元数据布局检查。元数据解析和三步编辑不属于本票。01 与 02 可先行，03 不被本票阻塞；最终交付配置的合并验证由 08 执行。

## Comments

2026-10-22：本 worktree 实现及自动验收完成，等待独立集成；按 triage 规则，未合入前保留 `in-progress`，不将 agent 完成等同 `closed`。

- 新增 `backend/tests/runtime_data.rs`，仅通过已批准 S2/S3 验证：真实可执行程序启动/停止/重启、公开 HTTP、真实隔离磁盘及 Git，Herdr 边界使用既有独立进程夹具。
- P40（运行数据）/P47：`default_and_delivered_public_save_and_two_creations_use_fresh_runtime_locations` 使用默认配置和真实交付 TOML（仅隔离网络与外部可执行文件），公开保存、登记和创建两个 Bot，检查实际独立空会话目录，再连续重启两次读回模板及新会话样例。
- P41/P46：`default_and_explicit_template_locations_survive_two_restarts_without_legacy_fallback` 覆盖非默认存储、两次重启及新旧源同时存在/新目标缺失时仍不回退旧源。
- P42：`delivered_ignore_rules_exclude_runtime_and_temporary_files_but_not_metadata` 在独立真实 Git 仓库使用交付忽略规则读取候选文件，不添加或提交运行数据。
- P43/P45：`cutover_moves_only_zone_registration_and_discards_authorized_legacy_data` 核对登记逐字节迁入、公开登记内容、旧模板/会话/临时文件清除、zone 二进制内容和旁置文件不变、无外部控制调用。
- P44：`existing_zone_target_wins_without_merging_or_stopping_startup` 独立预置不同登记，公开读取及逐字节观察确认保留新目标、删除旧源。
- P47：`runtime_directory_and_write_failures_are_not_reported_as_success` 制造真实目录冲突、会话分配失败及只读模板写入失败；`invalid_zone_migration_target_fails_without_removing_the_old_registration` 确认错误目标拒绝启动且保留旧源。
- P48：`real_restart_does_not_restore_registry_shutdown_intent_or_inflight_creation` 在真实进程中建立登记、监听、长倒计时及外部屏障阻塞的创建后重启，公开观察全部临时意图消失且不重放外部创建；不执行真实关机。
- TDD 已观察到迁移目标缺失、会话目录未落盘、忽略规则缺失、目录冒充登记目标仍启动的行为性 red，随后对应 green；已存在行为直接补足公开证据，未人为制造 red。
- 最终执行（`backend/`）：`cargo test --lib --test runtime_data --test bot_templates --test bot_creation --test bot_creation_detached --test zone_registry`，共 122 passed、0 failed，包含本票 8 个新验收测试。另执行指定 Rust 文件的 `rustfmt --edition 2024 --config skip_children=true`。
- 本票未完成项：无。完整 P40 的元数据布局归 02，最终合并配置验证归 08；受控 Herdr 记录和手写新会话样例不宣称真实 OMP/Pi 模型执行或会话恢复。
