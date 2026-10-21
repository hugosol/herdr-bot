# 无法启动（unstartable）夹具

这一组夹具覆盖“操作目标无法启动”的情形。它们都不会被执行，因此**都不会写入调用记录**——
记录里没有对应行，本身就是“目标没有跑起来”的证据。

在 `herdr-bot.toml` 中按下列任一方式绑定：

| 变体 | 配置 | 观测结果 |
|---|---|---|
| 目标不存在 | `script = '<仓库>\fixtures\operations\unstartable\missing-script.bat'`（该文件故意不存在） | 后端在调用前就判定目标不存在，不创建任何进程：`started=false`、`exitCode=null`、`outcome=failed`、`failure=not-started`，夹具没有运行，记录文件无新增行 |
| 存在但不可执行 | `script = '<仓库>\fixtures\operations\unstartable\invalid-target.exe'`（内容是文本，不是有效 PE 映像） | Windows 拒绝把它创建成进程（后端不再经 `cmd.exe` 折成退出码），同样报告 `started=false`、`exitCode=null`、`outcome=failed`、`failure=not-started`，夹具没有运行，记录文件无新增行 |
| 工作目录不存在 | 保留任意 `script`，把 `working_directory` 指向 `...\fixtures\operations\unstartable\missing-working-directory`（该目录故意不存在） | 进程无法创建：`started=false`、`exitCode=null`、`outcome=failed`、`failure=not-started`，夹具没有运行，记录文件无新增行 |

关于“不能被当作已启动成功”：以上三种情形都不会产生成功形状的结果（不会出现 `outcome=completed`，
也不会出现 `exitCode=0`）。它们都属于结果分类里的“执行失败→无法启动”，与“脚本运行后失败退出”
（`outcome=failed`、`failure=failed-exit`）可区分；字段定义见根目录 `README.md`
的“操作响应与执行结果分类”。

为什么三种情形都要在调用前判定：`cmd.exe` 找不到批处理时自己以退出码 1 结束，这与批处理自身
`exit /b 1` 无法区分，因此退出码不足以判断“脚本到底有没有启动起来”。

自动化检查见 `backend/tests/execution_result.rs` 中的三个 `*_is_reported_as_failed_not_started`
测试；`backend/tests/fixture_behavior.rs` 另有三个 `unstartable_*` 测试核对“从未被调用过”。
