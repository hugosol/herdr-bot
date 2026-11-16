# 08: 在既有 Bot 和 scheduler 页面维护及预检脚本目标

**What to build:** 用户在现有页面选择、保存和重载固定 Rust 执行目标，并在真实预检中得到可用或明确拒绝结果，无需虚假 Task 或 node.input。

**Delivers:** P30, P32（定义页面分区）

**Blocked by:** 02 — 通过真实 Map 异步创建空白 Bot 并交接报告。

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] Bot 维护入口和 scheduler 关联维护支持固定 Rust 目标，选择与公开 JSON 表示一致；切换目标保持 Task/Map/脚本互斥，不隐式复制或伪造 Task/manager 配置。
- [ ] 实际浏览器操作创建/编辑、保存、离开后重载，公开定义读回与页面一致，三指南可为空字符串；拒绝无效/未知能力或冲突目标时保留原定义和可修正草稿。
- [ ] scheduler 脚本节点不强迫有意义 node.input 或普通 Task/manager Task；真实预检对有效目标和材料条件给出合法结果，对未知能力/互斥冲突/必需文件不可用给出明确拒绝，不创建 Run/Chat。
- [ ] 脚本固定自动提交报告，定义页面不提供可造成 manual 等待或 Chat 关闭的配置；原 Task/Map 的报告/关闭设置保持原有功能。后端仍负责拒绝非法定义，不仅隐藏页面控件。
- [ ] 正确显示能力标识、空指南和脚本适用说明，相关 Task prompt 区不误报脚本“未绑定 Task”。不新增独立脚本执行器页面或 chief 直接执行入口。
- [ ] 更新现有页面/定义使用文档，保留实际页面截图及对应 HTTP 读回，不把组件测试或静态 HTML 当成实际表面验证。

## Coverage partition and ownership

本票唯一拥有 P30 全部实际浏览器场景及 P32 的定义页面约束。02 拥有后端自动报告/无关闭调用；09 拥有运行页面。11 拥有 P23“迁移后的专用 Bot 及空指南在实际页面读回”，本票使用独占自建定义，不重复认领迁移专属场景。

## Verification

从本 worktree 构建页面和独占后端，使用独立绝对 CARGO_TARGET_DIR。通过 npm 入口运行 `npm --prefix frontend test` 与 `npm --prefix frontend run test:browser:build`，浏览器连接真实 HTTP/公开预检，保留真实截图、定义读回、宿主无脚本启动调用证据。若更改后端定义逻辑，同时完成 `node scripts/test-backend.mjs`。遵守网络护栏、独占 HOME/APPDATA/XDG、动态回环端口和进程退出确认。
