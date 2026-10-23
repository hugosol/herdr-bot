# 目录登记概念由 workspace 更名为 zone

- 状态：accepted
- 日期：2026-10-21

本项目把「用户登记的、位于服务所在电脑上的目录」这一概念统一更名为 **zone**。更名覆盖领域词汇、前后端标识符与文件名、HTTP 路由（`/api/zones/*`）、查询参数（`zone`）、JSON 字段（`zones`/`zone`）、错误码（`zone-*`）、配置段（`[zones]`）、默认登记文件（`zones.json`，内容 key 为 `zones`）与界面文案。这是一次硬切换：不保留 `workspace` 路由、参数、字段或配置段别名，现有 `workspaces.json` 由一次性迁移改为 `zones.json`。

更名的直接原因是后续要引入 herdr 相关模块，而 herdr 自己也用 workspace 表示它自己的概念；同一个项目里让两个不同概念共用 workspace 会造成持续歧义，因此本项目这一侧改用 zone，把 workspace 留给 herdr。另一个原因是「工作区」在中文里也是 Git working tree 的常用译名，旧名让「登记的目录」与 Git 的工作树在文档中难以区分；本次同时把 Git working tree 统一称为「工作树」。

## 影响

- 旧 HTML 阅读器书签、旧 API 调用、旧 `[workspaces]` 配置与旧 `workspaces.json` 都不再兼容；配置写错会启动失败，登记文件需一次性迁移。
- ADR 0007/0008 保留原文，以免改写决策历史；它们在顶部注明术语已更新，并由本篇解释旧词。README 的重复历史变更记录已在 2026-10-23 文档精简时删除。
- `workspace` 一词在本项目文档中保留给未来的 herdr 概念，不再作为 zone 的同义词使用。

## 考虑过的替代方案

- **保留 workspace 并长期提供别名**：会产生两套路由、字段与配置的维护面，且无法消除与 herdr 的重名，因此拒绝。
- **只改界面文案，代码与契约继续叫 workspace**：名实不符，也会把重名问题留给 herdr，因此拒绝。
- **要求 herdr 改用别的词**：herdr 是外部模块，其概念命名不由本项目决定，因此本项目这一侧先让出 workspace。
