# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "07-chief-structured-deliverymd-e78a6a03-4a07-4ed",
  "heads": {
    "bot": "44a41508a40f9a55cc8ea5d1e3a35a4ca2e61c35"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票 07 已实现并提交：`44a41508a40f9a55cc8ea5d1e3a35a4ca2e61c35`。未合入集成分支，交由独立集成代理接手。

- **P22–P26、P29、P34**：Task／Map 委派在预览、确认、实际提交和持久记录中使用同一份 `outputRule`。Task 报告校验文件；Map 正式报告只取已完成 Run 的公开成果子集，不接受地址覆盖。无文件的中途报告和无文件要求的成功报告不受 finish 门禁；文字说明与 `outputs` 独立保存。
- **P27–P28**：分别验证真实保存失败和外部通知失败。TUI 分别观察 **46,019 ms**、**46,015 ms**，App／OMP 与 App／Pi 的通知失败也各观察超过 46 秒，未见重投或重新派发。原 chief 不在线时，接替者能按权限读回陌生报告并读取实际文件；人工重发仍沿用原报告与接收去重。
- **P57**：隔离清理后，原 chief Chat 可恢复并重新登记；旧委派、旧报告职责及旧 Run 管理和 finish 权限均被拒绝。保留的 Chat 历史、实际材料和归档 Run 记录通过字节对照。

最终 `chief_delegation` **32/32**、`chief_authority` **2/2**、定向 `workflow_start` 调用者 **3/3** 通过，前端构建成功。保留了预览、Task 报告、Map 子集三项红灯，以及 P57 初次夹具编译与 Workspace 配置失败；修正后通过，未改产品超时。最终日志中所报告的进程清理均为 `cleanup=[]`。[原始日志](local://ticket-07-chief-verification.log)；P59 对照入口为[交付与现场交接](docs/bot-map-contract-refactor-delivery.md#本轮产品自动证据)。

未执行 11–14 的跨仓原生确认、真实安装或现场切换；本结果不代表整体契约验收通过。
