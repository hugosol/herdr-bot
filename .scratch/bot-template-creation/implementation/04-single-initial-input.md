# 04: 按原文一次提交 Skill、prompt 与 input

**What to build:** 用户可选择 Skill、填写或留空多行固定 prompt 与本次 input；Bot 就绪后系统按约定合成一轮初始化输入，只提交一次，保留字面含义并准确呈现 Herdr 的实际提交结果。

**Delivers:** P28, P29, P30, P31；完成 P22 的创建与投递覆盖及 P49 的正文覆盖，另两部分分别由 01、02 负责。

**Test owner for:** P22 的空值/多行 prompt 与 input 创建和实际投递；P49 的正文经生产包装层保真且无额外执行。

**Blocked by:** 02 — 从 zone 创建默认配置 Bot，贯通进展与结果。

**Status:** ready-for-agent

## Acceptance criteria

- [ ] /teach 和 /code-review 分别生成真正的 /skill:teach、/skill:code-review 调用，以独立字面前缀及完整输入验证，不调用同名扩展命令或提示词模板。
- [ ] 按“可选 Skill、固定 prompt、用户 input”顺序合成一次提交。有 Skill 且有正文时，命令后以普通空格连接正文，固定 prompt 和本次 input 分段表达；用事先写定的完整多行样例核对正文和顺序，不用实现合成期望。
- [ ] 外部记录证明只提交一次，不先单独开启 Skill 轮次；prompt 中 slash 文本不被递归解析为额外初始化步骤，prompt 不是 system-role 消息。
- [ ] 无 Skill 的非空多行样例只提交正文，不补 Skill 前缀；分别覆盖空 prompt、空 input、两者的多行内容，保留应有含义。
- [ ] Skill-only 从公开入口进入既定提交；全空同样从页面和公开入口进入实际提交，由受控 Herdr 拒绝并显示普通失败。不能前置组合校验、补造内容、跳过提交或假报成功。
- [ ] 输入的引号、换行和 Shell 特殊字符经实际生产交互方式后外部接收值保真；用独立外部观察确认包装层无额外执行，不以字符串构造单测代替生产路径。此保证不限制 agent 收到任务后的正常执行。
- [ ] 就绪前无提交，提交确认后才成功且不等模型完成任务；本票的拒绝场景保留对象和已知定位信息。完整阶段故障矩阵由 06 负责，不重复认领。
- [ ] 完成自动检查和适用运行验证。观察接收正文与提交次数，不能以模拟回显冒充真实 Herdr/agent 收到输入。

## Shared-promise coverage partition

| Promise | Coverage item | Ticket | Test owner |
|---|---|---|---|
| P22 | 空值、多行 prompt 的保存、读取与编辑 | 01 | 01 |
| P22 | 空值、多行 prompt/input 的创建与实际投递 | 04 | 04 |
| P49 | 名称经实际生产包装层保真且无额外执行 | 02 | 02 |
| P49 | 初始化正文经实际生产包装层保真且无额外执行 | 04 | 04 |

每行由对应 owner 写测试，各覆盖项通过后承诺整体才完成。

## Human-led coverage preparation

本票负责 P28/P29 的 H1：在四组合共享验收中安排覆盖两项 Skill，准备预期输入原文、Skill 资源前提、原生输入路径和单轮提交的终端证据。用户确认真实 TUI 收到一轮初始化输入及原生 Skill 路径，不要求任务完成。受控记录不冒充真实终端证据；实现及全部自动检查先完成，未确认的人工部分保持未验证。

## Contract boundaries

依据已批准的《Contract — Bot 模板管理与从 Zone 创建 Bot》及来源 spec。输入合成是内部计算，不另公开私有接缝；不增加多 Skill、消息序列、多步脚本、Skill 安装、组合非空校验或自动重发。
