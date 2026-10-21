# 01: 管理共享、持久化的 Bot 模板

**What to build:** 用户在 herdr-bot 主界面的独立“Bot 模板”tab 新增、查看、编辑和删除完整模板；电脑与手机共享服务端保存值，服务重启后仍可读取。编辑时可以插入命名字段、查看示例，并按 agent/model 能力选择配置。

**Delivers:** P1, P2, P4, P5, P6, P7, P9, P15, P19, P20, P21, P47；P14、P17、P18、P22 的模板侧覆盖，完整承诺分别由 03 或 04 补齐。

**Test owner for:** P14 的页面与公开保存固定 agent 选项；P17 的界面与公开保存能力规则；P18 的页面与公开保存默认规则；P22 的 prompt 保存、读取和编辑。

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 真实浏览器进入独立“Bot 模板”tab，完成新增、查看、编辑、删除；既有 tab 仍可操作。删除后由独立客户端读取并尝试选择，目标模板不可用。
- [ ] 模板完整保存 name、bot-name、agent、model、thinking、skill、prompt。通过公开操作保存独立字面样例，由另一客户端独立读取比对全部字段，不读取私有状态或以同页缓存证明共享。
- [ ] 隔离真实存储，写入预先确定的模板，停止并重新启动 herdr-bot 进程后通过公开入口读取，值保持一致。
- [ ] name 与单行 bot-name 必填；agent 只能是 omp 或 pi。初始命名规则为“{zone.name} · {template.name} · {bot.id}”。不新增 Skill/prompt/input 非空组合校验。
- [ ] 命名编辑器支持插入且仅提供四个已承诺字段：zone.name、template.name、agent、bot.id；浏览器操作后能观察对应示例内容。不扩展未知占位符诊断的产品承诺。
- [ ] model 仅提供沿用默认值、gpt-6.1-sol、deepseek-flash，不提供自由输入。thinking 根据各自目标 agent 和具体模型的固定能力事实限制，不能把全局 CLI 级别当成模型能力，也不能复制 OMP 能力表作为 Pi 的事实。
- [ ] model 默认时 thinking 为默认；切换 agent 后 model/thinking 均重置；切换 model 时保留仍适用的 thinking，否则重置并提示。真实浏览器分别验证适用与不适用样例，公开保存遵守同一能力约束。
- [ ] Skill 仅提供无、/teach、/code-review，公开保存不允许任意 Skill 文本；多行及空 prompt 均可保存、编辑并按字面读取。
- [ ] 隔离真实持久化失败或注入受控保存故障，公开返回失败、页面不表示已保存；独立重新读取仍为此前已保存值。
- [ ] 自动检查与适用运行验证全部完成后交付；测试期望来自独立字面样例和能力事实，不复用实现计算生成期望。遵循契约 S1/S2，实际启动配置和输入投递的证明归后续票。

## Shared-promise coverage partition

以下分区在所列 ticket 中各维护相同边界；owner 负责该行测试。所有覆盖项通过后承诺整体才完成。

| Promise | Coverage item | Ticket | Test owner |
|---|---|---|---|
| P14 | 页面与公开保存仅接受 omp/pi | 01 | 01 |
| P14 | 外部启动目标对应所选 agent | 03 | 03 |
| P17 | 界面选项与公开保存符合各 agent/model 能力事实 | 01 | 01 |
| P17 | 实际启动交互符合能力限制，Pi 不收到 auto | 03 | 03 |
| P18 | 默认 model 的页面与公开保存 thinking 为默认 | 01 | 01 |
| P18 | 默认 model 的实际启动不传 thinking 覆盖 | 03 | 03 |
| P22 | 空值、多行 prompt 的保存、读取与编辑 | 01 | 01 |
| P22 | 空值、多行 prompt/input 的创建与实际投递 | 04 | 04 |

## Contract boundaries

依据已批准的《Contract — Bot 模板管理与从 Zone 创建 Bot》及其来源 spec。不规定内部存储格式、HTTP 资源命名或模块布局；不新增认证、模型发现、并发编辑版本控制或 Skill 安装能力。本票不宣称完成运行侧的 P14/P17/P18 或输入侧的 P22。
