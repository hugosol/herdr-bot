# Suite 资源目录技术验证

## 结论

- **已运行通过（junction，不是真 symlink）**：两个独立 suite 目录引用同一个自造技能实体、同一个自造扩展实体；Pi checkout 的技能读取及 manifest 入口解析成功；安装版 Pi 1.5.0 的真实扩展 loader 执行工厂、注册工具并调用成功，扩展相对 TS import 和 `import.meta.url` 相对资产读取均成功。临时根含空格，且 fixture 在 C:、源码/安装包在 E:/D:。这不是跨磁盘 junction 目标的验收：链接和目标同在 C:。
- **未验证**：真目录/文件符号链接。执行环境 `HERDR_BOT_MANUAL_SYMLINKS` 未设置，因此严格按 testing.md **未尝试**，也未自行设置开关、提权或修改开发者模式。junction 成功绝不等于文件 symlink 成功。独立 `.ts` 文件链接未测试。
- **OMP 运行阻塞**：checkout 的 skills/extension 模块均在导入期缺失 `pi_natives.win32-x64-modern.node`，没有执行到资源 loader。未构建、安装或修改宿主，不宣称 OMP 实际加载通过。
- **Pi checkout 完整 extension loader 阻塞**：缺 `packages/ai/src/providers/data/.manifest.json`；故显式改用已安装 Pi 的 loader 验证实际扩展加载，而保留 checkout 失败。不能把安装版运行结果外推为当前 checkout 完整 CLI 通过。
- **MCP 并发实测**：当前 PATH 二进制 0.12.1 支持同一私有 profile 的两个 MCP 客户端共享一个 daemon/UI；两者都 initialize 成功，daemon 显示 2 committed clients，一个 PID，动态回环端口 HTTP 200。独立 daemon namespace 使用同一动态端口时，两边 MCP initialize 仍成功，但 UI 端口仍是独占资源，不能认为两个独立 UI 都可成功监听。未占用、连接、停止 9749 或已有用户 daemon。
- **严格白名单存在功能依赖缺口**：特别是 grill-with-docs 仅转调 grilling/domain-modeling，两项均未在探索 suite 注册；完整列表见下。仅报告，不扩充清单。

## 入口、版本与原始证据

先读 `docs/agents/testing.md`、`spec.md`、`loading-contracts.md`、`initial-suites.toml`。

命令（仓库根执行）：

```text
where.exe node bun codebase-memory-mcp rust-analyzer typescript-language-server jdtls pyright-langserver
node -e "console.log(JSON.stringify({node:process.version,manualSymlinks:process.env.HERDR_BOT_MANUAL_SYMLINKS??null}));"
bun .scratch/suite-cli-research/verification/resources/resource-smoke.ts
codebase-memory-mcp --version
python .scratch/suite-cli-research/verification/resources/mcp-smoke.py
rust-analyzer --version
where.exe java python pyright pyright-langserver typescript-language-server jdtls
java -version
```

- Node `v24.18.0`：`D:/nvm4w/nodejs/node.exe`；Bun `1.4.4`。
- 安装版 Pi：`D:/nvm4w/nodejs/node_modules/@earendil-works/pi-coding-agent`，package.json version `1.5.0`。
- Pi checkout：`E:/Developer/myhub/pi`；OMP checkout：`E:/Developer/myhub/oh-my-pi`。
- MCP：`E:/Developer/tools/codebase-memory-mcp/codebase-memory-mcp.exe`；`--version` 输出 `codebase-memory-mcp 0.12.1`，运行 daemon build `0.12.1 (39f6a789cd85...)`。
- resource smoke 首次退出 1：checkout loader 缺 manifest（不是链接失败）；改为记录 checkout 错误并显式使用安装版 loader 后退出 0。最终 stdout 已持久化为 `resource-evidence.txt`（源自本次会话工具 artifact://42）；完整可复现隔离脚本保留为 `resource-smoke.ts`。MCP 的完整参数、stdout/stderr、PID、清理结果持久化在 `mcp-evidence.json`。

## 资源实际运行

自造 fixture 内容只有：

- `SKILL.md`，身份 `fixture-resource`，无技能执行；`references/value.txt` 为 `fixture-skill-value`。
- 扩展 `index.ts` 相对 import `./helper.ts`，读取 `new URL('./asset.txt', import.meta.url)`，只注册一个无副作用工具 `fixture_resource`。
- suite a、suite b 各自生成 `skills/fixture-resource` 与 `extensions/fixture-extension` junction；各自 `extensions/package.json` 的 `pi.extensions` 明确指定 `./fixture-extension/index.ts`。同时写有 `omp.extensions`，但 OMP 未完成读取，不能以此声明 OMP manifest 有效。

