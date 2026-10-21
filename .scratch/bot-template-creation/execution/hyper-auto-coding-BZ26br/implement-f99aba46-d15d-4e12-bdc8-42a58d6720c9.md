# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "05-instance-identity-and-snapshotsmd-cee92f4b-58",
  "head": "7d5ad47fbe1c21314602927a6669c5bb94b8a18c",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "completed",
  "ticket": "05-instance-identity-and-snapshots",
  "branch": "hyper/bot-template-creation/05-instance-identity-and-snapsho-7131e0-11684352cd16",
  "commits": [
    "047bbc6 feat: 保证 Bot 实例身份、模板快照与独立新对话",
    "7d5ad47 docs: 记录 Bot 新对话的会话目录与 autoResume 处理"
  ],
  "delivered": "每次创建使用独立 Tab/Pane/agent 控制别名；显示名严格按模板规则生成、允许同名且不追加后缀；模板在创建被接受时按值快照；模板编辑/删除不会改写或关闭已有 Bot；每次创建显式传入独立的 --session-dir（模板存储目录旁 bot-sessions/<创建标识>），使 OMP autoResume 不续接旧对话，Pi 同样从新会话启动。",
  "acceptance_evidence": [
    {
      "criterion": "同一 zone 两次公开创建具有不同 Tab、初始 Pane 和 agent 目标",
      "test": "backend/tests/bot_identity.rs::two_creations_in_one_zone_are_distinct_objects_under_the_same_display_name",
      "evidence": "外部 tabs.jsonl 记录两个不同 tab_id/pane_id；creation 结果分别对应；agent_start 别名与 Pane 均不同。"
    },
    {
      "criterion": "去掉 {bot.id} 后连续创建显示名完全相同，无自动后缀；使用独立对象标识与 Herdr 合法控制别名",
      "test": "backend/tests/bot_identity.rs::two_creations_in_one_zone_are_distinct_objects_under_the_same_display_name",
      "evidence": "两次预览与两个 Tab 的 label 均严格等于预写字面值“alpha · 同名”；tabId/paneId/agentName 互不相同；别名按 [a-z][a-z0-9_-]{0,31} 校验且不等于显示名；prompt 按别名投递。"
    },
    {
      "criterion": "两次同名创建各自收到正确的预写配置与普通任务，不依赖 04 的 Skill/复杂正文",
      "test": "backend/tests/bot_identity.rs::same_name_creations_receive_their_own_configuration_and_input",
      "evidence": "两个模板渲染同名；外部 agent_start 分别收到 omp deepseek/deepseek-flash high 与 pi openai-codex/gpt-6.1-sol medium；prompt 分别精确等于“OMP 固定指示\\n\\nOMP 本次任务”“Pi 固定指示\\n\\nPi 本次任务”。"
    },
    {
      "criterion": "保存旧配置、编辑为新配置后，下一次创建匹配预写的新配置",
      "test": "backend/tests/bot_identity.rs::an_accepted_creation_keeps_its_template_snapshot_while_another_client_edits_it",
      "evidence": "编辑后下一次创建的名字、agent、model/thinking、Skill 与 prompt 均匹配预写新字面样例。"
    },
    {
      "criterion": "挂起已接受创建的启动时，另一独立客户端编辑模板；本次仍按旧快照执行，下一次用新配置；覆盖所有影响创建的字段",
      "test": "backend/tests/bot_identity.rs::an_accepted_creation_keeps_its_template_snapshot_while_another_client_edits_it",
      "evidence": "agent_start 被夹具延迟期间通过独立 HTTP 连接 PUT 模板；本次的 name、agent、model/thinking、Skill、prompt 与 input 全部保持旧字面值；下一次使用新值。实现按值传递 BotTemplate，不持有可变模板引用。"
    },
    {
      "criterion": "创建后编辑并删除模板，已有 Bot 的显示名、对象目标和运行不被重写或关闭；不重复认领 01 的删除后不可选",
      "test": "backend/tests/bot_identity.rs::editing_and_deleting_a_template_does_not_touch_an_existing_bot",
      "evidence": "编辑并删除模板后外部 calls.jsonl 无任何新增控制调用、tabs.jsonl 不变；creation 仍为 succeeded 且 name/tabId/paneId/agentName/inputSubmitted 不变。"
    },
    {
      "criterion": "已有同 cwd 旧对话时验证新会话启动选择，覆盖 OMP autoResume 与 Pi",
      "test": "backend/tests/bot_identity.rs::each_agent_starts_in_its_own_fresh_session_directory_outside_the_zone",
      "evidence": "预置旧对话对照；OMP 与 Pi 的启动均无 --continue/--resume/--session，且各自带唯一的绝对 --session-dir，目录不在 zone 内、不含预置内容；两个目录不同。实现 session_root 位于模板存储文件旁，OMP 源码在存在 --session-dir 时直接创建新会话、跳过 autoResume；真实 TUI 上下文加载仍由 H1 确认。"
    }
  ],
  "verification": [
    {
      "command": "cargo test --manifest-path backend/Cargo.toml",
      "result": "330 passed, 3 ignored, 41 suites (先 npm --prefix frontend run build 生成 backend/static；首次未构建时 herdr_config 因缺 index.html 失败，构建后全绿)"
    },
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test bot_identity --test bot_creation",
      "result": "15 passed (5 identity + 10 creation)"
    },
    {
      "command": "npm --prefix frontend test",
      "result": "25 files / 176 tests passed"
    },
    {
      "command": "npm --prefix frontend run build",
      "result": "tsc --noEmit + vite build 通过"
    },
    {
      "command": "omp --help / pi --help",
      "result": "已安装的 OMP 18.12.4 与 Pi 1.3.0 均列出 --session-dir，真实 CLI 接受该启动参数"
    },
    {
      "command": "herdr workspace list",
      "result": "真实 Herdr 0.9.5 正在运行，bot-playground 存在（仅确认就绪，未创建真实 Tab/agent）"
    }
  ],
  "h1_preparation": "四组合真实环境待用户确认：omp+gpt-6.1-sol、omp+deepseek-flash、pi+gpt-6.1-sol、pi+deepseek-flash（预期启动参数分别为 --model openai-codex/gpt-6.1-sol 或 deepseek/deepseek-flash，thinking 可选 high，外加 --session-dir）。步骤：1) 在真实 Herdr 中确保 bot-playground 唯一存在；2) 对每个组合保存模板并创建，记录 creation id/name/tabId/paneId/agentName，与 Herdr Tab/Pane 及 cwd 对照；3) 同一 zone 用不含 {bot.id} 的命名规则连续创建两次、输入不同任务，确认两个对象同名但 id/Pane/别名不同且各自收到输入；4) 创建后编辑/删除模板，确认 Tab 仍在、名称与 cwd 不变；5) 在 OMP 设置中启用 autoResume，在同一 zone cwd 预建旧对话并记录 OMP 默认会话文件/会话 id，再创建 Bot，检查 <模板存储目录>/bot-sessions/<creationId>/ 下出现新会话文件且 id 与旧会话不同、TUI 未载入旧上下文；Pi 同样检查新会话文件。Herdr 不提供会话 id，证据使用真实会话文件路径/头中的 id 或真实上下文对照，不伪造标识；不要求 connect-bot。真实慢启动下的快照时序可由人工在创建被接受后立即编辑模板交叉确认。",
  "unverified": "真实 Herdr → OMP/Pi TUI 的四组合创建、实际账号/模型可用性、OMP autoResume 下真实 TUI 是否不加载旧上下文、真实移动端体验（H2）均未执行，属于契约 H1/H2 用户确认范围。自动测试使用受控 Herdr 夹具，证明生产 adapter 的启动选择/参数与 HTTP/持久化行为，不冒充真实 agent 会话结果。",
  "integration_notes": "改动文件：backend/src/bots.rs（新增 session_root；agent_launch_args 增加 session_dir 参数；run_creation 增加 session_dir）、backend/src/lib.rs（AppState.bot_session_root；create_bot 按创建标识生成会话目录）、backend/tests/bot_identity.rs（新增 5 个测试）、backend/tests/bot_creation.rs（默认启动参数断言更新，仍验证默认 model/thinking 不传，并断言 --session-dir 存在且为绝对路径）、docs/bot-template-agent-inputs.md、.gitignore（/bot-sessions/）。未修改前端、未改 connect-bot。若 04/06 也改 bots.rs 的 compose/args 或 bot_creation.rs，合并时以“独立会话目录始终随启动传入”的接口为准。worktree 已提交且干净，未操作集成分支。"
}
