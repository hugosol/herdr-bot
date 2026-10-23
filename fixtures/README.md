# 隔离操作夹具

真实 Windows 批处理，仅用于隔离验证操作执行机制，不控制用户现有服务，也不替代真实手机或整机关机验收。
测试必须复制到独立临时目录后运行，避免共享 PID、停止标记和记录。

## 安全与维护约束

- `long-running.bat` 和 `spawn-detached.bat` 会留下长期进程。用当前夹具副本中的 `stop-fixtures.bat` 清理；不要误用另一份副本或按不明 PID 杀进程。
- 记录默认写入 `runtime/`，可由 `HERDR_BOT_FIXTURE_RECORD_DIR` 隔离。退出码 97 表示夹具记录机制失败，不是被测操作结果。
- `.bat` 必须为纯 ASCII，PowerShell 脚本为 UTF-8 with BOM，均使用 CRLF。Windows cmd／PowerShell 5.1 的默认代码页可能将其他编码误解析为命令。
- 夹具只能依赖自身目录与系统工具，不得调用或修改用户的实际启动／停止脚本。

执行行为由 `backend/tests/fixture_behavior.rs`、`execution_result.rs` 等集成测试覆盖；
夹具实现与记录字段以 `operations/`、`lib/` 为准，不在文档中复制一套规格。
