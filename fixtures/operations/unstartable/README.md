# 无法启动（unstartable）夹具

按[夹具隔离约束](../../README.md)复制夹具。在隔离的服务配置中逐项测试；除本次改变的字段外，使用有效的夹具脚本和工作目录。

| 变体 | 配置 | 识别阶段 |
|---|---|---|
| 目标不存在 | 将 `script` 指向副本中故意不存在的 `operations/unstartable/missing-script.bat`。 | 后端在启动前拒绝。 |
| 存在但不可执行 | 将 `script` 指向副本中的 `operations/unstartable/invalid-target.exe`，它是文本而非有效的 Windows 可执行映像。 | Windows 在创建进程时拒绝，目标不经 `cmd.exe`。 |
| 工作目录不存在 | 将 `working_directory` 指向副本中故意不存在的 `operations/unstartable/missing-working-directory`。 | 后端在启动前拒绝。 |

三种情况都应返回 `failed` 下的 `not-started`，不能归为已启动后失败或成功。公开响应字段见[固定操作](../../../docs/usage.md#server-固定操作)。缺失批处理须先检查，因为 `cmd.exe` 找不到批处理时的退出码 `1` 与脚本自己的 `exit /b 1` 无法区分。

用独占空记录目录核对是否新增调用。先确认同一套记录设施能记录正常夹具调用；记录为空只能佐证未执行，须与响应分类和预检或 Windows 进程创建结果一起判断。退出码 `97` 表示记录设施失败，不能作为未执行的证据。

集成测试见 [execution_result](../../../backend/tests/server/execution_result.rs) 与 [fixture_behavior](../../../backend/tests/infrastructure/fixture_behavior.rs)。
