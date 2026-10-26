# 04: 共享模板关联与按需明细

**What to build:** 用户查看 node 时，先看到具体工作，主动展开模板明细后才看到其引用的共享业务能力说明；共享模板从独立服务端目录真实读取，可跨 node 和 Map 复用。

**Delivers:** P14, P15, P16；P6, P7, P8 的共享模板目录覆盖。

**Test owner for:** P6 的共享模板目录独立配置与 cwd 解析；P7 的模板默认目录创建与已有文件保留；P8 的模板目录创建／读取失败与正常空态。02 负责对应 Map 目录覆盖，各承诺在两票覆盖通过后才整体完成。

**Blocked by:** 03 — 合法 Map 的依赖图与 node 详情。

**Status:** in-progress

## Acceptance criteria

- [x] Map 与共享模板目录可独立配置并实际读取不同内容；隔离进程的 cwd 与配置文件位置不同，证明相对模板目录按 cwd 解析。
- [x] 未配置时使用 spec 已确认的默认共享模板目录，缺失目录及父目录自动创建；预置模板文件内容保持不变，不自动安装示例。
- [x] 模板目录只扫描直接包含的 JSON 文件，不递归读取；目录创建／读取失败明确报告，真实空目录与目录故障有区别。使用实际文件系统反例，不用空响应模拟读取失败。
- [x] 模板明细默认收起；用户点击入口后展示当前 node 对应模板，切换 node 后不显示错误关联或上一 node 的说明。
- [x] 逐项展示模板标题、use_when、description、命名 inputs／outputs 的文本说明，以及 chat 实现类型；使用独立编写的预期文本核对全部字段。
- [x] 通过仅含 Chat 的 Rust 枚举表达实现类型，JSON 表示为 chat；展示不等于执行，不增加实际执行配置或启动能力。
- [x] 模板身份来自 id 而不是文件名；使用与 id 不同的文件名，验证同 Map 多个 node 及多个 Map 引用同一说明。模板文件重命名后重新读取仍能关联，可通过重新进入页面验证，不提前认领 07 的完整显式重载覆盖。
- [x] 所有文本安全展示，正常模板读取不修改定义、不执行代码、不按参数读取文件；08 负责完整安全验收，06 接续模板局部错误与重复身份处理。
- [x] 通过 S1 与主界面实际调用的 S2 验证真实文件关联，沿用隔离、安全及 npm 网络护栏约定；更新共享模板配置和明细使用说明。

## Contract coverage

P6、P7、P8 的共享模板目录覆盖及测试归本票；对应 Map 目录覆盖及测试归 02。模板目录级失败归本票，单个模板文件不可读等局部故障归 06 的 P20。

## Comments

- 实现与本票验收完成，待独立集成；依照状态规则，尚未合入前保留 `in-progress`。
- `frontend/browser-tests/schedulerTemplates.spec.ts` 的 5 个真实 S1／S2 场景覆盖：P6 的独立配置与 cwd 解析、P7 的默认／多级目录创建与原文件保留、P8 的真实文件阻挡／Windows ACL 读取失败与正常空态、P14／P15 的按需展开和全部独立预期字段、P16 的同 Map／跨 Map／文件重命名复用。文本安全、无 Herdr 调用与定义字节不变亦已断言。
- TDD：默认模板能力与明细入口先实际失败，再实现后通过；目录错误文案的实际失败保持预期不变并修正实现。既有正确目录／身份行为直接经新增场景验证，不制造失败。
- 最终 `npm --prefix frontend test`：27 文件、205 测试通过；`npm --prefix frontend run test:browser:build -- scheduler --workers=2`：TypeScript／Vite／Rust 构建通过，15 浏览器测试通过（含本票 5 个），npm 网络护栏通过。
- `node scripts/test-backend.mjs`：构建通过，58 个测试目标通过、1 个失败；`zone_sync::configured_timeout_stops_the_running_command_and_keeps_effective_pull_state` 在超时恢复资格断言收到 409、预期 200。未改动 Git sync、未重跑此失败；doctest 因入口失败未运行。日志：`C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-8Hglm2/summary.json`。已向主代理报告，不将完整后端回归声称为通过。
- 06 的模板局部故障／重复身份、07 的完整显式重载、08 的完整安全与样例交付仍归原票；本票不提前认领。
