# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "08-paired-history-recovery-acceptancemd-ec81e25b",
  "heads": {
    "herdr-bot": "a7eb1cff8f17546c41f6c4aa11a3de50c720c38f",
    "toolkit": "4eff99ebb38b395965c99bbbfb1401357fa6b790"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

已恢复 Toolkit 工作树的跟踪基线，并将生成的 Pi 验收记录原样归档。两仓工作树均干净。保留原有专项提交，新增修复 `a7eb1cff8f`，未合入集成分支。

- **已验证：** OMP、Pi 双向跨端恢复四格通过，分别核对了旧进程退出、新进程的原生 ID／文件、实际 cwd 和宿主焦点。同端原 ID、跨端无匹配、人为同值、最新 Suite、无 trigger 重放、四类环境降级和硬失败均有独立证据。OMP 的 Git 主工作树回退实跑确认原容器 ID、历史记录和原生文件均未改变。
- **最终修复验证：** 后端定向 18 项、浏览器定向 9 项通过。收窄修复前，完整后端 76 个目标及 doctest 通过；前端 274 项通过。完整浏览器未全绿：8 项历史回归在最终定向运行中通过，另有 1 项 Chrome 退出未确认。
- **仍未验收：** 7 个原生格因 Pi 的缺失 cwd 原生选择框、Herdr 对同路径 checkout 去重，以及 Windows 目录占用或退出确认缺失而未通过。Toolkit typecheck、integration check、17 项原生 dock 通过；framework 767 通过／1 失败，全量 1944 通过／16 失败，另有 2 个错误。未把受控结果或部分运行计为整票通过。

原始日志与逐格证据保留于 `%TEMP%\hb08-evidence-8MY9K0`，交由独立集成代理接手。
