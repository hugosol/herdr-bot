# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "18-native-report-recoverymd-cb5ba3bb-6d3a-4e78-8",
  "heads": {
    "my-agent-toolkit": "ab91188826645224533f7f8ee154f378e466b237"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "18-native-report-recovery",
  "status": "已完成，工作树干净，可交独立集成",
  "head": "ab91188826645224533f7f8ee154f378e466b237",
  "commits": [
    "c366745",
    "bf00523",
    "ab91188"
  ],
  "recovery": {
    "action": "完整移出四个配对后端/构建目录；备份生成的 pi-acceptance.json 后恢复其已提交版本。未丢弃既有实现、日志或产物。",
    "archive": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-RGYC82-preserved-18-recovery",
    "manifest": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-RGYC82-preserved-18-recovery/preservation.json",
    "handoffCheck": "git status --porcelain=v1 输出为空；HEAD 保持 ab91188。"
  },
  "verification": {
    "reuse": "本次无源码修改；已核对既有源码、锁文件、产物和日志 SHA-256，全部匹配，未重复运行已完成验收。",
    "priorActualResults": {
      "nativeFocused": "12 passed / 0 failed",
      "bun test tests/harness-dock": "237 passed / 0 failed",
      "bun run test": "1922 passed / 0 failed",
      "bun run typecheck": "exit 0",
      "bun run check:integrations --json": "exit 0"
    },
    "criteriaEvidence": ".scratch/chief-bot/evidence/18-native-report-recovery/verification.json 中 criteria 逐项对应 AC1–AC6：持久接收、重复抑制、原身份人工重发确认、无自动补投、显式读取及恢复/撤权语义。"
  },
  "integrationNote": "e3d6f20 是启动指引依赖补丁，对应上游16的9397037；已集成等价补丁时勿重复应用。",
  "limitations": [
    "受控原生协议验收不证明真实模型行为；真实模型恢复/无自动行为及跨宿主旧 chief 失权仍分别由19、20覆盖。",
    "原证据中的配对后端/产物路径已整体迁入上述归档，原内容保留。"
  ],
  "unresolvedOwnedScope": []
}
