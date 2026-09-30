# omp-bot：脚本控制面板（首版）

在 Windows 上手动启动的网页控制面板：本机浏览器与同一局域网内的手机访问**同一个服务**，
用固定按钮操作预先配置好的批处理脚本。控制面板只反馈脚本调用情况，不检查后台程序是否就绪。

首版范围见 [`.scratch/omp-bot-v1/spec.md`](.scratch/omp-bot-v1/spec.md) 与
[`.scratch/omp-bot-v1/contract.md`](.scratch/omp-bot-v1/contract.md)。

## 目录结构

| 路径 | 内容 |
|---|---|
| `backend/` | Rust/Axum 服务：提供打包页面与操作接口 |
| `backend/src/config.rs` | 服务端配置：监听地址、固定操作绑定（脚本 + 工作目录） |
| `backend/src/operations.rs` | 启动配置的执行目标并等待其结束（能否启动、如何结束） |
| `backend/static/` | 打包页面构建产物（由 `frontend` 构建生成，不入库） |
| `frontend/` | React 前端源码（Vite） |
| `fixtures/` | 隔离验证夹具（真实 Windows 批处理，供自动化检查绑定为操作目标；见 [fixtures/README.md](fixtures/README.md)） |
| `omp-bot.toml` | 运行配置（默认读取当前工作目录下的这个文件） |

## 前置条件

- Rust 1.98+（`cargo`、`rustc`）
- Node.js 24+（`node`、`npm`）

## 构建

在仓库根目录执行：

```bat
npm --prefix frontend install
npm --prefix frontend run build
cargo build --release --manifest-path backend\Cargo.toml
```

`npm run build` 会把打包页面写入 `backend/static/`（同一个 Axum 服务从这里提供页面），
`cargo build --release` 生成 `backend\target\release\omp-bot.exe`。

## 手动启动

在仓库根目录执行（默认读取 `omp-bot.toml`）：

```bat
backend\target\release\omp-bot.exe
```

需要换一份配置时：

```bat
backend\target\release\omp-bot.exe --config 其他配置.toml
```

启动后控制台会打印监听地址、当前绑定的启动与停止脚本及其工作目录。
开发时也可以用 `cargo run --release --manifest-path backend\Cargo.toml`。

