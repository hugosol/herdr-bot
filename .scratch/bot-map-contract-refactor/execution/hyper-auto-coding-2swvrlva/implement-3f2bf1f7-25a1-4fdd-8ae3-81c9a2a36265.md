# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "02-bot-contract-and-consumersmd-5f75d211-bbdb-41",
  "heads": {
    "bot": "e30d7e2c2c65920fda71f371370427ed5eb3436d"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票据 02 已提交 `e30d7e2c2c`，未合入集成分支。

- **P3／P31**：四字段 Bot 贯通 HTTP、持久文件和真实页面编辑重载；非法输入、旧字段及过期 revision 拒绝后不改写。`schedulerBotEditing.spec.ts` **11/11**，另有修正后的相邻浏览器用例 **4/4**。
- **P29／P38／P39**：创建、修订、材料和真实 Map→Bun 执行链切换至新格式。覆盖完整读回、命名文件、并发保护，以及读回、材料或通知失败时保留实际效果。`workflow_dispatch` 首轮 **33/35**；两项在历史夹具对照时因未计入既有 `design-taste-frontend` Skill 条目而失败。仅修正该夹具预期后，独立复跑 `create_migration_` **2/2**，未重跑首轮其他用例。`python scripts/test-script-contracts.py` 通过。
- **P53**：`runtime_data` 定向 **2/2**，分别验证真实启动后离线 Bot／Map 不参与列表、读取或预检，以及既有布局标记阻止重导入。

未移动生产数据，也未验证未选旧定义、延期 tester 或人工业务效果。08／14 切换时须定位实际在线定义根及启动导入源，备份放在两者之外。
