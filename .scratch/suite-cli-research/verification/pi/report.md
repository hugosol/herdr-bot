# Pi suite adapter 隔离运行验证

## 结论与证据层级

**已通过完整原生 CLI smoke（本机已安装 Pi 1.5.0）**：固定加载同一个 `pi-suite.ts`；无 suite 时完全不注册替代 MCP/codemode；有 suite 时仅自定义 `loadConfig` 返回的服务器连接；可信项目和默认用户配置中的受控 MCP 均不启动。`on`、`only` 双向覆盖原设置均有效。`only` 下实际 HTTP 模型请求只声明 `codemode`，其脚本成功调用真实 MCP 协议工具和原生 `read`，结果进入下一次模型请求。不是 SDK 内部权限注入，也不是 echo mock。

**指定 checkout `E:/Developer/myhub/pi`：完成源码核查；尝试其原生 CLI 源码入口，但未成功启动，不能宣称 checkout 运行通过。** 此 checkout 没有 node_modules/dist；借用现有安装的第三方依赖、保留 checkout workspace source aliases 后，启动被缺失的生成文件 `packages/ai/src/providers/data/amazon-bedrock.json` 阻塞。没有联网生成目录、构建、改源码或用假数据补齐。已安装版本与 checkout package.json 都是 1.5.0，但不据此断言源码字节相同。

## 环境和命令

- Windows，Node `v24.18.0`，Bun `1.4.4`（执行采用 Node）。
- checkout package：`E:/Developer/myhub/pi/packages/coding-agent/package.json`，1.5.0。
- 已安装 package：`D:/nvm4w/nodejs/node_modules/@earendil-works/pi-coding-agent/package.json`，1.5.0。
- 实际运行入口：`D:/nvm4w/nodejs/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js`。
- 成功命令（仓库根）：

```powershell
node .scratch/suite-cli-research/verification/pi/smoke.mjs
```

- 最终输出：4 个场景全部 exit 0，1.69 秒；精确 JSON 见 [summary.json](summary.json)。各场景的完整 argv、stdout/stderr、HTTP 请求、MCP 请求、工厂期观察见 [no-suite.json](no-suite.json)、[on.json](on.json)、[only.json](only.json)、[empty.json](empty.json)。
- checkout 尝试命令：

```powershell
node .scratch/suite-cli-research/verification/pi/smoke.mjs --source
```

第一次暴露 Windows `--import` 绝对路径需要 `file:` URL，修正脚本后第二次进入 checkout 源码，但因 `ERR_MODULE_NOT_FOUND .../providers/data/amazon-bedrock.json` 退出 1。确切最终错误见 [source-no-suite.json](source-no-suite.json)，[source-summary.json](source-summary.json) 的 results 为空，**不是通过**。[dependency-resolver.mjs](dependency-resolver.mjs) 只解析已有安装中的第三方依赖；`@earendil-works/*` 仍由 checkout 的 `experimental/source-resolver.ts` 指向源码，不替换为已安装工作区实现。

## 临时原型与稳定工厂期输入

[pi-suite.ts](pi-suite.ts) 是实验原型，不是产品代码。启动器通过**子进程专属环境变量 `PI_SUITE_CONFIG=<绝对 JSON 路径>`**传递已解析 suite；工厂同步读取一次，闭包保留快照。该值先于 extension factory 可用，不依赖 `getFlag` 在加载期拿到自定义 CLI flag。各场景的 observations 都记录 factory 收到的绝对路径；无 suite 为 null。

- 缺少变量：立即 return，不调用两种工厂，不注册 `/mcp`、工具或事件，不要求 `--no-mcp`。
- 存在变量：调用公开 `createMcpExtension({ loadConfig })`，`loadConfig` 只返回 suite servers，完全不调用默认文件读取函数；调用 `createCodemodeExtension({ mode: suite.codemode_mode })`；`session_start` 用公开 `getActiveTools/setActiveTools` 增量激活 codemode，保留底层工具。
- 套件启动 argv 使用 `--no-extensions --no-mcp -e <固定 adapter>`。没有额外 `builtin:codemode`，所以没有重复 codemode 实例。常规无 suite 启动仅增加 `-e <固定 adapter>`，不加这两个禁止参数。
- 实验为了不读真实认证，用测试拥有的独立 agentDir；**这不是产品方案整体搬 agentDir**。adapter 不读写 agentDir、auth/settings/models/session，也不需要该重定向。实际产品保留原 agentDir，只需每次子进程环境和生成配置。
- 未使用用户全局/项目配置；fixture 的 agentDir 与 cwd 均在独占临时根。可信项目用 `--approve`，故不是通过关掉项目信任“证明”MCP 排除。统一 `--offline --no-session --no-skills --no-prompt-templates`；只在 `127.0.0.1` 动态端口运行模型响应服务；清空继承的模型凭据/代理环境，传入显式假的本地 key。

## 实际观察

