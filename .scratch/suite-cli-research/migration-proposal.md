# 旧 Task → Suite 已确认映射

状态：用户已确认映射；尚未执行迁移。稳定 Task ID 映射见 migration-map.json，当前共 22 个 Task。未修改 meta-data/tasks.json。

## 已确认 A：代码库探索（17 个）

- grill-with-docs
- teacher-bot
- wayfinder
- 能力创建者 · Bot＋Task
- 概要规划
- 概要方案转 Map
- Task 创建者
- 通用提示词优化者
- Bot 创建者
- Scheduler Map 修改者
- 空白 Bot 创建者
- Bot.when 编写者
- Bot.plan_guide 编写者
- Bot.node_guide 编写者
- 既有 Task.prompt 交互优化者
- 对话式架构重构
- 翻译科技类文章

理由：已有初始技能属于该 suite 的技能清单，或职责相近。此分组包含非代码任务，映射后会额外获得 suite 的 codebase-memory-mcp、codebase-tools、connect-bot；用户已接受此分组，不声称与旧资源集合完全等价。模型、思考级别、初始消息和初始 skill 保持不变。

## 已确认 B：代码实现（1 个，已补扩展定义）

- hyper-auto-coding，ID tpl-18e0e21b49001550-0。

当前可选扩展为 omp-codebase-tools、omp-connect-bot、omp-oh-my-wayfinder，初始消息是 /hyper-auto-coding。该命令在 oh-my-wayfinder/extensions/entry.ts 注册；用户已批准代码实现 suite 加入 oh-my-wayfinder，initial-suites.toml 已增加其 OMP 入口绑定，不会直接丢掉该命令。

## 已确认 C：Tibo-monitor 不选 suite（1 个）

- Tibo-monitor，Pi，ID tpl-18e0ceac0a89f9bc-0。

当前仅选 pi-web-access，初始任务按已有 URL 查询额度重置动态。用户明确批准本阶段不选 suite，web-access 整合留到下一阶段；不新增网页研究 suite，也不把 web-access 偷加进探索 suite。映射保留旧扩展选择作为迁移说明，不声称本阶段已实现等价网页访问能力。

## 已确认 D：不选 suite（3 个）

- C盘清理大师
- manager
- 通用文本替换者

目前均无可选扩展或初始 skill。保留原固定扩展及执行角色规则，manager 仍为 manager，不因不选 suite 失去角色扩展。

## 正式迁移执行约束

按稳定 Task ID 写映射，不按可变显示名匹配。正式迁移前重读数据并检查与确认时一致；不在用户编辑 Task 后套用过时清单。Task → suite 选择及实现类扩展补充已确认；用户另批准将发现的依赖 skills 加入探索 suite，优先 wayfinder 来源，其次 matt-pocock。运行验证和产品实现尚未完成，本文件记录映射批准，不表示已经执行迁移。
