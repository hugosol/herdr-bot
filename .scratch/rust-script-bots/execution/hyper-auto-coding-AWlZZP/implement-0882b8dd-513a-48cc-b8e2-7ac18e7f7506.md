# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "03-merge-save-guidesmd-90ab981a-22bc-4406-86d6-a",
  "heads": {
    "herdr-bot": "48ca8c7b92e7fd22e6af37c7b03c5e52c4360e5c"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "implemented-and-verified",
  "commit": "48ca8c7",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-hloJR7",
  "handoff": "worktree 已交出，可由独立集成代理接管；未操作 dev 或自行合入。",
  "delivered": "save_bot_guides 已编入后端，经真实 Map、manager dispatch、自动报告及命名文件交接运行。原样合并三指南、一次合法保存、完整读回；冲突、运行禁写和部分生效均如实报告。未修改通用替换者。",
  "coverage": {
    "P7": "rust_save_rejects_invalid_materials_without_corresponding_writes：27 类非法／歧义材料，完整对象及实际目标文件不变。",
    "P8": "rust_save_merges_exact_bytes_once_and_delivers_named_complete_saved；rust_save_write_and_complete_readback_failures_preserve_actual_effects：成功、真实写入失败和完整读回失败。",
    "P15": "rust_save_material_failure_preserves_saved_effect_and_original_baseline_conflicts_in_another_run：真实材料故障、已保存配置与失败报告独立读回；解除故障后另一真实 Run 显式派发原 baseline，仍拒绝 revision-conflict。",
    "P24": "固定三段互异 CRLF／LF／tab／首尾空白正文逐字验证；另覆盖合法空正文。",
    "P25": "rust_save_refuses_concurrent_bot_target_and_old_revision：真实并发编辑、旧 revision、目标配置变化；Task／Map 完整配置和非目标字面值对照。",
    "P26": "完整 saved 与真实 Bot／revision／目标一致；真实下游按显式 saved、handoff ID 读取，handoff 不复制正文或完整配置。",
    "P33": "rust_save_map_target_keeps_complete_configuration_and_refuses_active_definition：真实活动 Run 保护其 Map，保存拒绝，完整 Bot 与目标原文件不变。"
  },
  "verification": [
    {
      "command": "npm --prefix frontend run build",
      "result": "通过。"
    },
    {
      "command": "cargo test --offline --manifest-path backend/Cargo.toml --test workflow_dispatch <各 rust_save_* 用例> -- --nocapture",
      "result": "六个专项测试均通过；最初 tracer 按预期因能力未注册而红。"
    },
    {
      "command": "CARGO_BUILD_JOBS=1 CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=C:/Users/hugos/AppData/Local/Temp/hyper-ticket-hloJR7-cargo node scripts/test-backend.mjs --binary-jobs 1 --test-threads 1",
      "result": "完整通过：build、全部后端测试二进制、doctest；0 binaries not dispatched。",
      "evidence": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-PvSzpz/summary.json"
    }
  ],
  "limitations": "在线 Cargo registry 请求曾超时，随后使用已缓存依赖离线完成全部验证。早期未完成验证已取消，不计为通过。S2 原生 manager／H1／H2、页面及正式定义迁移不属于本票完成声明；票据关闭仍需集成完成。"
}
