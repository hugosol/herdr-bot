# 隔离批处理验证夹具（fixtures/）

这些夹具是**随交付保留的长期验证资产**：一组真实的 Windows 批处理，用真实本地进程执行机制
（后端以 `cmd.exe /C <批处理>` 调用、固定工作目录）覆盖各类操作情形，作为自动化检查的执行目标。

用途（对应行为契约承诺）：

| 情形 | 夹具 | 用于覆盖 |
|---|---|---|
| 立即以成功退出码结束 | `operations/success.bat`（退出码 0） | P3、P6、P8、P9 |
| 以明确失败退出码结束 | `operations/failure.bat`（退出码 3） | P6、P9 |
| 无法启动（目标不存在/不可执行） | `operations/unstartable/` | P6、P3 |
| 启动后长时间不结束，可由人工结束 | `operations/long-running.bat` | P5、P10、P12 |
| 分离长期子进程后立即退出 | `operations/spawn-detached.bat`（子进程 `operations/detached-child.bat`） | P6（完成判定不等待另开的长期进程） |

夹具**只替代“自动化检查所驱动的脚本”**：它们不替代 P2 的真实手机连通验收，也不替代 P4 对用户
真实启动/关闭脚本的人工验收。夹具全部位于本目录，不读取、不修改、也不依赖
`E:\Developer\myhub\my-streaming` 下的用户现有脚本。

## 目录

```
fixtures/
  README.md                     本文件
  operations/                   操作目标批处理（配置里指向它们）
    success.bat                 立即成功结束
    failure.bat                 以退出码 3 失败结束
    long-running.bat            长时间不结束，等待停止标记
    spawn-detached.bat          分离启动子进程后立即退出
    detached-child.bat          被分离启动的长期子进程
    unstartable/                无法启动情形
      README.md
      invalid-target.exe        存在但不是可执行映像
  lib/
    fixture-lib.ps1             记录机制与进程工具（UTF-8 with BOM）
    record-call.ps1             写调用记录；可选等待停止标记
    stop-fixtures.ps1           结束长期夹具
  stop-fixtures.bat             人工/脚本结束入口
  runtime/                      运行时产物（记录、PID、停止标记），已被 .gitignore 忽略
```

## 在服务端配置中指定为操作目标

夹具通过服务端配置（`herdr-bot.toml`）的 `[operations.start]` / `[operations.stop]` 绑定，
客户端无法替换命令、脚本路径或参数。绝对路径示例（把 `<仓库>` 换成本仓库所在目录）：

```toml
[operations.start]
script = '<仓库>\fixtures\operations\success.bat'
working_directory = '<仓库>\fixtures\operations'

[operations.stop]
script = '<仓库>\fixtures\operations\failure.bat'
working_directory = '<仓库>\fixtures\operations'
```

两个操作各自绑定一个夹具，就能用两个不同的夹具区分“启动”与“停止”各自调用了哪个脚本。

其他情形只需替换 `script`：

- 失败：`...\operations\failure.bat`
- 长时间不结束：`...\operations\long-running.bat`
- 分离子进程：`...\operations\spawn-detached.bat`
- 无法启动（目标不存在）：`...\operations\unstartable\missing-script.bat`（该文件**故意不存在**）
- 无法启动（存在但不可执行）：`...\operations\unstartable\invalid-target.exe`
- 无法启动（工作目录不存在）：保留 `script`，把 `working_directory` 指向不存在的目录，例如
  `...\fixtures\operations\unstartable\missing-working-directory`

`fixtures/operations/unstartable/README.md` 记录了两类“无法启动”的观测结果。

## 调用记录

每次夹具被调用都会向同一个记录文件追加一行，用于证明“仅调用了一次”“没有延迟补做”“调用时间与工作目录”：

- 默认位置：`fixtures/runtime/calls.tsv`
- 可配置：环境变量 `HERDR_BOT_FIXTURE_RECORD_DIR` 指向任意目录（进程级，后端与其子进程继承）
- 记录文件为 UTF-8（无 BOM），制表符分隔，一行一次记录或一条阶段记录，字段顺序固定：

| 列 | 含义 |
|---|---|
| `call` | 调用序号：该次调用的编号，从 1 起递增，同一次调用的各阶段相同 |
| `seq` | 记录行序号：记录文件内的全局行号，从 1 起、连续无缺口（可用于证明追加写不丢行） |
| `timestamp` | 调用时间，ISO 8601 含毫秒与本地时区偏移（`2026-09-30T16:45:49.711+08:00`），可直接排序 |
| `fixture` | 夹具标识：`success`、`failure`、`long-running`、`spawn-detached`、`detached-child`、`stop-fixtures` |
| `stage` | 阶段：`invoke`（一次调用开始）、`child-start`、`stop-observed`、`stop`（结束脚本自身） |
| `callid` | 本次调用的唯一标识，由夹具进程生成，同一调用的各阶段相同 |
| `pid` | 夹具进程 PID（运行批处理的 `cmd.exe`），可据此判断是否仍在运行 |
| `cwd` | 夹具进程当时的实际工作目录（即服务端配置的工作目录） |
| `detail` | 附加说明，例如夹具自报的退出码或结束方式 |

示例（一次成功调用，随后由结束脚本结束长期夹具）：

