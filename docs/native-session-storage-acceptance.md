# 原生会话存储回归：P23／P24／P25 与部署 H1

## 当前行为

zone 创建 Bot 仍新建 Herdr Tab／Pane／agent 控制对象；这不保证新建原生对话。
herdr-bot 不指定 `--session-dir`，不预分配原生目录，不用其他参数强制新建或恢复，
不改用户的 autoResume、原生目录或其他 agent 设置。Pi／OMP 的有效配置决定存储与续接。
启动期原有 zone／模板布局切换仍执行，但旧根目录 `bot-sessions/` 不再是清理对象；
`.data/bot-sessions` 和任何其他既有原生主文件、附属记录也不迁移、复制、转换或改写。
已有外置对话不会因本改动自动被正常目录的发现器扫描或导入历史。

## 自动与真实隔离证据

批准接缝为 S4；受控 Herdr 通过真实公开 HTTP 创建入口驱动，不把内部参数函数提升为测试接口。

- `backend/tests/bot_identity.rs` 的
  `each_agent_keeps_its_native_storage_selection_without_allocating_directories`：分别创建 OMP／Pi，
  核对完整外部启动参数只包含扩展选择、不分配目录、旧文件不变。
  首次 red 明确失败于旧 `--session-dir`，移除覆盖后 green。
- `backend/tests/runtime_data.rs` 的
  `default_and_delivered_creations_leave_old_native_tree_unchanged`：默认与随附配置下，
  预放旧主文件、二进制附属记录与空目录；实际交付进程两次公开创建、两次重启后比较全部字节与目录清单。
- 同文件 `cutover_preserves_old_native_main_and_sidecars_while_moving_zone_registration`：
  无布局标记的首次启动保留旧根目录原生文件。red 确认原有启动清理递归删除了它们；
  删除该清理后保留 zone／模板切换而保护原生文件。
- 同文件 `runtime_directory_and_write_failures_are_not_reported_as_success`：
  原来目录位置被普通文件占用也不妨碍 Bot 成功，不再以目录创建为前置条件；原文件字节不变。
- 创建、断开观察、快照与浏览器扩展参数检查沿用原接缝，删除的是已撤销的独立目录保证，
  不削弱 Herdr 对象独立性、配置快照或扩展集合的断言。
- 合并后的浏览器夹具按扩展 ID 生成独立入口文件，不依赖开发机 toolkit 路径；
  `templateExtensionLaunch.spec.ts` 对固定和可选扩展分别覆盖可读目录入口成功传给 agent，
  目录内部发现仍归 agent；缺失及不可读文件继续在创建前拒绝，检查不执行入口代码。
  其余浏览器断言同步当前交付元数据的显示名、Skill、官方固定扩展与可选扩展集合，
  保留独立字面预期；动态显示名测试修改实际存在的 label，不做无效替换。
  后端重启测试的磁盘清单包含持久 `session-history.json`，当前登记及关机意图仍不恢复。
  Windows 自动端口可能落在 Fetch／Chrome 禁止的 6566、6667 等服务端口；
  浏览器夹具在 49152–65535 范围探测空闲端口，避免将 `bad port` 误判为后端未就绪。
  此选择仅影响隔离测试监听，不修改生产服务配置。

[原始隔离真实运行证据](evidence/native-session-storage/isolated-real-agents.json) 不来自 argv 回显或迁移结果：
真实安装的 Pi **1.4.0** 与 OMP **18.12.6** 各运行两次原生 CLI；模型响应由仅监听 loopback 的
受控 OpenAI-compatible endpoint 提供。只有模型响应是替身，CLI、配置解析、原生写入与发现实现均为真实安装版本。
不使用真实账号或用户对话；HOME／USERPROFILE／agent 目录隔离，不读取用户 auth，未向旧对话提交输入。
首次运行生成的隔离旧主文件和随后预放的附属记录在第二次运行前后保持相同目录清单与 SHA-256。
配置文件运行前后逐字节比较相同，JSON 证据保留独立前后 hash。