| 场景 | 原设置 codemode.mode | 模型首个请求 tools | MCP 真正启动 | 请求数 |
|---|---|---|---|---:|
| 无 suite，adapter 仍加载 | on | read/bash/edit/write + global/project 的 inspect | global、project | 2 |
| suite on | **only** | read/bash/edit/write/codemode + selected inspect | **selected** | 2 |
| suite only | **on** | **仅 codemode** | **selected** | 2 |
| suite 空 MCP 清单 | on | read/bash/edit/write/codemode | **无** | 1 |

所有场景默认用户 `mcp.json` 和已信任项目 `.pi/mcp.json` 都包含可启动的受控 stdio MCP；无 suite 正控证实两者确实能连接并声明工具。有 suite 时两个默认服务没有 start 事件，也没有相应工具声明，故不是“配置覆盖但仍偷偷连接”。空清单没有回退默认发现。

[MCP fixture](mcp-fixture.mjs) 实现实际 newline JSON-RPC initialize/tools/list/tools/call，交由 Pi 原生 MCP transport/连接/工具注册来消费。inspect 读取测试临时文件，并计算真实 SHA256；driver 在服务之外独立计算预期 SHA256，断言工具结果到达第二次 HTTP 模型请求。不是将输入原样返回的 echo。

`only` 的模型服务发出真实 `codemode` tool_call，脚本执行：

```js
const digest = await tools.mcp__selected__inspect({});
const local = await tools.read({path: "<owned absolute fixture path>"});
text(digest);
text(local);
```

第二次模型请求包含 `Script completed`、MCP 计算的 digest、原生 read 得到的文件原文；driver 同时断言原文与独立 digest。说明工具只是从模型声明中隐藏，底层没有被 allowlist 禁掉。受控模型只负责确定性生成调用，不证明真实模型会自主遵循任务。

最后一次运行默认服务 PID 11292/22524，suite on PID 22896，only PID 6692；全部在 CLI exit 后用 `process.kill(pid, 0)` 有界确认已退出，见 summary cleanup。HTTP 服务逐场关闭，临时 fixtures 根在 finally 删除；证据与可复跑脚本保留于本目录。

## checkout 源码证据

以下仅为源码证据，不冒充上述 checkout 的运行结果：

- `packages/coding-agent/src/index.ts:414-415`：公开 MCP/codemode 工厂导出（对应既有 loading-contracts 核查）。
- `extensions/mcp/index.ts:69-91,279-310,1114-1150`：`loadConfig` 是公开 factory option；session_start 选择自定义读取器，随后原生启动连接。
- `extensions/mcp/config.ts:52-72`：`McpServerEntry`、`LoadedMcpConfig` 形状。
- `extensions/codemode/index.ts:14-47`：明确 `mode` 覆盖 settings；注册 `defaultActive:false`，故 adapter 显式激活不能省略。
- `core/resource-loader.ts:143-177,743-746`：根据工具/命令/flag 冲突省略 replaceable builtin，第三方 `/mcp` 可替换 builtin；本次 suite 命令另外显式 `--no-mcp`，无 suite 则没有 `/mcp` 冲突。
- `main.ts:792-799`：`--no-mcp` 只作为 disabled builtin extension 传入，不禁止自有 adapter。
- `core/extensions/loader.ts:377-381`：`getFlag` 读取 runtime/pending values，不能把任意加载时序当作 CLI flag 已赋值保证；本原型完全不依赖它。
- `docs/cli.md:194-200,237-242`：显式 `-e` 在 `--no-extensions` 下仍可加载，`--no-mcp` 不影响替代扩展，`--approve` 信任项目，`--offline` 禁自动联网。

## 精确限制

1. **本地 checkout 的完整运行仍缺生成模型目录**；现有已安装原生 CLI 的运行已完整证明，但不能上升为 checkout 构建通过。
2. 只验证受控 stdio MCP；未验证 OAuth、真实 HTTP MCP、真实凭据、外部模型、生产 codebase-memory-mcp 或 UI `/mcp` 操作。没有 build/lint/永久测试/formatter。
3. 固定 adapter 工厂期读取配置文件，产品必须在 spawn 前完成原子生成，并在未选 suite 时移除该子进程环境变量；启动后不热更新。原型仅消费已经解析的数据，不负责产品 schema 验证/根引用展开。
4. `loadConfig` 能排除默认用户/项目文件，但 MCP 工厂仍会通过 `pi.getMcpServers()` 合并**其他扩展主动注册**的服务器（`extensions/mcp/index.ts:348`；`core/extensions/types.ts:1918-1924`）。本 smoke 没有加载此类扩展，不声称是安全沙箱。产品固定/选中扩展契约必须明确不主动引入未选 MCP；不能从这次文件来源隔离推导任意第三方扩展隔离。
5. suite 运行禁掉默认扩展后产品仍须显式附加全部适用角色的既有固定扩展与 suite 扩展；此次仅测试 adapter，未覆盖 herdr 的角色选择/启动链路。
