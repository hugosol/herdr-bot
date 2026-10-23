# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "05-guard-concurrent-resumemd-22c0e6d8-1d06-4607-",
  "head": "f6fd6b6f37041f9637ec09a0dc1e9a1bc7233e05",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "05-guard-concurrent-resume",
  "implementation": "已实现并提交，worktree 可交由独立集成代理接管；未操作或合入集成分支。",
  "commit": "f6fd6b6",
  "branch": "hyper/chat-history-resume/05-guard-concurrent-resume-bef4f0-10144a1f16f7",
  "behavior": [
    "实例内按所选历史 Chat 获取非等待执行资格，覆盖 Herdr 查找、创建、启动、确认及聚焦。",
    "重复请求立即返回 HTTP 409 / resume-in-progress，不排队、不自动补做；浏览器显示后端权威拒绝消息。",
    "定位、恢复正常完成或明确失败后释放资格，后续用户新请求可执行；不同 Chat 和独立服务实例不互相阻塞。",
    "接受后的任务持有资格，不随原 HTTP 请求连接提前结束；未改变会话登记生命周期。"
  ],
  "acceptanceEvidence": [
    {
      "criteria": "P16：查找和启动阶段的并发、立即拒绝、不排队、恢复完成后释放",
      "test": "duplicate_requests_during_lookup_and_start_are_rejected_not_queued_then_fresh_request_can_run",
      "evidence": "独立 TCP 请求配合外部查询和启动屏障；重复请求释放屏障前已得到 409，副作用记录只有一次 Tab 创建和 agent 启动。"
    },
    {
      "criteria": "正常定位和明确失败后的资格释放",
      "test": "located_and_explicit_lookup_focus_or_start_failures_release_for_a_new_user_request",
      "evidence": "定位成功及 lookup / focus / start 明确失败后，新的用户请求均重新查找并定位成功。"
    },
    {
      "criteria": "不同 Chat 不全局串行，资格仅限实例内",
      "test": "other_chats_and_separate_instances_are_not_blocked_by_an_active_resume",
      "evidence": "首个 Chat 启动仍被屏障阻塞时，同实例另一 Chat 和另一实例相同 Chat 的请求均可完成。"
    },
    {
      "criteria": "真实浏览器呈现后端忙碌与接受结果",
      "test": "frontend/browser-tests/chatResumeConcurrency.spec.ts",
      "evidence": "真实 Chrome 操作交付页面，无网络 mock；独立 HTTP 请求持有资格，页面显示实际 409 消息，首个操作成功后仍保留拒绝反馈，只有新点击才重新执行。截图：resume-authoritative-busy.png、resume-after-busy-new-action.png。"
    },
    {
      "criteria": "现有 Resume 说明更新",
      "evidence": "docs/integration-boundaries.md 已补充实例内并发边界、忙碌状态及非排队语义。"
    }
  ],
  "verification": [
    {
      "command": "cargo test --test chat_resume_concurrency -- --nocapture（首次 red）",
      "result": "符合预期地失败：未加 guard 时，重复请求没有即时拒绝，有界等待到期。"
    },
    {
      "command": "cargo test --test chat_resume --test chat_resume_pi --test chat_resume_concurrency",
      "result": "17 passed，0 failed。"
    },
    {
      "command": "npm run test:browser -- chatResume.spec.ts chatResumePi.spec.ts chatResumeConcurrency.spec.ts",
      "result": "4 passed；测试网络策略检查通过。"
    },
    {
      "command": "cargo build --bins",
      "result": "通过。"
    },
    {
      "command": "npm run build",
      "result": "通过。首次缺少 tsc，安装锁定依赖 npm ci --prefer-offline --no-audit --no-fund 后构建和浏览器验证成功。"
    }
  ],
  "handoff": {
    "ticketStatus": "验收条目已勾选并记录证据；按仓库规则保留 in-progress，待独立集成合入后标记 closed。",
    "integrationNote": "resume.rs 新增 ResumeExecutions，post_resume 获取资格后通过独立任务调用 resume_selected；OMP 恢复分派应保留在 resume_selected 的无匹配分支中，继续受到同一资格保护。",
    "ownedUnfinishedItems": [],
    "notVerified": "未运行全仓测试；未认领或验证 06 的启动超时与交错场景；未执行其他票据的 H-Pi / H-OMP 人工真实原生对话验收。"
  }
}
