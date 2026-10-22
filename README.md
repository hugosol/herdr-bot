# herdr-bot 主界面

在 Windows 上手动启动的网页主界面：本机浏览器与同一局域网内的手机访问**同一个服务**，
用固定按钮操作预先配置好的批处理脚本，也可以用一个固定主机名打开页面、并从页面关闭这台电脑。
herdr-bot 主界面只反馈脚本调用情况，不检查后台程序是否就绪。

- 首版范围（脚本操作、执行结果分类、互斥、局域网 IP 访问）：[`.scratch/omp-bot-v1/spec.md`](.scratch/omp-bot-v1/spec.md)、
  [`.scratch/omp-bot-v1/contract.md`](.scratch/omp-bot-v1/contract.md)。
- 当前新增（四个顶层入口、zone 登记列表）：[`.scratch/zone-file-browser/spec.md`](.scratch/zone-file-browser/spec.md)、
  [`.scratch/zone-file-browser/contract.md`](.scratch/zone-file-browser/contract.md)；Bot 模板管理与从
  zone 创建 Bot：[`.scratch/bot-template-creation/spec.md`](.scratch/bot-template-creation/spec.md)、
  [`.scratch/bot-template-creation/contract.md`](.scratch/bot-template-creation/contract.md)。
- 较早版本（当时的 mDNS 主机名 `omp-bot.local`、页面关机按钮）：[`.scratch/omp-bot-v2/spec.md`](.scratch/omp-bot-v2/spec.md)、
  [`.scratch/omp-bot-v2/contract.md`](.scratch/omp-bot-v2/contract.md)；
  决策记录见 [ADR 0001](docs/adr/0001-mdns-local-hostname-for-lan-access.md)、
  [ADR 0002](docs/adr/0002-shutdown-button-default-on-client-countdown-no-auth.md)、
  [ADR 0003](docs/adr/0003-shutdown-uses-exitwindowsex-forceifhung.md)、
  [ADR 0004](docs/adr/0004-session-write-boundary-same-host.md)（connect-bot 会话桥：写入边界与 v1 单实例假设）；
  术语见 [`GLOSSARY.md`](GLOSSARY.md)。

## 目录结构

| 路径 | 内容 |
|---|---|
| `backend/` | Rust/Axum 服务：提供打包页面与操作接口 |
| `backend/src/config.rs` | 服务端配置：监听地址、固定操作绑定（脚本 + 工作目录） |
| `backend/src/operations.rs` | 启动配置的执行目标并等待其结束（能否启动、如何结束） |
| `backend/src/zones.rs` | zone 登记列表的持久化：路径校验、原子写入与读取失败分类 |
| `backend/src/browse.rs` | zone 只读浏览：目录输出、内容识别、预览上限与根边界（含链接/目录联接）检查 |
| `backend/src/git.rs` | Git zone 状态与审查 diff：当前分支、待提交变更、重命名/冲突类别、角标计数，以及 HEAD→最终本地内容的只读比较（两侧限额） |
| `backend/src/sync.rs` | 远端检查与按既有 upstream/配置的 Sync：fetch → pull → 按需 push、仓库资格复用与失败短路 |
| `backend/src/watch.rs` | zone 本地文件/Git 元数据监听、按 zone 合并节流与监听健康上报 |
| `backend/src/mdns.rs` | 发布 mDNS 主机名 `herdr-bot.local` 与 `_http._tcp` 服务记录，并给出可断言的诊断输出 |
| `backend/src/shutdown.rs` | 本机关机动作（启用 `SeShutdownPrivilege` + `ExitWindowsEx`）与可替换的关机接缝 |
| `backend/static/` | 打包页面构建产物（由 `frontend` 构建生成，不入库） |
| `frontend/` | React 前端源码（Vite）；`frontend/src/config.ts` 是关机的构建期常量 |
| `frontend/src/FileListPanel.tsx` | zone 入口：zone 登记、列表读取、数字待提交角标、取消登记与进入浏览 |
| `frontend/src/ZoneBrowser.tsx` | zone 界面：只读浏览、Git 变更列表与左右/统一 diff、待提交着色与祖先目录提示 |
| `frontend/src/browseApi.ts` | 目录/文件读取客户端与响应形状校验 |
| `frontend/src/gitApi.ts` | Git 状态与 zone 角标概览客户端，含响应形状校验 |
| `frontend/src/zoneWatch.ts` | 自动更新 SSE 数据源：首帧监听快照、合并后的刷新通知与监听健康 |
| `frontend/src/diff.ts` | 行级统一 diff 与左右对照配对（纯文本，不解析 HTML） |
| `frontend/src/diffApi.ts` | 审查 diff 客户端与响应形状校验 |
| `fixtures/` | 隔离验证夹具（真实 Windows 批处理，供自动化检查绑定为操作目标；见 [fixtures/README.md](fixtures/README.md)） |
| `herdr-bot.toml` | 运行配置（默认读取当前工作目录下的这个文件） |

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
`cargo build --release` 生成 `backend\target\release\herdr-bot.exe`。

## 手动启动

在仓库根目录执行（默认读取 `herdr-bot.toml`）：

```bat
backend\target\release\herdr-bot.exe
```

需要换一份配置时：

```bat
backend\target\release\herdr-bot.exe --config 其他配置.toml
```

启动后控制台会打印监听地址、当前绑定的启动与停止脚本及其工作目录。
开发时也可以用 `cargo run --release --manifest-path backend\Cargo.toml`。

