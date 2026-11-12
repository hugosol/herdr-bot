# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "09-map-bot-delegationmd-52586c6f-77f2-4e42-a0c9-",
  "heads": {
    "herdr-bot": "f062bc4c99e5db05961fcf52ac5193e768f4f0ed"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "09-map-bot-delegation",
  "status": "implemented_verified_committed",
  "commit": "f062bc4",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-aeVBCT",
  "delivery": "Map Bot 委派复用真实根 Run／manager 启动；显式 managerTaskId、业务输入及命名材料，保留内部 node zone／管理链。确认漂移拒绝旧预览，公开持久保存 runId、manager sessionId 及真实启动结果。使用与测试说明已更新，已交出 worktree，未自行合入 dev。",
  "acceptance_evidence": [
    {
      "criterion": "复用 Run／材料解析；不启动无 Run 的 manager、不伪造父执行",
      "tests": [
        "map_bot_starts_real_root_run_and_preserves_explicit_manager_and_node_zones",
        "map_startup_failure_retains_run_provenance_and_real_submission_outcome"
      ]
    },
    {
      "criterion": "显式业务输入、manager、zone、材料；内部节点保留自身 zone",
      "tests": [
        "map_preview_requires_explicit_manager_and_all_required_named_materials",
        "map_sender_is_saved_before_input_and_internal_node_keeps_workflow_gates"
      ]
    },
    {
      "criterion": "确认期间含义变化必须重新确认",
      "tests": [
        "map_confirmation_rejects_changed_map_manager_node_binding_and_material_meaning"
      ],
      "cases": "根／嵌套 Map、Bot、manager／node Task、独立内部 zone、命名／直接材料字节、材料删除、词典解析业务含义；均拒绝且无启动效果。"
    },
    {
      "criterion": "公开核实实际关联，保留节点角色及门禁",
      "tests": [
        "map_sender_is_saved_before_input_and_internal_node_keeps_workflow_gates",
        "map_bot_starts_real_root_run_and_preserves_explicit_manager_and_node_zones"
      ],
      "evidence": "prompt 接受前公开读回 manager／Run 委派关联；实际 node cwd／身份／所属 Run；未准备输入及未报告完成仍被门禁拒绝。"
    }
  ],
  "verification": [
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test chief_delegation map_ -- --nocapture",
      "result": "5 passed；开发反馈。另保留确认漂移、启动状态、词典含义的真实 red→green 记录。"
    },
    {
      "command": "node scripts/test-backend.mjs",
      "result": "exit 0；75 个测试目标及 doctest 全部通过，chief_delegation 13 passed（其中新增 Map 5 项）。",
      "evidence": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-kPx7s1/summary.json"
    },
    {
      "command": "npm --prefix frontend run test:browser:build",
      "result": "exit 0；356 passed，1 个既有文件符号链接案例 skipped。",
      "evidence": "artifact://471"
    }
  ],
  "identities": {
    "cargoTargetDir": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-aeVBCT/backend/target",
    "backendSha256": "887c6b87e47edeac2ab48f3958b0207d5ba690aae7aa1fbd8917fbdf1e1a9e8e",
    "herdrFixtureSha256": "76b9c92da237111979812f5bf4bbe3bb24c996e751b2806b8aa42d52390ed86a",
    "cargoLockSha256": "ecf45bd9047d8772dd8c3abddee0f503e843f91230cc12a00725f593246bb36f",
    "frontendLockSha256": "1712c4c2006b8de357902c6c6828e5237977676d1c5bccd0a0939bca5e77581c",
    "staticIndexSha256": "782d6e8009bbbb779b908ec45a8eb18b7c5e505b360410e32241f439b3b0b4d7"
  },
  "consumer_contract": "沿用 /api/chief/delegations[/preview]；Map 请求必须提供 managerTaskId，zoneId 为 manager zone，materials.id 对应 Map input_files。Map commit 返回既有 Run detail（201／启动失败500），Task 保持202。记录新增 runId，sessionId 为实际 manager。已向 Main 同步。",
  "limitations": "本票完成 S2 enabling 覆盖，无真实模型调用；受控 Herdr CLI／原生文件不构成 GPT-6-luna 或 OMP／Pi 原生批准证据。真实模型／宿主与阶段审批验收由13、报告资格由14、恢复由19拥有，未冒认通过。合入后才能按仓库规则关闭票据。"
}
