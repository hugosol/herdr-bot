# 03: 通过真实 Map 原样合并保存三指南

**What to build:** Rust 保存能力直接消费共同 baseline 和三组最终 text/handoff，将同一 Bot 的三指南汇总一次合法保存，完整读回并交付 saved 与独立 handoff；冲突和部分生效均如实报告。

**Delivers:** P7, P8, P15, P24, P25, P26, P33

**Blocked by:** 02 — 通过真实 Map 异步创建空白 Bot 并交接报告。

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 实现编入后端的第二个固定 Rust 能力，经真实 Map 派发和现有自动报告通路执行，不用直接函数调用冒充闭环。
- [ ] 按 01 的严格创建分支格式消费 baseline、when_text/when_handoff、plan_guide_text/plan_guide_handoff、node_guide_text/node_guide_handoff。正文不从 handoff、node.input、聊天或来源索引推导，不执行自然语言语义判断。
- [ ] 用预先固定、三段互异且包含可辨换行/空白的字面正文，按对应字段逐字保存；不裁剪、重写、优化或添加包装。同一 Bot 三字段组成一次合法完整保存。
- [ ] 独立重读真实 Bot、revision 和完整目标，与 baseline 核对身份、唯一绑定、目标、修改范围和有效确认信息。缺项、格式错误、字段/对象不对应、冲突或歧义不产生对应误写或成功声明。
- [ ] 完整保存和读回保持全部非目标字段及关联目标配置不变；使用独立字面非目标值、真实并发修改、旧 revision、目标配置变化分别验证拒绝与完整对象前后对照，不靠实现生成期望值。
- [ ] 成功 outputs 分别绑定完整 saved 和必要 handoff；saved 与真实完整读回一致，handoff 不复制正文/完整配置。真实下游从显式命名 ID 消费，不沿隐式地址查找。
- [ ] 写入失败或完整读回失败均报告 canComplete=false，保留可得原错误/实际效果。仅 HTTP 成功、函数返回、候选生成或报告保存均不证明业务成功。
- [ ] 用真实故障制造配置已保存但结果材料生成失败；独立读回已保存配置和失败报告，保留部分效果，不自动回滚或刷新 baseline。解除故障后，由 manager 在另一个隔离真实 Run 中显式派发同一能力并提供原 baseline，仍按真实冲突拒绝，不能因正文相同认定成功。这证明业务重跑冲突，不依赖尚未交付的同节点 retry 门禁；同节点 fail/retry 及身份保留由 05 负责。
- [ ] 真实运行中受保护定义禁止保存，原对象不变，错误报告可读；不因内置 Rust 执行而绕过原保存/禁写检查。
- [ ] 更新现有材料与运行说明，记录成功条件、部分生效和显式重跑边界，不改造原通用文本替换业务。

## Coverage partition and ownership

P7/P8/P33 的合并保存用例唯一 owner 为本票；02 拥有创建用例。P24/P25/P26 全部自动化归本票。

P15 全部用例由本票编写、执行并维护，包括“已保存后材料失败”和“另一真实 Run 显式派发原 baseline 后冲突”，本票可独立验证完整业务承诺，不留下等待 05 的红测试。05 拥有同节点通用 fail/retry 门禁；11 复用本票用例并增加真实 H2 组合演示，不复制认领 P15 测试。

## Verification

沿 S1 真实 HTTP、文件和完整配置读回验证，使用独占临时数据、字面正文和真实文件故障。当前 worktree 独立绝对 CARGO_TARGET_DIR，构建本次静态页后运行 `node scripts/test-backend.mjs`。遵守进程退出确认、网络及文件安全约束；P2/P29 联合构建运行证据由 11 拥有。
