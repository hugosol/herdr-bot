# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-06-plan-workflow-migrationmd-e4595a7f-",
  "heads": {
    "herdr-bot": "334667dc54131ab135c7542475721a464af8cc76"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "06-plan-workflow-migration.md",
  "result": "integration-acceptance-passed",
  "repository": "herdr-bot",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-ppMKuj",
  "branch": "hyper/workflow-file-handoff/06-plan-workflow-migration-fe2c85-9497b79afe0a",
  "head": "334667dc54131ab135c7542475721a464af8cc76",
  "sync": {
    "result": "宿主集成提交已在原开发分支，未遗留冲突。",
    "unmerged_entries": 0,
    "working_tree": "clean",
    "changes_required": false,
    "new_commits": []
  },
  "verification": [
    {
      "command": "git status --short --branch && git log -3 --oneline",
      "observed": "当前分支正确，HEAD 为指定集成提交 334667d；工作区无改动。"
    },
    {
      "command": "node scripts/test-backend.mjs",
      "observed": "全量通过：build、全部测试 binaries、doctest 均 passed；0 binaries not dispatched；114.8s。包含本票 workflow_dispatch 的公开服务 Map 保存／读回验收。",
      "evidence": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-0kGQhv/summary.json"
    },
    {
      "command": "node scripts/check-workflow-docs.mjs",
      "observed": "通过：17 documents、159 localLinks、0 errors。"
    },
    {
      "command": "npm --prefix frontend test -- --maxWorkers=2",
      "observed": "网络护栏通过；31 test files、218 tests 全部通过。"
    },
    {
      "command": "npm --prefix frontend run test:browser:build -- --workers=1",
      "observed": "当前 worktree 前后端与 safe_shutdown_shell 构建及完整浏览器回归成功；345 passed，1 skipped，10.0m。跳过项是按项目规则未授权的真实符号链接手动验收，不计作通过。",
      "evidence": "artifact://273"
    },
    {
      "command": "git status --porcelain=v1 && git branch --show-current && git rev-parse HEAD && git ls-files -u",
      "observed": "分支与 HEAD 正确，无工作区改动，无未合并条目。"
    },
    {
      "command": "git status --porcelain=v1",
      "observed": "全部验收结束后输出为空，工作区保持干净。"
    }
  ],
  "scope_notes": [
    "本票只列 herdr-bot 一个仓库；未操作共享集成目录、未切换分支或新建 worktree、未 reset、未直接合入 dev、未调用 retro。",
    "依据票据明确分工，P27/P28 真实宿主完整 outline → generate_map、真实业务确认及跨仓链路由 09 独占验收；本次不冒称其已经通过。"
  ],
  "blockers": []
}
