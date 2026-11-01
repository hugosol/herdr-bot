# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-01-map-named-inputsmd-c4636aeb-9bc9-4f",
  "heads": {
    "herdr-bot": "63d7007d923d7cee102ee0620a7ef53a57f05bb1"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "01-map-named-inputs.md",
  "result": "resolved-committed-and-verified",
  "repository": "herdr-bot",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-xyfvcs",
  "branch": "hyper/workflow-file-handoff/01-map-named-inputs-3496b7-9497b79afe0a",
  "head": "63d7007d923d7cee102ee0620a7ef53a57f05bb1",
  "commits": [
    {
      "commit": "7c0b090",
      "purpose": "完成宿主遗留 merge；workflow_dispatch.rs 同时保留 named_inputs、report_outputs、report_restart 三个独立测试模块。"
    },
    {
      "commit": "63d7007d923d7cee102ee0620a7ef53a57f05bb1",
      "purpose": "按 Main 配套联调要求，在本票原分支 merge 已验收的 04 提交 6d8c6eb，形成 01+02+04 稳定源码；自动合并无冲突。"
    }
  ],
  "verification": [
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch --test workflow_start -- --test-threads 2",
      "outcome": "110 passed，2 suites；在首次冲突解决后执行。"
    },
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch named_inputs:: -- --test-threads 1 --nocapture",
      "outcome": "6 passed；公开 HTTP 保存、读取及显式派发冒烟，包含实际受控宿主初始化、无隐式启动、背景/业务 input/绝对地址与 Run 隔离位置证据。",
      "evidence": "artifact://101"
    },
    {
      "command": "npm --prefix frontend test -- --maxWorkers=2",
      "outcome": "31 test files、218 tests passed；网络护栏通过。在 01+02 状态执行；后续 merge04 未修改前端产品或组件测试源码。"
    },
    {
      "command": "npm --prefix frontend run build",
      "outcome": "TypeScript、Vite 构建通过；最终 HEAD 的完整浏览器入口也再次执行并通过该构建。"
    },
    {
      "command": "node scripts/check-workflow-docs.mjs",
      "outcome": "最终 HEAD：17 documents、158 local links、0 errors。"
    },
    {
      "command": "node scripts/test-backend.mjs --binary-jobs 1 --test-threads 1",
      "outcome": "最终 HEAD 全量后端通过：build、全部测试二进制、doctest 均通过，0 未派发，655.7 秒。",
      "evidence": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-np3H82/summary.json"
    },
    {
      "command": "npm --prefix frontend run test:browser:build -- --workers=2",
      "outcome": "最终 HEAD：构建与网络护栏通过，完整浏览器 345 passed、1 manual-only skipped，5.2 分钟。",
      "evidence": "artifact://180"
    },
    {
      "command": "git status --short --branch",
      "outcome": "最终仅显示原开发分支名称，无修改、未跟踪文件或冲突。"
    }
  ],
  "earlier_failure": {
    "command": "node scripts/test-backend.mjs --binary-jobs 2 --test-threads 2",
    "error": "zone_sync::configured_timeout_stops_the_running_command_and_keeps_effective_pull_state：恢复 sync 返回 409 git-write-in-progress，预期 200；doctest 未执行。",
    "evidence": "C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-exGCj1/summary.json",
    "resolution": "按项目测试指引降为 binary-jobs1/test-threads1；01+02 及最终 01+02+04 状态的完整后端分别通过，未修改无关 Git 终止逻辑。首次串行通过证据 C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-lvwGLz/summary.json。"
  },
  "cross_repository_coordination": {
    "scope": "票01仅列 herdr-bot；真实 Chat 覆盖按票据归属07，不冒充为本票已验证。另按 Main 要求为票05提供本票原worktree冻结源码和独立构建窗口。",
    "reported_by_peer": "05 开发代理报告基于63d7007的真实后端dock22/22、workflow+角色分区30/30；05集成代理随后报告dock serial173/173、canonical global1788/1788、workflow24/24及typecheck全部通过。这些是同伴报告，不是本代理执行结果。",
    "resource_release": "05开发与集成代理均明确已释放本票worktree；全部构建/服务结束，未修改本票源码。独立产物 backend/target/toolkit05-integration 为ignored。"
  },
  "constraints": "未操作集成工作区、未直接合入dev、未切换分支、未新建worktree、未reset、未调用retro。原工作区干净，可由宿主排队合入。",
  "blockers": []
}
