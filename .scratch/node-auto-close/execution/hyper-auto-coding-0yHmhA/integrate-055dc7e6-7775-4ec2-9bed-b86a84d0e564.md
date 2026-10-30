# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-05-dispatch-retry-resume-and-restartmd",
  "head": "95238e4ede03b0709d3c543a369dad6f9e17060d",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "status": "synchronized_global_tests_passed",
  "ticket": "05-dispatch-retry-resume-and-restart.md",
  "branch": "hyper/node-auto-close/05-dispatch-retry-resume-and-res-0cf2bd-6987a96c7921",
  "head": "95238e4ede03b0709d3c543a369dad6f9e17060d",
  "synchronization": "当前 HEAD 即宿主提供的集成提交，包含本票原提交 8c9031b；merge-base 祖先检查成功，无冲突。",
  "commands": [
    {
      "command": "git status --short --branch && git log -4 --oneline && git merge-base --is-ancestor 95238e4ede03b0709d3c543a369dad6f9e17060d HEAD",
      "outcome": "成功；分支正确，HEAD 为 95238e4，工作区干净。"
    },
    {
      "command": "node scripts/test-backend.mjs && npm --prefix frontend test && npm --prefix frontend run test:browser:build",
      "outcome": "整条命令成功完成。后端全局回归通过（179.2 秒，build、全部测试二进制与 doctest 通过，0 个未派发）；前端 31 个文件、218 个测试全部通过；TypeScript/Vite 与后端交付构建通过；完整浏览器 339 passed、1 skipped（默认 manual-only 符号链接检查），耗时 2.5 分钟。网络护栏通过。"
    },
    {
      "command": "git status --short --branch",
      "outcome": "测试后工作区仍干净，无未提交或未跟踪文件。"
    }
  ],
  "evidence": {
    "full_output": "artifact://146",
    "backend_summary": "C:\\Users\\hugos\\AppData\\Local\\Temp\\herdr-backend-tests-nVvtRQ\\summary.json"
  },
  "changes": "无需修复，无新增提交。未切换分支、未操作集成工作区、未直接合入 dev、未调用 retro。",
  "remaining_scope": "复杂脚本、实机 smoke 及跨票据整体验收按指令留给最后独立门禁；本轮未执行。",
  "blockers": []
}
