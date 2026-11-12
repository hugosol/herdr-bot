# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-09-map-bot-delegationmd-92e93a09-59bf-",
  "heads": {
    "herdr-bot": "666a59bdb79a8523391c2a2fddf666360b1f9060"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "09-map-bot-delegation.md",
  "status": "integration_verified",
  "repository": "herdr-bot",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-aeVBCT",
  "branch": "hyper/chief-bot/09-map-bot-delegation-929b7a-91332fa5e5a1",
  "head": "666a59bdb79a8523391c2a2fddf666360b1f9060",
  "sync": {
    "implementationCommit": "f062bc4c99e5db05961fcf52ac5193e768f4f0ed",
    "implementationIsAncestor": true,
    "mergeInProgress": false,
    "conflicts": false,
    "finalWorktreeClean": true,
    "newCommits": [],
    "changes": "无需修复或额外提交；未操作集成工作区、切换分支或调用 retro。"
  },
  "verification": [
    {
      "command": "npm --prefix frontend run build",
      "exitCode": 0,
      "result": "当前 worktree 静态页面构建成功；存在既有 chunk-size 警告。"
    },
    {
      "command": "CARGO_TARGET_DIR='C:/Users/hugos/AppData/Local/Temp/hyper-ticket-aeVBCT/backend/target' node scripts/test-backend.mjs",
      "exitCode": 0,
      "result": "完整后端目标及 doctest 通过；chief_delegation 13 passed，包含全部 5 项 Map 回归。",
      "evidence": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-4dcH6t/summary.json",
      "ownedTestLog": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-4dcH6t/binary-016.stdout.log"
    },
    {
      "command": "npm --prefix frontend test",
      "exitCode": 0,
      "result": "34 files，253 tests passed。"
    },
    {
      "command": "CARGO_TARGET_DIR='C:/Users/hugos/AppData/Local/Temp/hyper-ticket-aeVBCT/backend/target' npm --prefix frontend run test:browser:build -- --workers=4",
      "exitCode": 1,
      "result": "355 passed，1 skipped，1 failed。workflowRefresh P55.01 在原生 Chrome 初始化要求 visible 时收到 hidden，超时 10000ms。",
      "evidence": "artifact://520"
    },
    {
      "command": "CARGO_TARGET_DIR='C:/Users/hugos/AppData/Local/Temp/hyper-ticket-aeVBCT/backend/target' npm --prefix frontend run test:browser:build -- --workers=1",
      "exitCode": 0,
      "result": "按项目允许的串行资源模式完成全部浏览器验收：356 passed，1 manual-only symlink skipped；P55.01 passed。没有削弱断言或修改源码。此前并行失败保留，不宣称已修复间歇故障根因。",
      "evidence": "artifact://559"
    },
    {
      "command": "git status --porcelain=v1 && git rev-parse HEAD",
      "exitCode": 0,
      "result": "状态输出为空；HEAD 666a59bdb79a8523391c2a2fddf666360b1f9060。"
    }
  ],
  "acceptance": {
    "realRootRunAndExplicitManager": "map_bot_starts_real_root_run_and_preserves_explicit_manager_and_node_zones passed",
    "explicitInputsAndMaterials": "map_preview_requires_explicit_manager_and_all_required_named_materials passed",
    "changedMeaningRequiresReconfirmation": "map_confirmation_rejects_changed_map_manager_node_binding_and_material_meaning passed",
    "publicProvenanceAndInternalGates": "map_sender_is_saved_before_input_and_internal_node_keeps_workflow_gates passed",
    "startupFailureFacts": "map_startup_failure_retains_run_provenance_and_real_submission_outcome passed"
  },
  "identities": {
    "backendSha256": "887c6b87e47edeac2ab48f3958b0207d5ba690aae7aa1fbd8917fbdf1e1a9e8e",
    "herdrFixtureSha256": "76b9c92da237111979812f5bf4bbe3bb24c996e751b2806b8aa42d52390ed86a",
    "safeShutdownShellSha256": "094c332ac7b5d19d3fe0e7496952857f548814baedbf6fdf1b8d97b96a1f5117",
    "cargoLockSha256": "ecf45bd9047d8772dd8c3abddee0f503e843f91230cc12a00725f593246bb36f",
    "frontendLockSha256": "1712c4c2006b8de357902c6c6828e5237977676d1c5bccd0a0939bca5e77581c",
    "staticIndexSha256": "782d6e8009bbbb779b908ec45a8eb18b7c5e505b360410e32241f439b3b0b4d7",
    "node": "v24.18.0",
    "npm": "11.16.0",
    "cargo": "1.98.1"
  },
  "scope": "本票仅 herdr-bot S2 enabling，无本票专属跨仓原生模型验收要求；未调用真实模型。两宿主原生确认与 GPT-6-luna Map/创建验收归13，报告资格归14，恢复归19，本结果不认领这些通过。",
  "blockers": []
}