实际调用 Pi `loadSkills({ includeDefaults:false, skillPaths:[suiteSkills], ... })`，断言只读到一个技能，并从其 `baseDir/references/value.txt` 读到预期值。`DefaultPackageManager.resolveExtensionSources([extensions])` 读取 manifest 后返回链接目录内精确 `index.ts` 入口。安装版 `loadExtensions` 返回 `errors:[]`、一项扩展，真实注册的工具执行返回：

```json
{"content":[{"type":"text","text":"relative-import-ok:relative-asset-ok"}]}
```

两个 suite 都达到同样断言，`realpath` 都落到同一 `shared entity/skill` 与 `shared entity/extension`。这证明各 suite 分别使用同一资源实体可行；不证明全产品 Chat 并发、资源原子发布、默认资源过滤或模型可见集合。这些不属于本资源 fixture 的观察面。

源码依据（先用图 search_graph 找符号，再按范围读取；图中部分行号落后于 live source，以 live 为准）：

- Pi `packages/coding-agent/src/core/skills.ts:129-171,414-451`：目录扫描、includeDefaults 和 realpath 去重机制。
- Pi `packages/coding-agent/src/core/package-manager.ts:1061-1076`：真实 `resolveExtensionSources` 走 package sources；本次实际执行该 resolver。
- Pi `packages/coding-agent/src/core/extensions/loader.ts:679-728`：`loadExtensions` 逐项执行 loadExtension，保留具体错误。
- OMP `packages/coding-agent/src/extensibility/skills.ts:236-300,490-520`：custom directory 扫描；运行未跨过 native dependency。
- OMP `packages/coding-agent/src/extensibility/extensions/loader.ts:445-488`：工厂加载入口；运行未跨过 native dependency。

## LSP 可用性（不安装、不启动语言服务）

已检查 PATH、仓库根/frontend/backend 的常见 node_modules/.bin、仓库递归 node_modules/.bin 目标名称、`.venv/Scripts`/`venv/Scripts`、工具安装目录和相关全局 Node package 路径。负结果仅覆盖这些位置，不声称扫描整机所有磁盘。

| 服务 | 结果 |
|---|---|
| rust-analyzer | PATH `C:/Users/hugos/.cargo/bin/rust-analyzer.exe`；实际 `--version` 成功：`rust-analyzer 1.98.1 (48a229ce 2026-09-01)`。未启动 LSP。 |
| typescript-language-server | PATH 与上述项目/全局常见 bin/package 未发现；frontend 只有 `tsserver`/`tsc`，不能冒充 typescript-language-server。 |
| jdtls | PATH 未发现，**但已安装在** `E:/Developer/tools/jdtls/bin/jdtls`、`jdtls.bat`、`jdtls.py`；`plugins/org.eclipse.equinox.launcher_1.7.100.v20251111-0406.jar` 与 `config_win/config.ini` 存在。Java 可用，`java -version` 输出 Java 26 build 26+35-2893。`jdtls.bat` 仅 `python %~dp0/jdtls %*` 后 pause。当前 suite 单写 command='jdtls'、继承当前 PATH 时仍找不到该安装；只认存在不认成功启动/可工作。未运行 launcher 或解压旁边归档。 |
| pyright-langserver | PATH、项目常见 bin、全局 Node pyright package 未发现。未安装。 |

## MCP UI 多进程边界

