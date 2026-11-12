# 09: 在 workflow 展示真实脚本执行与待交接状态

**What to build:** 用户从现有 workflow 查看脚本当前及旧执行、成功/失败报告和交接结果，明确业务执行已经结束不等于节点已经 complete，且不存在脚本 Chat 恢复入口。

**Delivers:** P5, P31, P32（运行页面分区）

**Blocked by:** 05 — 交付脚本失败、显式重跑与旧执行隔离。

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 实际页面从 S1 真实执行事实展示脚本类型、固定能力、执行 ID、实际文件输入/冻结规则、报告和输出，不根据当前 Bot 配置重建历史事实，不显示虚假 Task/agent/session 身份。
- [ ] 用真实 Map 派发成功和失败场景，分别在业务在途、报告保存、显式 complete 前后观察页面。执行结束、报告 canComplete、node 状态和依赖资格分别表达，不以“报告成功”展示节点已完成。
- [ ] 失败报告呈现原错误、可得实际效果和仍待 manager 处理的节点状态；显式 fail/retry 后当前与旧 execution、报告及输出归属不混淆，旧 true 不显示为新执行完成资格。
- [ ] 当前脚本、旧脚本和脚本来源报告均无 Chat Resume/恢复会话入口；manager 和真实 Task/Map Chat 的现有入口仍可用。不把业务已结束解释为待启动 Chat 或 manual 确认等待。
- [ ] 自动报告/资源结束不显示 Chat 关闭动作。查看、轮询、刷新和页面导航只读取事实，不产生脚本派发、retry、complete 或外部 Chat 调用。
- [ ] 使用真实浏览器截图与同一时刻公开 S1 读回交叉核对，更新现有 workflow 使用说明；不以组件 mock 或自行拼接静态运行记录代替关键状态转换。

## Coverage partition and ownership

本票拥有 P31 全部页面自动化，以及 P5 的无虚假身份/恢复入口页面分区和 P32 的运行页面分区。02 拥有 P5/P32 后端事实，08 拥有 P32 定义页面，11 准备 P5 的真实宿主 H1。重跑门禁和报告归属的后端用例唯一归 05，不在此重复认领；本票验证其在真实页面上的表达。

## Verification

本 worktree 独立绝对 CARGO_TARGET_DIR，经 npm 入口运行 `npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。浏览器连接本次构建的真实独占后端，以公开动作产生场景；保留截图、实际配置/执行/报告读回和外部调用记录。后端变更同时完成 `node scripts/test-backend.mjs`。遵守测试网络护栏和所属进程终止/退出确认，不使用用户 Run 或 Chat。
