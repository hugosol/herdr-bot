# 01: scheduler 导航与 server 入口迁移

**What to build:** 在 herdr-bot 主界面提供 scheduler 入口，将 server 固定操作迁移到右上角按钮打开的同页独立内容页；用户可返回此前 tab，原有固定操作及安全反馈不丢失。本票交付导航闭环，真实文件浏览由 02 接续。

**Delivers:** P1, P2, P3；P4 的 scheduler 与 server 导航入口覆盖。

**Test owner for:** P4 的 scheduler 与 server 入口聚焦、激活、tab 导航及返回操作；09 负责 node 明细入口，双方覆盖通过后 P4 才整体完成。

**Blocked by:** None (can start immediately).

**Status:** in-progress

## Acceptance criteria

- [x] 顶部 tab 依次为 chat、bot、zone、scheduler；首次打开及整页刷新默认 chat，不保留旧 server tab 或重复入口。
- [x] 右上角 server 按钮打开同一页面中的独立内容页，不打开新窗口、不以狭窄下拉菜单替代；从不同 tab 进入后可返回对应 tab。
- [x] 原有固定操作保持可用；使用既有记录型安全替身验证成功、失败及忙碌反馈，不执行真实关机或危险脚本，不把脚本调用成功表述为后台程序已就绪。
- [x] 在真实浏览器中使用键盘完成 scheduler 与 server 入口的聚焦、激活、导航及返回；不能仅调用内部处理函数作为证据。
- [x] 通过合同 S1 验证上述行为；按项目测试指引复用现有安全服务与 npm 浏览器入口，源代码变更后构建真实产物。
- [x] 更新受本票影响的导航及 server 使用说明；不提前宣称 scheduler 文件浏览已经交付。

## Contract coverage

依据已批准 contract 的 P1–P4 与 S1；本票仅认领上面列出的 P4 导航覆盖项。P4 的其余覆盖与测试由 09 负责。

## Comments

- 2026-10-26：实现与本票验收完成，等待独立集成；按 triage 规则，合入前保留 `in-progress`，不把 agent 执行结束当作 `closed`。
- S1/P1：`frontend/browser-tests/schedulerNavigation.spec.ts` 的 `P1` 用例核对四个字面标签、首次打开与整页刷新默认 chat。首次 red 确认旧第四 tab 为 server；修改后 green。
- S1/P2、所认领 P4：同文件 `P2/P4` 用例使用真实 Chrome 键盘 Tab、Shift+Tab、Enter、Space、Home、ArrowRight；覆盖四个来源 tab、同页无新窗口、独立内容页及返回后的焦点恢复。首次 red 观察到 server 按钮未居右；修改后 green。
- S1/P3：同文件 `P3` 用例使用既有 success/failure/long-running 记录型脚本及安全关机壳，核对成功退出码 0、失败退出码 3、执行中禁用、另一页面的真实忙碌拒绝及调用记录；未执行真实关机。此行为原已实现，迁移后验收直接通过。
- `npm --prefix frontend test`：27 文件、205 用例全部通过。
- `npm --prefix frontend run test:browser:build -- --workers=4`：真实产物构建与网络护栏通过；157 passed / 5 failed。本票 3 个 S1 用例及迁移后的 Bot 浏览器用例通过。5 个失败为 `chatResume.spec.ts:29`（期望 identity-verification-failed，实际 invalid-cwd）与 `chatResumeIntegrity.spec.ts:257` 的 pi/omp × filtered/discovery 四场景（期望 failed，实际 unconfirmed）。未修改这些场景及后端，不宣称完整浏览器回归通过，也未证明它们是既有基线失败。
- README 与 GLOSSARY 已同步 server 新入口；scheduler 仅导航，真实文件浏览与 node 明细仍不属于本票交付。
