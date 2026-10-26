# 共享 node_template 更名为 Bot，定义与逻辑不变

- 状态：accepted
- 日期：2026-10-27

将跨 Map 共享、由 node 引用的业务能力定义从 **node_template** 全量更名为 **Bot**，使名称与概念的实际定位及大众固有认知匹配。这是纯重命名，不改变能力内容、参数、依赖、校验规则或执行逻辑；Bot 不是运行中的 agent、Chat 或执行实例，也不是用于发起 Chat 的 Task。Bot 仍由 scheduler 管理，不新增独立入口。

## 与既有命名决策的关系

[ADR 0012](0012-rename-bot-preset-to-task.md) 将发起 Chat 的预设命名为 Task，该决策保持有效。本决策重新将 Bot 用于共享业务能力，而不恢复 [ADR 0011](0011-bot-chat-terminology.md) 中 Bot 表示发起 Chat 预设的旧含义。Map、node、Task、Chat 的含义与关系不变。

## 切换边界

界面、代码符号与文件名、API、配置、持久化字段与目录、相关工具、测试及当前文档统一更名。主要名称为 `Bot`、node 的 `bot_id`、`/api/scheduler/bots` 及其子接口、配置 `bot_directory`、默认目录 `.data/bots/`。不另设 bot_template 或 node_bot，也不保留旧名称的运行时路由、字段或配置别名；避免同时维护两套契约的代价是前后端与工具必须同步升级。项目名 herdr-bot、接入机制 connect-bot 等无关名称不变，历史 ADR 与历史票据保留原始事实。

## 存量数据

停止旧服务后显式执行一次性迁移，再启动新版。迁移转换相关配置名称、默认目录以及 Map node 的引用字段，保留 Bot ID、名称、配置值、Map 引用关系、参数与依赖。自定义目录不因更名移动，仍按服务进程 cwd 解析相对路径。ID 即使包含 template 也不重新生成，不对用户描述或嵌套业务参数进行字符串替换。新旧目录并存、字段冲突或目标冲突时明确报错，不静默覆盖、合并或清空数据；不以运行时兼容替代迁移。
