# 01: scheduler 导航与 server 入口迁移

**What to build:** 在 herdr-bot 主界面提供 scheduler 入口，将 server 固定操作迁移到右上角按钮打开的同页独立内容页；用户可返回此前 tab，原有固定操作及安全反馈不丢失。本票交付导航闭环，真实文件浏览由 02 接续。

**Delivers:** P1, P2, P3；P4 的 scheduler 与 server 导航入口覆盖。

**Test owner for:** P4 的 scheduler 与 server 入口聚焦、激活、tab 导航及返回操作；09 负责 node 明细入口，双方覆盖通过后 P4 才整体完成。

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 顶部 tab 依次为 chat、bot、zone、scheduler；首次打开及整页刷新默认 chat，不保留旧 server tab 或重复入口。
- [ ] 右上角 server 按钮打开同一页面中的独立内容页，不打开新窗口、不以狭窄下拉菜单替代；从不同 tab 进入后可返回对应 tab。
- [ ] 原有固定操作保持可用；使用既有记录型安全替身验证成功、失败及忙碌反馈，不执行真实关机或危险脚本，不把脚本调用成功表述为后台程序已就绪。
- [ ] 在真实浏览器中使用键盘完成 scheduler 与 server 入口的聚焦、激活、导航及返回；不能仅调用内部处理函数作为证据。
- [ ] 通过合同 S1 验证上述行为；按项目测试指引复用现有安全服务与 npm 浏览器入口，源代码变更后构建真实产物。
- [ ] 更新受本票影响的导航及 server 使用说明；不提前宣称 scheduler 文件浏览已经交付。

## Contract coverage

依据已批准 contract 的 P1–P4 与 S1；本票仅认领上面列出的 P4 导航覆盖项。P4 的其余覆盖与测试由 09 负责。