请在**控制台窗口**（cmd、PowerShell 或终端）中启动 herdr-bot 主界面：它会把自身的控制台继承给脚本进程，
脚本里 `timeout` 这类需要控制台输入的命令才会像你手动运行脚本时一样工作（详见
[现有脚本接入的已知现象](#现有脚本接入的已知现象)）。若在无控制台的环境启动，启动日志会打印提示，
脚本自身的这类命令会以非交互方式失败，操作结果会如实显示为该退出码。

## 访问（本机与局域网）

启动后控制台会打印三个入口：mDNS 主机名、本机地址与 IP 兜底提示。

```
[herdr-bot] 监听：http://0.0.0.0:8787/
[herdr-bot] 局域网设备请访问 http://<本机局域网 IPv4>:8787（ipconfig 查看地址）
[herdr-bot] mDNS 状态：published host=herdr-bot.local. address=192.168.5.4 port=8787 url=http://herdr-bot.local:8787/
```

| 入口 | 地址 | 用途 |
|---|---|---|
| 本机 | `http://127.0.0.1:8787/` | 在这台电脑上看页面，任何情况下都可用 |
| 局域网设备（推荐） | `http://herdr-bot.local:8787/` | 名字固定：换 Wi-Fi、重启路由器或 DHCP 重分配后也不用改地址 |
| 局域网设备（兜底） | `http://<本机局域网 IPv4>:8787/` | mDNS 解析不了时用（原因见下一节） |

### mDNS 主机名 `herdr-bot.local`（推荐入口）

herdr-bot 主界面启动时会在自己进程里发布 mDNS 主机名 `herdr-bot.local`（以及 `_http._tcp` 服务记录），
局域网设备用 `http://herdr-bot.local:<端口>/` 就能打开同一个页面。发布地址默认是本机主路由的 IPv4
（自动排除回环与 `169.254.0.0/16`），也可以用 `[mdns] address` 指定；端口取实际监听端口——以启动日志
`published` 那一行为准：

```
[herdr-bot] mDNS 状态：published host=herdr-bot.local. address=<IPv4> port=<端口> url=http://herdr-bot.local:<端口>/
```

**`.local` 能不能解析，取决于客户端和网络，不由本服务决定**——用这条路线前先看这段：

- **Windows**：`.local` 由系统的 mDNS 解析器处理，能否解析受系统解析策略与是否装了 Bonjour 之类的
  解析器影响。**本机（开发/验收这台）装了 Apple Bonjour，实测可解析**；没有 Bonjour 的 Windows 机器
  **不保证**能解析。
- **Android 13 之前的设备**不原生支持 mDNS 解析；Android 13+ 才具备系统级 `.local` 解析能力。
- **网络**：启用组播/客户端隔离（AP 隔离）、访客网络、公司网络的客户端隔离会挡住 mDNS 组播，
  名字也就解析不到。
- 结论：**能解析就用名字，解析不了就用 IP 兜底**。三条入口同时有效；本机 `127.0.0.1` 永远可用，
  mDNS 发布失败或被关掉也不会影响 IP 入口。

#### IP 兜底的具体做法

1. 在电脑上查局域网地址：

   ```bat
   ipconfig
   ```

   取**无线局域网适配器 WLAN**（或以太网）那一节的 `IPv4 地址`（例如 `192.168.5.4`）；也可以直接从
   启动日志的 `published … address=<IPv4>` 一行读出发布的地址。
2. 手机连**与电脑同一个 Wi-Fi**（不要用访客网络或热点），浏览器打开 `http://<上一步的 IPv4>:8787/`。
3. 这个地址不是固定的：换 Wi-Fi、重启路由器或 DHCP 重新分配后会变，变了重新 `ipconfig` 确认即可；
   端口固定为配置里的 `8787`。**不要在手机上用 `127.0.0.1`**（那指向手机自己）。

页面与操作接口**同源同端口**，都由这一个服务提供：手机不需要单独访问前端开发服务器，也不需要额外的
端口转发；照常打开页面即可，按钮请求打到同一地址的 `/api/operations/start`、`/api/operations/stop`，
关机按钮打到 `/api/shutdown/start`（手动开始意图）与 `/api/shutdown/cancel`（全局取消）。

### 用手机操作（步骤与预期结果）

真机连通与真机操作只能由**真实手机**确认，自动化验证（Chromium 窄屏仿真）不构成真机结论；当前状态
见[人工验收移交](#人工验收移交p2--p4未验收)。手机浏览器也可以直接打开 `http://herdr-bot.local:8787/`
（见上一节；解析不了就照下面的 IP 步骤做）。

1. 电脑侧按“构建”“手动启动”启动 herdr-bot 主界面，确认控制台打印了监听地址。
2. 在电脑上查局域网地址：

   ```bat
   ipconfig
   ```

   取**无线局域网适配器 WLAN**（或以太网）那一节的 `IPv4 地址`（例如 `192.168.5.4`）。
   这个地址不是固定的：换 Wi-Fi、重启路由器或 DHCP 重新分配后会变，重新用 `ipconfig` 确认即可；
   端口固定为配置里的 `8787`。**不要在手机上用 `127.0.0.1`**（那指向手机自己）。
3. 手机连接**与电脑同一个 Wi-Fi**（不要用访客网络或热点）。
4. 手机浏览器打开 `http://<上一步的 IPv4>:8787/`。
   - **预期看到**：标题“herdr-bot”、四个等宽入口（`chat`、`bot`、`zone`、`server`），
     默认停在 `chat`。
5. 点按 `server`，再点按“启动”：先出现“正在执行启动脚本……本次操作尚未结束。”，此时
   **两个按钮同时变灰、点不动**；脚本结束后显示本次调用结果。从**控制台窗口**启动面板时，
   默认绑定的现有脚本会显示“执行完成”（约 5 秒，脚本自身的等待会真正执行）；若面板是在没有控制台
   的环境里启动的，则会如实显示“执行失败：脚本以退出码 1 结束”，原因见
   [现有脚本接入的已知现象](#现有脚本接入的已知现象)。
6. 点按“停止”：同样先“执行中”、后结果；停止之后可以再次点按“启动”，会再次真实调用脚本
   （不检查服务是否已运行，也不做状态去重）。
7. 点按 `zone`：输入**服务器电脑上的**目录绝对路径（不是手机上的目录），点按“登记”；成功只会在
   后端持久保存之后显示。点按某个登记项的“取消登记”只移除登记，不删除目录或文件。

手机打不开页面时：

| 现象 | 检查 |
|---|---|
| 一直转圈/无法访问 | 手机是否与电脑同一 Wi-Fi；Windows 防火墙是否放行监听程序。已在电脑上核实（`netsh advfirewall firewall show rule name=herdr-bot`）：两条**入站允许**规则（TCP 与 UDP）、`配置文件=公用`、`程序=E:\developer\myhub\herdr-bot\backend\target\release\herdr-bot.exe`、任意端口；当前 WLAN 的网络类别正是“公用”，所以该规则覆盖这个地址 |
| 换了 exe 路径 / 重命名 / 换了网络类别 | 需要重新放行（管理员命令提示符）：`netsh advfirewall firewall add rule name="herdr-bot" dir=in action=allow program="<herdr-bot.exe 的绝对路径>" enable=yes profile=any` |
| 电脑上能开、手机上不能 | 路由器“AP 隔离/客户端隔离”、访客网络、手机 VPN/代理、公司网络的客户端隔离都会阻断，换普通家庭 Wi-Fi 再试 |
| 服务起不来 | 控制台打印“绑定监听地址失败”表示端口被占用：换配置里的端口，并同步改手机地址里的端口 |
| 页面能打开但按钮点了没反应 | 页面与接口同源同端口；若反向代理/端口转发只转发了页面，接口会失败（页面显示“无法确认本次调用结果”）。直接用 `http://<IPv4>:8787/` 访问 |

## 配置

`herdr-bot.toml`：

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

### `[mdns]` 配置（可选）

mDNS 发布只有两个开关，**整段可以缺省**（不写 `[mdns]` 就等于默认值：启用、地址自动选）：

```toml
[mdns]
enabled = true             # 缺省就是 true：不写这一段也会发布
address = "192.168.5.4"    # 可选：指定要发布的 IPv4；缺省自动选主路由地址
```

| 字段 | 缺省 | 含义 |
|---|---|---|
| `enabled` | `true` | `false` 时完全不发布 mDNS 记录，启动日志给出 `mDNS 状态：disabled`；HTTP 服务与 IP 入口照常可用 |
| `address` | 自动选 | 指定要发布的 IPv4。**适用场景**：多网卡/多网段，或自动选出的地址不是你想让手机用的那块网卡（自动选择只取主路由 IPv4，排除回环与 `169.254.0.0/16`） |

- `address` 必须是**本机网卡上真实存在的地址**：值本身不可发布（回环、`169.254.0.0/16`）→ 启动日志
  `no-address`；不在本机网卡上（即使是非回环公网地址）→ 启动日志 `error`。两种情况都**跳过发布**，
  HTTP 与 IP 入口照常可用（本次交付实测见下文「交付验证记录（v2，2026-09-30）」）。
- 主机名固定为 `herdr-bot.local`，**没有**改名配置项。局域网里已有别的设备占用这个名字时**不改名**：
  注册前探测到占用 → 不发布并告警（启动日志 `conflict phase=preprobe`）；注册后才发现被改名 → 立即注销
  记录并告警（`conflict phase=renamed`）。**绝不发布** `herdr-bot-2.local` 之类的名字。
- 配置解析保持 `deny_unknown_fields`：`[mdns]` 里写错字段名会启动失败并给出解析错误，不会静默忽略。
- 关掉 mDNS 只影响“名字入口”，不影响 IP 入口，也不影响页面与任何接口（含关机端点）。
- 诊断类别固定为 `published` / `disabled` / `no-address` / `conflict` / `error`（行首 `[herdr-bot] mDNS 状态：`），
  发布结果因此可以直接从启动输出断言。

### `[sessions]` 配置（可选）

会话登记的两个窗口在**启动期**读取一次，改了要重启后端（没有热加载）：

```toml
[sessions]
# 在线有效期：从最后一次被后端接受的 register／heartbeat／state 写入起算；缺省 10。
online_timeout_seconds = 10
# 离线保留期：上报过期后登记仍保留供观察与恢复的时长；缺省 30。
offline_retention_seconds = 30
```

| 字段 | 缺省 | 含义 |
|---|---|---|
| `online_timeout_seconds` | `10` | 在线有效期（秒）：上报年龄不超过它时该会话是**在线** |
| `offline_retention_seconds` | `30` | 离线保留期（秒）：超过在线有效期但在两个窗口之和以内时是**离线**且登记仍保留 |

- 两个字段**可以分别缺省**：只写一个时另一个取上表默认值；整个 `[sessions]` 段缺省等于两个都取默认
  （在线 10 秒／离线保留 30 秒，即上报年龄超过 40 秒后由后端维护清除登记）。
- 合法整数范围都是 `1..=4294967295`：零、负数、小数、字符串、布尔值、超过上界，或拼错的字段名／段名都会让
  **启动明确失败**并指出出错的配置，不会静默回退到缺省值。两个合法窗口之和（最大 `8589934590` 秒）按总阈值
  正常判定，不溢出、不提前过期。
- 运行中的实例不会因为改写配置文件而改变已加载的窗口；新值只在**重启后**生效。
- 阈值语义与可见行为见[会话登记的生命周期](#会话登记的生命周期chat-入口)。

### `[herdr]` 配置（必须显式配置）

从 zone 创建 Bot 时，每个新 Tab 都创建在这个服务端明确配置的 Herdr Workspace 中。这一段**没有
代码缺省值**：

```toml
[herdr]
workspace_name = 'bot-playground'
# executable = 'C:\Users\<you>\.herdr\packages\standalone\releases\<version>\herdr.exe'
```

| 字段 | 缺省 | 含义 |
|---|---|---|
| `workspace_name` | **无（必须显式配置）** | 目标 Herdr Workspace 的显示名；每次创建按它精确匹配且必须唯一 |
| `executable` | `herdr` | 生产调用使用的 Herdr CLI；缺省按 `PATH` 查找 |

- 整段缺失、`workspace_name` 为空字符串或纯空白都会让**进程启动失败**，不会回退到 `bot-playground`
  或其他猜测目标。
- 浏览器请求没有 Workspace 字段，不能替换这个目标；创建时即使 Herdr 当前焦点在别的 Workspace，
  也仍按配置名称精确查找。匹配到零个或多个时明确失败，不任取第一个、不新建 Workspace。
- 合法值去掉首尾空白后按字面保存；拼错的字段名会让启动失败。

### `[zones]` 配置（可选）

zone 登记列表由后端以文件持久保存，本机与局域网设备读取同一份；整段可缺省：

```toml
[zones]
# 新 zone 的创建父目录。必须显式配置；缺省未配置时只禁用“新增”。
create_parent_directory = 'E:\Developer\bot-playground'
# 登记文件位置；缺省为进程工作目录下的 zones.json。
registry_file = 'E:\data\herdr-bot-zones.json'
# 单文件文本预览上限；缺省 1048576 字节（1 MiB）。
preview_limit_bytes = 1048576
# 每个 zone 自动刷新合并事件的最小间隔；缺省 10 秒。
refresh_interval_seconds = 10
```

| 字段 | 缺省 | 含义 |
|---|---|---|
| `create_parent_directory` | 无（必须显式配置） | 新建 zone 的唯一父目录；必须在服务器上已存在 |
| `registry_file` | `zones.json` | 登记列表的持久文件；相对路径按进程工作目录解析 |
| `preview_limit_bytes` | `1048576` | 单个文件的文本预览上限（字节）；超限不加载完整内容 |
| `refresh_interval_seconds` | `10` | 每个 zone 合并文件事件并自动刷新的最小间隔（秒） |

- 登记文件不存在表示“还没有任何登记”，不是读取错误；列表在真实进程重启后仍然保留。
- 登记成功只表示登记文件已经成功写入（先写临时文件并落盘，再原子替换）。写入失败、读取失败或文件内容
  损坏时，接口返回明确错误，**不会伪装成空列表**，也不会覆盖原文件；`GET /api/zones` 再次读取与重启后
  看到的仍是原内容。
- 登记路径必须是服务器上的**绝对**路径，且指向真实存在的目录；普通目录与 Git 工作树根目录都可登记。
  登记已有目录不初始化 Git，也不扫描或聚合嵌套仓库；只有“新增 zone”会创建目录并执行 `git init`。
- 取消登记只移除登记项，不删除目录或文件；目录在登记后被外部删除时仍可取消登记。
- `create_parent_directory` 没有代码内静默兜底。旧配置缺少该字段时服务仍可启动，只是“新增”按钮禁用并提示
  配置字段。配置父目录缺失、不是目录或不是绝对路径时，创建请求明确失败，不自动创建父目录。
- 新 zone 名称只能是配置父目录下的一个直接子目录名，不能包含路径分隔符、`..` 或 Windows 设备名；
  同名目录已存在时拒绝，不覆盖、不复用。
- 创建顺序是**目录 → `git init` → 持久化登记**。全部成功后才显示成功并加入列表；不生成 README、首次提交
  或远程仓库。中途失败保留已产生的目录：失败反馈说明失败阶段与完整路径；若目录与 Git 初始化已完成而只有
  登记失败，可用“登记”入口按已有目录重新登记。
- `preview_limit_bytes` 必须是正整数；零、负数、小数、字符串或拼错的字段名都会让**启动明确失败**，
  不会静默回退到 1 MiB。缺省值只在字段或整段缺失时使用。
- `refresh_interval_seconds` 必须是正整数秒；零、负数、小数、字符串或拼错的字段名同样让**启动明确失败**，
  不会静默回退到 10 秒。缺省值只在字段或整段缺失时使用；运行中改写配置不会改变已启动实例。
- 上限只限制**内容预览**：不影响目录列表、文件可见性或元数据；恰好等于上限的文件完整显示，超过一个字节
  就只显示大小与上限说明。

### `[bot_templates]` 配置（可选）

Bot 模板由后端以文件持久保存，电脑与手机读取同一份；整段可缺省：

```toml
[bot_templates]
# 模板存储文件位置；缺省为进程工作目录下的 bot-templates.json。
storage_file = 'E:\data\herdr-bot-templates.json'
# 手工维护的选项元数据，与运行数据分离。
metadata_file = 'meta-data/bot-options.toml'
```

| 字段 | 缺省 | 含义 |
|---|---|---|
| `storage_file` | `bot-templates.json` | Bot 模板的持久文件；相对路径按进程工作目录解析 |
| `metadata_file` | `meta-data/bot-options.toml` | 必须存在的 TOML 选项目录；相对路径同样按进程工作目录解析 |

- 文本文件不存在表示还没有任何模板，不是读取错误；模板在真实进程重启后仍然保留。
- 保存成功只表示存储文件已经成功写入（先写临时文件并落盘，再原子替换）。写入失败、读取失败或内容
  损坏时，接口返回明确错误，**不会伪装成空列表**，也不会覆盖原文件；页面不会把失败的保存显示成
  “已保存”。
- 拼错的字段名或段名会让**启动明确失败**，不会静默回退到缺省路径。

### `[git]` 配置（可选）

本功能发起的每条 Git 命令都有执行时限；整段可缺省：

```toml
[git]
# 每条 Git 命令的执行上限（秒）；缺省 300。
command_timeout_seconds = 300
```

| 字段 | 缺省 | 含义 |
|---|---|---|
| `command_timeout_seconds` | `300` | 每条 Git 命令允许执行的最长时间（秒） |

- 时限覆盖状态读取、diff、暂存、提交、远端检查以及同步中的每一条 Git 命令。到达时限时本功能结束整棵命令进程树
  （包括 hook 派生出的子进程），停止本次执行并中断后续步骤；页面明确显示这是本功能时限，并保留取消前
  已有的 Git 输出，不伪造 Git 退出消息。
- 终止尚未确认时，该仓库的写操作资格继续被持有，后来的 Commit 不会与它重叠；确认终止后才释放。
- 合法整数范围是 `1..=4294967295`；缺段或缺少字段取 300 秒。零、负数、小数、字符串或拼错的字段名
  都会让**启动明确失败**，不会静默回退。
- 合法的慢操作可能被较短时限打断；需要更长时间时调大该值并重启。运行中改写配置不会改变已加载的实例。

### Bot 模板

主界面提供独立的 `bot` 入口，用于新增、查看、编辑与删除模板；模板由后端持久保存，电脑与手机
读取同一份。顶层顺序为 `chat → bot → zone → server`；每次打开或整页刷新仍从 `chat` 开始，
不恢复上次 tab。删除模板只移除模板，不改变任何已经创建的 Bot。

模板字段与固定规则：

- `name` 必填，是模板显示名，不是新 Bot 的固定名字。
- `bot-name` 必填且必须是单行规则；初始规则为 `{zone.name} · {template.name} · {bot.id}`。
  编辑时可插入四个已承诺字段 `{zone.name}`、`{template.name}`、`{agent}`、`{bot.id}` 并查看示例预览；
  页面不扩展未知占位符的产品承诺。模板列表显示用示例 zone `alpha` 与示例短标识 `a7f3c2` 渲染后的
  bot-name 示例，不显示原始规则；规则只在编辑弹窗中维护。
- `agent` 只能是 `omp` 或 `pi`。
- `model` 只能沿用 Agent 默认值、`gpt-6.1-sol` 或 `deepseek-flash`，没有自由输入。
- `thinking` 只提供当前 agent 与具体模型支持的固定级别；model 沿用默认值时 thinking 也只能沿用
  默认值。切换 agent 会把 model 与 thinking 重置为默认值；切换 model 时保留仍适用的 thinking，
  否则重置并提示。
- `skill` 可不选，选择时只能是 `/teach` 或 `/code-review`；不允许任意 Skill 文本。
- `prompt` 可多行、可为空；不增加 Skill / prompt / input 的非空组合校验。

以上是随附 `meta-data/bot-options.toml` 的初始选项，不是代码中的备用清单。初始能力事实依据
OMP 模型元数据 `thinking.efforts` 与 Pi `getSupportedThinkingLevels`，不是 CLI 可接受级别。
OMP GPT 为 low/medium/high/xhigh/max、DeepSeek 为 low/high/max；Pi GPT 为
minimal/low/medium/high/xhigh/max、DeepSeek 为 off/low/high/max。显式模型分别使用完整的
`openai-codex/gpt-6.1-sol` 与 `deepseek/deepseek-flash`，不做裸名匹配或 provider 回退。

#### 维护共享选项目录

直接编辑该 TOML 文档；它可纳入 Git，不属于运行数据。部署时须一并交付，程序不生成替代文件。
服务启动先完整读取并校验；缺失、读取失败、TOML 错误、重复标识、未知 Agent 引用或相互矛盾的
默认模型关系均阻止启动，并报告文件位置及原因。运行中每次重新打开模板编辑器、保存模板和接受
创建都重新读取当前文件；不需重启，没有文件监听或已打开页面实时推送，也没有旧缓存回退。

- `[[agents]]`：原生 Agent `id`（omp/pi）与 `label`；可维护顺序、显示与可用集合，不增加第三种原生 Agent。
- `[[models]]`：稳定 `id`、`label`、完整 `provider_model`；`thinking` 的各 Agent key 同时定义适用关系
  与显式级别，例如 `thinking = { omp = ['low', 'high'], pi = ['off', 'high'] }`。`default` 必须存在且
  不得携带模型或 thinking 覆盖；每组显式级别之外始终可选沿用默认。
- `[[skills]]`：稳定 `id`、`label`、原生 `native_name`；例如 teach 映射到 `/skill:teach`。
  不选仍为 null，不增加必选或空正文限制。
- `[[extensions]]`：唯一稳定 `id`、`label`、适用 `agent`、`kind`（fixed/optional）与 `entrypoint`。
  同名扩展在不同 Agent 下用不同标识；各 Agent 至少定义一个固定扩展。元数据结构校验不执行入口。

模板只持久保存选择标识与原有字段，不保存 provider/model、能力表或 Skill 原生映射副本。
已有模板不必重存即可在下次创建使用新映射；接受创建时固定本次模板和完整模型/Skill 映射，后续编辑
不改变已接受或已有的 Bot。公开 `options` 包含 `agents/models/skills` 标识列表、`thinking[agent][model]`、
`labels` 显示信息与 `extensions` 定义；前端选项、公开保存校验、原生启动解释共用这个文档。

接口：

- `GET /api/bot-templates` 读取完整列表与当前选项（`templates`、`options`）。
- `POST /api/bot-templates` 新增模板，JSON 字段为
  `{"name":"…","botName":"…","agent":"omp","model":"default","thinking":"default","skill":null,"prompt":"…"}`。
- `GET /api/bot-templates/{id}` 按标识读取单个模板；已删除的模板返回 `template-not-found`。
- `PUT /api/bot-templates/{id}` 编辑模板；标识保持不变。
- `DELETE /api/bot-templates/{id}` 删除模板。

列表及写入成功响应附带当前完整模板列表与当前选项。元数据错误返回 `metadata-invalid`，消息包含位置和原因，不使用旧缓存。其他失败响应是 `{"code":"…","message":"…"}`；字段与能力约束的
类别包括 `template-name-required`、`template-bot-name-required`、
`template-bot-name-not-single-line`、`template-agent-invalid`、`template-model-invalid`、
`template-thinking-invalid`、`template-thinking-requires-default-model`、
`template-thinking-unsupported` 与 `template-skill-invalid`；存储错误包括 `templates-unreadable`、
`templates-corrupt` 与 `templates-write-failed`。保存失败时此前已保存的模板保持不变。

### 从 zone 创建 Bot

`zone` 入口的每个 zone 行都有一个独立的“+”入口。打开弹窗后选择模板、填写可为空的初始 input；
弹窗只显示后端预览的本次名称，**没有手工命名输入**。确认后由后端执行创建，页面不再自己计算名称：

1. 按 `[herdr] workspace_name` 精确查找唯一 Workspace。当前焦点不会影响选择；零个或多个匹配都失败。
2. 在目标 Workspace 中新建独立 Tab，显示名严格使用模板 `bot-name` 的结果，cwd 显式设为该 zone 的目录。
3. 在初始 Pane 中启动模板指定的 agent（omp / pi），等待 Herdr 确认它已可交互。
4. 就绪后才把 Skill、模板 prompt 与本次 input 合成一次初始指示提交；不先单独发起 Skill 轮次。
5. 初始指示提交被确认后立即显示“创建成功，初始指示已提交”，不等待模型完成任务、Skill 执行或
   connect-bot 接入。

页面把过程分成“正在启动”与“正在提交初始指示”两个稳定阶段。只有就绪与提交都确认才显示成功；
失败或超时会保留已经创建的 Tab / agent 与已知标识，显示失败阶段，不自动删除、重建、重发。提交被
明确拒绝与投递超时是两种事实：前者说明 agent 已就绪但提交失败；后者只能说明投递结果未确认，不能
断定未发送，也不会显示成功。成功与失败都留在弹窗中由用户关闭，不自动切换到 `chat`。创建 Bot 不
登记 connect-bot 会话，也不修改会话监听、keepAlive 或关机安排。

创建由后端拥有：公开入口接受后，浏览器断开、刷新或关闭只让这个页面失去观察，不会取消已经接受的
创建；刷新后也不承诺找回这次结果。电脑与手机等独立客户端同时提交时，每次创建分别被接受并独立执行，
各自保留自己的 Tab / Pane、agent 控制别名、接受时的模板快照与 input，不合并、不复用，也不因此共享
固定操作的执行资格或增加全局忙碌拒绝、排队或并发上限。

接口：

- `POST /api/bots/preview`，JSON 为 `{"zone":"…","templateId":"…"}`，返回本次短实例标识与实际会使用的
  名称：`{"botId":"a7f3c2","name":"alpha · 审查 · a7f3c2","zone":"…","templateId":"…","agent":"omp"}`。
- `POST /api/bots`，JSON 为
  `{"zone":"…","templateId":"…","botId":"a7f3c2","input":"…","expectedName":"…"}`；接受后返回 HTTP 202
  与创建生命周期（`id`、`name`、`zone`、`stage:"starting"` 等）。请求没有 Workspace 字段；模板在请求
  被接受时固定为快照。若 `expectedName` 与当前模板规则算出的名称不同，返回 409 `name-preview-stale`，
  不会悄悄改名。
- `GET /api/bots/{id}` 读取同一次创建的当前阶段：`starting`、`submitting`、`succeeded` 或 `failed`。
  已取得的 `tabId` / `paneId` / `agentName` 只在确实取得后出现；失败时附加
  `failure:{stage,code,message}`，`inputSubmitted` 只有在提交被确认后才为 `true`。

`bot.id` 是显示名中的短实例标识（6 个小写十六进制字符），不是 Herdr Tab ID，也不充当对象身份。
Tab 显示名允许重复；内部对象身份使用 Herdr 返回的 Tab / Pane 标识与后端单独生成的 agent 控制别名。
名称与 input 经逐参数进程调用传给 Herdr CLI，不经过 Shell，因此引号、中文、空格、换行与 Shell
特殊字符都按字面传递。

**真实运行的人工验收准备（P25 / P26 / P27 / P33）**：自动用例使用受控 Herdr 替身，不能代替真实
Herdr / agent。用户确认时可以：

1. 在 `herdr-bot.toml` 明确写 `[herdr] workspace_name = 'bot-playground'`（或本次显式目标），并确认
   真实 Herdr 已在运行；在 Herdr 中让焦点停在另一个 Workspace。
2. 预先登记一个普通目录与一个 Git 工作树目录作为两个 zone；分别从它们的“+”创建，记录结果中的 Bot
   名称、Tab / Pane 标识。
3. 用独立 Herdr 命令核对对象：`herdr workspace list` 找到配置目标；`herdr tab list` / `herdr pane get`
   找到结果中的标识；`herdr pane process-info --pane <paneId>` 或 `herdr agent get <paneId>` 读取实际
   cwd，应与所选 zone 目录一致，而不是焦点 Workspace 的目录。
4. 观察页面先显示“正在启动”、目标可交互后才显示“正在提交初始指示”，最后才显示“创建成功，初始指示已
   提交”；成功出现时手动终端里应已收到一次初始输入。创建完成不要求模型任务结束，也不应在 `chat`
   入口中出现由本次创建自动生成的 connect-bot 登记。

**四组 agent / model 组合的人工验收准备（P14 / P16 / P17 / P18）**：自动用例只记录受控 Herdr 替身
收到的启动参数，不能替代真实 Herdr / agent；真实账号、登录与费用由用户最终确认。与 02 / 04 / 05
共用同一轮四组场景，本组负责目标 agent 类型、完整模型标识和 thinking 级别。

| 组 | agent | model | thinking | Skill（04 准备） | 预期原生启动参数 |
|---|---|---|---|---|---|
| A | omp | gpt-6.1-sol | high | /teach | `--model openai-codex/gpt-6.1-sol --thinking high` |
| B | omp | deepseek-flash | max | /code-review | `--model deepseek/deepseek-flash --thinking max` |
| C | pi | gpt-6.1-sol | xhigh | /code-review | `--model openai-codex/gpt-6.1-sol --thinking xhigh` |
| D | pi | deepseek-flash | low | /teach | `--model deepseek/deepseek-flash --thinking low` |

能力事实取自目标版本：OMP 18.12.4 的 `thinking.efforts` 对 gpt-6.1-sol 为 low..max、对
deepseek-flash 为 low/high/max；Pi 1.3.0 的 `getSupportedThinkingLevels` 对 gpt-6.1-sol 为
minimal/low/medium/high/xhigh/max（没有 off）、对 deepseek-flash 为 off/low/high/max。Pi 不接受
auto。默认 model 时不会传 model 与 thinking；显式 model 搭配默认 thinking 时只传 model。

1. 按上一节准备两个 zone 与真实 Herdr；在 `bot` 入口保存四个模板，字段按上表填写（Skill 列由
   04 的验收准备决定，不改变本表的 agent / model / thinking）。
2. 依次从 zone 行的“+”创建 A–D；A/C 使用普通目录 zone，B/D 使用 Git 工作树 zone；每次填入不同的
   字面 input（如“组 A 初始任务”），记录结果中的 Bot 名称、zone、`tabId`、`paneId` 与 `agentName`。
3. 对每组用独立 Herdr 命令核对：`herdr workspace list` 找配置目标；`herdr tab list` /
   `herdr pane get <paneId>` 确认对象位置；`herdr pane process-info --pane <paneId>` 的
   `foreground_processes[].argv` / `cmdline` 必须包含上表的完整 `--model` 与 `--thinking` 参数，
   同一条记录的 `cwd` 必须等于该 zone 的目录；`herdr agent get <paneId>` 应显示与所选 omp / pi
   一致的目标类型。
4. 核对终端：初始指示只出现一次，且是 Skill 前缀 + 固定 prompt + 本次 input 的合成文本（Skill 细节由
   04 验收）。账号未登录、模型无权限或终端未就绪时，如实记录实际错误；这不改变“创建成功”只表示
   已就绪且提交被接受的语义，也不允许改用其他 provider / model 自动回退。
5. 预期完整模型配置即上表最后一列；没有取得 `argv` / `cmdline` 时标记为未验证，不用页面文本或受控
   记录代填。

本票已完成的自动检查（受控 Herdr 记录，不等于真实运行）：`cargo test --manifest-path backend\Cargo.toml
--test bot_creation`（12 passed；覆盖四种显式组合与两个默认组合，核对 `agent_kind`、完整启动参数、
启动先于提交、仅一次提交）、`cargo test --manifest-path backend\Cargo.toml --test bot_templates`
（8 passed；能力选项与保存校验使用目标 Pi 1.3.0 能力事实）、`npm --prefix frontend run test`
（176 passed）。完整后端套件（327 passed、0 failed、3 ignored）与真实 Chrome 浏览器套件
（5 passed）也已在本次交付中通过；这些仍是受控替身 / 自动化证据，不是真实 Herdr / agent 的运行结果。

### zone 入口与 zone 登记

主界面顶层入口按 `chat → bot → zone → server` 排列；每次打开或整页刷新都从 `chat` 开始，
不恢复上次 tab。四个入口等宽，支持 Tab/Enter/Space 与方向键，左右滑动不切换顶层 tab。
主页面始终从顶部排布，保留原有顶部内边距与水平居中；内容不足一屏时不再垂直居中，超过一屏时正常滚动。

> **可信局域网提醒**：本功能没有登录或来源认证，沿用“仅可信局域网”的部署前提。任何能访问本服务
> 端口的设备都可以读取已登记 zone 中的文件内容（包括敏感文件），并可以触发已开放的
> Commit / Sync 写操作。会话事件写入的同机来源校验只限制那一类写入，**不等于认证**，也不会限制
> 手机使用文件浏览与 Git 操作。本版没有增加登录、多用户权限或公网部署能力，也没有任意命令、脚本
> 或终端入口；不要把本服务暴露到公网或不可信网络。

`zone` 入口直接展示 zone 列表和“登记”“新增”两个按钮；每行只显示末级目录名与数字待提交角标，
完整路径保留在浏览入口的悬停标题与无障碍名称中，并在进入 zone 后由浏览工具栏显示。两个按钮都不在
页面上常驻输入框，而是点按后打开弹窗，确认后才发送请求。

登记已有目录：

1. 点按“登记”，在弹窗中输入**herdr-bot 服务所在电脑上的**绝对目录路径（不是手机或浏览器所在设备的目录）。
   页面不提供客户端文件选择器。
2. 点按弹窗里的“登记”。后端校验路径并持久保存；只有保存成功后页面才关闭弹窗并显示“已登记”。
3. 已登记的 zone 显示在同一列表中；另一台设备进入 `zone` 入口时会重新读取后端列表，因此看到同一份
   登记。
4. 每个登记项的“取消登记”只移除登记，不删除目录或文件。

新增 zone：

1. 点按“新增”，弹窗只接受一个直接子目录名称，并显示配置的父目录。父目录来自服务端配置，客户端不能替换。
2. 确认后，服务端在父目录下创建目录、执行 `git init`，再写入登记列表；全部成功后才关闭弹窗并显示创建路径。
3. 未配置 `create_parent_directory` 时“新增”禁用并显示配置提示；“登记”不受影响。
4. 非法名称、父目录缺失或目标已存在都会被明确拒绝；中途失败保留已产生的目录并显示失败阶段与完整路径。
   同名目录不会覆盖或复用。

失败反馈保持明确：非法路径、重复登记、存储读取/写入失败或登记文件损坏都会显示后端返回的类别说明；
请求没有得到响应时页面显示“无法确认”，既不当成成功，也不当成失败或空列表。

接口：

- `GET /api/zones` 读取列表。
- `POST /api/zones` 登记已有目录（JSON `{"path":"服务器绝对路径"}`）。
- `POST /api/zones/create` 新建 zone（JSON `{"name":"直接子目录名称"}`）。
- `POST /api/zones/remove` 取消登记（JSON `{"path":"服务器绝对路径"}`）。

成功响应都是当前完整列表与创建能力：
`{"zones":[{"path":"…"}],"creation":{"available":true,"parentDirectory":"…"}}`。未配置父目录时
`creation` 为 `{"available":false}`，不返回伪造的父目录。失败响应是
`{"code":"…","message":"…"}`；创建失败类别包括 `create-parent-not-configured`、
`create-parent-not-found`、`invalid-name`、`name-already-exists`、`git-init-failed` 与
`zone-registration-failed` 等。

### 进入 zone：只读浏览与 Git 待提交变更

登记项旁的“浏览”进入该 zone。普通目录只提供只读文件浏览；Git 工作树另有「文件浏览」「变更」两个
内部子 tab，并在工具栏显示当前分支。变更视图可按最新本地提交到最终本地内容审查 diff，并在审查版本仍未
失效时执行整仓 Commit；工作树干净时也可按当前分支的既有 upstream 检查远端并 Sync。

- **电脑**：左侧目录树或变更列表、右侧文件内容或 diff。进入 zone 与手动刷新时一次读取完整目录树，
  展开目录只切换显示；选择变更后右侧显示从当前分支最新本地提交到最终本地内容的差异，默认左右对照。
- **窄屏（≤ 720px）**：逐层进入目录或变更；选择文件/变更后进入内容视图，并可用明确返回按钮回到原列表，
  列表滚动位置会恢复。diff 默认统一显示。
- **完整目录**：普通目录输出与磁盘真实内容一致。Git zone 使用 Git 原生忽略规则省略 ignored 未跟踪
  路径，并省略 `.git` 元数据；`.gitignore`、未被忽略的隐藏文件、tracked 文件与未被忽略的空目录继续显示。
  文件事件与返回页面只更新 Git 状态，目录树只在进入 zone 和手动刷新时重读。
- **边界**：只读取已登记 zone 内部。绝对路径、`..` 遍历、盘符/UNC/备用数据流写法、符号链接、Windows
  目录联接与其他 reparse point 都会被拒绝；链接条目在目录中显示目标与不可进入原因，目标内容不会被读取。
  登记根被外部删除、替换为文件或替换为链接时，读取返回明确错误，不伪装成空目录。新增的 diff 入口使用
  同一份边界检查，Git 旧路径也不能绕过它。
- **内容识别**：按内容识别文本（完整 UTF-8、无 NUL 字节），不要求后缀；无后缀文本可读，伪文本后缀的
  二进制文件给出“不可文本预览”原因。文件内容与 diff 默认都以纯文本显示，HTML/script 不会被执行；
  `.html`/`.htm` 文本文件另有显式的“阅读 HTML”入口，见下一节。
- **预览上限**：默认 1 MiB，可用 `[zones] preview_limit_bytes` 配置。文件预览在读取内容前应用上限；
  diff 对旧侧与新侧分别应用同一个上限。任一侧超限或二进制时不渲染完整 diff，明确说明限制；超限或二进制
  不改变文件/变更的可见性、着色、计数或后续提交资格。
- **自动刷新**：后端监听每个已登记 zone 的本地文件（Git 工作树还监听工作树之外的元数据目录），
  事件按 zone 合并后经只读 SSE 通知页面重新读取；普通目录的外部增删改也会反映到浏览与角标。
  没有等待窗口时立即刷新，窗口内的事件合并成窗口结束的一次补刷新；最小间隔默认 10 秒，可用
  `[zones] refresh_interval_seconds` 配置。连续写入不会让最终刷新无限延期，10 秒是刷新通知的最小
  间隔，不代表 Git 查询必须在 10 秒内完成。多个页面共享同一条 zone 监听与节流，不会各自放大刷新。
- **刷新边界**：进入 zone、回到 `zone` 入口以及点按“刷新”都会立即重新读取当前目录、所选文件、
  Git 状态与所选 diff，不受自动刷新节流限制；不把旧结果当作当前事实。外部切分支后点按“刷新”会显示新分支。
- **监听健康**：监听建立失败、zone 目录或其相关 Git 元数据目录消失/被替换时，页面明确显示
  “自动更新已停止”或“可能已过期”，不会继续宣称自动更新正常，也不会自动宣称恢复；监听正常时的刷新
  与手动读取仍走同一组公开读取接口。目录或文件在外部被删除、Git 状态读取失败时，页面显示明确错误，
  不伪装成空目录或干净仓库。
- **远端边界**：本地文件事件只触发本地状态刷新，不会产生周期性 fetch；远端 ahead/behind 只来自显式
  “检查远端”或 Sync 前检查，并显示最近检查时间。未检查时页面明确显示“尚未检查”，不把本地远端跟踪
  引用当成实时状态。

**待提交集合**（`GET /api/zones/git-status`）：

- 包含已跟踪文件的修改与删除（已暂存和未暂存合并观察），以及未忽略的未跟踪文件；排除被忽略的未跟踪文件。
  已跟踪、后来才匹配忽略规则的文件仍然包含；忽略规则文件自身按普通文件处理，不 force-add 被忽略文件。
- 只把登记根**自身**当作工作树根：登记根是父仓库的子目录时按其普通目录处理，不聚合父仓库变更；不递归
  管理嵌套仓库或子模块内部变更。
- Git 已识别的暂存重命名显示为一项蓝色变更（旧路径 → 新路径）；尚未被 Git 识别的文件系统改名保持删除加
  新增。查询不会为了识别重命名而暂存或修改真实索引。
- 颜色：新增绿、修改与已识别重命名蓝、删除红；冲突使用独立橙色警告，不与普通修改混淆。两个视图使用同一
  份状态快照，因此颜色一致。界面不显示普通 `A/M/D` 状态字母。
- 祖先目录只要包含待提交变更（包括已删除路径）就显示蓝色，并传播到 zone 根下一级；目录不会继承子
  文件的绿色或红色。删除项只在变更列表出现，浏览树不伪造仍然存在的文件。
- 角标按待提交变更项计数：同一文件暂存加未暂存只计一次，已识别重命名计一次，未识别删除加新增计两次；
  不计目录或 diff 行数。Git 识别结果变化可以改变项数。
- 冲突只来自 Git 的未合并文件条目；普通 Git 命令失败或单纯 ahead/behind 分叉不会被标成文件冲突。
  detached HEAD 明确显示为 `HEAD（已分离）`。

接口：

- `GET /api/zones/entries?zone=<登记路径>&path=<相对目录>` 返回
  `{"zone":"…","path":"…","entries":[{"name":"…","path":"…","kind":"directory|file|link","size":…,
  "linkTarget":"…","linkReason":"…"}]}`。
- `GET /api/zones/file?zone=<登记路径>&path=<相对文件>` 返回
  `{"zone":"…","path":"…","name":"…","size":…,"preview":{…}}`。`preview.kind` 为 `text`（带 `content`）、
  `binary`（带 `reason`）或 `too-large`（带 `reason`、`size`、`limit`，不含内容）。
- `GET /api/zones/git-status?zone=<登记路径>` 返回
  `{"zone":"…","repository":true|false,"branch":"…"|null,"detached":bool,"conflict":bool,
  "pendingCount":N,"changes":[{"path":"…","oldPath":"…","kind":"added|modified|deleted|renamed|conflicted"}],
  "sync":{"upstream":{"remote":"…","branch":"…","short":"…"}|null,"ahead":N|null,"behind":N|null,
  "remoteCheckedAt":毫秒|null}}`。普通目录返回 `repository:false` 与空变更；登记目录消失或 Git 不可用时
  是明确错误，不是干净状态。`sync` 只描述本地最近已知的远端跟踪事实：本读取**不发起 fetch**，
  `remoteCheckedAt` 为 `null` 表示本实例尚未成功检查远端，`ahead`/`behind` 为 `null` 表示跟踪引用或
  首次提交尚不可用。
- `GET /api/zones/diff?zone=<登记路径>&path=<相对新路径>`（重命名再加
  `oldPath=<Git 旧路径>`）返回一次变更的审查事实：
  `{"zone":"…","path":"…","oldPath":"…","old":{…},"new":{…},"complete":bool}`。
  旧侧取当前分支最新本地提交（没有首次提交时为空树），新侧取最终本地文件；已暂存与未暂存内容合并
  观察，不使用索引作为旧侧，也不比较远端。每一侧的形状与文件预览一致：`text`（带 `content`、`size`）、
  `binary`（带 `reason`、`size`）、`too-large`（带 `reason`、`size`、`limit`，不含内容），或 `absent`
  （新增文件的旧侧、删除文件的新侧）。任一非文本侧使 `complete:false`，页面不渲染完整 diff。
  新增路径与 `oldPath` 都执行同一份根边界检查；旧路径来自 Git 历史也不能绕过它。
- `GET /api/zones/overview` 返回每个登记项的角标事实：
  `{"zones":[{"path":"…","repository":bool,"pendingCount":N,"conflict":bool,"error":"…"}]}`；
  普通目录没有 `pendingCount`，读取失败的项带 `error`，不伪装成 0 项。
- Git 状态与审查结果还带 `reviewToken`（当前审查版本标识）与可选的 `blockedReason`（仓库级阻塞原因）。
  `reviewToken` 覆盖分支、HEAD、完整索引、待提交路径集合与每个路径的完整最终内容；预览上限不会截断它。
  提交请求必须原样带回它。
- `GET /api/zones/watch` 是自动更新的只读 SSE 长连接。首帧 `snapshot` 给出实际生效的
  `intervalSeconds` 与每个 zone 的监听视图
  `{"zone":"…","health":"live|error","message":"…","lastRefreshAt":…,"generation":N}`；
  之后同一 zone 的所有观察者共享合并后的 `refresh`（`{"zone":"…","at":…,"generation":N}`）
  与 `health` 变化。`error`/监听失败只让页面标注可能过期，不自动重启监听、不触发远端 fetch。

失败响应同样是 `{"code":"…","message":"…"}`，例如 `path-outside-zone`、`path-is-link`、
`path-not-found`、`zone-unavailable`、`read-failed`、`git-unavailable`、`git-command-failed`。

### 阅读本地 HTML

文件浏览中选中扩展名为 `.html` 或 `.htm` 的文本文件（大小写不敏感）时，文件头出现“阅读 HTML”按钮；
原有源码预览仍保留。文件超过 `preview_limit_bytes` 或不是文本时没有可阅读内容，页面继续显示明确原因。

- 点按后进入覆盖整个可用视口的阅读模式，不要求浏览器进入系统级全屏。右上角“退出”始终可见；退出后恢复
  进入前的 zone、当前目录、所选 HTML 文件和列表滚动位置，而不是回到 zone 列表。
- 阅读模式只支持静态 HTML：HTML 排版、CSS、图片、页内锚点与原生 `<details>` 可正常显示；**不执行
  JavaScript**，不提交表单，不嵌入 iframe/object 等其他网页。
- 内容先在父页面净化，再放入只允许同源、禁止脚本的 sandbox iframe，并附带禁止脚本、表单、嵌入、
  连接和外部资源自动加载的 CSP。净化移除 `script`、iframe/object/embed、表单、meta refresh、内联事件
  处理器和 `javascript:`/`file:` 链接；外部图片、字体、样式等资源不会自动加载。
- zone 内的相对 CSS、图片、字体等资源允许加载，读取仍经过与文件浏览相同的根边界检查：绝对路径、
  `..` 遍历、符号链接、目录联接和根外路径都会被拒绝。根目录始终是登记 zone，不会因进入阅读模式扩大。
- 页内锚点正常跳转；指向 zone 内其他 HTML 的链接在当前阅读模式内打开。`http`/`https` 普通链接在
  新标签页打开；`file:///…` 链接不会打开本机其他文件。
- HTML 源文件沿用单文件预览上限（默认 1 MiB）；相对 CSS、图片等资源不是文本预览，不按该文本上限截断。
  如果 HTML 通过 `../assets/...` 引用资源，登记目录必须包含这些资源所在的共同父目录。

阅读资源接口为
`GET /api/zones/reader/<zone-hex>/<相对路径>`：zone 路径以 UTF-8 十六进制编码，使 HTML 的
相对 URL 自然解析到同一个 zone；响应是原始字节，按扩展名给出 MIME，并带 `nosniff`、CSP 与
`Cache-Control: no-store`。未登记 zone、越界路径、链接与目录都明确拒绝。

### 经审查复核的整仓 Commit

Git zone 的变更视图在列表上方常驻提交信息输入框与 `Commit` 主按钮：

1. 提交信息必须非空；页面在信息为空时禁用按钮，直接请求也会以 `empty-message` 拒绝。
2. 点按 `Commit` 只弹出**一句**确认消息与“确认提交 / 取消”。取消不发送写请求，因此没有 Git 写入。
   弹框不重复列出 zone、分支、文件数、提交信息或文件清单。
3. 确认后，服务端用当前事实重新计算审查版本，并与页面展示的 `reviewToken` 比较。分支、HEAD、索引
   （包括外部暂存）、待提交路径集合或完整内容任何一处变化都会拒绝**当次**提交并返回 `review-invalid`；
   页面保留提交信息、刷新实际变更，用户必须重新审查并再次确认。旧审查不能向另一分支提交。
4. 复核通过后执行 `git add --all`，再执行 `git commit -m <信息>`。已经存在的部分暂存不会缩小范围；被忽略
   的未跟踪文件不会被强加；只创建本地提交，不自动 push，也不修改 Git 配置。
5. 同一实际仓库的写操作串行：不同调用者或同一路径的别名同时提交时，后到者得到 `git-write-in-progress`，
   不等待、不排队。资格由执行持有：连接断开不会取消已接受的提交；命令超时且终止未确认时，资格继续保持，
   不会与后续写操作重叠。zone Git 资格与固定操作的执行资格互不相同。
6. 任一步失败立即中断后续步骤，且不撤销已经成功的步骤：暂存成功后 hook 拒绝 commit，索引仍保留已暂存
   内容，页面显示 Git 返回的纯文本诊断（不执行其中的 HTML/script）。普通命令失败不会被误标为文件冲突。
7. detached HEAD、未解决冲突，以及未完成的 merge/rebase/cherry-pick/revert 流程会禁用 Commit 并在页面
   与直接请求中说明原因；冲突解决但流程尚未结束时仍然阻塞。用户在外部完成或撤销流程后，刷新即恢复。
   没有 upstream 不妨碍本地 Commit，也不会隐式配置 upstream。
8. 已初始化但没有首次提交的仓库按空树审查：已暂存与未忽略的未跟踪文件都显示为新增，Commit 创建首个提交。
   已有目录的登记与浏览不初始化 Git；“新增 zone”是唯一会执行 `git init` 的入口。
9. 嵌套仓库与子模块不递归管理：父仓库只记录其 gitlink 提交，内部文件不会作为父仓库普通内容提交；父仓库
   自身已跟踪的 gitlink 引用变化仍正常纳入。父仓库提交不会改写子仓库/嵌套仓库内部工作树。
10. Git 沿用服务器现有的身份、凭据与策略：`git commit -m` 不打开编辑器，`GIT_TERMINAL_PROMPT=0` 禁止在
    没有终端时等待输入；签名或凭据条件无法满足时明确失败或按 `[git] command_timeout_seconds` 超时，不新开
    终端/网页凭据流程，也不覆盖 Git 配置伪造成功。

接口：`POST /api/zones/commit`，JSON
`{"zone":"服务器目录","message":"非空提交信息","reviewToken":"展示的审查版本"}`。
成功返回 `{"zone":"…","outcome":"committed","commit":"<HEAD oid>","branch":"…","message":"…"}`。
失败响应为 `{"code":"…","message":"…"}`，例如 `empty-message`、`invalid-request`、`not-a-repository`、
`commit-blocked`、`review-invalid`、`nothing-to-commit`、`git-write-in-progress`、`git-timeout`、
`git-command-failed`、`zone-unavailable`。超时消息明确说明本功能时限并保留已有 Git 输出。

### 按现有 upstream 安全 Sync

工作树干净时，变更视图显示 Sync 主按钮、当前分支的既有 upstream、最近检查时间与已知 ahead/behind：

| 状态 | 页面与操作 |
|---|---|
| 存在普通待提交变更 | Commit 是主操作；Sync 不显示，避免对脏工作树同步 |
| 干净、有 upstream、已知 ahead/behind 非零 | Sync 可用；显示待拉取与待推送数量 |
| 干净、已知已同步 | 主按钮禁用以“已同步”显示；仍可主动检查远端 |
| 干净、远端尚未检查 | Sync 禁用并说明原因；“检查远端”可执行 |
| 干净、没有 upstream | 显示限制；不提供 Publish Branch，也不隐式配置 upstream |
| detached HEAD、冲突或未完成流程 | Sync 与 Commit 均禁用并显示仓库级原因 |

1. “检查远端”执行一次真实的 `git fetch <upstream.remote>`，更新本地远端跟踪引用并记录最近检查时间。
   普通文件事件、页面读取和自动刷新都不会触发 fetch；未检查时页面不会把旧引用说成实时状态。
2. Sync 只弹出一句确认与“确认同步 / 取消”。取消不发送任何请求，因此没有 fetch、pull、push 或其他
   Git 写入。
3. 确认后服务端重新核对目录、当前分支、既有 upstream、干净工作树与阻塞状态，再按
   **fetch → pull → 按需 push** 执行。fetch 使用当前分支 upstream 指向的远端；pull 与 push 不附加
   远端、分支或策略参数，完全遵循仓库已有的 merge/rebase/ff-only 与 push 配置。
4. 只有 pull 后仍存在本地待推送提交时才执行 `git push`；只有 behind 时不会产生无意义推送。不 stash、
   不隐式 commit、不强推、不配置 upstream、不修改 Git 配置。
5. 任一步失败立即停止后续步骤，页面显示 Git 返回的纯文本诊断（含 HTML/script 也不会执行）。已经生效
   的步骤不会被自动撤销：例如 pull 成功后 push 被拒绝，本地已拉取/合并的提交与引用保持实际状态，页面
   刷新后显示仍需推送的数量，不宣称事务回滚。
6. Sync 与 Commit 共用同一实际仓库的写操作资格：Sync/Sync、Commit/Sync 与路径别名并发都不会重叠；
   后到者得到 `git-write-in-progress`，不等待、不排队。连接断开不取消已接受的 Sync；命令超时且终止
   未确认时资格继续保持。
7. 每条 Git 命令都受 `[git] command_timeout_seconds` 限制；超时消息明确说明这是本功能时限并保留取消前
   已有的 Git 输出，不伪造 Git 退出诊断。
8. 沿用服务器现有的身份与凭据：`GIT_TERMINAL_PROMPT=0`，不新开终端或网页凭据流程；需要交互的凭据、
   编辑器或签名条件会明确失败或按配置超时。
9. 只同步登记根自身的工作树；不递归同步嵌套仓库或子模块内部仓库，它们的 HEAD、索引与内部工作树保持
   原样。父仓库自身已跟踪的 gitlink 变化仍按普通提交/同步事实处理。

接口：

- `POST /api/zones/check-remote`，JSON `{"zone":"服务器目录"}`。成功返回刷新后的完整 Git 状态
  （与 `GET /api/zones/git-status` 同形）。失败类别例如 `not-a-repository`、`no-upstream`、
  `sync-blocked`、`git-write-in-progress`、`git-timeout`、`git-command-failed`、`zone-unavailable`。
- `POST /api/zones/sync`，JSON `{"zone":"服务器目录"}`。成功返回
  `{"zone":"…","outcome":"synced","branch":"…","upstream":{"remote":"…","branch":"…","short":"…"},
  "ahead":N,"behind":N,"remoteCheckedAt":毫秒,"pulled":N,"pushed":N,
  "steps":[{"step":"fetch|pull|push","outcome":"completed|skipped"}]}`。
  失败类别例如 `not-a-repository`、`sync-dirty-worktree`、`sync-blocked`、`no-upstream`、
  `git-write-in-progress`、`git-timeout`、`git-command-failed`、`zone-unavailable`；失败消息会指出
  失败步骤，并在已有步骤完成时说明它们不会被自动撤销。

### 默认绑定与改绑

默认 `herdr-bot.toml` 绑定的是用户**现有的两个批处理**（与规格 Further Notes 记录的外部集成路径一致），
执行工作目录为它们所在的目录：

| 操作 | `script` | `working_directory` |
|---|---|---|
| 启动 | `E:\Developer\myhub\my-streaming\start-all.bat` | `E:\Developer\myhub\my-streaming` |
| 停止 | `E:\Developer\myhub\my-streaming\shutdown-all.bat` | `E:\Developer\myhub\my-streaming` |

- herdr-bot 主界面启动时会把这份绑定打印到控制台，人工核对应以这几行为准；脚本内容与 herdr-bot 主界面**互不影响**：
  面板只在自己的进程里以配置的工作目录执行它们，不修改脚本、不写入脚本所在仓库。
- **改绑方式**（只能改服务端配置，客户端无法改绑）：
  1. 在仓库根目录编辑 `herdr-bot.toml`，或另存一份配置文件并用
     `backend\target\release\herdr-bot.exe --config <配置文件路径>` 指定；`[operations.start]` /
     `[operations.stop]` 下的 `script` 与 `working_directory` 是两个脚本各自的执行目标与工作目录；
  2. 重启 herdr-bot 主界面（运行中的服务不会重读配置）。
- 配置文件使用 `deny_unknown_fields`：写错字段名会启动失败并给出解析错误，不会静默忽略。

### 隔离夹具（fixtures/）的用途

`fixtures/` 是一组随交付保留的真实 Windows 批处理，用来把 herdr-bot 主界面绑到**不会影响用户程序**的目标上，
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

脚本卡住（长时间不结束）时 herdr-bot 主界面不会做任何事：既不超时终止脚本，也不释放执行资格。表现是页面一直
停在“执行中”（按钮不可点），其他页面/设备/直接请求都得到 `409 operation-in-progress`。处理办法：

1. **确认脚本进程是否还在运行**。herdr-bot 主界面不提供这个信息：它没有执行历史，重启后也不记得旧脚本，
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
3. **重启 herdr-bot 主界面不证明旧脚本已结束**：面板不做终止、不做等待，也不保留跨重启的状态。实测：结束面板
   进程本身后，正在运行的脚本进程**仍在运行**；重新启动面板后页面回到“尚未提交操作。”并立即接受
   新操作，对旧脚本一无所知。所以“重启一下就好”是错觉——**先按第 1 步确认、再按第 2 步处理**。
   （未验证的推论：若脚本进程与面板共用同一个控制台窗口，用关闭窗口或 Ctrl+C 结束面板时，Windows
   可能连同该控制台上的脚本进程一并结束；这不改变“重启后无从确认旧脚本”的结论，处理前仍应先确认。）

## 现有脚本接入的已知现象

### 脚本需要控制台：从控制台窗口启动面板，页面显示“执行完成”

默认绑定的两个现有脚本的最后一条命令都是 `timeout /t N /nobreak >nul`（`start-all.bat` 中间还有一条）。
`timeout.exe` 需要**真实控制台**作为标准输入：在控制台窗口里手动运行时它正常等待、脚本以退出码 0 结束；
标准输入被重定向（非交互）时它立即失败并让脚本以退出码 1 结束——即使脚本主体（`start` 打开窗口、
启动或终止 Python 程序）已经生效。herdr-bot 主界面按“与手动运行脚本一致”的方式处理：

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
herdr-bot 主界面不修改脚本（规格要求保持现有脚本内容不变），也不假设“脚本没报错就是没出错”。
需要与手动运行脚本一致的反馈，请在控制台窗口里启动 herdr-bot 主界面。

### 启动脚本打开的窗口

既有启动脚本用 `start` 打开两个独立窗口（标题 `My Streaming Worker` / `My Streaming Server`），
herdr-bot 主界面调用时同样出现**两个各自独立的可见顶层窗口**；本机由系统默认终端应用承载——交付前的真实验证中
观测到的是 Windows Terminal（`CASCADIA_HOSTING_WINDOW_CLASS`，注册表 `HKCU\Console\%%Startup` 两个委托项都是
全零 GUID，即“让 Windows 决定”），每个 Python 进程各有自己的 `conhost.exe`。是否与用户心目中的
“原有窗口行为”（例如经典 conhost 独立窗口）一致，需要人工验收确认（见[人工验收移交](#人工验收移交p2--p4未验收)）。

## 会话登记的生命周期（chat 入口）

`chat` 入口展示后端当前**全部**已登记会话，包括离线登记。在线／离线只表示最近一次上报是否过期，
与 agent 是否在跑、浏览器能否连上后端都是不同的维度：

| 距最后一次被后端接受的写入 | 登记与展示 |
| --- | --- |
| 不超过**在线有效期**（缺省 **10 秒**） | 登记保留，**在线** |
| 超过在线有效期、不超过**两个窗口之和**（缺省 **40 秒**） | 登记保留，**离线**：最后上报的 agent 状态原样显示，并标注它是最后上报值 |
| 超过两个窗口之和 | 后端维护清除登记，列表里不再出现 |

- 计时起点是最后一次被接受的 `register`／`heartbeat`／`state` 写入（缺省在线有效期 10 秒、离线保留期
  30 秒，由服务进程启动时读取 `[sessions]` 配置确定，见 [`[sessions]` 配置](#sessions-配置可选)；
  运行中改写配置文件不改变已运行实例，重启后才用新值）；被拒绝的事件不重新计时，也不改变业务状态。
- 后端每 5 秒跑一遍维护：先发布离线，超过两个窗口之和后发布清除。因此可见变化可能比阈值晚约一个
  扫描周期；**页面不自己计时、也不自己删除记录**，移除一律来自后端观察流。
- 离线的登记仍可被操作：保留期内接受普通上报会刷新它并恢复在线；显式 `unregister` 立即删除登记。
  删除登记只删除 herdr-bot 对会话的跟踪记录，**不删除 agent 会话或聊天记录**（`sessionFile` 只用于诊断）。
- 上报年龄超过两个窗口之和并不等于立刻拒收：结果由**登记是否已被维护实际删除**决定。维护尚未执行到那一遍
  时，迟到的 `heartbeat`／`state` 仍会被接受并让登记按这次上报重新计时；维护已经删除登记之后，普通上报得到
  `404 {"code":"unknown-session"}`，**`register` 是唯一建立登记的入口**，普通上报不会隐式建行。重新
  `register`（同一 `sessionId` 即可）会建立新登记，但**不会**自动恢复已经被移出的完成后关机监听。
- 新注册但还没同步 agent 状态的会话是「在线 + 状态同步中…」，与离线不同：它没有被判为上报失联。
- 登记只存在于运行中的进程内：**后端重启从空表开始**，不落盘、不恢复任何旧登记（重启后普通上报得到
  `404 {"code":"unknown-session"}`，需要重新 `register`）；监听意图与关机安排同样不跨进程恢复
  （见[后端行为与配置](#后端行为与配置)）。

机器可读标识：`[data-testid=session-card][data-keepalive=online|offline][data-state=working|awaiting-input|idle|unknown]`、
`[data-testid=session-state][data-reported=current|last-reported]`、离线标记 `[data-testid=offline-tag]`、
最后上报标记 `[data-testid=last-reported-tag]`。

## 关机

页面有一个“关机”按钮（是否显示由前端构建期常量决定，默认显示）。它关的是**整台 Windows 电脑**，
而不是后台程序——与“停止”是两个不同的动作（见 [`GLOSSARY.md`](GLOSSARY.md)）。

关机有两条入口，共用**同一个**后端倒计时：

- **手动关机**：页面上的“关机”按钮，确认之后立即开始倒计时（不等任何会话）。
- **完成后关机监听**：在 `chat` 入口为指定的在线工作中会话勾选“完成后关机”，等它们全部停下来
  等待用户输入之后再由后端自动开始同一个倒计时（详见[完成后关机监听](#完成后关机监听chat-入口)）。

### 页面流程与结果反馈

1. 点“关机”→ 整屏确认页（“确定要关闭电脑？”）；接管期间底层的“启动/停止”不可操作。确认页的“返回”是
   **纯前端事件**：不发送任何命令，也不形成安排。
2. 点确认页的“关机”→ 提交**开始意图**（`POST /api/shutdown/start`）。后端立即建立**唯一**的全局倒计时。
   这一步的回执只反馈**本次操作的归宿**（已被接受／明确拒绝／未确认）：**只有共享观察流才给出关机安排**，
   页面按服务端给出的**截止时间**与**服务端时间**显示剩余秒数——本机时钟偏差不改变它，本地倒计时也不决定
   执行（执行由后端到期发起）。因此回执先到、观察流还没更新时页面不会乐观翻出倒计时；观察流断开期间即使
   这次开始被接受，也只显示「已接受」这条归宿说明。
3. 倒计时期间点底部“取消关机”→ 提交 `POST /api/shutdown/cancel`（全局取消）。**取消同样只由观察流给出结果**：
   拿到服务端 200 时页面显示「已确认这次取消」这条归宿说明，等到观察流给出 `cancelled` 才显示“取消成功”；
   请求没有回应时保留提醒并明确“取消未确认，电脑仍可能关机”，且**不会自动重发**。执行已经开始时服务端
   明确拒绝取消（`409 {"code":"shutdown-not-cancellable"}`），页面不显示取消成功，也不再提供取消入口。
4. 到点由**后端自己**执行关机：不需要任何前端请求，刷新或关闭页面都不影响安排。本地倒计时归零而结果还没到
   时，页面显示“等待服务端确认”，**不**声称电脑已经关闭。
5. 结果按后端状态反馈：`issued`（指令已发出——不等于已经观察到断电）、`failed`（明确失败，不自动重试）、
   以及前端侧的“无法确认”（命令没有回应）。结果页的“返回”同样是纯前端事件，不影响后端与其他前端。
6. 倒计时**从完成后关机监听自动触发**时不再要求前端补一次手动确认：任何打开着的主界面会直接进入上面的
   提醒／取消界面；没有任何前端打开时，后端照样在到期时执行。
7. **命令回执只反馈归宿，业务展示只随观察流更新**：回执里的安排状态、提交过的目标值都不会被拿来
   填补关机安排或监听集合。因此迟到的回执不会覆盖较新的观察结果；回执先到而观察流还没更新时页面
   不会乐观翻转；观察流断开期间即使命令被确认，也只显示归宿说明，最后一份快照与「可能已经过期」的
   标注保持到观察流恢复。

机器可读标识（浏览器自动化据此断言类别，不依赖文案）：`[data-testid=shutdown-button]`、
`[data-testid=shutdown-takeover][data-phase=confirm|counting|waiting|executing|issued|failed|cancelled|start-accepted|cancel-accepted|…]`、
`[data-testid=shutdown-confirm]`、`[data-testid=shutdown-back]`、`[data-testid=shutdown-confirm-button]`、
`[data-testid=shutdown-countdown][data-seconds]`、`[data-testid=shutdown-waiting]`、
`[data-testid=shutdown-executing]`、`[data-testid=shutdown-cancel]`、`[data-testid=shutdown-warning]`、
`[data-testid=shutdown-refusal]`、`[data-testid=shutdown-result][data-outcome=issued|failed|cancelled]`、
`[data-testid=shutdown-result-back]`、`[data-testid=shutdown-start-accepted]`、`[data-testid=shutdown-cancel-accepted]`、
`[data-testid=shutdown-accepted][data-outcome=accepted]`（「本次操作已被接受」这条归宿说明，倒计时屏里也会出现）。

前端只保留展示开关与请求等待上限（`frontend/src/config.ts`：`SHUTDOWN_BUTTON_VISIBLE`、
`SHUTDOWN_RESPONSE_TIMEOUT_MS`）；**倒计时时长不在前端**，改它要改后端配置并重启。

### 完成后关机监听（chat 入口）

给“等它跑完再关机”的会话勾上监听，全部成员都停下来之后由**后端**自动开始上面那个倒计时。
全屏倒计时提醒与取消入口是全局展示：停留在 `chat` 入口时也会自动出现，**无需切换 tab 或刷新**；
取消后返回仍保留原先选择的 tab。手动“关机”入口仍只在 `server` 入口显示。

1. 卡片上的“完成后关机”按钮只在**当前在线且工作中**的会话上出现——这是**新增**监听的资格。
   离线（上报过期）、空闲、等待用户输入、以及在线但尚未同步 agent 状态的会话都不提供这个入口；
   无登记的会话提交监听得到 `404 {"code":"unknown-session"}`，资格不符得到
   `409 {"code":"session-not-eligible"}`。已经进入权威监听集合的卡片是另一回事：它保留可撤销的按钮
   （见下一条），重申“已选中”也不按新增资格重新判定。点一下＝提交一次监听
   （`POST /api/shutdown/monitor`，请求体 `{"sessionId":…,"selected":true}`：明确的目标选中值，
   不是“翻转”）。**按钮的选中态只随共享观察流的权威集合变化**（`data-selected=true`）：本地不做乐观
   切换。命令回执只反馈**本次命令的归宿**——被接受时给出“已接受”的说明（`data-outcome=accepted`），
   请求没有回应时只标记“未确认”，也**不会自动重发**。
2. 再次点击＝提交明确的“取消监听”（`selected:false`），同样以观察流的权威集合为准。**已经监听的会话**
   即便之后进入“等待用户输入”或**离线**（上报过期但登记仍在），也保留这个可撤销的按钮：撤销不要求成员
   当前在线或工作中，因此短暂离线不会让页面失去撤销能力。回执仍只反馈归宿，选中态与剩余集合以观察流为准。
3. 启动条件是**非空**监听集合的**全部成员当前都在线并**等待用户输入——不是任意一个成员结束，也不是
   “曾经结束过一次”。在线、空闲、状态尚未同步、工作中与离线**任何一项不满足都不触发**，不记忆成员
   历史上曾经结束过一次的资格。还有成员在跑时，等待中的已选卡片会说明“正在等待其他被监听会话”。
   不被监听的会话（包括一直在工作的）不参与判定，也不阻塞；离线成员按上面的规则阻塞。
4. 会话**注销**、**登记被清除**或被**撤销监听**，都会把该成员移出集合并立即重判：
   如果剩下的非空集合已经全部在线并等待用户输入，立刻开始倒计时；移出最后一个成员只清空集合，
   不会开始倒计时。移出的成员重新注册**不会**自动回到监听集合。
   仅仅**离线**（上报过期但登记仍在）的成员**留在集合里并阻塞自动触发**，即使它最后上报的是等待
   用户输入；保留期内恢复在线后原监听继续有效，不需要重新选择。恢复上报按它**当时**的 agent 状态参与
   判定：恢复成“工作中”继续阻塞，恢复成“等待用户输入”才可能满足条件——恢复在线本身不被当作轮次结束。
   离线的已监听成员也可以随时明确撤销监听，撤销不因离线被拒绝。
5. 集合里有监听、但还没有倒计时时，`chat` 入口顶部出现“取消关机并清空监听”：一次命令
   （`POST /api/shutdown/cancel`）**原子**清空整组监听（不是逐个撤销，因此不会在中间触发关机），
   拿到服务端确认（`200`）时给出“本次命令已被接受”的说明；**集合是否真的清空仍由观察流给出**。
   它与每个会话的撤销按钮、结果页的“返回”各占一处，不是同一个东西。
6. **开始倒计时（手动或自动）即消耗整组监听**：之后必须重新选择；期间不接受新增监听
   （`409 {"code":"shutdown-in-progress"}`）。取消之后也不会恢复监听，重复的“等待用户输入”上报
   不会自动重新开始倒计时。
7. 倒计时开始之后，会话重新工作、离线、**登记被后端维护清除**、注销以及关闭前端**都不会**取消这个安排，
   也不会改变创建时定死的截止时间（既不取消也不延长）；它照旧在原截止点执行一次。

监听命令的拒绝类别（可机器读取，与“没有回应”的未确认互不混淆）：`404 {"code":"unknown-session"}`
（会话未注册）、`409 {"code":"session-not-eligible"}`（不在线或不是工作中）、
`409 {"code":"shutdown-in-progress"}`（已有活动倒计时）；请求体解析失败是
`400 {"code":"invalid-monitoring"}`。

机器可读标识：`[data-testid=monitor-button][data-selected=true|false]`、
`[data-testid=monitor-note]`、
`[data-testid=monitor-status][data-outcome=accepted|refused|unconfirmed][data-code]`、
`[data-testid=monitoring-bar]`、`[data-testid=monitoring-clear]`、
`[data-testid=monitoring-clear-status][data-outcome=cleared|refused|unconfirmed]`
（`cleared` 只表示服务端确认了这次取消命令，不表示集合已经在展示上清空）。

### 多前端、刷新、重连与传输故障

关机安排只属于**后端进程**，页面只是读它、提交意图。因此：

- **同一个后端可以被多个前端同时看**（本机与局域网设备、同一台机器上的多个标签页）。打开、刷新、重连
  都以共享观察流（`GET /api/sessions/events`）的**首帧完整快照**恢复会话与关机状态：这一帧恰含当前**全部**
  登记（含离线暂留的）与当前关机事实（当前监听集合，或正在倒计时的活动安排），已经被维护清除的身份不在其中；
  断线期间由另一个调用者改掉的监听或安排，会在这份首帧里覆盖本地已经过期的视图。每一页都只由**自己那条**
  观察连接驱动（不读本地历史、不等待事件重放、也不依赖其他页面的命令回执），因此各页面会独立收敛到同一份
  事实。断线期间只保留最后一帧并在页面内标注「可能已经过期」，不会清空、也不会假装实况。
- **刷新/重连不发任何业务命令**：页面重新连上只是重新读一次权威状态，既不会重新开始或延长安排，
  也不会补发之前没得到回应的命令。首帧之后仍只收 `upsert`/`remove` 会话增量与完整的 `shutdown` 事件。
- **恢复实况只有一条路径**：一份**完整**的首帧快照（会话与关机两部分都读得懂）。`open` 事件只说明这条连接
  又通了，手上的读数仍可能是断线前的旧数据；`upsert`/`remove`/`shutdown` 这些命名增量各只覆盖一个域，
  同样不算恢复证据。因此断线之后（含浏览器自行重连、只 `open` 而还没有首帧、首帧读不懂）展示**冻结**在最后
  一帧并继续标注「可能已经过期」，直到完整首帧到达才整体替换。半份快照整帧丢弃：宁可保留已知的旧数据，
  也不把「新会话 + 旧关机状态」这种谁也不是的状态当作当前实况。打开、刷新、关闭页面与观察断线同样不发
  任何隐式写入——页面从不 `POST /api/sessions/events`，因此页面行为不可能刷新任何会话的 `last_seen`。
- **剩余时间按服务端时间算**：页面用服务端给的 `deadline` 与 `serverTime` 显示剩余秒数，本机时钟偏差
  不影响它，也不决定到期执行——到期只由后端自己的时钟推进。刷新后剩余秒数只会继续走，不会回到满值。
- **命令没有回应 = 未确认**，不是拒绝、也不是成功：页面保留提醒并显示「取消未确认，电脑仍可能关机」，
  说明状态可能已经过期；**不会自动重发**，也不会因为连接恢复或本地关掉展示而变成「取消成功」。
  回执里读不出可确认的数据（例如监听命令没有可识别的类别）同样按未确认处理，不拿提交过的目标值充数；
  明确拒绝则给出可机器读取的类别码（见上）。
- **回执与观察流分工固定**：回执只反馈这次操作的归宿（已被接受／明确拒绝／未确认），**不**更新业务展示；
  关机安排、监听集合与会话只随共享观察流更新。因此迟到的回执不会把展示拉回旧状态，回执先到而观察流
  还没更新时也不会乐观翻转；观察流断开期间即使命令被确认，也只显示归宿说明，「可能已经过期」的最后
  一份快照保持到观察流给出新事实为止。
- **观察连接不拥有安排生命周期**：即使某个前端把读取阻塞住甚至完全不读，命令照样能提交、取消与到期照样
  由后端裁定，到期仍然只发出一次实际关机调用；恢复读取后按权威状态收敛。多个读者之间也不会互相覆盖。

### 后端行为与配置

倒计时时长在**启动期**读取一次，改了要重启后端（没有热加载）：

```toml
[shutdown]
# 手动关机与完成后关机监听共用的倒计时秒数；缺省 10。
# 合法整数范围 1..=4294967295：零、负数、小数、超过上界，或拼错的字段／段名都会让**启动明确失败**，
# 不会静默回退到缺省值；缺整个 [shutdown] 段的旧配置仍可加载（等于 10 秒）。
countdown_seconds = 10
```

- 命令：`POST /api/shutdown/start`（手动确认后的开始意图）、`POST /api/shutdown/cancel`（全局取消）、
  `POST /api/shutdown/monitor`（设置／撤销某个会话的完成后关机监听，请求体 `{"sessionId":…,"selected":…}`）。
  前两条命令的请求面是封闭的：带查询串或请求体 → `400 {"code":"unexpected-parameters",…}`；监听命令的
  参数只来自请求体（带查询串同样被拒）。三条命令的拒绝都不产生任何副作用。
- 旧的即时关机路径 `POST /api/shutdown` **不再存在**（`404 {"code":"unknown-endpoint"}`）：没有绕过倒计时的
  即时关机别名，也没有生产假关机开关。
- 同一实例同一时刻只有**一个**活动倒计时：重复发起沿用同一安排标识与截止时间（`200`，`outcome` 是当前阶段），
  不重置、不延长，也不会多调用一次实际关机。
- 到点由后端调用既有的 Windows 关机能力（`SeShutdownPrivilege` +
  `ExitWindowsEx(EWX_SHUTDOWN | EWX_FORCEIFHUNG, …)`）：成功一次，**失败不自动重试**，结果可读。
  诊断输出 `[herdr-bot] 关机状态：invoked` 或 `…failed step=privilege|exit-windows-ex|internal error=<原始错误>`；
  原始系统错误**只**进诊断输出，不进 HTTP 结果类别，也不进观察帧。
- 取消与到期由后端裁定唯一结果：执行开始之前生效的取消阻止本次关机，执行开始之后不可取消；延迟送达的取消
  作用于**收到请求时**的当前安排（不按调用者看到的安排标识校验），这是已接受的取舍。
- 登记、监听意图与安排都只属于当前进程：**后端重启即清空**会话登记、监听与倒计时，不持久化、不恢复旧意图
  （重启后同一 `sessionId` 需要重新 `register`，旧的监听意图与安排都不会回来）。
- 手动入口与自动入口共用**同一个**全局倒计时：手动开始不等监听集合是否满足自动触发条件
  （集合里还有工作中或离线的成员也照样立即开始），并在开始的一刻消耗整组监听。
- 浏览器读取会话与关机状态用**同一条**只读长连接（`GET /api/sessions/events`）：首帧同时给出会话快照与关机
  状态，之后关机变化以独立命名事件 `shutdown` 携带**完整**状态（监听集合、安排标识、阶段、截止时间、服务端时间）；
  没有第二条关机观察连接。
- 与“启动/停止”**完全解耦**：不获取执行资格、不排队、不执行“停止”脚本，脚本正在执行时也能形成安排；
  关机也不结束那个脚本、不释放它的执行资格。关机**不注销** mDNS 记录（下次启动重新发布的记录会刷新客户端缓存）。

### 安全边界：无认证，且监听局域网

> **三条关机命令都没有认证。** 任何能连到这个端口的设备都可以直接调用它们：开始一个倒计时，取消当前安排，
> 或为某个会话设置／撤销完成后关机监听。页面上的确认与倒计时只是**防误触的 UX 安全网**，不是服务端强制；
> 真正的边界是端口可达性。

> 注意两条边界的区别：**关机操作**（开始／取消／监听）对局域网全开，而**扩展上报的会话事件**
> （`POST /api/sessions/events`）仍然只接受本机来源（回环或本机网卡地址），见
> [ADR-0004](docs/adr/0004-session-write-boundary-same-host.md)。

- 服务监听 `0.0.0.0`（非回环）：同一个局域网里能访问这台电脑的设备都能打开页面并调用**所有**接口，
  包括关机端点——它不需要任何口令、令牌或会话。
- 现有的 Windows 防火墙规则决定**谁能连上端口**，但它不做身份区分。本机实测（`netsh advfirewall
  firewall show rule name=herdr-bot`）：两条**入站允许**规则（TCP 与 UDP）、任意端口、程序为
  `herdr-bot.exe`、配置文件 `公用`；而本机 WLAN 当前的网络类别正是“公用”（`Get-NetConnectionProfile`），
  因此这条规则覆盖本机地址——**同一个 Wi-Fi 里的任何设备都在放行范围内**。
- 因此这条边界就是“**仅可信局域网**”：不要暴露到公网、不要用不信任的 Wi-Fi/访客网络，也不要做端口
  转发。要收回访问只能改防火墙放行范围或换网络；本版没有运行时开关，也没有口令。

## 测试

```bat
cargo test --manifest-path backend\Cargo.toml
```

前端交互测试与真实浏览器验收分开运行；浏览器用例会启动隔离的真实 herdr-bot 进程，需要先构建
打包页面与后端二进制：

```bat
npm --prefix frontend run test
npm --prefix frontend run build
cargo build --manifest-path backend\Cargo.toml
npm --prefix frontend run test:browser
```

`test:browser` 使用本机已安装的 Google Chrome（Playwright `channel: "chrome"`），不额外下载浏览器；
用例覆盖 `bot` 入口的真实新增、查看、编辑、删除、公开 HTTP 共享读取、保存失败与进程重启保留，
以及从普通目录与 Git 工作树 zone 行创建 Bot 的真实页面链路、名称预览与 Herdr 侧名称一致、启动/
提交阶段、成功与失败留在弹窗。`mobileWorkflow.spec.ts` 在 1280×900 与 390×844 两种尺寸各完成一遍
模板 CRUD 与创建全流程，移动档同时检查无横向溢出、弹窗落在视口内并内部滚动、操作按钮不被挤出屏幕、
触控目标不小于 44px、输入字号不小于 16px、键盘换行与成功/失败结果。`lanAccess.spec.ts` 让服务绑定
`0.0.0.0`，真实浏览器经本机局域网 IPv4 完成模板 CRUD 与创建，独立 HTTP 客户端经同一地址读取模板，
请求不携带认证头。`backend/tests/bot_source_boundary.rs` 用受控局域网来源证明模板管理与创建被接受，
而同一来源的 `POST /api/sessions/events` 仍是 `403 forbidden`，两种能力不共用同机来源限制。

创建流程的自动化验收使用 `backend/src/bin/herdr-fixture.rs`：这是**受控 Herdr 替身**，不是生产
adapter。测试把它复制到独立临时目录，并让 `[herdr] executable` 指向这个副本；herdr-bot 仍走生产
调用包装层（逐参数、无 Shell），而调用记录、Tab / Pane 标识、延迟与失败由这个独立进程写下。
`backend/tests/bot_creation.rs` 用它验证 Workspace 精确选择、显式 cwd、就绪后提交、已知对象标识、
Skill / prompt / input 的单次合成（含空值组合、两项原生 Skill 前缀与正文逐字保真）、
不触碰会话/关机状态，以及 Workspace 不可达/零匹配/多匹配、Tab 创建后启动失败、提交被拒绝与提交超时
时停止后续阶段、保留现场且不自动重发或清理。`backend/tests/bot_creation_detached.rs` 继续验证创建
被接受后观察连接断开不取消执行、提交请求连接断开仍完成唯一一次提交，以及两个独立客户端同时向同一
zone 与不同 zone 提交时各自创建；受控替身还支持按调用内容匹配的启动/提交屏障，让一次创建停住而另一
次继续，用来证明不存在全局单飞、排队或模板快照串用。`frontend/browser-tests/botCreationDetached.spec.ts`
在真实浏览器中覆盖断线、刷新与关闭页面：页面在未收到确认结果时不显示成功，后端仍完成同一次创建。
`backend/tests/herdr_config.rs` 用真实 TOML 与真实进程
验证 Workspace 配置缺失、空白或合法显式值。受控记录不能替代真实 Herdr / agent 的人工验收。

mDNS 集成测试默认带 `#[ignore]`：它们会绑定真实网络并可能触发 Windows Defender 防火墙的入站提示，
且 `mdns_conflict` 需要独占 `herdr-bot.local.`。需要时先停止正在运行的面板，再显式运行：

```bat
cargo test --manifest-path backend\Cargo.toml -- --ignored
```

运行全部测试并包含这些用例：

```bat
cargo test --manifest-path backend\Cargo.toml -- --include-ignored
```

如果希望手工运行 mDNS 测试时不再每次构建都新增防火墙规则，使用稳定副本脚本：

```powershell
.\scripts\run-mdns-tests.ps1 -ConfigureFirewall   # 首次：UAC 提权，为稳定路径添加规则
.\scripts\run-mdns-tests.ps1                      # 之后：构建并运行稳定副本
```

脚本会把带 hash 的测试 exe 复制到 `backend\target\debug\mdns_failure_runner.exe` 与
`mdns_conflict_runner.exe`，再运行这些稳定路径；`-ListOnly` 只构建/复制并打印路径，
`-NoBuild` 复用现有副本。`mdns_conflict` 仍需先停止正在运行的面板。

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

本版（mDNS 与关机）新增的测试：

- `backend/tests/shutdown.rs`：关机端点的契约（8 个用例）——空 POST → 200 `shutdown-issued` 且动作恰好
  一次；带查询串/请求体 → 400 `unexpected-parameters` 且 0 次调用；动作失败 → 500 `shutdown-failed`
  且系统错误详情不进响应体；脚本占着执行资格时关机照样被接受，且不结束脚本、不释放资格、不执行
  “停止”脚本；端点永远注册（换 HTTP 方法得 405 而不是 404）。**这些用例全部注入记录型关机动作，
  自动化里绝不会真的关机。**
- `backend/tests/mdns_conflict.rs`：`herdr-bot.local` 被占用时不改名、不发布，面板照常服务；以及注册后
  被改名时立即注销（第二个用例借环境开关强制走这条兜底路径）。
- `backend/tests/mdns_failure.rs`：`[mdns] address` 不在本机网卡上时只告警、跳过发布，HTTP 照常服务。
- `backend/src/mdns.rs`、`backend/src/config.rs` 的内联单测：地址选择（`[mdns] address` 优先、未设置时
  取主路由地址而非 WSL/链路本地、回环与 `169.254.0.0/16` 永不发布）与 `[mdns]` 段的缺省值/拼错字段名。

### 诊断用环境开关（不是用户配置）

`HERDR_BOT_MDNS_SKIP_PREPROBE`：设置后跳过注册前的冲突预探测，用于验证“注册后被改名”的兜底路径
（`backend/tests/mdns_conflict.rs` 的第二个用例就是这么跑的）。不设置即正常走预探测；**正常使用与日常
排查都不需要它**——它不是配置项，也不出现在 `herdr-bot.toml` 里。

## 首版限制

- **无身份验证**：任何能连上监听端口的设备都可以调用操作接口。只在可信局域网内使用，
  Windows 防火墙放行应限制在专用网络范围，不要暴露到公网。
- **单实例**：只运行一个 herdr-bot 主界面进程；同一时间只执行一个脚本操作的约束以单实例为前提（多实例之间
  没有互斥）。同一实例内的重叠请求立即得到 `409 operation-in-progress`，不会被排队补做。
- 页面只反映脚本调用情况，不表示后台程序已经启动、已经就绪或已经彻底停止。
- 关闭脚本按端口强制终止进程树，可能误杀占用相同端口的其他程序（既有脚本行为，首版保留）。
  验收或使用前先确认 `8899`/`8888` 没有被无关程序占用（`netstat -ano | findstr LISTENING | findstr "8899 8888"`）。
- 脚本未结束时不会自动终止；卡住需要在 Windows 上人工处理（重启 herdr-bot 主界面不能证明旧脚本已结束），
  具体步骤见[脚本卡住时的人工处理](#脚本卡住时的人工处理)。
- 本版新增的 `POST /api/shutdown` 属同一条无认证边界，而且后果更重（整台电脑关机）：
  见[安全边界：无认证，且监听局域网](#安全边界无认证且监听局域网)。

## 变更记录：项目更名为 herdr-bot（2026-10-21）

项目标识由 `omp-bot` 硬切换为 `herdr-bot`：仓库与本地目录、Rust crate 与二进制、默认配置 `herdr-bot.toml`、mDNS 主机名 `herdr-bot.local`、`HERDR_BOT_*` 环境变量、日志前缀与前端标题同步更新。既有 API、会话协议与 `connect-bot` 行为不变。历史 ADR 与下方交付验证/变更记录保留原文；原因与影响见 [ADR 0010](docs/adr/0010-rename-project-to-herdr-bot.md)。`my-agent-toolkit` 的真实面板 interop 辅助文件作为独立后续更新。剩余迁移步骤见 [`docs/herdr-bot-rename-remaining-steps.md`](docs/herdr-bot-rename-remaining-steps.md)。

> 术语注记（2026-10-21）：以下交付验证记录与变更记录保留当时的 **omp-bot** 名称与路径；项目现名 **herdr-bot**，mDNS 主机名现为 `herdr-bot.local`。原因与硬切换范围见 [ADR 0010](docs/adr/0010-rename-project-to-herdr-bot.md)。正文保留原文。

## 交付验证记录（首版，2026-09-30）

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
| 浏览器加载 | Chromium 打开 `http://192.168.5.4:8787/`（375×667） | 标题与 `h1`「omp-bot」、两个按钮均启用（158×69 px）、结果框「尚未提交操作。」（`data-outcome=idle`）、无横向滚动；截图 `.scratch/omp-bot-v1/evidence/14-delivery/p13-delivery-lan-375x667.png` |
| 操作接口（夹具） | `curl -X POST .../api/operations/start`、`.../api/operations/stop` | `200` + `{"outcome":"completed","exitCode":0}`、`200` + `{"outcome":"failed","failure":"failed-exit","exitCode":3}`；夹具记录 `fixtures/runtime/calls.tsv` 写入对应行，`cwd` 为配置的工作目录 |
| 越界请求 | `POST /api/operations/format`、带请求体的合法操作 | `404 unknown-operation`、`400 unexpected-parameters`，没有脚本被执行 |
| 卡住与互斥 | 夹具 `long-running.bat` + 第二个客户端请求 | 脚本运行期间第二个请求（经局域网地址）立即得到 `409 operation-in-progress` |
| 人工处理（首选） | `fixtures\stop-fixtures.bat` | 夹具写 `stop-observed` 并自行结束；卡住的请求返回 `{"outcome":"completed","exitCode":0}`，PID 文件被清理 |
| 人工处理（兜底） | `taskkill /PID <脚本 PID> /T /F` | 卡住的请求返回 `{"outcome":"failed","failure":"failed-exit"}`，随后 `POST .../stop` 被正常接受并真实执行 |
| 重启不清理 | 结束面板进程后 `tasklist /FI "PID eq <脚本 PID>"` | 脚本进程**仍在运行**；重启面板后页面为 `idle` 并立即接受新操作 |
| 真实脚本 + 控制台启动（“退出码 1”修复验证） | 在控制台窗口启动面板（默认 `omp-bot.toml`）→ `curl -X POST .../start`、`.../stop` | `start`：`200` + `{"exitCode":0,"outcome":"completed"}`（约 4.8 s），随后 `127.0.0.1:8899`、`0.0.0.0:8888` LISTENING、`worker.py`/`server.py` 运行、`My Streaming` 窗口出现；`stop`：`200` + `{"exitCode":0,"outcome":"completed"}`（约 0.5 s），两个 Python 进程与端口全部释放，窗口消失 |
| 无控制台启动（回退路径） | 无控制台启动面板 + `timeout` 探针批处理 | 启动日志打印“本次启动没有连接控制台…”提示；请求返回 `{"exitCode":1,"outcome":"failed","failure":"failed-exit"}`，如实反映脚本在该环境下的退出码 |
| 无自动终止复核 | “执行生命周期”一节的两条关键字复核命令（`findstr` / `grep`） | 两条都**没有任何输出**（退出码 1 = 无匹配），与文档一致 |
| 现有脚本未修改 | `sha256sum`、`git -C E:\Developer\myhub\my-streaming status --porcelain`、`rev-parse HEAD`、`hash-object` | 与实现票 12 的记录一致：`875b915a…` / `9b6720c5…`；工作树干净；HEAD `27f0a795…`；`517c03f0…` / `0f6c9eb7…` |
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

1. 在仓库根目录启动 omp-bot 主界面（默认 `omp-bot.toml`，绑定 `start-all.bat` / `shutdown-all.bat`），
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

P2 与 P4 在用户逐条确认前保持**未验收**；上表自动化检查已全部执行，但它们只证明 omp-bot 主界面一侧的行为
与证据已备好，不证明真机连通、真机操作，也不替代对现有脚本实际控制与窗口行为的人工确认。

## 交付验证记录（v2，2026-09-30）

按本文档 v2 的新增内容实际构建并启动一次**默认配置**的 omp-bot 主界面并逐项核对；原始观测与截图见
[`.scratch/omp-bot-v2/evidence/07-delivery/`](.scratch/omp-bot-v2/evidence/07-delivery/delivery-checks.txt)
目录（本节表格是 `delivery-checks.txt` 的摘要）。**本次核对没有对生产关机端点发过任何请求**：
页面只走到确认页就点“返回”，没有进入倒计时；关机端点本身的行为由注入记录型动作的契约测试验证
（不会真的关机）。

| 步骤 | 执行的命令 | 观测结果 |
|---|---|---|
| 前端构建 | `npm --prefix frontend install`、`npm --prefix frontend run build` | `up to date in 216ms`；`tsc --noEmit && vite build`：20 modules，`backend/static/index.html` 0.40 kB、`assets/index-Cxv-kr9h.css` 3.54 kB、`assets/index-DZovPMgF.js` 225.99 kB |
| 后端构建 | `cargo build --release --manifest-path backend\Cargo.toml` | `Finished release profile [optimized]`（9.82 s），`backend\target\release\omp-bot.exe`（2 916 352 B） |
| 启动（默认 `omp-bot.toml`，**没有** `[mdns]` 段） | `backend\target\release\omp-bot.exe` | 打印绑定、监听与 `[omp-bot] mDNS 状态：published host=omp-bot.local. address=192.168.5.4 port=8787 url=http://omp-bot.local:8787/`——默认开启 mDNS 并发布了主机名 |
| 三个入口 | `curl -D -` 对 `http://127.0.0.1:8787/`、`http://omp-bot.local:8787/`、`http://192.168.5.4:8787/` | 三者都 `200 OK`、`content-type: text/html`、408 B（同一构建产物） |
| 本机解析（**不等于手机解析**） | `Resolve-DnsName -Type A`、`ping`、`dns-sd -G v4 omp-bot.local`、`dns-sd -B _http._tcp local` | 分别得到 `A 192.168.5.4`、ping 通、`Add … omp-bot.local. 192.168.5.4`、服务实例 `omp-bot` |
| `[mdns] enabled = false` | 另起实例（`evidence/07-delivery/cfg-mdns-disabled.toml`，监听 8788） | `[omp-bot] mDNS 状态：disabled（… 未发布任何 mDNS 记录）`；同实例 `curl` 得 `200`、408 B |
| `[mdns] address` 非本机地址 | 另起实例（`cfg-mdns-foreign-address.toml`，监听 8789，`address = "203.0.113.7"`） | `[omp-bot] mDNS 状态：error（… 不是本机网卡上的地址 …）`；跳过发布，同实例 `curl` 得 `200`、408 B |
| 关机端点契约（**不触碰真实动作**） | `cargo test --manifest-path backend\Cargo.toml --test shutdown` | `test result: ok. 8 passed; 0 failed`（空 POST→200 且动作恰好一次；带参数→400 且 0 次；失败→500 `shutdown-failed`；脚本占资格时仍被接受；不跑停止脚本、不动资格；换方法得 405 而不是 404） |
| 交付二进制链入系统关机 API | `grep -a -c "ExitWindowsEx" backend\target\release\omp-bot.exe` | `2`（PE 导入表中存在该导入；本次只查导入表，**不执行**它） |
| 页面关机入口（真实 Chromium 390×844，`http://omp-bot.local:8787/`） | 点“关机”→ 点“返回” | 初始按钮 = `启动 | 停止 | 关机`、`operation-result` 为 `idle`；点击后 `shutdown-takeover` 的 `data-phase=confirm`、`shutdown-confirm` 文案为“关机 / 确定要关闭电脑？ / 关机后需要重新开机。 / 返回 / 关机”、底层按钮 `启动:disabled=true \| 停止:disabled=true`；点“返回”后接管消失、恢复 `idle`；**整个过程 `/api/shutdown` 请求数 = 0**。截图：`browser-01-idle.png`、`browser-02-confirm.png`、`browser-03-after-back.png` |
| 防火墙与网络类别 | `netsh advfirewall firewall show rule name=omp-bot`、`Get-NetConnectionProfile` | 两条入站允许规则（TCP、UDP、任意端口、程序 `omp-bot.exe`、配置文件 `公用`）；WLAN（`ZTE-1007 3`）网络类别为 `Public`，因此该规则覆盖本机地址——这是“仅可信局域网”边界的依据 |

自动化证据的界限：上表证明的是 omp-bot 主界面一侧的行为、构建产物与端点契约；**本机解析是这台装有 Bonjour 的
电脑上的解析，不是手机解析**；`ExitWindowsEx` 从未在自动化里真实执行。

## 人工验收移交（v2，未验收）

契约在本版保留**两项人工验收**：**真实手机解析 `omp-bot.local` 并打开 omp-bot 主界面**（P1）与**真实断电一次**（P8）。
在用户逐条确认之前一律记为**未验收**。本节的自动化证据只证明 omp-bot 主界面一侧的行为与证据已备好，
**不构成**这两项的结论，也没有用替身替代真实行为。

### P1：真实手机解析 `omp-bot.local` 并打开 omp-bot 主界面（未验收）

前置：电脑上的 omp-bot 主界面已启动（默认 `omp-bot.toml` 即可）；手机与电脑连**同一个普通家庭 Wi-Fi**
（不要访客网络或热点，也别开 VPN/代理）。

1. 看启动日志的 mDNS 行，记下发布的地址与端口：

   ```
   [omp-bot] mDNS 状态：published host=omp-bot.local. address=<IPv4> port=<端口> url=http://omp-bot.local:<端口>/
   ```

2. 电脑上先确认解析（这一步只代表**这台电脑**，手机上成不成要看第 3 步）：

   ```bat
   ping omp-bot.local
   ```

3. 手机浏览器打开 `http://omp-bot.local:<端口>/`。
   - **预期看到**：标题“omp-bot”、一行范围说明、两个大按钮（“启动”“停止”）与“关机”按钮，
     结果框显示“尚未提交操作。”——与用 IP 打开的是同一个页面。
4. 手机解析不了（转圈、超时、找不到服务器）时：按 [IP 兜底的具体做法](#ip-兜底的具体做法) 用
   `http://<IPv4>:<端口>/` 打开，并记录手机型号、系统版本与所用 Wi-Fi——它们决定命中上文「mDNS 主机名
   `omp-bot.local`」一节里的哪一条限制。

请逐条确认：

- [ ] 手机能用 `http://omp-bot.local:<端口>/` 打开 omp-bot 主界面（不是只有 IP 才能打开）。
- [ ] 页面上的“启动”“停止”“关机”三个按钮都可见、可点按，不需要放大或横屏。
- [ ] 名字入口与 IP 入口打开的是同一个页面（同一份内容）。
- [ ] 若名字解析失败，IP 兜底照常能用（这不算失败，但请记录失败原因的种类）。
- [ ] 只有在你亲自确认之后，才把本条记为“已验收”。

证据位置（真机确认前均为**待确认**）：

- 本机（电脑）解析观测：`.scratch/omp-bot-v2/evidence/07-delivery/delivery-checks.txt` §4
  （`Resolve-DnsName` / `ping` / `dns-sd -G v4` / `dns-sd -B`）——这是**电脑上的**解析，不是手机。
- 三个入口的 HTTP 观测与页面断言/截图：同文件 §3、§8 与 `evidence/07-delivery/browser-0*.png`。
- mDNS 状态类别与地址选择：`evidence/01-03-mdns/*.toml`（可原样重跑）、
  `evidence/07-delivery/run-mdns-disabled.log`、`run-mdns-foreign-address.log`。
- **限制**：没有任何手机侧证据。真实手机解析属人工验收，本目录没有、也不应伪造这一证据。

### P8：真实断电一次（未验收）

前置：确认这台电脑可以接受关机（先保存好工作）；不需要先执行“启动/停止”脚本。

1. 在**控制台窗口**启动 omp-bot 主界面（默认 `omp-bot.toml`）。
2. 在页面（本机或手机都行）点“关机”→ 确认页点“关机”→ 等 10 秒倒计时归零。
   - 归零前点“取消关机”或刷新/关闭页面：**不会**发送请求、电脑不会关，这是预期行为。
3. 倒计时归零后，页面应显示“关机指令已发出”；随后 Windows 开始通知各应用保存/退出，对超时未响应的
   进程才强制结束，然后断电。
4. 恢复后，在启动面板的那个控制台窗口里核对诊断输出：

   ```
   [omp-bot] 关机状态：invoked
   ```

请逐条确认：

- [ ] 归零后页面显示的是“关机指令已发出”（不是“关机没有成功”，也不是“结果无法确认”）。
- [ ] 各应用收到系统的关闭流程、有机会保存；超时未响应的进程才被强制结束。
- [ ] 机器最终确实断电（之后能重新开机）。
- [ ] 关机前**没有**执行“停止”脚本：后台 Python 程序不是被 `shutdown-all.bat` 停掉的。
- [ ] 若出现“关机没有成功”或 `[omp-bot] 关机状态：failed step=privilege|exit-windows-ex error=…`，
      请把原始错误一并记录后再复验（这说明本次关机没有发出）。
- [ ] 只有在你亲自做完这一次之后，才把本条记为“已验收”。

预期结果：页面反馈“关机指令已发出”，Windows 走自己的关闭流程，机器断电；恢复后重启 omp-bot 主界面即可
（关机不注销 mDNS 记录，下次启动会重新发布）。

证据位置（人工确认前均为**待确认**）：

- 端点契约与三类结果：`backend/tests/shutdown.rs`（8 个用例，注入记录型动作，**不会真的关机**）
  与 `.scratch/omp-bot-v2/evidence/07-delivery/delivery-checks.txt` §7。
- 提权与执行策略：`backend/src/shutdown.rs`（`enable_shutdown_privilege` + `ExitWindowsEx(EWX_SHUTDOWN | EWX_FORCEIFHUNG)`）、
  [ADR 0003](docs/adr/0003-shutdown-uses-exitwindowsex-forceifhung.md)；交付二进制导入表含 `ExitWindowsEx`
  （`delivery-checks.txt` §6）。
- 提权可成功启用：原型验证记录 [`.scratch/omp-bot-v2/prototype-verification.md`](.scratch/omp-bot-v2/prototype-verification.md)
  （本机 `SeShutdownPrivilege` 为“已禁用”，`AdjustTokenPrivileges` 可成功启用）——原型证据，不替代真实断电。
- **限制**：`ExitWindowsEx` 本身从未在自动化里被执行；真实断电只做这一次人工验收。

### 尚未验收的声明（v2）

P1 与 P8 在用户逐条确认前保持**未验收**；本版自动化证据（同机解析、契约测试、浏览器断言）与首版一样，
只证明 omp-bot 主界面一侧的行为与证据已备好，既不代表真机解析通过，也不代表真实断电通过。

## 本次特性（v2）的证据索引

每条结论的证据位置按票汇总在
[`.scratch/omp-bot-v2/evidence/README.md`](.scratch/omp-bot-v2/evidence/README.md)（含“哪些是一次性、
已删除”的说明），供最终移交逐条核对。要点：

- **票 01–03（mDNS）**：`backend/src/mdns.rs` / `backend/src/config.rs` 的内联单测、
  `backend/tests/mdns_conflict.rs`、`backend/tests/mdns_failure.rs`；本次交付的解析观测与 `[mdns]` 行为日志
  在 `evidence/07-delivery/`（`delivery-checks.txt`、`run-mdns-disabled.log`、`run-mdns-foreign-address.log`）；
  一次性运行配置在 `evidence/01-03-mdns/`。
- **票 04（关机端点与 Windows 动作）**：`backend/tests/shutdown.rs`（8 用例）；二进制导入表观测见
  `evidence/07-delivery/delivery-checks.txt` §6。
- **票 05（关机接管界面）**：`evidence/05-shutdown-takeover-ui/*.png`（初始态、整屏确认、倒计时、窄屏各一张）；
  逐条点击断言在票 05 的回报里，未另行落盘。
- **票 06（关机请求与三类反馈）**：`evidence/06-shutdown-request/*.webp`（`sent` / `failed` / `unconfirmed` /
  忙碌中）；一次性 harness 的夹具调用记录与网络记录已随该目录删除，**未入库**。
- **票 07（本文档）**：`evidence/07-delivery/`（构建/启动输出、三个入口、解析观测、页面断言与截图）。
- **两项人工验收（真机解析、真实断电）没有任何自动化证据**：它们等你的逐条确认。

## 变更记录：观察流独占业务快照，回执如实反馈归宿（2026-10-08）

本次只改**前端**的合并规则，后端接口、字段与关机能力不变；上面「关机」各节的当前行为描述已按本次改动更新。

- **改了什么**：会话列表与关机展示不再把命令回执里的关机状态（以及提交过的目标选中值）合并进展示
  （原先 `ShutdownControl`／`SessionsPanel` 各有一份「回执桥接」临时状态）。关机安排、监听集合与会话
  只随共享观察流更新；回执只反馈本次操作的归宿：**接受**（`shutdown-start-accepted`／`shutdown-cancel-accepted`／
  `shutdown-accepted`／`monitor-status[data-outcome=accepted]`）、**明确拒绝**（沿用既有类别码）、**未确认**。
  监听命令的回执里没有可识别的类别时按未确认处理，不再拿提交过的目标值充数。
- **用户可见结果**：回执先到时不再乐观翻出倒计时或选中态；迟到的回执不再覆盖较新的观察结果；
  观察流断开期间即使命令被确认，也只显示归宿说明，最后一份快照与「可能已经过期」保持到观察流给出新事实。
- **自动检查**（在 `frontend/` 目录执行）：
  - `npm --prefix frontend run test` → `Test Files 10 passed (10)`、`Tests 68 passed (68)`
    （含新增的到达次序与三类归宿用例：`tests/shutdown.test.tsx`、`tests/monitoring.test.tsx`；
    它们用固定 `serverTime`，并特意让回执里的时刻更早，因此不依赖严格递增 `serverTime` 或版本协议）。
  - `npm --prefix frontend run build`（`tsc --noEmit && vite build`）→ 成功，产物写入 `backend/static/`。
- **浏览器 smoke**：真实 Chromium + 真实 HTTP/SSE 页面，后端只用安全壳
  `cargo build --example safe_shutdown_shell`（记录型关机替身，**不接触系统调用**）；逐项观测、命令与原始结果见
  [`.scratch/session-lifecycle/evidence/01-command-feedback/smoke-browser.md`](.scratch/session-lifecycle/evidence/01-command-feedback/smoke-browser.md)。
  该 smoke 从未运行交付二进制、从未调用真实关机路径。
- **未由本次改动承担的范围**：会话登记的两阶段生命周期、监听集合裁定与自动触发条件的后端迁移属于同一
  spec 的其它交付工作；本节只声明页面侧的回执／观察流规则，未修改的扩展重试与重注册策略不因本节新增任何保证。

## 变更记录：会话登记的两阶段生命周期与离线监听（2026-10-09）

本次改**后端**的会话生命周期与完成后关机监听裁定，并让页面区分「最后上报」状态；接口路径、字段与关机
能力不变；上面「会话登记的生命周期」「完成后关机监听」两节的当前行为描述已按本次改动更新。

- **改了什么**：
  - 会话登记从「固定 30 秒在线窗口 + 离线无限保留」改为两个窗口（缺省在线有效期 10 秒、离线保留期
    30 秒）：超过在线有效期即离线，超过两个窗口之和由后端维护清除登记并发布 `remove`。计时起点仍是
    最后一次被接受的写入，不另存「首次发现离线」的时刻。
  - 完成后关机监听不再在成员离线时移出：离线成员留在集合里并**阻塞**自动触发；只有登记被清除、明确
    注销或用户撤销监听才移出，再对剩余集合统一判定。触发条件因此收紧为「集合非空且全部成员当前都在线
    并等待用户输入」。
  - 监听命令在提交新增后会核对一次登记是否仍在：命令处理期间被清除或注销的登记不会以幽灵成员的形式
    进入集合，也不需要等下一遍维护修复。
  - 时间来源统一为实例内注入的时钟（`backend/src/clock.rs`）：生产用进程单调时钟，验收用受控时刻把
    窗口边界变成精确的 t=10／t=11／t=40／t=41，而不是「大约」。
  - 页面对离线登记的最后上报状态加「最后上报」标记（`data-reported=last-reported`、
    `[data-testid=last-reported-tag]`）；离线登记照常显示、不隐藏，旧状态不再像是当前完成证据。
- **用户可见结果**：上报中断超过 10 秒先显示离线并继续等待，不会立刻缩小监听集合促成关机；异常退出
  未注销的会话最迟约 45 秒后被清除；清除登记只删除跟踪记录，不动 agent 对话与聊天记录。
- **自动检查**：
  - `cargo test --no-fail-fast`（在 `backend/` 执行）→ 各目标全部通过，仅
    `mdns_conflict` 的一条用例失败，原因是本机存在**先于本次改动**残留的 `omp-bot.exe`（release，
    0.0.0.0:8787）已占用 mDNS 主机名 `omp-bot`，使该用例「受控占用者必须在线」的前提不成立；它与本次
    改动无关，本记录不把它算作通过。
  - `npm --prefix frontend run test` → `Test Files 10 passed (10)`、`Tests 70 passed (70)`。
  - `npm --prefix frontend run build` → 成功，产物写入 `backend/static/`。
- **实际 smoke**（安全 adapter：记录型关机替身，**不接触系统调用**）：
  - 真实进程（安全壳 `cargo build --example safe_shutdown_shell`，默认 10／30 策略、真实每 5 秒维护循环）：
    超过在线有效期后的一个扫描周期内发布离线（11.5s），超过两个窗口之和后的一个扫描周期内发布清除
    并同步移出监听（41.5s）；全程 `phase=none`，安全替身零调用；隔离对话文件逐字节未变。
  - 真实 Chromium：在线 working、离线 working、离线 awaiting-input、在线状态缺省四类同屏，离线卡带
    「最后上报」；已清除的登记随后从页面卡片流消失（移除来自观察流）。
  - 逐项观测、原始帧时间线、DOM 数值与截图见
    [`.scratch/session-lifecycle/evidence/02-bounded-lifecycle/smoke.md`](.scratch/session-lifecycle/evidence/02-bounded-lifecycle/smoke.md)。
- **未由本次改动承担的范围**：会话窗口的启动期配置接入、写入／删除协议的完整覆盖、监听恢复与离线撤销、
  倒计时独立性与进程重置、多前端收敛与重连新鲜度属于同一 spec 的其它交付工作；扩展的重试、失败预算与
  重注册策略不变，本次也不声称任何新的恢复保证。

## 变更记录：会话窗口由启动期配置决定（2026-10-09）

本次改**后端**的启动配置与会话窗口来源；接口路径、字段与关机能力不变；上面「配置」「会话登记的生命周期」
两节的当前行为描述已按本次改动更新。

- **改了什么**：
  - 新增启动期配置 `[sessions] online_timeout_seconds` / `offline_retention_seconds`（缺省 10／30 秒，
    两个字段可分别缺省，值域都是 `1..=4294967295`）。缺 `[sessions]` 段的旧配置仍可加载并取这两个缺省值；
    非法值（0、负数、小数、字符串、布尔值、超过上界，以及拼错的字段名或段名）让**启动明确失败**并指出
    出错的配置，不静默回退到缺省值。
  - 在线有效期与离线保留期不再硬编码：实例在启动时把配置转成一份策略
    （`SessionsConfig::policy`，`backend/src/config.rs`），快照读取、维护扫描、监听新增资格与自动触发
    共用它——不同路径没有第二处阈值，也没有只解析配置却继续用硬编码阈值的路径。
  - 启动日志新增一行 `[omp-bot] 会话窗口：在线 <n> 秒、离线保留 <m> 秒`，实例实际加载到的窗口可以直接
    从启动输出读到。
  - 两个合法窗口之和按总阈值正常判定（最大 `8589934590` 秒），不溢出、不提前过期。
- **用户可见结果**：改 `[sessions]` 只影响**下一次**启动的实例；运行中的实例继续按启动时读到的窗口判定，
  重启后才用新值。非法配置在启动阶段就被拒绝，实例不会带着被悄悄替换过的阈值跑起来。
- **自动检查**：
  - `cargo test --test session_window_config`（在 `backend/` 执行）→ `11 passed`（P1–P7：缺省与单字段缺省、
    两个字段的 1 与 4294967295、非法输入矩阵、7／13 与缺省 10／30 的边界、受控时间下 `8589934590` 秒总阈值、
    监听新增资格与自动触发，以及真实进程拒绝非法配置、真实进程改写文件不生效两例）。
  - `cargo test --lib config::` → `17 passed`（含 7 条本票新增的 `[sessions]` 解析用例）。
  - `cargo test --no-fail-fast` → 各目标全部通过，仅两条与本次改动无关的用例失败：`mdns_conflict` 的一条
    （本机存在先于本次改动残留的旧 `omp-bot.exe` 占用 mDNS 主机名 `omp-bot`）与 `no_auto_termination` 的一条
    （全量并行负载下夹具未按自己的停止标记结束，单独重跑通过）。
- **实际 smoke**（安全 adapter：记录型关机替身 `cargo build --example safe_shutdown_shell`，**不接触系统调用**）：
  安全壳三实例 + 真实 HTTP／SSE。缺 `[sessions]` 的实例观察到 10 秒后离线、40 秒后清除；7／13 实例观察到
  8 秒后离线、20 秒后清除；运行中把同一份配置文件改写为 10／30 后原实例不受影响，新进程按 10 秒在线、
  40 秒清除运行；非法配置的真实启动退出码 1 且错误定位到字段与配置文件；三个实例全程安全替身零调用。
  逐项时间线、原始输出与命令见
  [`.scratch/session-lifecycle/evidence/03-startup-session-window-policy/smoke.md`](.scratch/session-lifecycle/evidence/03-startup-session-window-policy/smoke.md)。
- **未由本次改动承担的范围**：会话窗口之外的会话生命周期与监听规则（写入／删除协议、监听恢复与离线撤销、
  倒计时独立性与进程重置、多前端收敛与重连新鲜度）属于同一 spec 的其它交付工作；扩展的重试、失败预算与
  重注册策略不变，本次也不声称任何新的恢复保证。

## 变更记录：有效上报刷新与删除后的既有恢复协议（2026-10-09）

本次**没有改动后端行为**（写入／删除协议自会话生命周期迁移起就是这样），而是把这条协议固定为可执行证据，
并把「超过阈值但尚未删除 vs 已删除」的区别写进上面的用户说明；接口路径、字段、关机能力与扩展协议均不变。

- **记录下来的行为**（现在有专门用例与真实进程 smoke 覆盖）：
  - `register`／`heartbeat`／`state` 三类被接受的写入**各自**重新开始同一条身份登记的有效期；被拒绝的事件
    （非法输入 `400 invalid-event`、来源不被接受 `403 forbidden`）不重新计时，也不改变 agent 状态或监听集合。
  - 上报年龄超过两个窗口之和**不等于**立刻拒收：维护尚未执行到那一遍时，迟到的普通上报仍被接受并让登记
    重新计时；维护已实际删除登记之后，`heartbeat`／`state` 得到 `404 {"code":"unknown-session"}`，
    **`register` 是唯一建立登记的入口**。重新 `register`（同一 `sessionId` 即可）重建登记，但不自动恢复已被
    移出的完成后关机监听（监听意图不复活）。
  - 显式 `unregister` 在线阶段与离线暂留阶段都立即删除登记、立即发布 `remove`，不等任何窗口结束。
- **用户可见结果**：上面「会话登记的生命周期」一节新增了这条区别的说明；页面上不需要新行为——移除与刷新
  照旧只来自后端观察流。
- **自动检查**：
  - `cargo test --test session_report_recovery`（在 `backend/` 执行）→ `8 passed`（P12–P17、P50；真实 TCP +
    真实 SSE + 受控时刻，期望值是缺省窗口的独立字面值 t=20／30／31／40／41／60／61）。其中 P14 还含一例
    受控并发交错，断言「被接受的刷新绝不会因陈旧的清理判断被误删」。
  - 突变验证（临时改一处生产代码、确认对应用例失败、再还原并核对 `sha256`）覆盖每条 promise：心跳不刷新、
    维护按注册时刻判过期、来源校验恒真、`404` 改 `400`、`unregister` 不删行、监听集合不移出已消失的成员，
    各自都被对应用例抓住。
  - `cargo test --no-fail-fast`（在 `backend/` 执行）→ 各目标全部通过，仅 `mdns_conflict` 的一条失败
    （本机存在先于本次改动残留的旧 `omp-bot.exe` 占用 mDNS 主机名 `omp-bot`；与本次改动无关，不计为通过）。
- **实际 smoke**（安全 adapter：记录型关机替身 `cargo build --example safe_shutdown_shell`，**不接触系统调用**）：
  真实进程 + 真实 HTTP／SSE，缺省 10／30 策略、真实每 5 秒维护循环、扩展同一事件形状。观察到的关键点：
  t=20 三类刷新后 29.4s 在线、31.5s 离线、60.5s 维护前仍保留、64.5s 清除；t=42（已过 40 秒阈值、未执行清理）
  迟到心跳仍 `200 accepted` 并重新计时，84.5s 按新起点清除；t=44.5 维护删除后 `heartbeat`／`state` 精确
  `404 unknown-session`，同一 `sessionId` 重新 `register` 后登记在线而监听集合仍为空；在线与暂留阶段
  `unregister` 都立即移除；全程安全替身零调用。逐行时间线与安全壳 stdout 见
  [`.scratch/session-lifecycle/evidence/04-accepted-reports-and-registration-recovery/`](.scratch/session-lifecycle/evidence/04-accepted-reports-and-registration-recovery/)。
  **一条证据未满足**：`403 forbidden` 需要「非本机对端地址」，单一主机上无法用真实 TCP 构造，因此该条沿用仓库
  已批准的受控对端地址接缝验证（同 `tests/session_write_boundary.rs`），并在 smoke 记录里如实标注。
- **未由本次改动承担的范围**：监听恢复与离线撤销、倒计时独立性与进程重置、多前端收敛与重连新鲜度属于同一
  spec 的其它交付工作；扩展的重试、失败预算、自动重注册与心跳频率**未修改**，本次也不声称任何新的恢复保证。

## 变更记录：监听资格、恢复后的当前状态裁定与离线撤销（2026-10-09）

本次**没有改动行为**（这些规则在会话生命周期迁移时就已经是这样：新增资格只认「当前在线且工作中」、
离线成员暂留并阻塞、触发条件是「集合非空且全部成员当前在线并等待用户输入」、已监听的离线成员仍可撤销），
而是把它固定为**本票自有**的可执行证据，并把新增资格、恢复语义与离线撤销写进上面的用户说明。
接口路径、字段、关机能力与扩展协议均不变。

- **记录下来的行为**（现在有专门用例覆盖）：
  - **新增资格**只看**命令到达时**的当前状态：在线且工作中的已登记会话可新增；离线（上报过期）、空闲、
    等待用户输入、以及在线但状态尚未同步的会话都得到 `409 {"code":"session-not-eligible"}`；无登记得到
    `404 {"code":"unknown-session"}`。重申**已经选中**的目标值不是新增，不按当前资格重新判定。
  - **离线暂留**：已监听成员进入离线后仍在集合里并阻塞自动触发；保留期内的恢复上报按它**当时**的 agent
    状态参与判定——恢复成「工作中」继续阻塞，恢复成「等待用户输入」才可能满足条件。不需要重新选择。
  - **触发条件唯一**：集合非空且**全部成员当前都在线并等待用户输入**。工作中、空闲、状态尚未同步与离线
    都不能满足；不记忆成员历史上曾经结束过一次的资格。
  - **离线撤销**：仍有登记的离线已监听成员提交 `selected=false` 会被接受并立即移出，再按同一套剩余集合
    判定（移出最后一个成员只清空集合，不触发倒计时）。页面保留这个撤销入口，选中态与剩余集合仍只随共享
    观察流变化。
- **自动检查**：
  - `cargo test --test monitoring_recovery`（在 `backend/` 执行）→ `9 passed`（P24、P27、P28、P29、P32、
    P33、P34；真实 TCP + 真实 SSE + 受控时钟，缺省窗口的独立字面值 t=11；倒计时用合法最小值的真实到期）。
  - 突变验证（临时改一处生产代码、确认对应用例失败、再还原并核对 `sha256`）：新增资格丢掉 `online` 或
    `state==Working`、去掉「已选中重申」的短路、监听集合只保留在线成员、触发条件丢掉 `online` 或状态判断、
    撤销路径复用新增资格 —— 各自都被本票用例抓住（其中「触发条件丢掉 `online`」最初没被抓住，因此补了
    `an_offline_member_awaiting_input_never_satisfies_the_trigger`：离线但最后上报是等待用户输入的成员
    不满足条件）。改动已全部还原，`countdown.rs`／`sessions.rs`／`SessionsPanel.tsx` 的 `sha256` 与突变前一致。
  - `npm --prefix frontend run test` → `10 files / 71 tests passed`（含本票新增的离线撤销用例：
    离线已监听成员仍可用撤销入口、回执不翻转选中态、观察更新后选中态与剩余集合展示一起改变）。
  - `npm --prefix frontend run build` → 成功（`tsc --noEmit && vite build`）。
  - `cargo test --no-fail-fast`（在 `backend/` 执行）→ 各目标全部通过，两条失败与本票改动无关：
    `mdns_conflict` 的一条（本机存在先于本票残留的 `omp-bot.exe` 占用 mDNS 主机名 `omp-bot`）与
    `no_auto_termination` 的一条（全量并行负载下夹具未按自己的停止标记结束；**单独重跑通过**
    —— `1 passed`，80.49s）。
- **实际 smoke**（安全 adapter：记录型关机替身 `cargo build --example safe_shutdown_shell`，**不接触系统调用**）：
  - **真实进程 + 真实 HTTP／SSE**（缺省 10／30 窗口、倒计时 3 秒）：在线 working 新增接受；空闲、等待输入、
    状态缺省与离线都精确 `409 session-not-eligible`、未登记精确 `404 unknown-session`；重申既有选中值
    `200` 且集合不变；成员离线后集合不变、恢复上报后仍保有原监听（恢复成 working 继续阻塞）；离线成员
    `selected=false` 得到 `200 unselected` 并立即移出；恢复携带 `awaiting-input` 后集合全部在线等待 →
    进入倒计时，**到期安全替身恰好被调用一次**（`记录到关机动作调用 #1（结果 ok）`，全程只有这一行）。
  - **真实 Chromium 页面**：离线的「离线已监听」成员上撤销按钮可见可用（`data-selected=true`），
    真实鼠标点击发出 `POST /api/shutdown/monitor` → `200`，观察更新后该按钮消失、汇总由 `2 个` 变 `1 个`；
    再撤销剩余成员后监听栏消失；逐字捕获到的请求体是 `{"sessionId":"s-z","selected":false}`。
  - 两个阶段的原始时间线、安全壳 stdout、截图与 HAR 见
    [`.scratch/session-lifecycle/evidence/05-monitoring-recovery-and-offline-withdrawal/`](.scratch/session-lifecycle/evidence/05-monitoring-recovery-and-offline-withdrawal/)。
    到期执行只观察到「安全 adapter 被调用一次」，**不推断**真实电脑已经断电。
- **未由本次改动承担的范围**：倒计时独立性与进程重置、多前端收敛与重连新鲜度属于同一 spec 的其它交付工作；
  扩展的重试、失败预算、自动重注册与心跳频率**未修改**，本次也不声称任何新的恢复保证。

## 变更记录：倒计时独立性与进程重置的验证（2026-10-09）

本次**没有改动后端行为**（这些规则在既有关机安排与会话生命周期里就已经是这样：手动与自动入口共用同一个
全局倒计时、开始即消耗整组监听、已开始的安排不因会话变化取消或延长、活动倒计时期间拒绝新增监听、显式取消
全局清空并撤销尚可取消的倒计时、重启清空一切），而是把它固定为**本票自有**的可执行证据与真实进程 smoke；
接口路径、字段、关机能力与扩展协议均不变。上面「关机」「完成后关机监听」「会话登记的生命周期」三节的当前
行为描述已按本次记录对齐（补上了重启对登记的影响、已开始倒计时的独立性、手动入口不等监听集合）。

- **记录下来的行为**（现在有本票用例与安全壳 smoke 覆盖）：
  - **两个入口共用同一个全局倒计时**：手动开始不等监听集合是否满足自动触发条件（集合里还有工作中或离线的
    成员也立即开始，smoke 实测回执 15ms），两个入口都在开始的一刻消耗整组监听；取消之后被消耗的成员不回来，
    重复的「等待用户输入」上报也不重启安排。
  - **已开始的倒计时独立于会话**：成员离线、登记被维护清除、明确注销与重新工作都不改变安排标识与截止时间
    （不取消、不延长），到期照旧只执行一次。
  - **活动倒计时期间不接受新增监听**：对新的在线 working 登记新增监听得到精确
    `409 {"code":"shutdown-in-progress"}`，权威集合不新增。
  - **全局取消**：仅监听（没有倒计时）与尚可取消的倒计时两种情况都清空整组监听并撤销安排，回执与观察都体现
    清空，原截止点不执行；执行已开始时保留既有 `409 {"code":"shutdown-not-cancellable"}`，不宣称撤回。
  - **进程重启**：会话登记、监听意图与活动倒计时都从空状态开始（同一 `sessionId` 需要重新 `register`，
    旧监听意图与旧安排都不会回来）。
- **真实进程验证改用既有安全 adapter**：`backend/tests/manual_shutdown_process.rs` 现在启动**安全壳**
  （`examples/safe_shutdown_shell.rs`：与交付运行时同一份路由与装配、关机动作是记录型替身）而不是交付二进制，
  因此本票的全部自动化与 smoke 都不会执行系统关机。该目标会先就地构建安全壳（`cargo test --test <目标>`
  不编译 examples，只按存在性判断会驱动上一次留下的旧二进制），所以单独运行本目标也驱动当前源码；
  没有新增任何线上测试端点，交付二进制与真实关机能力未改。
- **自动检查**：
  - `cargo test --test countdown_independence`（在 `backend/` 执行）→ `7 passed`（P35、P36、P37、P38、P39；
    真实 TCP + 真实 SSE + 受控时刻，倒计时用配置里的独立字面值 3／5／10 秒真实到期，记录型关机替身）。
  - `cargo test --test manual_shutdown_process`（在 `backend/` 执行）→ `3 passed`（P19 的登记＋活动倒计时、
    登记＋监听意图两组独立场景，以及「倒计时时长只在启动期读取一次」的既有行为）。
  - 突变验证（临时改一处生产代码、确认对应用例失败、再还原并核对 `sha256`）覆盖每条 promise：手动开始等待
    集合就绪、会话变化取消活动安排、手动入口不消耗集合、活动倒计时期间接受新增、仅监听时不清空、取消不撤销
    安排、执行开始后回报取消成功、新进程恢复陈旧结果——各自都被本票用例抓住；`countdown.rs`／`lib.rs`／
    `sessions.rs` 改动已全部还原，`sha256` 与突变前一致。
  - `cargo test --no-fail-fast`（在 `backend/` 执行）→ 各目标全部通过（24 个目标 ok，含
    `no_auto_termination` 80.41s 通过），**一条失败与本票改动无关**：`mdns_conflict` 的
    `conflicting_hostname_is_neither_published_nor_renamed_and_panel_still_serves` —— 本机存在**先于本次改动**
    残留的 `omp-bot.exe`（0.0.0.0:8787）已占用 mDNS 主机名 `omp-bot`，使「受控占用者必须在线」这一前提不成立
    （实际观察到的发布者是它，本测试实例被改名为 `omp-bot (2)`）。本记录不把它算作通过。
- **实际 smoke**（安全 adapter：记录型关机替身 `cargo build --example safe_shutdown_shell`，**不接触系统调用**）：
  三段真实安全壳进程 + 真实 HTTP／SSE，40 项断言全部 PASS。阶段一（`countdown_seconds = 3`）：集合含一条离线
  成员与一条在线工作中成员时，手动开始 15ms 内建立倒计时并消耗整组监听；活动倒计时期间新增监听两次都得到
  `409 shutdown-in-progress`；「可取消的倒计时」与「仅监听」两种取消都清空集合、越过原截止时间零调用；自动入口
  同样消耗整组监听，取消后不恢复。阶段二（55 秒）：同一条安排跨过成员离线（11.5s）、明确注销＋重新工作、
  登记被维护清除（46s）三个变化，标识与截止时间逐项保持，原截止点安全 adapter **恰好调用一次**。阶段三
  （3600 秒，两组独立场景）：旧实例的「登记＋活动倒计时」与「登记＋监听意图」在重启后首帧都为空，旧 `sessionId`
  的普通上报与监听命令都得到 `404 unknown-session`，安全 adapter 零调用。逐行时间线、四份安全壳 stdout、
  三份配置与 `cargo test` 原始输出见
  [`.scratch/session-lifecycle/evidence/06-countdown-independence-and-process-reset/`](.scratch/session-lifecycle/evidence/06-countdown-independence-and-process-reset/)。
- **未由本次改动承担的范围**：多前端收敛与重连新鲜度属于同一 spec 的其它交付工作；扩展的重试、失败预算、
  自动重注册与心跳频率**未修改**，本次也不声称任何新的恢复保证；真实电脑断电不在本票验收范围（smoke 只观察到
  「安全 adapter 被调用一次」）。

## 变更记录：共享观察持续送达、慢观察者隔离与多页面收敛（2026-10-09）

本次**没有改动行为**（这些规则在共享观察流与会话生命周期里就已经是这样：一条只读长连接的首帧是与会话快照
同帧的全量关机状态、之后只推命名增量、订阅者落后太多时重发当下的完整快照、发布方不因慢读者阻塞、多条观察
连接各自收敛），而是把它固定为**本票自有**的可执行证据与真实 smoke；接口路径、字段、关机能力与扩展协议均不变。
上面「多前端、刷新、重连与传输故障」一节的当前行为描述已按本次记录对齐（补上了首帧恰含哪些登记与关机事实、
以及每一页只由自己那条连接驱动）。

- **记录下来的行为**（现在有本票用例与两段真实 smoke 覆盖）：
  - **首帧恰含当前事实**：新观察连接（含带 `Last-Event-ID` 的重连）的第一个有效快照恰含当前**全部**登记
    （在线、离线暂留）与当前关机事实（当前集合，或正在倒计时的活动安排）；已被维护清除的身份不在其中，
    恢复不读本地历史、不等待事件重放。
  - **打开期间只推命名增量**：state 写入、离线维护、登记清除与监听变化分别以 `upsert` / `upsert(offline)` /
    `remove` / `shutdown` 送达，页面无需刷新；不强制每次全量（打开期间不会出现第二次全量快照）。
  - **慢观察者被隔离**：没有读者、或读者故意不读时，写入、过期、清理与监听裁定照常完成（正常调用逐次返回），
    新观察者立刻取得当前事实，原落后观察者按有界缓冲丢增量、由重发的快照收敛到当下；不新增连接数量、
    吞吐或跨域每帧原子性保证。
  - **多页面各自收敛**：两页各经**自己的**观察连接收敛到同一实例的会话与关机事实（一页设置／撤销监听、
    离线维护与登记清除都反映于两页），不依赖把命令回执送到另一页。
- **自动检查**：
  - 新增 `backend/tests/shared_observation.rs`（P21×2、P22、P23×2、P48 的 S1 支撑证据，6 例；真实 TCP +
    真实 SSE + 受控时刻；窗口边界用缺省策略的独立字面值 t=11／41，记录型关机替身零调用）→ `6 passed`。
  - 新增 `frontend/tests/multipage.test.tsx`（P48 的 S2 自动检查：两个页面各用真实观察数据源 +
    独立假 `EventSource`，一页设置／撤销、两页看清离线与清除；另一页始终没有发出任何命令）。
  - 突变验证（临时改一处生产代码、确认对应用例失败、再还原并核对 `sha256`）覆盖每条 promise 的判别力：
    快照隐藏离线行、首帧丢掉关机状态、落后重发的快照不含当前会话、维护不发布离线、监听变化不下发、两页共用
    一条连接、按回执乐观选中——各自都被本票用例抓住；改动已全部还原，五个文件的 `sha256` 与突变前一致。
  - `cargo test --no-fail-fast`（在 `backend/` 执行）→ 24 个目标 ok；**一条失败与本票改动无关**：
    `mdns_conflict` 的 `conflicting_hostname_is_neither_published_nor_renamed_and_panel_still_serves`
    —— 本机存在**先于本次改动**残留的 `omp-bot.exe`（0.0.0.0:8787）已占用 mDNS 主机名 `omp-bot`，使
    「受控占用者必须在线」这一前提不成立。本记录不把它算作通过，本票不做该环境处置。
  - `npm --prefix frontend run test` → `Test Files 11 passed (11)`、`Tests 72 passed (72)`；
    `npm --prefix frontend run build` → 成功（`tsc --noEmit && vite build`）。
- **实际 smoke**（安全 adapter：记录型关机替身 `cargo build --example safe_shutdown_shell`，**不接触系统调用**）：
  - S1（真实进程 + 真实 HTTP／SSE，缺省 10／30 窗口，`countdown_seconds = 3600`）：47 项断言全部 PASS——
    新连接首帧恰含当前登记与当前关机事实（含活动安排那一例）；打开期间 state 写入、离线维护、登记清除与监听
    裁定分别以 `upsert` / `upsert(offline)` / `remove` / `shutdown` 送达，且只有一次全量快照；安全 adapter 零调用。
  - S2（真实 Chromium，两个标签页各自观察连接）：设置／撤销监听、离线维护与登记清除都反映于两页，另一页在
    没有任何交互、也没有命令回执的情况下经自己的连接收敛；两个标签页相互独立（一页冻结重载期间另一页仍在实况，
    重载后经自己的首帧重新收敛）；安全 adapter 零调用。
  - 逐行时间线、安全壳 stdout、两份配置、双页面 HAR 与本节全部观测见
    [`.scratch/session-lifecycle/evidence/07-shared-observation-and-multipage-convergence/`](.scratch/session-lifecycle/evidence/07-shared-observation-and-multipage-convergence/)。
- **未由本次改动承担的范围**：重连新鲜度与页面断线风险保持属同一 spec 的其它交付工作；到期执行与真实断电不在
  本票范围（两段 smoke 都取 3600 秒倒计时，安全 adapter 零调用）；扩展的重试、失败预算、自动重注册与心跳频率
  **未修改**，本次也不声称任何新的恢复保证。

## 变更记录：断线风险保持至有效首帧、页面观察不写业务（2026-10-09）

本次改动只在**页面侧**的观察新鲜度与只读边界上（接口路径、字段、后端行为、关机能力与扩展协议均不变），
修掉一个既有漏洞，并把「打开／刷新／关闭页面与观察断线不写业务」固定为本票自有的可执行证据与真实 smoke。
上面「多前端、刷新、重连与传输故障」一节的当前行为描述已按本次改动对齐（补上了「恢复实况只有一条路径」）。

- **改了什么**（`frontend/src/sseSessionFeed.ts`）：
  - 修掉「`open` 即实况」的旧行为：浏览器重连只是「这条连接又通了」，手上那份快照**仍可能过期**。恢复实况
    现在只有一条路径——一份**完整**的 `snapshot` 首帧（会话与关机两部分都读得懂）。
  - 断线期间（含只 `open`、尚无有效首帧）**展示冻结**在最后一帧并继续标注「可能已经过期」：
    `upsert`/`remove`/`shutdown` 增量各只覆盖一个域，都不算恢复证据，也不改变展示。
  - 半份快照整帧丢弃：会话那一半读得懂、关机那一半缺失（或反之）的帧不再「沿用上一份关机状态」按实况处理——
    半份快照既会凭空少行，也会把上一份好数据换成「新会话 + 旧关机状态」这种谁也不是的状态。
- **用户可见结果**：断线后卡片与已有安排照旧保留、明说可能过期；浏览器自行重连但还没拿到有效首帧时不会
  假装实况；无效首帧不清提示、不替换展示；一份完整首帧到达后一次性替换断线期间陈旧的卡片、监听集合与安排；
  打开／刷新／关闭页面与观察断线只产生读取／观察，页面从不写会话状态，因此无从刷新 `last_seen`。
- **自动检查**（在仓库根目录执行）：
  - 新增 `frontend/tests/reconnectFreshness.test.tsx`（P44 ×1、P45 ×3、P46 ×1、P47 ×1、P49 ×1；真实主界面 +
    真实 SSE 接线 + 记录型 `fetch` 替身）→ `npm --prefix frontend run test` 全绿：
    `Test Files 12 passed (12)`、`Tests 79 passed (79)`（含按新行为迁移的 `tests/sseSessionFeed.test.ts`、
    `tests/sseWiring.test.tsx` 旧断言）。
  - `npm --prefix frontend run build` → 成功（`tsc --noEmit && vite build`）。
  - `cargo test --manifest-path backend/Cargo.toml --test session_events_stream` → `7 passed`（S1 侧的读路径
    不刷新 `last_seen` 既有用例：`an_idle_read_path_keeps_alive_and_reports_stale_sessions_as_offline`）。
  - 突变验证（临时改一处生产代码 → 确认对应用例失败 → 还原并核对 `sha256`）：恢复「`open` 即实况」被 4 条用例
    抓住；快照接受缺失关机状态、去掉「断线期间不认增量」早退各被对应 P45 用例抓住；断线时清空会话并顺手发写请求
    被 5 条抓住；订阅时发写请求被 4 条抓住。改动已全部还原（`sseSessionFeed.ts` `sha256=cae8c01d…`）。
- **实际 smoke**（安全 adapter：记录型关机替身 `cargo build --example safe_shutdown_shell`，**不接触系统调用**）：
  真实 Chromium + 真实后端进程 + 真实 HTTP／SSE，37 项断言全部 PASS——断线保留卡片与已有安排并标注可能过期、
  只 `open` 时过期提示保持、受控无效首帧不清提示也不替换展示、完整首帧恢复实况并替换断线期间被删除的卡片／
  变化的监听集合／被取消的安排、打开刷新关闭与断线只产生读取请求、登记按自身窗口离线（13.16 秒）与清除
  （43.03 秒）都来自后端观察。逐行时间线、安全壳 stdout、HAR 与配置见
  [`.scratch/session-lifecycle/evidence/08-reconnect-freshness-and-readonly-observation/`](.scratch/session-lifecycle/evidence/08-reconnect-freshness-and-readonly-observation/)。
  安全 adapter 全程零调用（未驱动到期执行；3600 秒安排被显式取消）。
- **未由本次改动承担的范围**：扩展的重试、失败预算、自动重注册与心跳频率**未修改**，本次也不声称任何新的
  恢复保证；到期执行与真实断电不在本票范围；观察连接版本／重放协议、新观察连接与扩展恢复策略均未引入。

## 变更记录：三入口导航与 workspace 登记列表（2026-10-19）

本次改**前端导航与新的 workspace 登记能力**；固定操作、会话生命周期、关机安排与既有 ADR 边界不变。

- **改了什么**：
  - 顶层入口调整为「会话列表 → 文件列表 → 控制面板」；打开与整页刷新默认会话列表，不恢复上次 tab。
    三个入口等宽，都有可见选中态与键盘焦点；Tab 可逐个聚焦，Enter/Space 触发，方向键在三个入口之间
    循环切换并同步焦点；左右滑动不切换顶层 tab。控制面板名称与既有功能不变。
  - 新增「文件列表」tab：输入**服务器电脑上的**绝对目录路径并登记。页面没有客户端文件选择器；登记列表
    由后端持久保存，电脑与手机共享；取消登记只移除登记项，不删除目录或文件。
  - 新增 `backend/src/workspaces.rs`：登记文件缺省为进程工作目录下的 `workspaces.json`，可用
    `[workspaces] registry_file` 改到别处。登记先写临时文件并落盘、再原子替换；成功只表示持久化已经完成。
    登记文件不存在是空列表；读取失败、损坏内容与写入失败都是明确错误，不会伪装成空列表，也不会覆盖
    原文件。路径必须是服务器上的绝对路径且指向真实存在的目录（普通目录或 Git 工作树根目录均可）；
    登记不初始化 Git，也不扫描、聚合嵌套仓库。
  - 新增 `GET /api/workspaces`（读取）、`POST /api/workspaces`（登记）、`POST /api/workspaces/remove`
    （取消登记）；请求体为 `{"path":"服务器绝对路径"}`，成功响应为当前完整列表
    `{"workspaces":[{"path":"…"}]}`，失败响应为 `{"code":"…","message":"…"}`。
- **自动检查**：
  - `cargo test --test workspace_registry`（在 `backend/` 执行）→ `7 passed`：普通目录与真实 Git 工作树根目录
    登记、非法路径/文件/相对路径/重复登记的明确拒绝、两个独立 TCP 调用者读取同一列表、真实进程重启后
    列表仍在、取消登记前后文件内容逐字节不变、登记文件损坏/不可读/只读写入失败的隔离验证与重启复核。
  - 突变验证（临时改生产代码、确认对应用例失败、再还原并核对 `sha256`）：损坏/读取失败被当成空列表、
    跳过目录校验、吞掉写入失败，三种违反都被本票用例抓住；`workspaces.rs` 已还原
    （`sha256=c9602fbd…`）。
  - `cargo test --lib`（在 `backend/` 执行）→ `31 passed`（含 `[workspaces]` 缺省、显式路径与拼错字段名三例）。
  - `cargo test --no-fail-fast`（在 `backend/` 执行）→ 全部测试目标通过（合计 `197 passed`；含既有
    `mdns_conflict`、`no_auto_termination` 等全量并行目标），退出码 0。
  - `npm --prefix frontend run test` → `13 files / 90 tests passed`；本票新增三 tab 导航与文件列表交互用例，
    并迁移了依赖“默认控制面板/两个 tab”的旧导航检查。`npm --prefix frontend run build` → 成功
    （`tsc --noEmit && vite build`）。
  - 前端突变验证：答复到达前提前显示登记成功、把列表读取失败显示为空列表，两种违反都被本票用例抓住；
    `FileListPanel.tsx` 已还原（`sha256=dfbcefe5…`）。
- **真实 Chromium smoke**（隔离目录、隔离登记文件、真实后端进程；未执行真实关机）：
  - 桌面 `1280×800` 三个入口宽度 `135.65625 / 135.671875 / 135.65625 px`，窄屏 `390×844` 为
    `111.328125 / 111.34375 / 111.328125 px`；默认会话列表，刷新后仍为会话列表。
  - 键盘：Tab 依次聚焦会话列表、文件列表、控制面板；Enter/Space 切换；方向键循环切换并同步焦点；
    对 tablist 派发真实 `TouchEvent` 左右滑动后选中项不变。
  - 登记真实目录 `alpha` 与真实 `git init` 的 `gitrepo` 均成功；不存在的路径显示“该路径在服务器上不存在。”；
    两个浏览器 tab 读取到同一份列表；后端进程重启后列表仍在。
  - 取消登记 `alpha` 后页面显示“已取消登记…（目录与文件未删除）”，独立读取 `alpha/note.txt` 与
    `alpha/nested/inner.txt` 内容逐字节未变；登记文件只剩 `gitrepo`。
- **未由本次改动承担的范围**：workspace 文件浏览、Git 变更、Commit/Sync 与事件刷新属于同一 spec 的后续
  实现票；手机实机触摸体验（H1）不由本票宣称通过。真实浏览器窄屏检查只是 Chromium 仿真，不构成真机结论。

## 变更记录：workspace 只读文件浏览（2026-10-19）

本次新增进入已登记 workspace、浏览完整目录并只读查看文件内容；普通目录与 Git 工作区共用浏览能力，
本票不开放 Git 变更视图或 Commit / Sync 写入口。

- **改了什么**：
  - 新增 `backend/src/browse.rs`：列目录（隐藏文件、忽略文件、`.gitignore` 与 `.git` 元数据都不过滤）与
    单文件读取。每次读取都重新核对登记根与每个路径分量；绝对路径、`..` 遍历、盘符/UNC/备用数据流、
    末尾会被 Windows 改写的空格与点、符号链接、目录联接及其他 reparse point 都被拒绝；根目录被替换成
    链接/文件或删除时返回明确错误，不伪装成空目录。
  - 文本按内容识别（完整 UTF-8、无 NUL、控制字符有限），不依赖后缀；二进制返回不可文本预览原因。
    `[workspaces] preview_limit_bytes`（缺省 1 MiB）在读取内容之前生效：超限只返回大小与上限，不加载
    完整内容；恰好等于上限的文件完整预览。
  - 新增 `GET /api/workspaces/entries?workspace=…&path=…` 与 `GET /api/workspaces/file?workspace=…&path=…`；
    失败响应沿用 `{"code":"…","message":"…"}`（如 `path-outside-workspace`、`path-is-link`、
    `path-not-found`、`workspace-unavailable`、`read-failed`）。
  - 新增 `frontend/src/WorkspaceBrowser.tsx` 与 `browseApi.ts`：电脑左树右内容；窄屏逐层进入目录、从文件
    内容明确返回并恢复列表滚动。登记项新增“浏览”入口；顶层 tab 往返保留 workspace、展开状态、所选文件
    与浏览滚动，回到页面或点按“刷新”重新读取当前事实。
  - `omp-bot.toml` 与 README 补充预览上限配置；启动日志打印实际生效的上限。
- **自动检查**（在 `backend/` 或仓库根目录执行）：
  - `cargo test --test workspace_browse` → `8 passed`：真实目录集合与隐藏/忽略/`.git` 元数据一致并在页面
    夹具中可选择读取；遍历/绝对路径/UNC/末尾空格与点反例；真实目录联接、真实（WSL 创建的 NTFS reparse
    point）符号链接与文件符号链接均被拒绝且不读取根外内容；子目录与根被替换、删除时明确报错；读取前后
    文件字节、真实 Git 索引语义与待提交状态不变；文本按内容识别、二进制原因、64 字节边界、默认 1 MiB
    精确边界；超限文件在独占共享模式下仍只读元数据而不打开内容。
  - `cargo test --lib config::tests` → `25 passed`：旧配置缺 `[workspaces]`/新字段仍可解析并取默认
    1048576；显式覆盖生效；零、负数、字符串类型与拼错字段名分别被拒绝。
  - `cargo test`（全部测试目标）→ 全部通过；`npm test` → `14 files / 99 tests passed`；
    `npm run build`（`tsc --noEmit && vite build`）→ 成功。
  - 前端新增 `tests/workspaceBrowser.test.tsx` 9 例，覆盖完整目录读取、无 Git 控件、二进制/超限说明、
    目录与文件读取失败、跨顶层 tab 状态与滚动保留、窄屏进入/返回/滚动恢复、HTML 纯文本、根外链接提示
    与无响应不伪装空目录。
- **真实 Chromium smoke**（隔离真实目录、隔离登记文件、真实后端进程；未执行关机）：
  - 桌面 `1280×900`：左树右内容（树右边界 `644`、内容左边界 `656` CSS px）；展开目录后树
    `scrollHeight=2770 > clientHeight=583`，选择 `many/020.txt` 后右侧显示对应内容并保留滚动。
  - 跨顶层 tab：切到会话列表再回文件列表，workspace、展开的 `src`/`many`、所选 `README` 与树
    `scrollTop=200` 全部保留。
  - 窄屏 `390×844`：只渲染逐层进入布局；进入 `many`、打开 `many/015.txt`、返回后列表
    `scrollTop=300` 精确恢复；目录返回再进入同样恢复 300；跨顶层 tab 后目录与滚动也保留。
  - `src/page.html` 中的 `<script>window.__pwned=1</script>` 只作为文本显示，页面里没有执行标记，
    只有应用自身的打包脚本；`huge.txt` 显示 `1048577` 字节超过 `1048576` 上限且未加载内容；
    `binary.bin` 显示 NUL 字节原因；根外目录联接显示目标与不可进入原因且没有进入按钮。
  - 外部删除所选文件、子目录与 workspace 根后，页面分别显示明确错误，不显示空目录。
  - 截图见
    [`.scratch/workspace-file-browser/evidence/02-browse-workspace-files/`](.scratch/workspace-file-browser/evidence/02-browse-workspace-files/)。
- **限制与未由本票承担的范围**：标准 Windows 符号链接创建需要管理员权限或开发者模式；本机以真实
  Windows 目录联接 + WSL 在 NTFS 上创建的真实 reparse point 符号链接完成证据（测试输出
  `native=false`）。目录联接的目标显示已验证；WSL 符号链接的目标本身无法由 Windows `read_link`
  读取，页面显示“目标不可读取”，因此标准 Windows 符号链接的目标文本未单独观察。手机实机触摸体验
  （H1）仍由 08 汇总证据后交用户确认。Git 变更、diff、Commit/Sync 与文件事件刷新属于后续票。



## 变更记录：Git 待提交变更定位（2026-10-19）

本次在已登记 workspace 上新增只读 Git 状态与「文件浏览 / 变更」内部子 tab；完整回答哪些变更待提交，
但不开放 diff、Commit 或 Sync。普通目录仍只有浏览。

- **改了什么**：
  - 新增 `backend/src/git.rs`：只读查询登记根自身的工作树根身份、当前分支与待提交变更。集合遵循 Git
    原生规则：已跟踪修改/删除（已暂存与未暂存合并）与未忽略的未跟踪文件；排除 ignored untracked，保留
    后来匹配忽略规则的已跟踪文件；不 force-add，也不为识别重命名而修改真实索引。未暂存文件系统改名保持
    Git 的删除加新增；Git 已识别的暂存改名显示为旧路径 → 新路径的一项。冲突只来自未合并文件条目，普通
    命令失败与单纯 ahead/behind 不会被标成冲突。登记根不是工作树根时按普通目录处理，不聚合父仓库。
  - 新增 `GET /api/workspaces/git-status?workspace=…` 与 `GET /api/workspaces/overview`。后者为文件列表
    提供每个 workspace 的角标项数或明确错误；普通目录没有待提交数量，读取失败不会伪装成 0 项。
  - 前端新增 `gitApi.ts`；`WorkspaceBrowser` 增加当前分支、Git workspace 的内部子 tab、两视图一致的待
    提交颜色、祖先目录蓝色提示与冲突警告；`FileListPanel` 显示 `N 项待提交` 角标。删除项只出现在变更
    列表，浏览树仍只反映磁盘内容；忽略文件与 Git 元数据可浏览但不参与着色或角标。
  - README 的 workspace 章节与接口说明同步更新。
- **自动检查**（在 `backend/`、`frontend/` 或仓库根目录执行）：
  - `cargo test --test workspace_git_status` → `7 passed`：真实 Git 的 tracked 修改/删除、未忽略 untracked、
    ignored untracked、后来匹配忽略的 tracked、忽略规则文件自身；真实未暂存改名与暂存后识别改名，前后
    核对 `git ls-files --stage` / `git diff --cached` / `git diff` 语义不变；同一文件暂存+未暂存只计一次；
    真实 merge 冲突为独立类别且解决后消失；外部切分支与 detached HEAD；ahead/behind 分叉不报告冲突；
    普通目录、仓库子目录、登记目录消失与未登记路径的明确结果；角标的固定期望（暂存+未暂存一次、识别
    重命名一次、未识别删加两次、新目录内文件一次、仅 Git 元数据变化为零）与概览错误项；
    `.git/HEAD` 损坏时状态接口与概览都明确报错，不返回干净仓库或零计数。
  - 后端突变验证：临时把 `A` 分类改成 `modified`，`pending_set_follows_git_tracking_and_ignore_rules`
    因 `staged-new.txt` 未显示为 `added` 失败；还原后 7 例全部通过。
  - 前端 `npm test` → `15 files / 105 tests passed`；新增 `tests/gitChanges.test.tsx` 6 例，覆盖分支与
    内部子 tab、两视图颜色与祖先提示、删除项只在变更列表、忽略文件/`.git` 无着色、冲突独立警告、主动
    刷新反映外部切分支、状态读取失败不冒充干净、普通目录无 Git 入口，以及角标固定期望与概览失败。
  - 前端突变验证：临时把祖先目录提示收窄为精确路径、把冲突色合并为普通修改蓝，两处都被新增用例抓住；
    还原后通过。`npm run build`（`tsc --noEmit && vite build`）成功。
  - `cargo test --no-fail-fast`（全部测试目标）→ 全部通过（`218 passed` / `29 suites`，含既有 mDNS、会话、
    关机与浏览用例）。
- **真实 Chromium smoke**（隔离真实 Git 仓库、隔离登记文件、真实后端进程；未执行关机）：
  - 浏览树计算颜色：新增绿 `rgb(126, 231, 135)`、修改/识别重命名/祖先目录蓝 `rgb(121, 192, 255)`、
    删除红 `rgb(255, 123, 114)`、冲突橙 `rgb(255, 166, 87)`；忽略文件、`.git/HEAD` 与无变更目录保持
    默认色。删除项不在树中，`src` 与 `src/nested` 仍为蓝色。
  - 变更列表逐项类别与颜色一致；识别重命名显示 `src/old-name.txt → src/renamed.txt` 一项；冲突提示条
    「冲突不是普通修改」；页面没有 Commit / Sync。
  - 页面打开期间外部执行 `git merge --abort` 与 `git switch -c feature`，主动刷新后分支显示
    `分支：feature`、变更数 `6 → 5`、冲突警告消失；返回列表角标 `5 项待提交`。
  - 状态/概览查询前后 `git ls-files --stage` 与 `git diff --cached --binary` 的 SHA-256 相同。
  - 窄屏 `390×844` 的两个视图颜色一致。截图与细节见
    [`.scratch/workspace-file-browser/evidence/03-discover-git-changes/`](.scratch/workspace-file-browser/evidence/03-discover-git-changes/)。
- **限制与未由本票承担的范围**：真实浏览器证据来自无头 Chromium 与仿真窄屏，手机实机触摸/软键盘体验
  （H1）仍由 08 汇总后交用户确认。Git 识别结果可能随仓库内容变化；本票如实采用当前结果，不保证所有
  改名都会被识别。diff、Commit、Sync、文件事件自动刷新与冲突恢复流程属于后续票。

## 变更记录：审查最终内容与完整 diff（2026-10-19）

本次让 Git workspace 的变更视图逐个审查从当前分支最新本地提交到最终本地内容的 diff；新增、删除、
部分暂存与 Git 已识别重命名都使用同一份事实。Commit / Sync 仍由后续票提供。

- **改了什么**：
  - `backend/src/git.rs` 新增 `change_diff`：旧侧只取 `HEAD`（尚无首次提交时按空树处理），新侧读取最终
    本地文件，合并观察已暂存与未暂存内容；索引与远端都不作为旧侧。Git 已识别重命名通过 `oldPath`
    定位 HEAD 中的旧路径；新增与删除分别用 `absent` 明确表示空的一侧。
  - 新增 `GET /api/workspaces/diff`，两侧各自应用 `[workspaces] preview_limit_bytes`（默认 1 MiB）：
    超限时在读取完整内容之前停止，二进制给出不可文本 diff 原因，`complete:false` 时不渲染完整 diff。
    该入口先校验新路径与旧路径写法，再核对登记根；新侧复用浏览读取（链接、目录联接、根替换均拒绝），
    旧侧只从核对过工作树根的 Git 对象库读取。`git ls-tree` / `git cat-file` 不写索引、不暂存、不改文件。
  - 前端新增 `diff.ts`（行级 LCS 与左右配对）和 `diffApi.ts`；变更列表成为选择入口，电脑左侧列表、右侧
    diff 并默认左右对照；窄屏默认统一 diff，提供明确返回并恢复列表滚动位置。切换内部视图或顶层 tab 保留
    所选变更、diff 与滚动位置。diff 内容只作为文本节点渲染，不解析 HTML。
  - README 的目录结构、workspace 章节与接口说明同步更新。
- **自动检查**（在 `backend/`、`frontend/` 或仓库根目录执行）：
  - `cargo test --test workspace_diff` → `4 passed`：真实部分暂存仓库（HEAD `beta`、索引 `STAGED`、最终
    `STAGED/FINAL`，另有与 HEAD 不同的远端跟踪引用 `origin/main`）证明旧侧是 HEAD，而不是索引或远端；
    未暂存新增与删除分别可审查；已识别改名读取旧路径；无首次提交仓库按空树比较且待提交项仍为绿色新增；32 字节精确边界、旧侧超限/新侧很小、
    旧侧很小/新侧超限、两侧二进制；根外路径、遍历、绝对路径、真实符号链接、Windows 目录联接、根替换、
    旧路径遍历、未登记 workspace 与封闭查询面都被拒绝且不泄露根外内容；请求前后 `.git/index`、
    `ls-files --stage`、已暂存/未暂存 diff 与所有文件字节的 SHA-256 完全不变。
  - `cargo test --test workspace_git_status` → `7 passed`：状态、分支、待提交集合与角标回归；确认抽出
    `worktree_root` 后普通目录、父仓库子目录与损坏 Git 元数据的行为未改变。
  - `cargo test --no-fail-fast` → `222 passed / 30 suites`（含既有 mDNS、会话、关机、浏览与 Git 状态用例；
    0 failed）。
  - `npm test` → `16 files / 113 tests passed`；新增 `tests/diffReview.test.tsx` 8 例，覆盖宽屏左右对照、
    重命名旧路径、新增/删除空的一侧、HTML/script 纯文本、超限与二进制不渲染完整 diff、内部视图与顶层
    tab 往返上下文、窄屏统一 diff、明确返回与精确滚动恢复。
  - 前端突变验证：临时把 `complete:false` 也渲染成 diff 行，超限与二进制两例失败；还原后 8 例通过。
    后端突变验证：临时把旧侧从 HEAD 改成索引对象，`diff_reads_head_to_final_content_without_touching_index_or_files`
    因旧侧返回 `STAGED` 而不是 `beta` 失败；还原后 4 例通过。
    `npm run build`（`tsc --noEmit && vite build`）成功。
- **真实 Chromium smoke**（隔离真实 Git 仓库、隔离登记文件、真实后端进程；未执行关机）：
  - 桌面 `1024×800`：左列表右 diff，默认 `data-mode="side-by-side"`；`src/modified.txt` 旧侧显示 HEAD 的
    `beta`，新侧显示 `STAGED` 与 `FINAL`；`src/new-name.txt` 标题为
    `src/old-name.txt → src/new-name.txt`，旧侧读 HEAD 旧路径。
  - 列表 23 项时 `scrollHeight=789 > clientHeight=518`，滚动到 `260` 后仍可选择并显示 diff；切换
    「文件浏览 / 变更」与「会话列表 / 文件列表」后，所选变更、diff 与 `scrollTop=260` 均保留。
  - `src/limit-old.txt`（旧侧 4 950 000 字节）与 `src/limit-new.txt`（新侧 4 950 000 字节）分别显示超过
    1 048 576 字节上限、未加载完整内容；`src/binary.txt` 显示 NUL 字节原因；两者都没有 `diff-view`，但
    变更行、类别着色与角标（`变更（23）`）不变。
  - `src/script.html` 的 `<script>` 与 `onerror` 图片标签作为字面量文本显示，diff 容器 HTML 已转义，
    `window.__pwned` 未定义且没有 `img` 元素；唯一 `script` 元素是应用自身打包脚本。
  - 窄屏 `390×844`：默认列表，进入后 `data-mode="unified"`，显示 `- beta / - gamma / + STAGED / + FINAL`
    与「返回变更列表」；返回后列表 `scrollTop` 从 `260` 精确恢复为 `260`。
  - 连续读取目录、文件与四类 diff 前后，`.git/index` 与 `git ls-files --stage` / `git diff --cached
    --binary` / `git diff --binary` 及所有文件字节的 SHA-256 全部相同（`unchanged: true`）。
  - 截图与细节见
    [`.scratch/workspace-file-browser/evidence/04-review-final-content-diffs/`](.scratch/workspace-file-browser/evidence/04-review-final-content-diffs/)。
- **限制与未由本票承担的范围**：宽屏与窄屏证据来自无头 Chromium，窄屏为视口仿真；手机实机触摸、diff
  滚动与返回手感仍属于 human-led H1，由 08 汇总后交用户确认。Commit / Sync、审查版本复核、文件事件
  自动刷新与冲突恢复流程属于后续票。

## 变更记录：经审查复核的整仓 Commit（2026-10-19）

本次在变更视图加入常驻提交信息与 `Commit` 主按钮：一句确认后重新核对页面展示的审查版本，再暂存整仓
符合 Git 规则的最终变更并创建本地提交；不自动 push。Sync 仍由后续票提供。

- **改了什么**：
  - `backend/src/git.rs` 为 `GitStatus` 增加 `reviewToken` 与 `blockedReason`。`reviewToken` 覆盖分支、
    HEAD、完整索引、待提交路径集合与每个路径的完整最终内容（直接读文件，不受预览上限影响）；嵌套仓库
    与子模块只记录 gitlink 提交，不递归内部文件。冲突、detached HEAD 与未完成的 merge/rebase/…/revert
    流程作为仓库级阻塞原因明确返回；普通命令失败不会被标成冲突。状态与 diff 查询同样应用
    `--ignore-submodules=dirty`，子模块内部脏文件不产生父仓库变更。
  - 新增 `backend/src/commit.rs` 与 `POST /api/workspaces/commit`：空信息、普通目录、阻塞状态、失效审查、
    干净仓库与同仓库并发写都明确拒绝；复核通过后 `git add --all` + `git commit -m`，既有部分暂存不会
    缩小范围，ignored untracked 不会被强加，失败与超时都保留已成功步骤和 Git 输出。每个实际仓库按规范化
    工作树根持有独立写资格，路径别名共用同一资格；请求断开不取消已接受执行，终止未确认时资格继续保持。
  - 所有 Git 命令改为可配置时限（`[git] command_timeout_seconds`，缺省 300 秒）：超时结束 Git 及其派生
    进程树，页面显示本功能时限与已有输出；超时不是 Git 退出消息。
  - 前端 `gitApi.ts` 增加提交客户端；`WorkspaceBrowser` 增加常驻提交信息、状态驱动的 Commit 按钮、一句
    确认/取消门禁、成功/拒绝/未确认反馈与阻塞原因。提交信息在内部视图与顶层 tab 往返中保留；确认后失败
    仍保留信息并刷新实际内容，必须重新审查确认。窄屏下输入框与确认按钮保持 44px 触摸目标。
  - `omp-bot.toml` 与 README 的配置、workspace、接口与变更记录同步更新。
- **自动检查**（在 `backend/`、`frontend/` 或仓库根目录执行）：
  - `cargo test --test workspace_commit` → `19 passed`：真实隔离仓库、bare 远端与 hooks 覆盖完整待提交
    集合与最终提交树（tracked 修改/删除、后来匹配忽略的 tracked、未忽略 untracked、ignored 不强加）、
    部分暂存不缩小范围、不 push；同路径同数量内容变化、外部暂存、HEAD 变化、切分支、超限/二进制内容
    变化都使审查失效且不写入；空信息/未知字段/查询参数被拒绝；hook 拒绝保留索引并返回纯文本诊断；无 HEAD
    首提交；普通目录、detached HEAD、未解决冲突、冲突已解决但 merge 未完与外部 abort 恢复；嵌套仓库与
    子模块 gitlink 范围且不改内部工作区；并发、断线、路径别名、超时与派生进程终止、资格独立于固定操作；
    缺失目录/被文件替换；无 upstream 可本地提交且配置不变；编辑器不打开、签名失败、凭据不交互；预览限制
    不改变提交树；干净仓库拒绝空提交。
  - `cargo test --lib commit::tests` → `2 passed`：终止未确认时资格保持、已确认或普通结果释放资格。
  - `cargo test --no-fail-fast` → 除一次既有 `fixture_behavior::long_running_fixture_stays_running_until_the_stop_entry_point`
    在满载并行下被夹具自带结束脚本强制结束外，其余全部测试目标通过；该用例单独重跑通过。本票新增的
    19 个集成用例与 2 个单元用例全部通过。
  - `npm test` → `17 files / 126 tests passed`；新增 `tests/commit.test.tsx` 13 例，覆盖常驻输入与空信息、
    tab/内部视图往返、一句确认与取消无写、成功、失效后保留信息并再次确认、响应中断未确认、执行中切 tab、
    重载不重放、HTML/script 纯文本、超时文字、普通目录/detached/冲突阻塞。
  - `npm run build`（`tsc --noEmit && vite build`）成功。
- **真实 Chromium smoke**（隔离真实 Git 仓库、bare 远端、真实交付后端进程；未执行关机）：
  - 桌面 `1024×720`：常驻提交信息与 Commit、一句确认框；取消后 HEAD、索引与远端引用均未变化。
  - 真实 `pre-commit` hook 拒绝：页面显示 Git 返回的 `<script>…</script>` 字面量，反馈区域内无 `script`/
    `img` 元素且 `window.__pwned` 未定义；HEAD 不变，暂存内容保留。
  - 确认前把同路径同数量的文件内容从 A 改成 B：以 `review-invalid` 拒绝，提交信息保留，刷新后显示实际
    内容；HEAD/索引/远端未变化。
  - 重新审查后确认：提交 `98dc4b34…`，提交树包含 `.gitignore`、`added.txt`、`tracked.txt`，删除与部分
    暂存范围正确，`ignored.log` 不在树中；远端保持 `4a04214…`（未 push）。
  - Commit 执行中切到会话列表再返回：执行未取消，返回显示真实成功结果；整页重载：默认回到会话列表，
    `rev-list --count HEAD` 只增加一次，随后变更视图显示实际干净状态。
  - 窄屏 `390×844`：提交信息与 Commit 在变更列表上方，输入框高 44px；确认/取消按钮均 44×112px；窄屏
    提交成功并显示空变更列表。
  - 截图与逐项 CLI 核对见
    [`.scratch/workspace-file-browser/evidence/05-commit-reviewed-repository/`](.scratch/workspace-file-browser/evidence/05-commit-reviewed-repository/)。
- **限制与未由本票承担的范围**：真实手机触摸与软键盘体验（H1）仍未确认；窄屏截图只是 agent 准备的证据。
  外部进程不受本服务原子锁定，确认后的复核保留规格接受的竞争窗口。Sync、文件事件自动刷新与冲突恢复
  流程属于后续票。真实环境中未能让系统 `taskkill` 失败以制造“终止未确认”的进程级样例；该分支由单元
  测试验证资格保持，并以真实超时执行中的重叠请求验证不重叠，仍如实列为未覆盖的进程级场景。

## 变更记录：跨流程验收与手机 H1 证据包（2026-10-19）

本次完成 01–07 的跨流程验收：文件功能与既有固定操作、会话、全局关机互不破坏；主动刷新不由节流缓存
代替查询；本地事件不触发持续远端 fetch；独立调用者可浏览并执行约定 Git 操作，而会话写入的同机来源
边界保持不变。新增代码只用于验收用例与 H1 证据，不改变产品实现。

- **自动检查**：
  - 新增 `backend/tests/workspace_integration.rs`（3 例）：P42 真实监听期间文件与 Git 元数据事件
    不 fetch（远端独立前进后本地跟踪引用与检查时间不变，显式检查才更新）；P46 受控局域网来源可浏览、
    读取文件并 Commit，非本机会话写入仍返回 `403 forbidden` 且不改动会话，封闭请求面没有命令/脚本/
    终端入口；P3 真实监听存在时固定操作、会话写入、关机开始/取消命令仍按原语义工作，记录型关机动作
    零调用 → `3 passed`。
  - 新增 `frontend/tests/integration.test.tsx`（4 例）：全局关机提醒在三个顶层 tab 均可见且切 tab 不
    发命令；文件功能存在时固定操作仍执行；节流窗口内进入 workspace、回到页面与手动刷新都立即显示
    外部改变的文件与分支；读取失败不伪装成空目录或干净仓库。
  - `cargo test --no-fail-fast` → `286 passed / 34 suites`（0 failed）；`npm --prefix frontend run test`
    → `21 files / 150 tests passed`；`npm --prefix frontend run build` → 成功。
  - 首次与前端构建并发运行时 `mdns_conflict` / `mdns_failure` 曾因静态页面目录正在替换而得到 404；
    构建完成后单独重跑 `3 passed`，不是产品回归。
- **真实 Chromium smoke**（隔离目录、隔离仓库与 bare 远端、真实交付进程；未执行关机、未推送业务远端）：
  - 窄屏 `390×844`：三个入口各 `116.66 px`；浏览树包含隐藏/忽略/`.git` 文件，删除项只在变更列表；
    变更颜色为新增绿、修改/识别重命名蓝、删除红；统一 diff 92 行可滚动。
  - Commit：输入框与按钮均高 44px，确认/取消按钮均 `112×44`；取消无写入；确认前外部改内容得到
    `review-invalid`，HEAD 不变且提交信息保留；重新确认后提交树与忽略/删除范围正确，未 push。
  - Sync：检查远端只更新跟踪引用；取消无拉取；确认后 fast-forward 拉取 1 个提交且不产生无意义推送。
  - P41：第一个本地事件 23ms 自动刷新；窗口内第二个事件 1.5s 后仍未自动显示，回到页面后 81ms 显示；
    手动刷新 56ms 显示；重新进入 workspace 14ms 显示。长变更列表 `scrollTop=300` 在 diff 返回后保持 300。
  - P42：4 次本地事件后远端跟踪引用仍为旧值且 `remoteCheckedAt=null`；显式“检查远端”后才更新。
  - 宽屏 `1280×900`：左树右内容、左右对照 diff（180/180 行）。
- **H1 交接**：唯一手机实机验收包在
  [`.scratch/workspace-file-browser/evidence/08-verify-integration-and-mobile-acceptance/`](.scratch/workspace-file-browser/evidence/08-verify-integration-and-mobile-acceptance/)，
  含 22 张真实 Chromium 截图、可重复执行的隔离场景脚本、局域网访问步骤（本次 WLAN IPv4
  `192.168.5.3`）、逐项触摸/返回/滚动/软键盘/确认取消检查清单，以及待用户填写手机型号、系统与浏览器。
  当前状态为 **未验收**；本票保持 in-progress，等待用户确认。
- **限制**：真实手机触摸与软键盘体验仍未确认，窄屏截图只是 `390×844` 视口仿真；未执行真实关机；
  Git 结果只针对隔离 bare 远端，未向业务远端推送。可信局域网无认证提醒已补入“文件列表与 workspace
  登记”一节。

## 变更记录：新增 workspace 与 HTML 阅读模式（2026-10-21）

本次把“文件列表”改成直接展示列表与两个弹窗入口，并加入由服务端配置父目录的新建 workspace 流程；同时为
workspace 内的本地 HTML 增加整页静态阅读模式。文档记录见 [ADR-0007](docs/adr/0007-workspace-creation-explicit-parent-and-partial-failure.md)
与 [ADR-0008](docs/adr/0008-html-reader-static-workspace-boundary.md)。

- **改了什么**：
  - `[workspaces] create_parent_directory` 为必须显式配置的可选字段，随附 `omp-bot.toml` 使用
    `E:\Developer\bot-playground`。旧配置缺字段仍可启动，但“新增”禁用；父目录缺失不自动创建。
  - 新增 `POST /api/workspaces/create`（JSON `{"name":"直接子目录名称"}`）；列表、登记、创建与取消登记
    的成功响应带 `creation.available` 与可选 `parentDirectory`。请求只接受名称，父目录不能由客户端替换。
  - 创建顺序为目录 → `git init` → 登记；同名目录拒绝，名称限制为单一直接子目录。任一步失败保留已经
    产生的目录，错误包含失败阶段与完整路径；登记失败时目录与 Git 初始化不撤销。
  - 新增 `GET /api/workspaces/reader/<workspace-hex>/<relative-path>` 原始资源路由，按扩展名返回 MIME、
    `nosniff`、`no-store` 与 CSP；仍复用 workspace 根、链接与路径遍历边界检查。
  - “文件列表”去掉常驻路径输入与说明文字，直接显示列表及“登记”“新增”按钮；两者改为弹窗确认。
  - `.html`/`.htm` 文本文件增加“阅读 HTML”入口。内容在父页面移除脚本、表单、嵌入、meta refresh、
    事件处理器和外部资源属性后，放入 `sandbox="allow-same-origin"` iframe；CSS、图片与原生折叠内容
    可读。退出恢复原 workspace、目录、所选文件与滚动位置。
  - 阅读器内 workspace HTML 链接在当前模式打开，`http`/`https` 链接在新标签页打开，`file://` 与其他
    scheme 被拦截；外部资源不会自动加载。`GLOSSARY.md`、README、`omp-bot.toml` 同步更新。
- **自动检查**（在 `backend/`、`frontend/` 或仓库根目录执行）：
  - `cargo test -- --skip conflicting_hostname_is_neither_published_nor_renamed_and_panel_still_serves`
    → `297 passed / 36 suites`，1 个环境受限用例被跳过。
  - `cargo test` 的完整运行中，除上述 `mdns_conflict` 用例外全部通过。当前机器已有另一个 omp-bot 服务
    监听 `8787`，该用例无法建立自己的受控 mDNS 占用者；单独重跑同样受环境阻塞，与本次改动无关。
  - `cargo test --test workspace_create --test workspace_reader` → `6 passed`：覆盖真实目录创建与
    `git init`、持久化登记、未配置/父目录缺失/非法名称/同名冲突、创建请求面封闭、登记失败保留目录、
    HTML/CSS 原始响应与 MIME/CSP、越界与未登记 workspace 拒绝。
  - `npm test` → `23 files / 167 tests passed`；新增 `htmlReader.test.ts` 及文件列表、浏览器交互用例，
    覆盖弹窗确认、创建成功/失败、阅读入口与退出、净化规则、链接分类和 iframe 内链接分流。
  - `npm run build`（`tsc --noEmit && vite build`）成功；新增 Rust 文件通过 `rustfmt --check`。
- **真实 Chromium smoke**（隔离登记文件与 HTML 资源、真实交付后端进程；新增 workspace 落到配置父目录）：
  - 页面“新增”弹窗显示 `E:\Developer\bot-playground`；确认后反馈
    `已创建：E:\Developer\bot-playground\omp-smoke-html-20261021`，列表从 1 项变为 2 项；
    `git rev-parse --is-inside-work-tree` 返回 `true` 且 `.git` 存在。
  - HTML 阅读层矩形为 `0,0,1280×800`，与 `1280×800` 视口一致；workspace CSS 生效
    （`color: rgb(1, 2, 3)`），相对图片加载 (`naturalWidth > 0`)，原生 `<details>` 保留。
  - iframe 内 `script`/`iframe`/`form` 数量均为 0，`window.__pwned` 与 `dataset.scriptRan` 为 `null`，
    外部图片数量为 0；CSP 与指向 workspace 资源路由的 `<base>` 均存在。
  - 点击 workspace 内 `next.html` 链接后阅读路径变为 `next.html` 且显示目标标题；外部链接实际打开
    `https://example.com/article` 新标签页，阅读路径不变；`file:///C:/Windows/win.ini` 链接的 `href`
    已在净化后移除。
  - 点击右上角“退出”后阅读层消失，仍停留在原 workspace，文件路径仍为 `index.html`，源码预览可见。
- **限制与未覆盖**：阅读模式设计上不执行 JavaScript，因此依赖脚本的交互页面不在支持范围；真实手机触摸、
  软键盘与窄屏阅读体验尚未做实机验收。当前环境存在另一个 omp-bot 实例，`mdns_conflict` 全量测试的
  端口冲突仍需在无既有实例时复验。
## 变更记录：目录登记概念由 workspace 更名为 zone（2026-10-21）

为引入 herdr 相关模块、避免与 herdr 自己的 workspace 概念重名，本项目把「用户登记的服务器目录」统一更名为 zone。更名覆盖前后端标识符与文件名、HTTP 路由 `/api/zones/*`、查询参数 `zone`、JSON 字段 `zones`/`zone`、错误码 `zone-*`、配置段 `[zones]`、默认登记文件 `zones.json` 与界面文案。这次是硬切换：旧 `/api/workspaces/*`、旧 `[workspaces]` 配置与旧 `workspaces.json` 不再兼容，现有登记已一次性迁移。Git working tree 在文档与界面中统一称「工作树」。历史变更记录保留旧词；原因与取舍见 [ADR 0009](docs/adr/0009-rename-workspace-to-zone-for-herdr.md)。

## 人工验收移交（Bot 模板与创建，H2，未验收）

契约 P45 的人工 H2 由用户在日常手机上确认触控、键盘输入、创建阶段与结果体验。自动化已覆盖桌面与
移动尺寸的真实浏览器、局域网地址访问、模板共享与来源边界，但这些证据**不构成**真机结论；在用户
逐条确认前保持“未验收”。

### 访问入口

- 手机与电脑连**同一个普通家庭 Wi-Fi**；在电脑上启动 herdr-bot（监听地址默认
  `0.0.0.0:8787`，见[访问（本机与局域网）](#访问本机与局域网)）。
- 手机浏览器打开 `http://herdr-bot.local:8787/`；mDNS 解析不了时用
  `http://<本机局域网 IPv4>:8787/`（本次自动验证使用的地址是 `192.168.5.3`，以 `ipconfig` 实际值为准）。
- 不需要登录、账号或 token；打开页面即可管理模板并创建 Bot。仅限可信局域网：任何能访问该端口的
  设备都具有这些能力。

### 日常手机验收步骤（约 3 分钟）

1. 点 `bot` 入口 → “新增模板”：输入显示名，用字段按钮插入 `{zone.name}` 等占位符并看示例预览；
   点击变量旁的 × 或在变量后按退格可整块删除；选择 agent / model / thinking / Skill；在固定 prompt 中
   用回车输入两行以上；保存。回到列表确认字段完整，再编辑一处并保存，最后删除确认消失。若还要创建
   Bot，请保留或重新新增一个模板。
2. 点 `zone` 入口 → 在某个 zone 行点“+”。列表行只显示末级目录名和数字角标，不再提供独立详情按钮；
   完整路径可在进入 zone 后从浏览工具栏核对。弹窗应显示该 zone 路径；选择模板，在多行 input 中用手指
   和键盘输入两三行；确认只读名称预览可见且**没有**手工改名入口。
3. 点“创建”。应依次看到“正在启动”→“正在提交初始指示”→（真实 Herdr / agent 就绪并提交后）
   “创建成功，初始指示已提交”。结果中的 Bot 名、zone、Herdr Tab / Pane 标识应完整可读；较长内容可在
   弹窗内上下滚动，底部“关闭”按钮始终可以滚动到达。
4. 点“关闭”，页面应停留在 `zone` 入口，不自动跳到 `chat`。可在 Herdr 中按结果里的 Tab / Pane
   标识核对目标 Workspace 与实际目录。
5. 失败样本（可选）：把 `[herdr] workspace_name` 改成不存在的名称后重启，或让目标 agent 拒绝提交；
   确认失败阶段与“现场已存在 / 未确认投递”的说明可读、成功文案不出现、没有自动重发，并能自行关闭。

### 已准备的自动证据（真机确认前不构成 H2 结论）

- `frontend/browser-tests/mobileWorkflow.spec.ts`：1280×900 与 390×844 真实 Chrome 各自完成模板新增、
  读取、编辑、删除，以及从 zone 打开弹窗、选择模板、键盘输入、提交和关闭；移动档分别操作成功与失败
  结果，并检查无横向溢出、弹窗完整落在视口内、操作按钮不被挤出屏幕、触控目标 ≥44px、输入字号
  ≥16px、键盘换行与两个稳定阶段。
- `frontend/browser-tests/lanAccess.spec.ts`：服务绑定 `0.0.0.0`，真实浏览器经本机局域网 IPv4 完成
  模板 CRUD 与创建；独立 HTTP 客户端经同一地址读取同一份模板；请求无 `Authorization`，页面无登录步骤。
- `backend/tests/bot_source_boundary.rs`：同一受控局域网来源可以读取/保存模板、预览并提交创建（202），
  而 `POST /api/sessions/events` 仍为 `403 forbidden`；`backend/tests/session_write_boundary.rs` 继续
  覆盖原同机来源边界。
- 本次实际运行：`npm --prefix frontend run test:browser` → `10 passed`；`npm --prefix frontend run test`
  → `25 files / 178 tests passed`；`cargo test --manifest-path backend\Cargo.toml` →
  `334 passed / 42 suites`（3 个默认忽略的 mDNS 用例）。

### 未覆盖与已知限制

- 移动尺寸 Chromium 触摸仿真不等于真实手机：软键盘高度、浏览器动态工具栏、iOS 聚焦缩放策略和手指
  触控手感只能在实机上确认；当前状态为**未验证**。
- 本票在 02 之后实现，03–07 后续合入的配置、输入与结果文案还需在最终交接时做一次同样的手机回归；
  本票不重复认领它们的业务测试。
- 真实 Herdr / OMP / Pi 的账号、模型、登录与审批属于 H1（02–05 准备），本票不把受控 Herdr 记录或
  移动尺寸仿真当作真实运行结论。

## 变更记录：桌面与移动端完成模板管理与创建（2026-10-21）

本次完成 P45：电脑与手机都能管理共享 Bot 模板并从 zone 创建 Bot；窄屏布局按真实浏览器暴露的问题
修复，不新增账号、token 或权限系统，也不改变 connect-bot 会话事件写入的同机来源边界。

- **改了什么**：
  - 弹窗遮罩改为可滚动容器，弹窗自身限制在动态视口高度内并在内部滚动；内容变长（阶段、成功/失败、
    定位信息）后顶部不被裁掉、底部操作仍可滚动到达。窄屏遮罩留白收紧到 `0.75rem`。
  - 窄屏下弹窗操作区改为等分两列，“关闭/取消”不再被主操作挤出屏幕左侧；字段插入按钮提升为
    至少 44px 高的触控目标。
  - 新增桌面/移动真实浏览器工作流用例、局域网浏览器用例与后端来源边界用例；README 的测试说明与
    H2 手机验收步骤同步更新。
- **自动检查**：
  - `npm --prefix frontend run build`（`tsc --noEmit && vite build`）成功。
  - `npm --prefix frontend run test:browser` → `10 passed`，其中 `mobileWorkflow.spec.ts` 3 项、
    `lanAccess.spec.ts` 1 项。
  - `npm --prefix frontend run test` → `25 files / 178 tests passed`。
  - `cargo test --manifest-path backend\Cargo.toml` → `334 passed / 42 suites`（3 个默认忽略的 mDNS 用例），
    包含新增的 `bot_source_boundary`。
- **限制与未覆盖**：真实手机触控、软键盘与浏览器工具栏仍需 H2 用户确认；03–07 合入后的配置、输入与
  结果文案需要一次移动回归。移动尺寸仿真与局域网地址请求都不冒充真机或非本机来源结论。

## 变更记录：移动端 zone 行与 Bot 模板命名交互（2026-10-22）

本次调整四个顶层入口的可见文案为 `chat` / `bot` / `zone` / `server`，并修正移动端 zone 列表与
Bot 模板编辑体验；不改后端接口、存储格式、默认入口或业务状态。

- **改了什么**：
  - `bot` 入口的 bot-name 规则改为行内变量编辑器：固定文字可自由编辑，变量可点击 × 或在变量后按
    退格整块删除，插入按钮跟随当前光标位置；名称示例移到表单顶部并继续实时预览。模板列表展示渲染后的
    bot-name 示例，不再显示原始规则。
  - 四个下拉框显式使用深色控件配色并固定选中项的可见文字颜色，包括“沿用 Agent 默认值”和“无”。
  - `zone` 入口每行只显示末级目录名；待提交角标只显示数字，读取失败显示紧凑警告图标，并保留可访问的
    错误原因。列表不再提供独立详情按钮；完整路径保留在浏览入口的悬停标题与无障碍名称中，进入 zone 后
    由浏览工具栏显示，同名末级目录仍按完整路径区分。
  - 顶层入口保持原有顺序与功能归属：`chat`（会话）、`bot`（Bot 模板）、`zone`（登记与文件浏览）、
    `server`（固定操作）；打开与刷新仍默认进入 `chat`。
  - `GLOSSARY.md` 与 README 当前操作说明同步入口术语；历史变更记录与 ADR 保留原文。
- **自动检查**：
  - `npm --prefix frontend run test` → `26 files / 184 tests passed`。
  - `npm --prefix frontend run build`（`tsc --noEmit && vite build`）成功。
  - `cargo build --manifest-path backend/Cargo.toml` 成功，刷新供浏览器验收使用的交付二进制。
  - `npm --prefix frontend run test:browser` → `17 passed`；覆盖真实 Chromium 中的变量块 ×/退格删除、
    光标插入、下拉选中项对比度、390px 与 320px 下 zone 行无横向溢出、数字角标和完整路径标题。
- **限制**：真实手机触控、软键盘和浏览器工具栏仍未验证；390px / 320px 结论来自 Chromium 视口仿真。

