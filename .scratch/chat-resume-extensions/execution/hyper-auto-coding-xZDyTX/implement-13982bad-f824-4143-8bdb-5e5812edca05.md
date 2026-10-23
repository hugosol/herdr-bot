# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "04-filter-unavailable-extensionsmd-c6e9056b-1690",
  "head": "c270a0e92d7a08baff942eb9f660bf41ab02754e",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "04-filter-unavailable-extensions",
  "implementation": "完成并已提交，可交由独立集成代理接管 worktree。",
  "commit": "c270a0e",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-vOuzuW",
  "delivered": [
    "Resume 对固定／所选可选扩展逐项检查映射与入口；失效项独立跳过，其他有效项继续提交。",
    "全部过滤后仍提交关闭自动发现的空显式集合；身份未确认时如实返回 502／unconfirmed。",
    "集中降级反馈包含扩展标识、具体原因及跳过策略；普通发起仍严格拒绝相同无效配置且不启动 agent。",
    "更新 README、集成说明、本票验收记录，保存页面截图、受控提交及真实 native smoke 证据。"
  ],
  "coverage": {
    "P5": [
      "missing_optional_mapping_is_filtered_only_on_resume",
      "missing_fixed_and_optional_entry_mappings_are_item_local",
      "missing_entrypoints_preserve_other_extensions_on_resume_only",
      "unreadable_fixed_and_optional_entries_keep_the_other_valid_item"
    ],
    "P6": "empty_explicit_environment_retains_each_warning_without_assuming_confirmation",
    "P15": "chatResumeExtensions.spec.ts：Pi／OMP 交付页面分别验证部分集合的已确认降级，以及空集合的逐项警告和未确认结果。",
    "P18": "上述同一失效 Bot 同时用于普通发起和 Resume；普通发起明确拒绝且没有新增 Herdr 调用。"
  },
  "actual_verification": [
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test chat_resume_environment --test chat_creation --test dynamic_catalog --test bots",
      "result": "44 passed，0 failed；8 + 26 + 3 + 7。"
    },
    {
      "command": "npm run build --prefix frontend",
      "result": "成功。"
    },
    {
      "command": "cargo build --manifest-path backend/Cargo.toml --bins",
      "result": "成功。"
    },
    {
      "command": "npm run test:browser --prefix frontend -- chatResumeExtensions.spec.ts",
      "result": "2 passed，网络护栏通过。"
    },
    {
      "command": "独立 Herdr session 中执行真实 Pi／OMP Resume smoke",
      "result": "两者均确认原始身份／复制文件／历史 cwd 和聚焦，显示已知历史并保持 idle；原生副本哈希不变，隔离模型端点零请求；后续 Resume 均仅定位。专用测试服务与 session 已停止。"
    }
  ],
  "evidence": "docs/evidence/chat-history-resume/extension-filter-04-*；真实 smoke：extension-filter-04-native-smoke.json。",
  "integration_notes": [
    "与票03共享 resume.rs：保留03的 Feedback::environment_args 来源选择、默认发现和读取错误策略；有来源分支用本票 self.environment(&catalog, agent, &bot.env.extensions) 替代 resolve_env＋check_entrypoints，再构造 agent_launch_args。",
    "固定映射缺失样例保留当前 id/kind 并删除 entrypoint，经真实公开配置读取触发。删除整条固定项表示不再属于最新固定集合，不补历史快照；整体配置解析／读取错误仍失败。",
    "项目规定 merged 后才 closed；本票自动验收完成，暂保留 in-progress，供集成后关闭。"
  ],
  "unverified": [
    "H-Pi／H-OMP 完整真实扩展显式／默认／ambient 集合人工确认仍按票07分工，未宣告通过；不新增空集合人工项。",
    "未运行全仓测试；集成后的共享文件与完整回归由独立集成代理负责。"
  ]
}
