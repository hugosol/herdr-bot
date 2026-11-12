# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "18-native-report-recoverymd-36925e09-96c4-489e-a",
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
  "status": "已实现、验收并提交，待独立集成",
  "commits": [
    "c366745",
    "bf00523",
    "ab91188"
  ],
  "prerequisite": {
    "local": "e3d6f20",
    "equivalentUpstream16": "9397037",
    "note": "若已集成16，不要重复应用此启动指引补丁。"
  },
  "delivered": [
    "OMP/Pi 原生输入先取得后端完整持久接收授权，重复或无效确认不触发模型。",
    "原发送方人工重发完整预览原 ID/内容，再次原生确认；取消仍保留原保存事实。",
    "当前 chief 显式查看原报告，真实身份拒绝及提交后不确定事实保持不变；无自动补投机制。"
  ],
  "verification": {
    "nativeFocused": "12 passed，0 failed；含真实配对后端持久 receipt",
    "dock": "237 passed，0 failed",
    "canonicalFull": "1922 passed，0 failed",
    "typecheck": "exit 0",
    "integrations": "exit 0"
  },
  "criteriaEvidence": "逐项 AC1–AC6、TDD 原始失败、命令、源码/锁文件/产物身份及日志索引见 .scratch/chief-bot/evidence/18-native-report-recovery/verification.json。",
  "pairedBackend": "a9c28664d1c0d63cc2c7a4cf5a51fd13f0dd5f92，只读快照与独占构建目录",
  "limitations": [
    "本票仅使用受控 provider；不声称真实模型恢复行为通过。19 的真实模型恢复/无自动行为及20的跨宿主失权覆盖保持独立。",
    "17的持久重启、恢复关联及维护窗口证据按依赖引用，未复制认领。"
  ],
  "unresolvedOwnedScope": [],
  "handoff": "全部运行已结束；独占窗口已交12，之后由其交13。"
}
