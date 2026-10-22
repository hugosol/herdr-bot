# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-05-stale-selection-recoverymd-e466e1f6",
  "head": "34c934a26987c7f0cb4c8e5478bc40d7321cb943",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "result": "passed",
  "ticket": "05-stale-selection-recovery.md",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-V4TMFf",
  "branch": "hyper/bot-template-extensions/05-stale-selection-recovery-97738b-8713386ed6b5",
  "head": "34c934a26987c7f0cb4c8e5478bc40d7321cb943",
  "synchronization": "当前开发分支已位于宿主同步的集成提交 34c934a；实现提交 950830d 位于其历史中。无待解决冲突。",
  "commands": [
    {
      "command": "git status --short --branch && git log -3 --oneline && git show --stat --oneline HEAD",
      "cwd": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-V4TMFf",
      "outcome": "确认当前分支与同步提交，初始工作区干净。"
    },
    {
      "command": "npm test",
      "cwd": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-V4TMFf/frontend",
      "outcome": "通过：26 个测试文件，184 个测试。"
    },
    {
      "command": "cargo test",
      "cwd": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-V4TMFf/backend",
      "outcome": "通过：363 passed，3 ignored，无失败；包含 doc-tests。"
    },
    {
      "command": "git status --porcelain=v1",
      "cwd": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-V4TMFf",
      "outcome": "无输出，最终工作区干净。"
    }
  ],
  "changes": "本轮无测试失败、无冲突，无需修改或新增提交。",
  "scope": "未操作集成工作区、切换分支、新建 worktree、reset、直接合入集成分支或调用 retro。未运行复杂脚本、实机 smoke 或跨票据整体验收；这些仍由最后独立门禁负责。",
  "blockers": []
}
