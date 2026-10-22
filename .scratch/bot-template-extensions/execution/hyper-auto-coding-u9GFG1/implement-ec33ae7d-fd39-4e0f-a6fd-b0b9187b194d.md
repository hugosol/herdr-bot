# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "01-runtime-data-cutovermd-5382facb-3a23-4ebc-a67",
  "head": "e08e5dfd663bb8b4eba0418077a697d04571b134",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "01-runtime-data-cutover.md",
  "result": "本票实现与自动验收完成，已提交，可交由独立集成代理接管。未操作或合入集成分支。",
  "commit": "e08e5df",
  "branch": "hyper/bot-template-extensions/01-runtime-data-cutover-cb2396-8713386ed6b5",
  "delivered": [
    "默认及随附配置改用 .data/zones.json、.data/bot-templates.json；新 Bot 使用模板文件旁 bot-sessions 下独立、排他创建的新目录。",
    "启动执行一次性切换：完整迁移旧 zone 登记；已有新登记优先且保持原字节；删除授权旧模板、会话及临时文件；成功标记避免重启重复清理。",
    "保留显式持久位置配置，无旧位置自动回退；目录、迁移和写入错误明确失败，不假报成功。",
    "同步 README、随附配置及 Git 忽略规则；不增加临时登记、关机意图或创建进度的持久化恢复。"
  ],
  "acceptance_evidence": {
    "P40_runtime_P47": "default_and_delivered_public_save_and_two_creations_use_fresh_runtime_locations：默认及真实交付 TOML、公开保存与创建、真实独立空会话目录及连续两次进程重启。",
    "P41_P46": "default_and_explicit_template_locations_survive_two_restarts_without_legacy_fallback：非默认模板位置、两次重启、新会话样例保留，以及目标缺失也不回退旧源。",
    "P42": "delivered_ignore_rules_exclude_runtime_and_temporary_files_but_not_metadata：隔离真实 Git 使用交付忽略规则，仅元数据及忽略文件进入候选集；未添加或提交运行数据。",
    "P43_P45": "cutover_moves_only_zone_registration_and_discards_authorized_legacy_data：公开登记、逐字节迁移、授权清理、zone 二进制及无关文件不变、零外部控制调用。",
    "P44": "existing_zone_target_wins_without_merging_or_stopping_startup：不同新旧登记下新目标字节及公开结果保持不变，旧源删除。",
    "P47_failures": "runtime_directory_and_write_failures_are_not_reported_as_success；invalid_zone_migration_target_fails_without_removing_the_old_registration：真实目录冲突、只读写入失败、会话分配失败、错误迁移目标保留旧源。",
    "P48": "real_restart_does_not_restore_registry_shutdown_intent_or_inflight_creation：真实登记、监听、长倒计时及外部屏障阻塞的创建在重启后均不恢复或重放。"
  },
  "verification": {
    "command": "cargo test --lib --test runtime_data --test bot_templates --test bot_creation --test bot_creation_detached --test zone_registry",
    "cwd": "backend",
    "result": "122 passed，0 failed；包含本票新增的 8 个 S2/S3 验收测试。",
    "formatting": "已对本票 Rust 文件执行 rustfmt --edition 2024 --config skip_children=true。",
    "tdd": "已观察迁移目标缺失、会话目录未落盘、运行数据未忽略及错误登记目标仍被接受的行为性 red，随后 green；已有正确行为补充公开验证。"
  },
  "handoff_notes": [
    "本票验收项均已勾选并记录实际证据。按仓库规则，未合入前票据保留 in-progress，不将 agent 完成标记为 closed。",
    "共享集成文件为 config.rs、main.rs、templates.rs、zones.rs、lib.rs、herdr-bot.toml、README.md；已与 02 协调变更位置。",
    "完整 P40 的元数据布局仍归 02，最终合并配置验证归 08。受控 Herdr 和手写会话样例不代表真实 OMP/Pi 模型执行或会话恢复。"
  ],
  "unfinished_owned_items": []
}
