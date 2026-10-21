# 项目标识由 omp-bot 更名为 herdr-bot

- 状态：accepted
- 日期：2026-10-21

本项目把项目标识统一更名为 **herdr-bot**。这次改名覆盖仓库与本地目录名、Rust crate/二进制名、前端包名、默认配置文件、mDNS 主机名与服务实例名、环境变量、日志前缀与当前文档中的项目称谓。这是一次硬切换：不保留旧配置文件名、旧环境变量或旧 mDNS 主机名别名。

改名的原因是产品定位已经超出 `omp-bot` 这个名字：`herdr-bot` 仍提供既有的固定操作、zone 浏览与 agent session 视图，未来还会提供会话模板管理与按模板创建会话等上层能力；而 `herdr` 是另一个项目，负责管理 session，`herdr-bot` 不重复实现 `herdr` 已经实现的 session 管理。`connect-bot` 是 agent 侧的扩展与接入协议，不绑定 OMP；任何实现 connect-bot 的 agent 都可以把 agent session 上报到 `herdr-bot` 主界面。本次改名不改变现有协议、API、会话数据或行为。

## 影响

- 旧 `omp-bot.toml`、`OMP_BOT_*` 环境变量与 `omp-bot.local` 不再兼容；旧书签与旧防火墙规则需要一次性迁移。
- 历史 ADR 与 README 的交付验证/变更记录保留原文，并在顶部注明项目现名；术语以 [`GLOSSARY.md`](../../GLOSSARY.md) 为准。
- `docs/omp-*.md`、`.omp/`、`omp` 命令与 `omp agent` 等 OMP 平台词汇不属于本项目标识，不随本次改名改动。
- `my-agent-toolkit` 中连接真实面板的 interop 测试辅助文件需要独立后续更新；它的运行时端点走 loopback，不因面板改名而失效。
- GitHub 仓库原地改名以保留 issues、PR 与提交历史；本地目录改名放在最后执行。

## 命名边界

- `herdr`：外部项目，负责管理 session；`herdr-bot` 不重复实现其 session 管理。
- `herdr-bot`：本项目面板，承载固定操作、zone 浏览、agent session 视图，以及未来的会话模板管理与按模板创建会话。
- `connect-bot`：agent 侧的扩展与接入协议；它不是 OMP 专属，也不合并进 `herdr` 模块。
- `agent session`：一个实现 connect-bot 的 agent 的对话单元；与未来由 `herdr` 管理的 session 是两个不同来源。后者的具体词条等模块落地时再定义，本 ADR 不提前占用 `agent session` 一词。

## 考虑过的替代方案

- **保留旧名并提供长期别名**：会产生两套配置、环境变量与 mDNS 入口的维护面，也无法让名称与产品定位一致，因此拒绝。
- **新建 `herdr-bot` 仓库再迁移**：会丢失旧仓库的 issue/PR 关联，并增加一次不必要的迁移，因此拒绝；改为原地 rename。
- **全仓库全局替换历史文档**：会让历史验证记录与决策记录失真，因此拒绝；历史正文保留并加术语注记。
- **同时把 `connect-bot` 改名为 `herdr` 相关名称**：会把 agent 侧接入协议与外部 session 管理项目混为一谈，因此拒绝。
