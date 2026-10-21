# 03: 按 agent 和模型能力启动指定配置

**What to build:** 用户保存的 omp/pi、显式模型和适用 thinking 在 Bot 启动时确定地生效；选择默认值时沿用 agent 原生配置，不在启动后用模型选择器或 slash 命令改配。

**Delivers:** P16；完成 P14、P17、P18 的运行侧覆盖，模板侧由 01 负责。

**Test owner for:** P14 的实际启动目标；P17 的运行侧能力限制；P18 的默认 model 启动参数省略行为。

**Blocked by:** 02 — 从 zone 创建默认配置 Bot，贯通进展与结果。

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 从公开创建入口分别选择 omp 和 pi，独立对外交互记录证明启动目标类型正确；不增加第三种 agent 或自由输入入口。
- [ ] 自动覆盖 omp/pi 分别搭配两个显式模型的四种组合。GPT 完整配置为 openai-codex/gpt-6.1-sol，DeepSeek 为 deepseek/deepseek-flash；独立预写期望，不用实现映射生成期望。
- [ ] 模型与 thinking 经 Herdr 透传原生启动参数，在首次任务提交前设置；不使用裸模型名模糊匹配、自动 provider 回退或启动后切换。
- [ ] model 默认时实际启动不传 model 和 thinking 覆盖；显式模型搭配默认 thinking 时不传 thinking 覆盖。
- [ ] 依据各 agent 目标版本的固定能力事实及独立样例验证实际启动参数；Pi 不收到 auto，非推理模型只沿用默认 thinking，不能以参数解析成功证明模型支持。
- [ ] 公开入口、已保存配置与实际外部参数一致；不通过直接调用私有参数构造函数证明运行行为。保留既有就绪、单次提交和成功语义。
- [ ] 完成自动检查及适用运行验证，明确区分受控 adapter 对外记录与真实 Herdr/agent 运行结果。账号不可用不能触发隐式回退或伪造通过。

## Shared-promise coverage partition

| Promise | Coverage item | Ticket | Test owner |
|---|---|---|---|
| P14 | 页面与公开保存仅接受 omp/pi | 01 | 01 |
| P14 | 外部启动目标对应所选 agent | 03 | 03 |
| P17 | 界面选项与公开保存符合各 agent/model 能力事实 | 01 | 01 |
| P17 | 实际启动交互符合能力限制，Pi 不收到 auto | 03 | 03 |
| P18 | 默认 model 的页面与公开保存 thinking 为默认 | 01 | 01 |
| P18 | 默认 model 的实际启动不传 thinking 覆盖 | 03 | 03 |

各行由其 owner 写测试，所有覆盖项通过后相应承诺才整体完成；P16 还须取得 H1 确认。

## Human-led coverage preparation

本票负责 P16 的 H1：准备四组复现步骤、预期完整模型配置、启动记录、Tab/Pane 定位及已完成自动检查。与 02/04/05 协同使用同一组四组合场景，覆盖两种 Skill 和新对话观察。真实账号、费用及登录相关运行最终由用户确认，不要求模型完成任务；未经确认的真实部分保持未验证，不暂停其他可执行实现或自动检查。

## Contract boundaries

依据已批准的《Contract — Bot 模板管理与从 Zone 创建 Bot》及来源 spec。仅两个显式模型与默认值；不增加模型目录系统、运行时发现、凭据安装、权限筛选或自由输入。此票不重复认领 01 的界面/保存测试。
