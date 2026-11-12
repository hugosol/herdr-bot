# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-18-native-report-recoverymd-b7c1a3b1-d",
  "heads": {
    "my-agent-toolkit": "5d26e385b475f5c7ac7fa9e39904bd62c54a3d68"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "18-native-report-recovery.md",
  "status": "完成，可由宿主排队合入",
  "repository": "my-agent-toolkit",
  "branch": "hyper/chief-bot/18-native-report-recovery-c5a4f9-72787c9f5da7",
  "commits": [
    "5e71fa79cee5a4e602467e28ba66a3777e910128",
    "5d26e38"
  ],
  "resolution": "已完成遗留 merge，包含集成提交845d8be4d8daa6fa84985c97eda75226eeff7bfe。保留新增 chief 路由/创建授权指引，同时保留本票显式查看、当前身份校验和持久接收去重语义，删除过时GET查看说明。",
  "commands": [
    {
      "command": "bun run typecheck",
      "exitCode": 0,
      "result": "Authoring 93 files / 8 declarations / 0 diagnostics"
    },
    {
      "command": "bun run check:integrations --json",
      "exitCode": 0,
      "result": "complete:true；仅静态检查"
    },
    {
      "command": "cargo build --manifest-path .scratch/chief-bot/paired-backend-final/backend/Cargo.toml --locked --target-dir .scratch/chief-bot/paired-target --example safe_shutdown_shell --bin herdr-fixture",
      "exitCode": 0
    },
    {
      "command": "bun test tests/workflow-chat/chief-report-recovery.test.ts",
      "exitCode": 0,
      "result": "8 passed / 0 failed"
    },
    {
      "command": "bun test tests/harness-dock",
      "exitCode": 0,
      "result": "237 passed / 0 failed，58 files"
    },
    {
      "command": "bun run test",
      "exitCode": 0,
      "result": "1922 passed / 0 failed，243 files"
    },
    {
      "command": "Get-CimInstance Win32_Process（筛选本票路径的原生/后端进程）",
      "exitCode": 0,
      "result": "无匹配进程"
    },
    {
      "command": "git status --porcelain=v1",
      "exitCode": 0,
      "result": "空输出，工作区干净"
    }
  ],
  "evidence": ".scratch/chief-bot/evidence/18-native-report-recovery/integration-sync/verification.json；同目录保留common-recovery.log、dock.log、full.log和native证据。",
  "resources": "联调源码与构建均来自本票worktree；结束后将配对后端/构建完整移回C:/Users/hugos/AppData/Local/Temp/hyper-ticket-RGYC82-preserved-18-recovery。独占测试窗口已正式交接12续跑会话。",
  "limits": "受控原生协议与配对真实持久后端回归通过；未将其冒充19负责的真实模型恢复/无自动行为或20负责的跨宿主失权证据。",
  "blockers": []
}
