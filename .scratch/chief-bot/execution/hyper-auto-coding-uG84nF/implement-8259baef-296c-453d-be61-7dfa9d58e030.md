# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "11-chief-routing-policymd-eb55d882-4ba5-4dbc-907",
  "heads": {
    "my-agent-toolkit": "c0e40345f3e4626ea7689b3fa9c8961cf13abb86"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "11-chief-routing-policy",
  "commit": "c0e4034",
  "branch": "hyper/chief-bot/11-chief-routing-policy-a9423d-72787c9f5da7",
  "status": "实现已提交，worktree可交独立集成；canonical全仓回归仍有非chief失败，不宣称全绿。",
  "delivered": [
    "匹配/含糊/无匹配分流，未知输入和唯一候选不得偷补参数。",
    "单执行者/多步骤缺口分别选择既有完整Task/Map创建能力，不新增假工具。",
    "明确区分仅否决、创建Task、创建Map、放弃；缺原生批准不启动。",
    "创建授权不扩大到业务执行或后续阶段审批；结果呈现后等待，不自动检索或确认下一次执行。"
  ],
  "criterionEvidence": {
    "1": "chief-role原生指引消费测试；red-clarification→green-clarification。",
    "2": "同一公开消费者测试；red-creation-routing→green-creation-routing。",
    "3": "同一公开消费者测试；red-veto-routing→green-veto-routing；既有委派批准协议回归。",
    "4": "同一公开消费者测试；red-creation-authorization→green-creation-authorization；既有独立批准回归。"
  },
  "verification": [
    {
      "command": "bun run typecheck",
      "result": "exit 0"
    },
    {
      "command": "bun run check:integrations",
      "result": "exit 0"
    },
    {
      "command": "bun test tests/workflow-chat/chief-delegation.test.ts",
      "result": "5 pass / 0 fail"
    },
    {
      "command": "bun test tests/harness-dock",
      "result": "215 pass / 0 fail，含OMP/Pi新增指引消费；exit 0"
    },
    {
      "command": "bun run test",
      "result": "1891 pass / 1 fail / 1 error；exit 1"
    },
    {
      "command": "bun test tests/model-cost/foundation/session/session-recovery.test.ts",
      "result": "串行诊断13 pass / 0 fail；不能替代canonical失败"
    }
  ],
  "unresolved": [
    "canonical唯一失败：tests/model-cost/foundation/session/session-recovery.test.ts:794 unreadable-target恢复用例5000ms超时，随后savedDocument :246/:832发布错误。原始失败完整保留；未修改无关model-cost源码，已报告Main，由集成层处理。根因未确定。",
    "受控原生测试仅证明指引实际送达，不证明真实模型遵从。GPT-6-luna行为验收按既定归属由12/13/16承担，本票未重复认领或调用替代模型。"
  ],
  "evidence": ".scratch/chief-bot/evidence/11-routing-policy-20261112（commands.json、各次红绿/回归日志、原生历史、源码及依赖哈希、coverage.json、tested-source.json）",
  "handoff": "重型窗口已明确释放给integrate-07-accept-chief-entrymd-fac42a88-f3e7-；全部检查命令已await退出，临时门控runner已停止；未操作集成工作区。"
}