请在**控制台窗口**（cmd、PowerShell 或终端）中启动控制面板：它会把自身的控制台继承给脚本进程，
脚本里 `timeout` 这类需要控制台输入的命令才会像你手动运行脚本时一样工作（详见
[现有脚本接入的已知现象](#现有脚本接入的已知现象)）。若在无控制台的环境启动，启动日志会打印提示，
脚本自身的这类命令会以非交互方式失败，操作结果会如实显示为该退出码。

## 访问（本机与局域网）

启动后控制台会打印监听地址与局域网访问提示：

```
[omp-bot] 监听：http://0.0.0.0:8787/
[omp-bot] 局域网设备请访问 http://<本机局域网 IPv4>:8787（ipconfig 查看地址）
```

- 本机：`http://127.0.0.1:8787/`
- 局域网设备：`http://<本机局域网 IPv4>:8787/`（用 `ipconfig` 查看地址，例如 `192.168.5.4`）

页面与操作接口**同源同端口**，都由这一个服务提供：手机不需要单独访问前端开发服务器，也不需要额外的
端口转发；照常打开页面即可，按钮请求打到同一地址的 `/api/operations/start`、`/api/operations/stop`。

### 用手机操作（步骤与预期结果）

真机连通与真机操作只能由**真实手机**确认，自动化验证（Chromium 窄屏仿真）不构成真机结论；当前状态
见[人工验收移交](#人工验收移交p2--p4未验收)。

1. 电脑侧按“构建”“手动启动”启动控制面板，确认控制台打印了监听地址。
2. 在电脑上查局域网地址：

   ```bat
   ipconfig
   ```

   取**无线局域网适配器 WLAN**（或以太网）那一节的 `IPv4 地址`（例如 `192.168.5.4`）。
   这个地址不是固定的：换 Wi-Fi、重启路由器或 DHCP 重新分配后会变，重新用 `ipconfig` 确认即可；
   端口固定为配置里的 `8787`。**不要在手机上用 `127.0.0.1`**（那指向手机自己）。
3. 手机连接**与电脑同一个 Wi-Fi**（不要用访客网络或热点）。
4. 手机浏览器打开 `http://<上一步的 IPv4>:8787/`。
   - **预期看到**：标题“omp-bot 控制面板”、一行范围说明（页面只反馈脚本调用情况）、两个大按钮
     （“启动”“停止”），结果框显示“尚未提交操作。”。
5. 点按“启动”：先出现“正在执行启动脚本……本次操作尚未结束。”，此时**两个按钮同时变灰、点不动**；
   脚本结束后显示本次调用结果。从**控制台窗口**启动面板时，默认绑定的现有脚本会显示“执行完成”
   （约 5 秒，脚本自身的等待会真正执行）；若面板是在没有控制台的环境里启动的，则会如实显示
   “执行失败：脚本以退出码 1 结束”，原因见[现有脚本接入的已知现象](#现有脚本接入的已知现象)。
6. 点按“停止”：同样先“执行中”、后结果；停止之后可以再次点按“启动”，会再次真实调用脚本
   （不检查服务是否已运行，也不做状态去重）。

手机打不开页面时：

| 现象 | 检查 |
|---|---|
| 一直转圈/无法访问 | 手机是否与电脑同一 Wi-Fi；Windows 防火墙是否放行监听程序。已在电脑上核实（`netsh advfirewall firewall show rule name=omp-bot`）：两条**入站允许**规则（TCP 与 UDP）、`配置文件=公用`、`程序=E:\developer\myhub\omp-bot\backend\target\release\omp-bot.exe`、任意端口；当前 WLAN 的网络类别正是“公用”，所以该规则覆盖这个地址 |
| 换了 exe 路径 / 重命名 / 换了网络类别 | 需要重新放行（管理员命令提示符）：`netsh advfirewall firewall add rule name="omp-bot" dir=in action=allow program="<omp-bot.exe 的绝对路径>" enable=yes profile=any` |
| 电脑上能开、手机上不能 | 路由器“AP 隔离/客户端隔离”、访客网络、手机 VPN/代理、公司网络的客户端隔离都会阻断，换普通家庭 Wi-Fi 再试 |
| 服务起不来 | 控制台打印“绑定监听地址失败”表示端口被占用：换配置里的端口，并同步改手机地址里的端口 |
| 页面能打开但按钮点了没反应 | 页面与接口同源同端口；若反向代理/端口转发只转发了页面，接口会失败（页面显示“无法确认本次调用结果”）。直接用 `http://<IPv4>:8787/` 访问 |

## 配置

`omp-bot.toml`：

```toml
listen_addr = "0.0.0.0:8787"   # 0.0.0.0 表示本机与局域网都可访问

[operations.start]
script = 'E:\Developer\myhub\my-streaming\start-all.bat'
working_directory = 'E:\Developer\myhub\my-streaming'

[operations.stop]
script = 'E:\Developer\myhub\my-streaming\shutdown-all.bat'
working_directory = 'E:\Developer\myhub\my-streaming'
```

- 脚本目标与工作目录来自这份服务端配置，**不作为请求参数**：客户端只能提交“启动”或“停止”
  这个操作名（`POST /api/operations/start`、`POST /api/operations/stop`），
  无法替换命令、脚本路径、工作目录或附加参数。
- 操作请求面是封闭的：请求体、查询串都是越界请求；其他操作名与 `/api/` 下的怪路径也一律被拒绝。
  拒绝时返回 JSON 类别（`404 unknown-operation`、`404 unknown-endpoint`、`400 unexpected-parameters`），
  并且不会执行任何脚本。
- 命令执行发生在后端所在的这台电脑上，不在手机或浏览器上。
- 运行中的服务不会自动重读配置，改完配置需要重启。

### 默认绑定与改绑

默认 `omp-bot.toml` 绑定的是用户**现有的两个批处理**（与规格 Further Notes 记录的外部集成路径一致），
执行工作目录为它们所在的目录：

| 操作 | `script` | `working_directory` |
|---|---|---|
| 启动 | `E:\Developer\myhub\my-streaming\start-all.bat` | `E:\Developer\myhub\my-streaming` |
| 停止 | `E:\Developer\myhub\my-streaming\shutdown-all.bat` | `E:\Developer\myhub\my-streaming` |

- 控制面板启动时会把这份绑定打印到控制台，人工核对应以这几行为准；脚本内容与控制面板**互不影响**：
  面板只在自己的进程里以配置的工作目录执行它们，不修改脚本、不写入脚本所在仓库。
- **改绑方式**（只能改服务端配置，客户端无法改绑）：
  1. 在仓库根目录编辑 `omp-bot.toml`，或另存一份配置文件并用
     `backend\target\release\omp-bot.exe --config <配置文件路径>` 指定；`[operations.start]` /
     `[operations.stop]` 下的 `script` 与 `working_directory` 是两个脚本各自的执行目标与工作目录；
  2. 重启控制面板（运行中的服务不会重读配置）。
- 配置文件使用 `deny_unknown_fields`：写错字段名会启动失败并给出解析错误，不会静默忽略。

### 隔离夹具（fixtures/）的用途

`fixtures/` 是一组随交付保留的真实 Windows 批处理，用来把控制面板绑到**不会影响用户程序**的目标上，
验证“能否启动 / 何时算结束 / 工作目录 / 互斥 / 卡住”等执行机制；也可以用来在没有后台程序的环境里
复现页面与接口行为。把一个操作改绑到夹具的写法（绝对路径示例）：

```toml
[operations.start]
script = '<仓库>\fixtures\operations\success.bat'
working_directory = '<仓库>\fixtures\operations'

[operations.stop]
script = '<仓库>\fixtures\operations\failure.bat'
working_directory = '<仓库>\fixtures\operations'
```

夹具清单、调用记录格式、结束长期夹具的方式见 [fixtures/README.md](fixtures/README.md)。

**夹具不替代真实运行证据**：它不替代 P4 对现有脚本的人工验收（真实启动/停止 Python 程序与窗口行为），
也不替代 P2 的真实手机连通验收；夹具自身不读取、不修改、也不依赖 `E:\Developer\myhub\my-streaming`
下的现有脚本。本文档“交付验证记录”里的构建/启动/访问验证就是绑夹具完成的，因此没有触发用户真实脚本。

### 操作响应与执行结果分类

两个操作都以 HTTP 200 返回同一种 JSON（`backend/src/lib.rs` 的 `OperationResponse`）：

```json
{"operation":"start","ended":true,"started":true,"exitCode":3,"outcome":"failed","failure":"failed-exit"}
```

| 字段 | 含义 |
|---|---|
| `operation` | 本次调用的固定操作名：`start` 或 `stop` |
| `ended` | 本次调用是否已经结束（响应返回时必然为 `true`） |
| `started` | 脚本进程是否成功创建 |
| `exitCode` | 脚本进程的退出码；未创建或没有退出码时为 `null` |
| `outcome` | **结果类别**：`completed`（脚本执行完成）或 `failed`（执行失败） |
| `failure` | 失败的子类别，仅失败时出现：`not-started` 或 `failed-exit` |

结果类别只看脚本进程本身能否启动及其退出结果：**不看脚本打印了什么**，也不推断后台 Python
进程的状态（脚本结束 ≠ 后台服务已就绪）。判定规则：

| 情形 | `outcome` | `failure` | `started` / `exitCode` | 样例响应 |
|---|---|---|---|---|
| 脚本以退出码 0 结束 | `completed` | （无此字段） | `true` / `0` | `{"operation":"start","ended":true,"started":true,"exitCode":0,"outcome":"completed"}` |
| 脚本运行后没有以退出码 0 结束（非零退出码，或没有退出码） | `failed` | `failed-exit` | `true` / 退出码或 `null` | `{"operation":"start","ended":true,"started":true,"exitCode":3,"outcome":"failed","failure":"failed-exit"}` |
| 脚本无法启动：目标不存在、目标不可执行或工作目录不存在 | `failed` | `not-started` | `false` / `null` | `{"operation":"start","ended":true,"started":false,"exitCode":null,"outcome":"failed","failure":"not-started"}` |

- 上方三类在同一个 HTTP 接缝上可直接观察、互相区分；页面按 `outcome` 显示“执行完成/执行失败”，
  需要区分失败原因时再看 `failure`。
- 页面把结果类别放在同一个结果节点上：`data-testid=operation-result`，机器可读的类别标识是
  `data-outcome`（`completed`、`failed`、`busy`，以及尚未结束/结果不可达时的 `idle`、`unreachable`），
  失败时另有 `data-failure`（`not-started` / `failed-exit`）。因此执行完成、执行失败与操作进行中
  在页面上互相可区分；页面文案只描述本次脚本调用，不据此推断后台程序或服务的状态。
- 脚本另开的长期进程（例如现有启动脚本打开的 Python 窗口）**不参与**完成判定：脚本一退出，
  本次操作就报告结果；脚本被强制结束而没有退出码时按 `failed-exit` 处理。
- 脚本目标以 `.bat` / `.cmd` 结尾时由 `cmd.exe` 解释执行，其他目标直接作为可执行映像启动；
  两者都使用配置的工作目录、非交互执行。

### 操作进行中（忙碌结果）

同一后端实例内**任意时刻至多一个脚本操作在执行**：两个操作共用同一份执行资格，资格从操作被接受
一直持有到脚本确认结束（覆盖整个脚本执行期间，不是只到进程创建完成）。执行期间到达的请求——同一
页面重复点击、其他页面、其他设备，甚至绕过页面直接发请求——都不会执行脚本，而是立即得到忙碌结果：

```json
{"code":"operation-in-progress","message":"已有脚本操作正在执行：本次请求没有被执行，也不会在当前操作结束后被补做。"}
```

| 属性 | 值 |
|---|---|
| HTTP 状态码 | `409` |
| `code` | `operation-in-progress` |

忙碌结果是**拒绝**（`409` + `code`），不是一次执行结果：响应里没有 `outcome`，因此与
`completed` / `failed` 结果以及 `400 unexpected-parameters`、`404 unknown-operation`、
`404 unknown-endpoint` 互相区分。页面据此显示“操作进行中”的反馈。

- **不排队**：被拒绝的请求只被拒绝这一次，不会在当前操作结束后被补做；是否重试由调用者自己决定。
- **连接中断不取消操作**：已接受的操作不因提交它的连接断开而取消，执行资格也不提前释放——脚本继续
  执行到它自己的结束，期间其他请求仍然得到忙碌结果，脚本确认结束后恢复接受操作。断线只意味着这次
  结果没有接收者：不重放、不恢复、不留操作历史。
- **不依赖页面**：忙碌判定完全在服务端；页面按钮禁用只是提示，绕过页面直接提交同样被拒绝。
- **范围限单个后端实例**：多实例运行不在此保证内（见“首版限制”）。
- **不覆盖脚本另开的长期进程**：脚本一退出，本次操作就结束、资格随之释放；脚本打开的长期程序不影响。

### 执行生命周期：没有自动终止策略

脚本一旦被接受，后端只做两件事：创建配置绑定的进程，然后**等它自己结束**。整条执行路径里不存在
时间阈值：既没有超时终止脚本，也没有超时释放执行资格。卡住时只能人工处理。

| 检查点 | 位置 | 观察到的结论 |
|---|---|---|
| 唯一的进程创建与等待 | `backend/src/operations.rs`（`ScriptOperation::invoke`） | `command.spawn()` 之后只有 `child.wait().await`；没有 `timeout`、`kill`、`start_kill`、`kill_on_drop`、job object，也没有任何“按时间放弃等待”的分支 |
| 唯一的执行资格持有 | `backend/src/lib.rs`（`run_fixed_operation`） | 资格由 `try_lock_owned()` 取得、由执行任务持有到 `invoke()` 返回（脚本确认结束）；没有 `tokio::time`、`sleep`、`interval`、`select!`、`abort()` 或定时释放资格的路径 |
| 交付构建连超时 API 都不可用 | `backend/Cargo.toml` 的 `tokio` 特性 `macros, net, process, rt-multi-thread, sync` | **不含 `time`**：发布构建里 `tokio::time::timeout` / `sleep` 不可用（`time` 只在 dev-dependencies 中为测试开启） |
| 关闭服务不牵连脚本 | `backend/src/main.rs` | `axum::serve(listener, app)` 直接等待服务结束；没有 `with_graceful_shutdown`、信号处理，也没有关闭时等待或终止脚本的路径 |

复核命令（工作目录为仓库根目录），**两条都应没有任何输出**：

```bat
findstr /S /I /N "timeout tokio::time kill start_kill kill_on_drop abort Duration select! ctrl_c signal" backend\src\*.rs
```

```bash
grep -rniE "timeout|tokio::time|\.kill\(|start_kill|kill_on_drop|abort\(|Duration|select!|ctrl_c|signal" backend/src/
```

### 脚本卡住时的人工处理

脚本卡住（长时间不结束）时控制面板不会做任何事：既不超时终止脚本，也不释放执行资格。表现是页面一直
停在“执行中”（按钮不可点），其他页面/设备/直接请求都得到 `409 operation-in-progress`。处理办法：

1. **确认脚本进程是否还在运行**。控制面板不提供这个信息：它没有执行历史，重启后也不记得旧脚本，
   所以必须自己看操作系统。脚本以 `cmd.exe /C <脚本>` 的形式运行，可以直接看：

   ```bat
   tasklist /FI "IMAGENAME eq cmd.exe"
   ```

   需要精确定位时用 PowerShell 按脚本路径找（把 `start-all.bat` 换成实际脚本名）：

   ```powershell
   Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*start-all.bat*' } |
     Select-Object ProcessId, CommandLine
   ```

   脚本打开的 Python 程序本身也可以直接看（窗口是否存在、端口是否仍在监听）：

   ```bat
   netstat -ano | findstr LISTENING | findstr "8899 8888"
   ```

   夹具场景另有现成入口：`fixtures\runtime\pids\<夹具>.pid` 记录夹具脚本进程 PID；
   `fixtures/README.md` 说明了记录文件与停止标记。
2. **结束脚本进程**：
   - 首选让脚本自己结束：夹具执行 `fixtures\stop-fixtures.bat`（写停止标记，夹具自行退出）。
   - 兜底在任务管理器“详细信息”里按命令行找到那个 `cmd.exe`，右键“结束任务”；或按 PID 结束脚本
     进程树：

     ```bat
     taskkill /PID <脚本进程 PID> /T /F
     ```

   - 结束脚本进程后，正在等待的请求会立刻得到结果；实测（夹具）：脚本被强制结束后请求返回
     `{"outcome":"failed","failure":"failed-exit"}`，执行资格随之释放，下一次操作被正常接受。
   - `/T` 会连同脚本的**子进程**一起结束。对用户现有启动脚本，它打开的 Python 程序是否随之结束
     未在本次交付中验证（没有触发真实脚本）；要停止那些 Python 程序请照旧用“停止”操作
     （`shutdown-all.bat`）。
3. **重启控制面板不证明旧脚本已结束**：面板不做终止、不做等待，也不保留跨重启的状态。实测：结束面板
   进程本身后，正在运行的脚本进程**仍在运行**；重新启动面板后页面回到“尚未提交操作。”并立即接受
   新操作，对旧脚本一无所知。所以“重启一下就好”是错觉——**先按第 1 步确认、再按第 2 步处理**。
   （未验证的推论：若脚本进程与面板共用同一个控制台窗口，用关闭窗口或 Ctrl+C 结束面板时，Windows
   可能连同该控制台上的脚本进程一并结束；这不改变“重启后无从确认旧脚本”的结论，处理前仍应先确认。）

## 现有脚本接入的已知现象

### 脚本需要控制台：从控制台窗口启动面板，页面显示“执行完成”

默认绑定的两个现有脚本的最后一条命令都是 `timeout /t N /nobreak >nul`（`start-all.bat` 中间还有一条）。
`timeout.exe` 需要**真实控制台**作为标准输入：在控制台窗口里手动运行时它正常等待、脚本以退出码 0 结束；
标准输入被重定向（非交互）时它立即失败并让脚本以退出码 1 结束——即使脚本主体（`start` 打开窗口、
启动或终止 Python 程序）已经生效。控制面板按“与手动运行脚本一致”的方式处理：

- **面板在控制台窗口里启动（推荐，也是手动启动的常态）**：后端把面板自己的控制台继承给脚本进程，
  脚本按设计等待并结束，页面显示“执行完成”（`outcome: completed`、退出码 0）。
- **面板在没有控制台的环境里启动**：后端保持非交互执行（不会额外弹窗），脚本里的 `timeout` 会失败，
  脚本以退出码 1 结束，页面如实显示“执行失败”（`outcome: failed`、`failure: failed-exit`）；
  此时 Python 程序仍可能已经启动/停止。启动日志会打印这一提示。

用真实脚本实测（同一台机器）：

| 面板启动方式 | 启动操作 | 停止操作 | 实际效果 |
|---|---|---|---|
| 控制台窗口 | “执行完成”（退出码 0，约 5 s） | “执行完成”（退出码 0，约 1 s） | Python 程序按脚本启动/停止，窗口与端口行为不变 |
| 无控制台 | “执行失败”（退出码 1，约 0.1 s） | “执行失败”（退出码 1） | Python 程序仍被启动/停止（`start`/`taskkill` 已生效） |

因此：**页面反馈只反映脚本调用结果**。“执行失败”不代表程序没起来，“执行完成”也不代表程序已就绪；
控制面板不修改脚本（规格要求保持现有脚本内容不变），也不假设“脚本没报错就是没出错”。
需要与手动运行脚本一致的反馈，请在控制台窗口里启动控制面板。

### 启动脚本打开的窗口

既有启动脚本用 `start` 打开两个独立窗口（标题 `My Streaming Worker` / `My Streaming Server`），
控制面板调用时同样出现**两个各自独立的可见顶层窗口**；本机由系统默认终端应用承载——交付前的真实验证中
观测到的是 Windows Terminal（`CASCADIA_HOSTING_WINDOW_CLASS`，注册表 `HKCU\Console\%%Startup` 两个委托项都是
全零 GUID，即“让 Windows 决定”），每个 Python 进程各有自己的 `conhost.exe`。是否与用户心目中的
“原有窗口行为”（例如经典 conhost 独立窗口）一致，需要人工验收确认（见[人工验收移交](#人工验收移交p2--p4未验收)）。

## 测试

```bat
cargo test --manifest-path backend\Cargo.toml
```

测试经 HTTP interface 请求操作，并用真实批处理（写入 `%CD%`、退出码等）验证调用与工作目录。
`backend/tests/execution_result.rs` 还验证执行结果分类：无法启动、成功退出、失败退出三类结果，
以及“脚本另开长期进程不参与完成判定”。`backend/tests/mutual_exclusion.rs` 用真实监听端口与**多个独立
HTTP 客户端**（各自一条 TCP 连接）验证单实例互斥：重叠请求立即得到忙碌结果、夹具只被调用一次、
当前操作结束后没有被延迟补做的调用，覆盖启动/启动、停止/停止、启动/停止与停止/启动。
`backend/tests/next_operation_after_finish.rs` 验证操作结束后的再次提交：成功结束后与失败结束后
（退出码 3）都能提交下一次操作且被真实执行；提交发生在前一次响应返回之后，因此不得被拒绝为
`409 operation-in-progress`；连续两次“启动”产生两次启动脚本调用——不检查服务是否已运行，也不做运行
状态去重。`backend/tests/connection_drop.rs` 用**真实连接中断**验证断线与操作生命周期分离：客户端
发出操作请求后，在脚本仍在运行时主动关闭那条 TCP 连接，夹具记录证明脚本继续执行到它自己的结束、
断线期间其他客户端得到忙碌结果、脚本结束后恢复接受操作，且断线不产生第二次调用、重放或补做。
`backend/tests/no_auto_termination.rs` 用同一个长跑夹具验证**不自动超时终止、不提前释放执行资格**：
脚本仍在执行时等待 75s（长于任何合理默认超时窗口），期间经独立 TCP 连接反复提交两个操作，
每次都立即得到 `409 operation-in-progress`，夹具进程始终是同一个 PID 且一直存活，记录里始终只有一条
`invoke`；随后经 `fixtures\stop-fixtures.bat` 人工结束，夹具按停止标记自行结束、资格释放并恢复接受操作。
**验证限度**：有限等待不能单独证明“永不超时”，它证明的是这段等待里没有任何阈值行为；
“不存在自动终止策略”另行由上面的执行生命周期检查补强。

## 首版限制

- **无身份验证**：任何能连上监听端口的设备都可以调用操作接口。只在可信局域网内使用，
  Windows 防火墙放行应限制在专用网络范围，不要暴露到公网。
- **单实例**：只运行一个控制面板进程；同一时间只执行一个脚本操作的约束以单实例为前提（多实例之间
  没有互斥）。同一实例内的重叠请求立即得到 `409 operation-in-progress`，不会被排队补做。
- 页面只反映脚本调用情况，不表示后台程序已经启动、已经就绪或已经彻底停止。
- 关闭脚本按端口强制终止进程树，可能误杀占用相同端口的其他程序（既有脚本行为，首版保留）。
  验收或使用前先确认 `8899`/`8888` 没有被无关程序占用（`netstat -ano | findstr LISTENING | findstr "8899 8888"`）。
- 脚本未结束时不会自动终止；卡住需要在 Windows 上人工处理（重启控制面板不能证明旧脚本已结束），
  具体步骤见[脚本卡住时的人工处理](#脚本卡住时的人工处理)。

## 交付验证记录（2026-09-30）

按本文档实际执行一次并观测。自动化验证主要使用隔离夹具配置
（`fixtures\operations\*`，见[隔离夹具](#隔离夹具fixtures的用途)）；此外为验证“退出码 1”修复，
另行在**控制台窗口**中启动面板并对真实脚本执行了一次启动/停止——执行前确认 `8899`/`8888` 空闲，
执行后环境恢复为停止状态，两个脚本内容与哈希未变。临时配置用后已删除，交付配置仍是仓库根的 `omp-bot.toml`。

| 步骤 | 执行的命令 | 观测结果 |
|---|---|---|
| 依赖 | `npm --prefix frontend install` | `up to date in 216ms` |
| 前端构建 | `npm --prefix frontend run build` | 17 个 module；产出 `backend/static/index.html`（408 B）、`assets/index-Dk_JJD5G.css`（1349 B）、`assets/index-DS9NApdi.js`（222168 B） |
| 后端构建 | `cargo build --release --manifest-path backend\Cargo.toml` | 成功，`backend\target\release\omp-bot.exe` 可用 |
| 启动（夹具配置） | `backend\target\release\omp-bot.exe --config <夹具配置>` | 打印配置文件、启动/停止绑定与工作目录、打包页面路径、`[omp-bot] 监听：http://0.0.0.0:8787/`、局域网访问提示 |
| 启动（交付默认配置） | `backend\target\release\omp-bot.exe`（默认读 `omp-bot.toml`） | 打印的真实绑定与本文档“默认绑定与改绑”表格一致；**不点按任何按钮**，8899/8888 无监听、无 `python.exe`；本机与局域网地址都能加载页面（与本机地址逐字节相同） |
| 本机地址 | `curl -D - http://127.0.0.1:8787/` | `HTTP/1.1 200 OK`、`content-type: text/html`、408 字节 |
| 局域网地址 | `curl -D - http://192.168.5.4:8787/` | `HTTP/1.1 200 OK`、408 字节，与本机地址的页面**逐字节相同**；`assets/…js`、`assets/…css` 经局域网下载后哈希与服务端 `backend/static` 构建产物相同 |
| 监听与连接 | `netstat -ano`、`ipconfig` | `TCP 0.0.0.0:8787 LISTENING`（非回环）；同时出现 `192.168.5.4:<临时端口> ↔ 192.168.5.4:8787` 的连接（`ipconfig` 中 WLAN 的 IPv4 为 `192.168.5.4`） |
| 浏览器加载 | Chromium 打开 `http://192.168.5.4:8787/`（375×667） | 标题与 `h1`「omp-bot 控制面板」、两个按钮均启用（158×69 px）、结果框「尚未提交操作。」（`data-outcome=idle`）、无横向滚动；截图 `.scratch/omp-bot-v1/evidence/14-delivery/p13-delivery-lan-375x667.png` |
| 操作接口（夹具） | `curl -X POST .../api/operations/start`、`.../api/operations/stop` | `200` + `{"outcome":"completed","exitCode":0}`、`200` + `{"outcome":"failed","failure":"failed-exit","exitCode":3}`；夹具记录 `fixtures/runtime/calls.tsv` 写入对应行，`cwd` 为配置的工作目录 |
| 越界请求 | `POST /api/operations/format`、带请求体的合法操作 | `404 unknown-operation`、`400 unexpected-parameters`，没有脚本被执行 |
| 卡住与互斥 | 夹具 `long-running.bat` + 第二个客户端请求 | 脚本运行期间第二个请求（经局域网地址）立即得到 `409 operation-in-progress` |
| 人工处理（首选） | `fixtures\stop-fixtures.bat` | 夹具写 `stop-observed` 并自行结束；卡住的请求返回 `{"outcome":"completed","exitCode":0}`，PID 文件被清理 |
| 人工处理（兜底） | `taskkill /PID <脚本 PID> /T /F` | 卡住的请求返回 `{"outcome":"failed","failure":"failed-exit"}`，随后 `POST .../stop` 被正常接受并真实执行 |
| 重启不清理 | 结束面板进程后 `tasklist /FI "PID eq <脚本 PID>"` | 脚本进程**仍在运行**；重启面板后页面为 `idle` 并立即接受新操作 |
| 真实脚本 + 控制台启动（“退出码 1”修复验证） | 在控制台窗口启动面板（默认 `omp-bot.toml`）→ `curl -X POST .../start`、`.../stop` | `start`：`200` + `{"exitCode":0,"outcome":"completed"}`（约 4.8 s），随后 `127.0.0.1:8899`、`0.0.0.0:8888` LISTENING、`worker.py`/`server.py` 运行、`My Streaming` 窗口出现；`stop`：`200` + `{"exitCode":0,"outcome":"completed"}`（约 0.5 s），两个 Python 进程与端口全部释放，窗口消失 |
| 无控制台启动（回退路径） | 无控制台启动面板 + `timeout` 探针批处理 | 启动日志打印“本次启动没有连接控制台…”提示；请求返回 `{"exitCode":1,"outcome":"failed","failure":"failed-exit"}`，如实反映脚本在该环境下的退出码 |
| 无自动终止复核 | “执行生命周期”一节的两条关键字复核命令（`findstr` / `grep`） | 两条都**没有任何输出**（退出码 1 = 无匹配），与文档一致 |
| 现有脚本未修改 | `sha256sum`、`git -C E:\Developer\myhub\my-streaming status --porcelain`、`rev-parse HEAD`、`hash-object` | 与实现票 12 的记录一致：`875b915a…` / `9b6720c5…`；工作区干净；HEAD `27f0a795…`；`517c03f0…` / `0f6c9eb7…` |
| 防火墙放行 | `netsh advfirewall firewall show rule name=omp-bot` | 两条入站允许规则（TCP、UDP），`配置文件=公用`、`程序=E:\developer\myhub\omp-bot\backend\target\release\omp-bot.exe` |
| 基线检查命令 | `netstat -ano` 配合 `findstr` 过滤 `LISTENING` 与 `8899`/`8888`；`tasklist /FI "IMAGENAME eq python.exe"` | 真实脚本验证前后各查一次：执行前 8899/8888 无监听者、无 `python.exe`；执行后再次确认无监听者、无 `python.exe`、无 `My Streaming` 窗口 |

## 人工验收移交（P2 / P4，未验收）

契约保留两项人工验收，**在用户逐条确认之前一律记为“未验收”**。自动化证据（窄屏仿真、夹具调用、
HTTP 观测、浏览器加载）**不构成**这两项的结论，也没有用替身或模拟屏幕替代真实运行证据。

### P2：真实手机连通与真机操作（未验收）

操作步骤：见[用手机操作（步骤与预期结果）](#用手机操作步骤与预期结果)（`ipconfig` 取地址 → 手机连同一
Wi-Fi → 打开 `http://<IPv4>:8787/` → 点按两个按钮）。请逐条确认：

- [ ] 手机能打开 `http://<IPv4>:8787/`（页面正常加载，不是空白或超时）。
- [ ] 两个按钮都可见、可用手指点按，不需要放大、不需要横屏、不需要外接鼠标/键盘。
- [ ] 点按“启动”：先出现执行中提示且两个按钮点不动，结束后能读到结果反馈。
- [ ] 点按“停止”：同样能读到结果反馈；停止后能再次点按“启动”。
- [ ] 窄屏下不需要横向滚动，反馈文字可读。
- [ ] 手机上看到的启动/停止反馈与桌面一致（从控制台窗口启动面板时为“执行完成”）。
- [ ] 横屏且浏览器工具栏占用高度时，结果反馈框的最后一行没有被工具栏永久遮挡。

预期结果与打不开时的排查见“用手机操作”一节。

证据位置（真机确认前均为**待确认**）：

- 窄屏断言与截图：`.scratch/omp-bot-v1/evidence/13-lan-phone/*.png`（含交付配置下仅加载页面的
  `delivery-375x667.png`、`delivery-320x568.png`；以及 `pending-*`、`busy-*`、`unreachable-*` 等状态截图）。
- 局域网地址加载、触摸点按、请求次数与不可达反馈的观测记录：
  `.scratch/omp-bot-v1/13-lan-phone-acceptance-draft.md`、
  `.scratch/omp-bot-v1/implementation/13-lan-phone-access.md`。
- 本次交付复核（同一局域网地址、同一构建产物、375×667）：
  `.scratch/omp-bot-v1/evidence/14-delivery/p13-delivery-lan-375x667.png`。
- 遗留项：浏览器动态工具栏可能遮挡反馈框末行属**未验证推论**（窄屏仿真无法复现），需真机横屏时看一眼，
  详见票 13 材料的“遗留风险与未验证项”。

### P4：现有脚本的实际控制与窗口行为（未验收）

操作步骤（会真的启动/停止后台程序，请在可接受该影响的环境执行）：

1. 在仓库根目录启动控制面板（默认 `omp-bot.toml`，绑定 `start-all.bat` / `shutdown-all.bat`），
   并记录基线：

   ```bat
   netstat -ano | findstr LISTENING | findstr "8899 8888"
   tasklist /FI "IMAGENAME eq python.exe"
   ```
2. 页面上点按“启动”，观察窗口、端口与程序是否起来。
3. 点按“停止”，再观察一次；停止后再次点按“启动”，应被正常接受。

请逐条确认：

- [ ] 点击“启动”后，现有 `start-all.bat` 实际启动了 `worker.py`（`127.0.0.1:8899` 监听）与
      `server.py`（`0.0.0.0:8888` 监听）。
- [ ] “启动”打开的窗口数量、标题、窗口内程序，与自己双击运行 `start-all.bat` 的原有行为一致。
- [ ] 点击“停止”后，现有 `shutdown-all.bat` 实际终止了两个 Python 进程树，端口释放、窗口关闭。
- [ ] “停止”的行为与窗口表现，与自己直接运行 `shutdown-all.bat` 的原有行为一致。
- [ ] 从控制台窗口启动面板时，启动与停止都显示“执行完成”（退出码 0），且结束方式与直接运行脚本一致。
- [ ] 认可[窗口由 Windows Terminal 承载](#启动脚本打开的窗口)以及首版限制（无认证、单实例、
      按端口强杀、卡住需人工处理）。

预期结果：

- 在控制台窗口启动面板的前提下：启动后出现两个可见窗口（`My Streaming Worker` / `My Streaming Server`），
  `8899`/`8888` 出现 LISTENING，页面结果框显示“执行完成”（退出码 0，约 5 s）。
- 停止后：两个窗口消失，`8899`/`8888` 不再监听，页面同样显示“执行完成”（约 1 s），而进程确已被终止。
- 若面板不是从控制台启动，结果会如实显示为“执行失败：脚本以退出码 1 结束”（原因见已知现象一节）；
  请在控制台窗口重新启动面板后复验。

证据位置（人工确认前均为**待确认**）：

- 默认绑定值、脚本内容未修改的证据：本文档“配置”“交付验证记录”。
- 真实调用时间线（HTTP 200、0.1 s 返回、退出码 1、端口/进程/窗口快照）、`timeout` 在非交互下的行为
  对照、与隔离夹具形状一致性：`.scratch/omp-bot-v1/12-real-script-acceptance-draft.md`（§5）、
  `.scratch/omp-bot-v1/implementation/12-real-script-integration-evidence.md`。
- 风险提示（按端口强杀、窗口由 Windows Terminal 承载）：同上材料的 §6。

### 尚未验收的声明

P2 与 P4 在用户逐条确认前保持**未验收**；上表自动化检查已全部执行，但它们只证明控制面板一侧的行为
与证据已备好，不证明真机连通、真机操作，也不替代对现有脚本实际控制与窗口行为的人工确认。
