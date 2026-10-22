# Bot 模板 H1：Skill 与单轮初始化提交（04 负责部分）

状态：准备工作已完成，**尚未人工确认**。受控 Herdr 夹具的自动记录只证明 herdr-bot 发出的调用，
不能冒充真实 Herdr / OMP / Pi TUI 收到输入。

## 范围

本文件准备契约 P28 / P29 的人工 H1 部分：在四组 agent / model 组合（omp、pi 分别搭配
`gpt-6.1-sol`、`deepseek-flash`）的共享验收里，覆盖 `/teach` 与 `/code-review` 两项 Skill：

- 真实 TUI 收到的是一条合成后的初始化输入，Skill 调用使用原生 `/skill:` 路径；
- 只有一轮初始化输入，没有先单独开启 Skill 轮次；
- 提交被确认后创建即成功，不要求模型完成任务或 Skill 执行成功。

自动检查（`backend/tests/bot_creation.rs`、`frontend/tests/botCreation.test.tsx`、
`frontend/browser-tests/botCreation.spec.ts`）已覆盖公开入口与页面的正文、顺序、单次提交、空值组合
和包装层保真；它们不替代本文件的人工终端证据。

## 资源前提

- herdr 正在运行，且恰好存在一个名称精确为 `bot-playground` 的 Workspace。
- omp 与 pi 已安装，对应账号与模型凭据可用；两项固定模型的选择不代表账号一定有权调用。
- 目标 zone 目录下，目标 agent 能发现名为 `teach` 与 `code-review` 的 Skill。项目级资源还受
  agent 的项目信任设置限制；OMP 需要 `skills.enableSkillCommands` 开启。Pi 的
  `enableSkillCommands` 只影响命令发现，手工输入仍会展开。无法确认 Skill 被加载时，不要把它写成
  已验证。
- 四组组合各自使用一个可辨认的 zone，便于记录 cwd、Tab / Pane 与会话路径。

## 预写样例

固定 prompt（两个模板共用，含会被原样保留的 slash 文本）：

```text
固定指示第一行
固定指示第二行：保留 /skill:teach 与 /code-review 字面
```

本次 input：

```text
本次任务第一行
第二行带引号 "quoted" 与 & 符号
```

建议分配：`teach` 与 `code-review` 各覆盖两组组合，例如 omp + `gpt-6.1-sol` 用 `/teach`，
omp + `deepseek-flash` 用 `/code-review`，pi 的两组反之。两项 Skill 都必须至少被一组真实组合观察到。

模板字段（除 Skill 外相同）：

```json
{
  "name": "H1 输入核对",
  "botName": "{zone.name} · {template.name} · {bot.id}",
  "agent": "<omp 或 pi>",
  "model": "<gpt-6.1-sol、deepseek-flash 或 default>",
  "thinking": "<按 agent / model 的固定选项或 default>",
  "skill": "<teach 或 code-review>",
  "prompt": "固定指示第一行\n固定指示第二行：保留 /skill:teach 与 /code-review 字面"
}
```

## 预期唯一一轮初始化输入

`/teach` 模板：

```text
/skill:teach 固定指示第一行
固定指示第二行：保留 /skill:teach 与 /code-review 字面

本次任务第一行
第二行带引号 "quoted" 与 & 符号
```

`/code-review` 模板：

```text
/skill:code-review 固定指示第一行
固定指示第二行：保留 /skill:teach 与 /code-review 字面

本次任务第一行
第二行带引号 "quoted" 与 & 符号
```

命令名后是普通空格；固定 prompt 与本次 input 之间是一个空行。预期**不是** `/teach …`、
`/code-review …` 的同名扩展命令或提示词模板形式，也不是两轮输入。

## 操作步骤

1. 启动已构建的 herdr-bot，打开主界面 `zone` 入口。
2. 在目标 zone 行点击“+”，选择对应模板，填写上面的本次 input；确认只读预览名称。
3. 提交创建，观察到“正在启动”→“正在提交初始指示”→成功。成功文案表示提交已确认，不等待任务完成。
4. 在真实 herdr TUI 中检查目标 Tab / Pane：确认收到的是上面完整的一段初始化输入，并且只有一轮；
   确认首行是 `/skill:teach ` 或 `/skill:code-review `，不是 `/teach ` 或 `/code-review `。
5. 保存终端证据：包含输入原文与轮次边界的截图或文本记录；同时记录弹窗结果中的 Bot 名、zone、
   Tab / Pane 标识与控制别名。

## 记录表

| 组合 | agent / model / thinking | Skill | 预期正文 | 终端实际正文 | 单轮证据 | Tab / Pane | 结果 |
|---|---|---|---|---|---|---|---|
| 1 |  | teach | 见上 | 未确认 | 未确认 |  | 未确认 |
| 2 |  | code-review | 见上 | 未确认 | 未确认 |  | 未确认 |
| 3 |  |  | 见上 | 未确认 | 未确认 |  | 未确认 |
| 4 |  |  | 见上 | 未确认 | 未确认 |  | 未确认 |

## 未验证边界

- 本文件不证明模型完成任务、Skill 内容被正确执行或账号具备模型权限。
- 受控夹具记录、模拟回显和单元测试都不能填写“终端实际正文”或“单轮证据”。
- 用户确认前，P28 / P29 的人工 H1 部分保持未验证。
