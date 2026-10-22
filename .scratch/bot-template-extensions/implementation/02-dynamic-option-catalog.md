# 02: 由动态元数据驱动现有模板选项与启动映射

**What to build:** 用户直接维护集中选项目录后，无需重启即可在重新打开的模板编辑器、保存请求和后续 Bot 创建中使用当前 Agent、model、thinking、Skill 选项及映射。完成现有字段从页面到保存、持久结果和外部启动的纵向路径；启动时元数据无效则明确失败。

**Delivers:** P7、P8、P9、P10、P11、P25（现有字段部分）、P26（现有字段部分）、P31、P40（独立元数据布局部分）

**Test owner for:** P25、P26 的现有字段覆盖；P40 的独立元数据布局覆盖。P25/P26 的扩展覆盖由 03、04 拥有，P40 的运行数据布局由 01 拥有；各承诺在全部覆盖完成后才完成。

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 一个手工维护的 TOML 元数据文档集中描述 Agent、模型与完整 provider/model 映射、按 Agent 和模型区分的 thinking、Skill、固定与可选扩展定义、显示信息及适用关系；使用规格规定的独立元数据位置并纳入交付配置。
- [ ] 现有字段的公开选项、真实页面展示、服务端保存校验及实际启动解释消费同一份当前元数据，不继续维护独立代码清单或隐藏备用配置；模板仅保存稳定标识，不固化 provider/model 或能力映射副本。
- [ ] 初始 Agent 为 OMP、Pi，模型为默认、gpt-6.1-sol、deepseek-flash；两个显式模型分别使用 openai-codex/gpt-6.1-sol 和 deepseek/deepseek-flash，不裸名模糊匹配或回退 provider。默认模型不产生 model/thinking 覆盖。
- [ ] 按独立字面样例验证四组显式 thinking 能力：OMP GPT 为 low/medium/high/xhigh/max，OMP DeepSeek 为 low/high/max，Pi GPT 为 minimal/low/medium/high/xhigh/max，Pi DeepSeek 为 off/low/high/max；均可沿用默认值。非法组合通过直接公开请求也被拒绝，不能用 CLI 接受代替模型能力证据。
- [ ] 默认模型只能搭配默认 thinking；页面联动与直接请求均受约束，外部启动记录没有多余 thinking 覆盖。
- [ ] 真实页面切换模型时保留仍适用的 thinking；不适用则重置为默认并提示，分别核对具体值及提示。
- [ ] Skill 保留不选、teach、code-review 单选，prompt 保留多行及空值；独立样例验证完整字段读回和原生初始输入。不增加 Skill 必选、空正文组合限制或新的成功分支。
- [ ] 在同一真实程序实例中外部修改现有字段的选项、合法性及映射，分别重新打开编辑、提交保存和公开创建；观察独立预写的新选项、校验结果与启动值，相同已保存标识不必重存即可使用新映射。
- [ ] 真实启动分别覆盖元数据缺失、不可读、语法错误、重复标识和矛盾关系；均明确启动失败并给出可定位原因，不生成替代文件或回退清单。
- [ ] 以随附配置形状真实启动，证实读取独立元数据位置，不仅验证私有解析或路径函数。同步公开选项、配置及运行说明。

## Verification and scope

依据：[批准契约](../contract.md)、[规格](../spec.md)。使用现有真实页面、公开 HTTP、真实程序/文件和独立受控 Herdr 记录；预期不调用实现的解析、映射或组装逻辑生成。

本票完整拥有 P7–P11、P31 的自动检查。扩展选择编辑/保存由 03 完成，扩展实际启动由 04 完成；运行中损坏、旧页面和失效持久选择的完整修正路径由 05 完成。无需等 01 的运行数据切换才能开始，不引入额外存储 port、文件监听或后台推送。