```
1	1	2026-09-30T16:45:49.460+08:00	success	invoke	p1804-3f0a1b2c9d4e	1804	C:\...\fixtures\operations	exit-code=0
2	2	2026-09-30T16:52:10.130+08:00	long-running	invoke	p28192-77aa11bb22cc	28192	C:\...\fixtures\operations	holds-until-stop-marker
2	3	2026-09-30T16:52:31.902+08:00	long-running	stop-observed	p28192-77aa11bb22cc	28192	C:\...\fixtures\operations	ended-by-stop-marker
3	4	2026-09-30T16:52:32.145+08:00	stop-fixtures	stop	0c9d...	33964	C:\...\fixtures	long-running=ended-by-stop-marker;detached-child=not-running
```

核对约定：

- **调用次数** = 该夹具 `stage=invoke`（子进程为 `child-start`）的记录行数；
- **仅一次调用** = 目标时间窗内该夹具的 `invoke` 行恰好一行；
- **无延迟补做** = 时间窗之后不再出现该夹具的 `invoke` 行；
- 记录写入由命名互斥保护并带重试，重叠/并发调用下不丢行、`seq` 连续、`call` 互不重复。

`fixtures/runtime/` 只是运行时产物，可随时整体删除（记录是证据，不是状态）；`pids/` 与 `stop/` 子目录
分别存放 PID 文件与停止标记。

## 结束长期夹具（不留进程）

`long-running.bat` 与 `detached-child.bat` 不会自行结束。结束方式有两种，都会写进调用记录：

1. **停止标记（首选）**：创建 `<记录目录>\stop\<夹具>.stop` 文件，夹具见到后写入 `stage=stop-observed`
   记录并退出；`stop-fixtures.bat` 会为两个长期夹具写标记。
2. **PID 文件 + 强制结束（兜底）**：`<记录目录>\pids\<夹具>.pid` 记录夹具进程 PID，可用
   `taskkill /PID <pid> /T /F` 结束进程树（分离子进程的窗口标题为 `herdr-bot detached child`）。

人工或脚本统一入口：

```bat
fixtures\stop-fixtures.bat
```

它先写停止标记、在宽限期内等待夹具自行结束，宽限期后按 PID 文件强制结束进程树，最后把
`long-running=`、`detached-child=` 的实际结束方式追加到记录（`fixture=stop-fixtures`，`stage=stop`）。
没有夹具运行时执行同样安全。执行后应确认两个 PID 都不再存活（`tasklist /FI "PID eq <pid>"`）。

关于安全的两个细节：

- **PID 文件先核对身份**：使用 PID 文件前会确认该 PID 的进程命令行里确实带着本夹具目录与对应批处理名，
  PID 被系统回收给无关进程时只报告 `stale-pid-file`，不会误杀无关进程。
- **兜底清理只作用于本夹具目录**：脚本最后会按命令行查找“仍在运行但还没登记 PID”的夹具进程
  （例如刚被分离启动、尚未写入 PID 文件的子进程），并强制结束，报告为 `sweep-killed`。
  查找以本脚本所在的夹具目录为界，因此并行的夹具副本（例如自动化检查各自拷贝一份）互不影响：
  在自己的副本里执行结束脚本，不会动到另一份拷贝或仓库夹具正在运行的进程。

夹具在开始时还会清掉残留的停止标记，因此上一次运行留下的标记不会让下一次调用立刻结束。

## 自动化检查

```bat
cd backend
cargo test --test fixture_behavior
cargo test --test execution_result
```

`backend/tests/fixture_behavior.rs` 经 HTTP interface（S2）把上述夹具绑定为操作目标并真实调用，
再核对调用记录与进程状态；`backend/tests/execution_result.rs` 同样经 S2 驱动这些夹具，核对执行
结果分类（无法启动 / 成功退出 / 失败退出）与“脚本另开长期进程不参与完成判定”；
`backend/tests/connection_drop.rs` 用 `long-running.bat` 验证真实连接中断下的操作生命周期
（断线后脚本继续执行到自行结束、期间其他客户端得到忙碌结果、结束后恢复接受操作）；
`backend/tests/common/mod.rs` 提供复制夹具目录、解析记录、判断进程存活的共用设施。
每个测试都把 `fixtures/` 复制到独立临时目录后再驱动，因此并行运行的测试不会互相污染记录文件。

## 维护约定

- **批处理文件（`.bat`）内容保持纯 ASCII**：`cmd.exe` 按系统 ANSI 代码页解析批处理，非 ASCII 注释
  会让字节配对错位，甚至把注释片段当成命令执行（`chcp 936` 下实测如此）。中文说明写在本文件里。
- **`lib/*.ps1` 使用 UTF-8 with BOM**：Windows PowerShell 5.1 没有 BOM 时按系统 ANSI 代码页解析，
  会把中文注释解析坏。
- 批处理与 PowerShell 脚本使用 CRLF 行尾。
- 夹具不得修改用户现有启动/关闭脚本，也不得依赖它们；只使用自身目录、系统 `powershell.exe`、
  `cmd.exe` 与 `taskkill`。
- 退出码 97 表示夹具自身的记录机制出错（记录没写成功），不是被验证脚本的行为。
