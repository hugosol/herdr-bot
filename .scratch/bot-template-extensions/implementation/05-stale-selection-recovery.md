# 05: 让旧页面和失效模板可辨认、可拒绝、可修正

**What to build:** 元数据变化后，旧页面提交不能沿用旧选项；已保存的失效选择仍可识别并通过编辑修正，不静默丢弃或默认化。失效仅影响相关模板，运行中元数据错误明确失败，修复文件后后续操作无需重启即可恢复。

**Delivers:** P27、P28、P29、P30、P32

**Blocked by:** 04 — 按模板扩展集合检查依赖并创建 Bot。

**Status:** in-progress

## Acceptance criteria

- [x] 页面打开后移除或改变适用选项，再提交旧页面保存及创建，均按当前元数据拒绝；直接公开请求同样不能依旧选项通过。
- [x] 分别移除已保存模型、适用 thinking 级别或扩展元数据后，打开模板仍能辨认原选择并明确不可用，不静默删除、替换、重置为默认或隐藏失效信息。
- [x] 未知已保存模型标识不导致整个持久文件解析失败；同一文件中的失效模板仍可辨认，其他有效模板仍可读取、编辑并创建。
- [x] 用户能显式移除失效扩展并修正模型/thinking 等选择，修正后保存成功且独立公开读回准确；仅查看或步骤导航不构成修正或保存。
- [x] 修正前，页面禁用或拒绝保存和创建并说明原因，直接公开请求也拒绝；复用 04 的无外部副作用观察，确认没有 Tab、agent 或输入提交，不重复认领其通用故障测试。
- [x] 在同一真实程序实例运行中损坏或移走元数据，相关读取、保存和创建明确失败，页面显示配置位置与原因；不显示保存成功，也不使用旧缓存继续操作。
- [x] 写回另一份有效元数据后不重启，后续读取、保存和创建恢复，并使用独立预写的新选项及映射，而非故障前缓存。
- [x] 记录完整真实浏览器修正操作和公开请求结果，更新失效选择及配置修复说明；不新增元数据编辑界面、后台推送或自动修正。

## Verification and scope

依据：[批准契约](../contract.md)、[规格](../spec.md)。本票完整拥有 P27–P30/P32 的自动检查。使用真实持久文件、运行中外部修改、实际页面和直接公开请求，不以私有解析器或全局 mock 证明局部失效。

04 拥有 P35 的各类前置故障无副作用测试；本票的 P29 场景可复用其独立外部观察机制，但拥有失效选择修正前后的消费者行为测试。所有预期来自预写有效/失效样例，不能用当前实现生成期望。

## Comments

### 2026-10-22 — 实现与本票验收完成，待独立集成

批准 seam：S1 真实浏览器；S2 公开 HTTP 与独立 Herdr 调用/Tab 记录；S3 隔离真实持久文件、运行中外部修改与真实进程。未使用私有解析器或全局 mock。

- P27：`frontend/browser-tests/staleSelectionRecovery.spec.ts` 四个“旧页面…”用例，预先打开真实编辑和创建页面后，分别移除模型、改变模型 Agent 适用关系、移除 thinking 与扩展；页面保存/创建和直接 POST/PUT/创建均拒绝，原模板不变。
- P28/P29：三个失效选择修正用例，保留原模型/级别的不可用选项、保留可显式取消的失效扩展；步骤往返公开读回不变，明确保存后独立读回修正值并成功创建。移动视口完整走扩展修正及创建。修正前观察 Herdr 调用/Tab 均为空（包含无 agent_start/agent_prompt），复用 04 的外部记录，不新增通用 P35 故障矩阵。
- P30：外部预写同一 JSON 中未知模型模板和有效邻居，公开列表与单项读取成功，失效模板仍可辨认，邻居通过页面编辑、独立读回并成功创建，失效记录保持原样。
- P32：两个“运行中…”用例，分别损坏和移走真实元数据；列表/单项读取、保存、创建及页面都明确报配置路径与原因。原持久文件不变、无外部创建。写回另一份具有新模型、级别和固定扩展入口的有效目录后，不重启，读取、页面/公开保存及页面/公开创建恢复，外部启动记录使用字面预期的新映射。
- 文档：`docs/bot-template-agent-inputs.md` 已补充显式修正、局部失效与配置修复说明。浏览器完整操作/网络记录位于每个 `frontend/test-results/staleSelectionRecovery-*/trace.zip`；JSON 附件 `public-requests-and-external-observation` 及 PNG 保留请求结果、零副作用与修复前后画面。

TDD 实测 red：移除模型/级别时浏览器实际错误显示 default；失效扩展警告缺失；运行中单项公开读取错误返回 200。修复后对应行为全部 green。移除扩展后节点消失，Playwright `uncheck` 等待已脱离节点导致超时，改用真实点击并观察移除与持久结果，未降低预期。

最终实际验证：

- `cargo build --bins`：通过。
- `cargo test --test bot_templates --test dynamic_catalog --test bot_creation --test runtime_data`：40 通过。
- `npm test -- tests/botTemplates.test.tsx tests/botCreation.test.tsx`：13 通过。
- `npm run build`：通过（TypeScript 与 Vite）。
- `npx playwright test browser-tests/staleSelectionRecovery.spec.ts browser-tests/steppedTemplates.spec.ts browser-tests/dynamicCatalog.spec.ts browser-tests/templateExtensionLaunch.spec.ts`：37 通过，其中本票 10 个场景。

本票自动验收无未完成项；真实 OMP/Pi 加载与真实手机触摸仍是其他票据负责的 H1/H2，受控参数记录不冒充其通过。依 triage 规则，`closed` 须已合入，因此交出 worktree 时保留 `in-progress`，由独立集成者合入后关闭。