| Agent | 有效配置与实际非固定位置 | 第二次实际原生 ID |
|---|---|---|
| Pi | 隔离 `PI_CODING_AGENT_DIR` 下 `settings.json` 的 `sessionDir` 指向 `pi/nondefault-native-sessions`；无 CLI 目录参数 | `01a1646d-6e34-75af-9d2d-8f6e4cc7b197` |
| OMP | `PI_CODING_AGENT_DIR` 指向 `omp/configured-agent`；`config.yml` 保持 `autoResume: false`，原生 sessions 在此 agent 目录下按 cwd 分组 | `01a1646d-77ab-7370-a79a-655c82397254` |

实际根目录：`C:\Users\hugos\AppData\Local\Temp\herdr-native-storage-ur203kx9`。
完整绝对引用、真实 session header（OMP 的 header 不一定是第一行）、cwd、进程 stdout、
旧文件目录与 hash、原生发现返回值均在 JSON 中。
发现证据调用安装版 `SessionManager.list`，即原生选择器使用的列举实现，并核对 ID 与精确文件路径；
Pi 由安装版 SettingsManager 读取有效 sessionDir，OMP 使用自身默认解析（没有硬编码项目目录给发现函数）。
这证明真实落盘与原生发现能力，**不声称观察了部署 TUI 的 `/resume` 列表或真实 Herdr 创建**。
OMP 使用当前 `models.yml`，避免把 `models.json` 的 agent 自身格式迁移混入配置不改写比较。

复现（只在独立临时目录写入，保留目录供核对）：

```powershell
python scripts/test-native-session-storage.py `
  --pi-package D:/nvm4w/nodejs/node_modules/@earendil-works/pi-coding-agent `
  --omp-package D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent
```

依赖 Python 标准库、Node、Bun 与两种已安装原生 agent。其他机器按实际安装位置替换参数。
脚本成功输出 JSON；任何 CLI、目录、配置、字节保护或发现断言失败都会非零退出。
受控模型不评估模型能力，CLI `--print` 不证明 TUI；部署闭环由下列 H1 独立确认。

## P23-H1：用户实际部署最小核对（尚未通过）

以下仅为准备好的核对步骤，不是用户验收结论。不需要修改配置、不对用户已有对话发送测试输入、
不执行恢复或迁移。若现有 autoResume 会选择旧对话，先使用用户认可的全新隔离 zone 进行创建，
不要为了测试切换 autoResume。

1. 分别记录部署所用 Pi／OMP 版本、Herdr 受管进程继承的 HOME／USERPROFILE、
   `PI_CODING_AGENT_DIR`，以及有效原生存储设置与来源。Pi 同时核对 `sessionDir`／
   `PI_CODING_AGENT_SESSION_DIR` 与命令行优先级；OMP 核对 profile、agent 目录与 autoResume。
   不把本机隔离示例路径或固定 `~/.pi`／`~/.omp` 当作部署有效配置。
2. 从 zone 各创建一个不勾选 connect-bot 的 Pi／OMP。只使用用户已认可的无模型原生命令
   （如实际安装版已确认的 `/settings`），不要把未知 slash 命令当作无模型保证。
   记录创建响应 `id`、`name`、`tabId`、`paneId`、`agentName`、zone 与成功状态；
   显示名可能相同，必须用这些返回对象定位，不能按 cwd 最近文件猜身份。
3. 在 Herdr 受管终端使用 `herdr pane process-info --pane <本次 paneId>` 核对没有原生目录覆盖
   或替代强制参数；这是辅助信息，不代替落盘。结合本次 agent 的 `/session` 或 Herdr 官方
   集成上报的精确原生引用取得路径／ID（OMP 可由 session header 核对 ID）。
   只读核对该引用的 session header 中 `id`／`cwd`、真实目录与有效配置一致；不要扫描后选择最近文件。
4. 同一 cwd、同一有效配置正常启动相应 agent，打开原生 `/resume`（只查看，不选中恢复），
   按本次精确 ID／名称核对能发现它。记录列表／路径／header 证据；若该版本界面只显示名称，
   用其原生详情与精确引用交叉核对，不以名称唯一性代替身份。
5. 独立比较前后有效设置文件字节／hash 与旧主文件、附属记录清单；不要把 agent 正常续接并
   写入所选对话误称为 herdr-bot 的迁移行为。两种 agent 各记录通过／失败／无法判断及证据。

H1 未获用户实际部署确认，因此 P23 的 H1、整体验收与合入状态不标为通过。
P25 的显式 register 与精确引用读取覆盖归其他实施票，本记录只证明本票的原生存储／旧文件保护分区。
