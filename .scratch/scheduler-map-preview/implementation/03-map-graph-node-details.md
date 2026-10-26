# 03: 合法 Map 的依赖图与 node 详情

**What to build:** 用户选择真实的合法 Map 后，可以看到完整依赖图，选择 node 核对本次工作的参数和前置依赖，而不是看到模板默认值或运行状态。

**Delivers:** P10, P11, P13, P17。

**Blocked by:** 02 — 浏览服务级 Map 目录。

**Status:** closed

## Acceptance criteria

- [x] 读取受支持格式的真实 Map；全部 node 都以标题与 Map 内 ID 显示，包括独立节点，不能只显示有连接的节点。
- [x] 边仅来自 blocked_by；B 依赖 A 时显示 A → B，不增加执行路由、材料传递或隐含边。
- [x] 使用事先固定的 A、B、C、D 节点及仅 A → C、B → C 的预期，通过真实图形核对箭头方向、连接端点、并行与汇合，并确认 D 无连接；预期不从实现结果反推。
- [x] 依次选择引用同一模板但参数与依赖不同的 node，逐项显示当前 node 的参数与前置依赖，不残留其他 node 内容，不补入模板默认值。
- [x] parameters 接受命名参数对象中的直接 JSON 值，包括结构化值、未在模板 inputs 声明的键及不存在的 zone、Bot 名称；这些内容本身不能导致资源有效性或参数契约错误。
- [x] 业务文本按文本安全渲染，结构化参数可读地格式化；不执行文本、不按参数读取路径、不启动 Chat、不修改 Map 或模板。08 负责跨入口安全验收，本票实现不得先引入不安全临时方案。
- [x] 通过 S1 的真实页面和真实文件验证节点、连线与选择行为；基本图形无需手工安排节点位置，完整布局与视图交互验收由 09 负责。
- [x] 使用隔离服务和项目既有浏览器入口验证，不使用硬编码 UI 示例作为真实文件读取证据；同步更新受影响的预览与参数展示说明。

## Contract coverage

本票认领合法输入下的 P10、P11、P13、P17。05 负责无效 Map 的结构错误、依赖错误、旧展示清理及恢复；04 负责模板明细。

## Comments

- S1 验收位于 `frontend/browser-tests/schedulerGraph.spec.ts`，真实隔离进程、真实 Map／模板文件，未替换业务 HTTP 响应。
- P10／P11：固定 A、B、C、D，逐一核对标题与 ID，仅 A → C、B → C；检查实际 SVG 笔画命中、箭头标记与两端节点矩形，截图人工核对并行、汇合、独立 D。先红于缺少文件选择按钮，后绿。
- P13／P17：同模板 A → B → C → A 逐项核对，包含空参数、结构化值、未声明键、不存在的 zone／Bot；不补模板内容、不残留前一 node。先红于缺少 node 详情，后绿。标记样式文本保持文字，路径参数不泄露受控文件，Herdr 调用／Tab 记录及 Map／模板字节均未改变。
- `npm --prefix frontend run test:browser:build -- schedulerGraph.spec.ts --workers=1`：2 passed；截图在 `frontend/test-results/schedulerGraph-*/`。
- `npm --prefix frontend test`：27 文件、205 tests passed。`node scripts/test-backend.mjs`：build、全部 binaries、doctest passed，日志目录 `C:\Users\hugos\AppData\Local\Temp\herdr-backend-tests-FeHSqG`。
- `npm --prefix frontend run test:browser:build -- --workers=4`：164 passed、5 failed；本票两例及 scheduler 既有八例全部通过。失败在未修改的 Chat 恢复验收：`chatResume.spec.ts:29` 预期 `identity-verification-failed` 实得 `invalid-cwd`；`chatResumeIntegrity.spec.ts:257` 四种 agent／mode 预期 `failed` 实得 `unconfirmed`。未跨票修补或重跑以掩盖失败；全量浏览器不宣称通过。
- README 已同步真实文件读取、格式、参数展示与安全边界。本票自动验收完成；完整视图交互与窄屏由 09 认领，跨入口安全由 08 认领，P33 主观体验仍待用户。原实现交接时保留 `in-progress`，待独立集成合入后按规则关闭。
- 独立集成验收：当前开发分支已包含首次合入提交 `d1023d977664c0ee6f15021f88272ee7b6f3588d`，接管时工作区干净且无冲突；不再次直接操作集成分支。
- 全局浏览器故障定位：既有浏览器用例仍把不可核实的 inventory candidate 当作全局 lookup 失败，而生产实现和后端回归已经明确跳过坏候选。更新 `chatResume.spec.ts` 对空匹配后 cwd 检查的预期；`chatResumeIntegrity.spec.ts` 改用身份匹配但导航信息非法的不可跳过故障，保留 lookup 错误、无重复启动、目标保留等断言，不回退生产匹配策略。清除本包 Cargo 缓存并重建未消除这五个旧预期失败，缓存假设被排除；修正测试输入与预期后全量通过。
- 本轮全局验收：`npm --prefix frontend test`：27 文件、205 tests passed；`npm --prefix frontend run test:browser:build`：169 passed（16 workers）；`node scripts/test-backend.mjs`：build、全部 binaries、doctest passed，0 binaries not dispatched，最终日志 `C:\Users\hugos\AppData\Local\Temp\herdr-backend-tests-G4wUC1\summary.json`。本票已实现、测试并首次合入，状态关闭；本轮验收修复提交至开发分支，交宿主排队合并。
- 本轮未执行复杂脚本、实机 smoke 或跨票据整体验收；它们仍交最后独立门禁，P33 未代用户确认。