`mcp-smoke.py` 生成独占 Temp，并给每个 profile 设置自己的 HOME/USERPROFILE/APPDATA/LOCALAPPDATA/XDG_CONFIG_HOME/XDG_CACHE_HOME/**CBM_CACHE_DIR 与 CBM_RUNTIME_DIR**。后者至关重要：源码 `tests/windows/test_daemon_lifecycle.py:28-43` 明确 cache 不移动账户级 rendezvous，只改 cache 会误连接现有 daemon。敏感 token/key 环境过滤，cwd 是空临时根；未 index 项目、未运行业务工具。

以动态端口 **10910** 代替 9749，调用真实 `--ui=true --port=10910`，stdin 保持开启，只发送 MCP initialize：

1. profile a：客户端 PID 11612、468 都取得 initialize result；`daemon status`：PID **26680**、committed clients **2**；HTTP `http://127.0.0.1:10910/` **200，722 bytes**。
2. profile b（完全独立 namespace）：客户端 PID 11260 也 initialize 成功；其 daemon PID **22412**，committed clients **1**，status 只说 UI configured 同端口且明确 readiness not checked。这条 status **不是 UI ready**。未将其误算成第二个监听成功。
3. 所有三个客户端正常退出 0；私有 profile 下 `daemon stop` 都回复 not running，随后 status 都退出 1/not running；Temp 删除成功。

源码依据：

- MCP `README.md:139-142`：UI 由共享 coordination daemon 所有，并发 agent session 不各起 HTTP server。
- `src/main.c:3280-3305`：MCP client bootstrap 成功后将 UI flags 作为 daemon 配置 mutation，且是持久化设置，不是无共享影响的每连接参数。
- `src/ui/httpd.c:247-251`：仅 127.0.0.1 bind；`src/ui/http_server.c:2094-2104`：端口占用时记 `ui.unavailable reason=in_use` 并返回 NULL。

因此，同版本、同 coordination namespace 的普通多 Chat 可以共享一个 UI，不必因参数固定 9749 就推断每 Chat 互相争抢。**独立 daemon namespace 或其他占用者**仍可能导致固定端口 UI 冲突，且 MCP 协议可继续初始化；UI 未就绪不能等同 MCP 不可用。同 namespace 的 flags 会修改共享 daemon 的 UI 配置，若各 suite 传不同端口不是独立 UI。实际没有触碰 9749；该端口的本机当前占用及真实用户现有 daemon 状态未检查。

## 追加：选定 skill 的直接依赖（只读取，不执行）

以下名称均按 initial-suites.toml 的严格 suite 清单逐项对照。没有因本报告扩充 suite、链接真实技能或运行技能。选中一个 skill 目录可带齐其内部 references/assets，但**不会自动注册兄弟 skill**。

| 选定 skill | 源码直接要求 / 条件依赖 | 严格清单结果 |
|---|---|---|
| 探索 grill-with-docs | `.../skills/engineering/grill-with-docs/SKILL.md:7` 唯一句：`Call the Skill tool twice, for "grilling" and "domain-modeling".` | 两者都未注册；当前 wrapper 的核心动作不可满足。 |
| 探索 improve-codebase-architecture | SKILL.md:13 codebase-design；:64 grilling；:66 domain-modeling；:71 再用 codebase-design | 三者都未注册。 |
| 探索 wayfinder | `oh-my-wayfinder/skills/wayfinder/SKILL.md:97-99,150,154` research、prototype、grilling、domain-modeling；:135,171,179 lighthouse；:172 backtracer，后两不可用明确 stop | 六者都未注册。Notes 中动态指定的其他技能还取决于具体 map，无法静态穷举。 |
| 探索 code-review | SKILL.md:13 缺 issue tracker 时要求用户 `/setup-matt-pocock-skills` | 条件入口未注册；并非每次都直接调用。 |
| 实现 retro | matt-pocock `.../engineering/retro/SKILL.md:11` 直接调用 writing-for-agents | writing-for-agents 只在探索 suite，不在实现 suite；单 suite 模型不会从另一个 suite 补进来。 |
| 实现 tdd | `oh-my-wayfinder/skills/tdd/SKILL.md:30` 接口形状有疑问时调用 codebase-design；:45 将 refactor 指向 code-review | 两项不在实现 suite；后者是阶段引用，不是无条件直接工具调用。 |
| 实现 to-tickets | `.../to-tickets/SKILL.md:11,19` 缺 tracker 时 `/setup-matt-pocock-skills`；缺 approved contract 时必须 `/to-contract` | 两项不在实现 suite；条件前置入口。 |

本次所查 teach、writing-for-agents、resolving-merge-conflicts、expert-translate 的 SKILL.md 未发现其他直接 Skill-tool 调用；只代表这些入口文本，未对其全部链接参考文档做传递依赖闭包。teach 的格式文件、writing-for-agents 的 SKILL-MECHANICS.md 等是目录内参考文件，并非额外已注册 skill。

实体存在性：`.../skills/productivity/grilling/SKILL.md`、`.../skills/engineering/{domain-modeling,codebase-design,research}/SKILL.md` 真实存在，但“磁盘存在”不等于 suite 注册。wayfinder 还通过 `../setup-matt-pocock-skills/issue-tracker-local.md` 读兄弟目录参考（:161）：只链接单 skill 根并未物理包含其兄弟目录，不应把这种相对引用默认视为可用；本轮未执行该真实技能去验证宿主路径边界。

## 用户批准后的依赖路径补充（仅报告，父负责配置）

优先级：oh-my-wayfinder/skills 优先，其次 matt-pocock/skills/skills。逐项检查了以下 18 个入口的 frontmatter name 与真实目录。原探索 7 项 + 显式 Skill 调用新增 7 项 = **14 项严格调用闭包**；再计入明确要求用户执行的条件前置/交接命令 4 项 = **18 项实用工作流闭包**。仅提及示例不计。

| 分类 | name | 真实技能目录 |
|---|---|---|
| 原7 | grill-with-docs | E:/Developer/myhub/agent-skills/matt-pocock/skills/skills/engineering/grill-with-docs |
| 原7 | teach | E:/Developer/myhub/agent-skills/matt-pocock/skills/skills/productivity/teach |
| 原7 | code-review | E:/Developer/myhub/oh-my-wayfinder/skills/code-review |
| 原7 | wayfinder | E:/Developer/myhub/oh-my-wayfinder/skills/wayfinder |
| 原7 | writing-for-agents | E:/Developer/myhub/agent-skills/matt-pocock/skills/skills/productivity/writing-for-agents |
| 原7 | improve-codebase-architecture | E:/Developer/myhub/agent-skills/matt-pocock/skills/skills/engineering/improve-codebase-architecture |
| 原7 | expert-translate | E:/Developer/myhub/expert-tranlate |
| 显式调用 | grilling | E:/Developer/myhub/agent-skills/matt-pocock/skills/skills/productivity/grilling |
| 显式调用 | domain-modeling | E:/Developer/myhub/agent-skills/matt-pocock/skills/skills/engineering/domain-modeling |
| 显式调用 | codebase-design | E:/Developer/myhub/agent-skills/matt-pocock/skills/skills/engineering/codebase-design |
| 显式调用 | research | E:/Developer/myhub/agent-skills/matt-pocock/skills/skills/engineering/research |
| 显式调用 | prototype | E:/Developer/myhub/oh-my-wayfinder/skills/prototype |
| 显式调用 | lighthouse | E:/Developer/myhub/oh-my-wayfinder/skills/lighthouse |
| 显式调用 | backtracer | E:/Developer/myhub/oh-my-wayfinder/skills/backtracer |
| 条件前置命令 | setup-matt-pocock-skills | E:/Developer/myhub/oh-my-wayfinder/skills/setup-matt-pocock-skills |
| 条件前置命令 | to-contract | E:/Developer/myhub/oh-my-wayfinder/skills/to-contract |
| 条件前置命令 | to-spec | E:/Developer/myhub/oh-my-wayfinder/skills/to-spec |
| 明确交接命令 | to-tickets | E:/Developer/myhub/oh-my-wayfinder/skills/to-tickets |

递归边：backtracer/GAP-FOLLOWUP.md:4 明确 load and execute /grilling（已有）；prototype/SKILL.md:34 交接要求接手 session 在准备好后运行 /to-spec；to-contract/SKILL.md:17,19 明确调用 codebase-design/domain-modeling（已有），:25 缺 spec 必须先 /to-spec，:151 明确 tell user run /to-tickets；to-spec/SKILL.md:9 缺 tracker 则 /setup-matt-pocock-skills，:17 下一步 /to-contract；to-tickets/SKILL.md:11,19 回指 setup/to-contract，至此闭合。其余新增目录的 SKILL.md 与附属参考文档未搜得新的具名调用。

不计入：setup/SKILL.md:29,51 的 triage 仅探测“是否已安装”，缺失要求跳过该分支，不要求安装/调用；文档中的 /triage、/to-tickets 作为消费方说明不是新增强制调用；backtracer/lighthouse 互链及 wayfinder 链接属于参考/现有名称；prototype 中 route `/prototype/...` 等为路径示例，不是技能。wayfinder Notes 动态列出的任意名称无法静态闭包。

实现 suite 保持用户原4项，不因把依赖加入探索而自动可见：retro→writing-for-agents；tdd→codebase-design，review阶段引用code-review；to-tickets→setup-matt-pocock-skills/to-contract（再到to-spec）；这些实现侧缺口仍保留，未擅改。

## 安全与清理

仅新增本报告目录内隔离脚本、报告、MCP JSON 证据；没有修改宿主源码、全局/项目配置、用户技能或扩展、凭据和数据。所有资源 fixture 和私有 MCP Temp 已删除，所属客户端已等待退出并确认私有 daemon 不存活。没有 build/lint/formatter/永久测试，也没有启动现有 Herdr、真实业务扩展、语言服务或外网服务。
