# OMP suite 隔离核查与安全 smoke

## 结论

**发现工具层可行路径，不必修改宿主、不必迁移整个 agentDir，也不必重写 MCP 协议或原生工具桥。** OMP 公开导出的 `MCPManager` 接受自定义 `MCPConfigLoader`；普通 async extension 工厂可以创建独立 manager，仅返回 suite 配置，复用 manager 的连接管理及其返回的 `MCPTool.execute`，通过公开 `pi.registerTool` 暴露工具。本次已通过真实 extension loader + 受控 stdio MCP 的 initialize、tools/list、tools/call。

该路径不等同于替换宿主自有 manager。原生 `/mcp` 界面、`mcp://` 资源、MCP prompts/notifications 热刷新不会自动指向 extension 的独立 manager。这些是需要明确告知/接受的功能差异，而不是把工具层路线判成失败的理由。工具调用自身已通过，不是纯静态推断。

原先“suite package mcp.json + 禁用其他 providers”仍不能抵御用户名单；实际运行已复现 denylist 压制和 enabledServers 强制启用。要使用上述 adapter，**原生 MCP providers 包括 omp-plugins 全部禁用**，suite 的 MCP 不再同时写入原生发现根，避免重复连接。

## 补充：实际读取文件并计算 SHA256（取代只回显入参的行为证明）

按父任务补充要求，仅定向运行增强后的 MCP bridge，没有重跑 skills、CLI help/version 或 checkout 导入。

命令：`python .scratch/suite-cli-research/verification/omp/run-resources.py bridge.ts`，退出 **0**。最新完整证据：`bridge.ts-file-output.json`；之前的 `bridge.ts-output.json`（echo 成功）、`resources-output.json`（早期失败/成功）和 `resources-retry-output.json` 均原样保留。

- Python runner 在独占 Temp 中创建 `owned binary fixture.dat`：固定说明字节、0..255 全字节序列及 73 个随机字节，总计 **363 bytes**。
- **OMP 之外的 Python `hashlib.sha256(fixture.read_bytes())` 独立计算期望值**，并用 stat 独立记录长度。期望 digest 在启动 MCP 子进程前从环境移除，不通过工具参数或 server 源码传给服务器。
- 受控 MCP 的 `sha256_file` 工具 schema 只接受 `path:string`。工具参数仅包含本次独占文件路径，不包含文件内容/预期 digest。服务器严格校验工具名与唯一允许的 fixture 路径，再实际 `fs.readFileSync(path)`，以 Node/Bun `crypto.createHash('sha256').update(bytes).digest('hex')` 计算并返回 digest 和实际读取长度。
- 仍通过真实公开 extension import、`MCPManager` 自定义 loader 和原生 `MCPTool.execute` 调用，最终注册工具为 `mcp__suite_selected_sha256_file`。返回值经过原生工具桥后，独立断言 digest 与 Python 期望相等且 bytes=363。
- 本次期望与实际 digest 均为 `657a1f4531c9637a39dbb86502a240257401194e701da4aace94de965800f185`；原生 `details.mcpToolName` 为 `sha256_file`、`serverName` 为 `suite-selected`。**因此证明远端受控进程真实读取 fixture 并计算哈希，不再只证明 echo 转发。**
- 同名 suite-selected 仍在 user disabledServers 中，not-selected 仍在 enabledServers 与用户默认定义中；原生 configs=[]，suite-only 工具调用成功。
- 完整 initialize、tools/list、tools/call 请求及实际 JSON-RPC 响应、Python 期望与原生桥结果保存在最新 JSON stdout 字段。manager disconnectAll 后正常退出，runner 删除本次 Temp。未改宿主/产品源码或真实配置。

以下早期 echo 描述是保留的历史证据；当前桥接脚本已增强为文件 SHA256，重新执行上面的定向命令会生成新的随机 fixture 和对应 digest。

## 环境与安全边界

- 研究 checkout：`E:/Developer/myhub/oh-my-pi`。
- 可执行安装：`D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js`，真实 `--version` 输出 `omp/18.16.0`，Bun `1.4.4`。
- launcher：`D:/nvm4w/nodejs/omp.cmd`，调用 Bun 和上述 dist/cli.js。
- 安装包提供 `src`，可直接导入并成功运行；checkout 导入仍因 `packages/natives/native/pi_natives.win32-x64-modern.node` 缺失失败。没有构建、重装或补入 addon；运行成功证据针对安装的 18.16.0，而非声称 checkout 可运行。
- 所有可变 fixture 位于独占系统 Temp 根，隔离 HOME、USERPROFILE、APPDATA、LOCALAPPDATA、XDG_*、PI_CODING_AGENT_DIR 和 cwd；去除继承的 PI_/OMP_/HERDR_ 变量及名称含 API_KEY/TOKEN/SECRET/PASSWORD 的变量。
- 隔离 agentDir 仅是 smoke 安全设施；建议产品路线仍保留原 agentDir，由进程配置禁用来源、adapter 自定义 loader 选择 MCP，不迁移或链接认证/会话。
- 不读用户 auth、不连接模型、不启动真实 codebase-memory-mcp、不操作 Herdr、不外联。受控 MCP 是本次自建 Bun stdio 子进程。
- adapter `finally`/错误分支调用 `disconnectAll()`；测试正常结束，无 timeout，runner 随后删除 Temp fixture。证据与隔离脚本留在本目录。
- 未设置 `HERDR_BOT_MANUAL_SYMLINKS=1`，未创建真实 symlink。这里的 customDirectories 使用普通隔离目录；真实软链接/跨磁盘能力由其他验证任务覆盖，不在此声称通过。

## 可复现命令与证据

从 herdr-bot 根运行：

```text
python .scratch/suite-cli-research/verification/omp/probe.py
python .scratch/suite-cli-research/verification/omp/run-resources.py
python .scratch/suite-cli-research/verification/omp/run-resources.py resources.ts
python .scratch/suite-cli-research/verification/omp/run-resources.py bridge.ts
bun --version
```

说明：首次 resources smoke 误用 `invalidate()` 无参数，返回 ERR_INVALID_ARG_TYPE；改为传入本次 user mcp.json 路径后定向重试成功。未掩盖首次失败，保存在 resources-output.json。之后 bridge 定向运行用于验证公开包子路径 import，再补录完整响应 wire；最终 evidence 为 bridge.ts-output.json。

| 证据文件 | 实际结果 |
| --- | --- |
| `probe-output.json` | 安装 CLI version/help 退出 0；checkout import 退出 1（缺 native addon）；安装源码 import 退出 0 |
| `resources-output.json` | 首次 resources 失败原始错误；早期绝对路径 manager import 的 bridge 成功 |
| `resources-retry-output.json` | resources 纠正后退出 0，skills、显式固定扩展、三种 MCP 名单均精确断言 |
| `bridge.ts-output.json` | **公开包子路径 import** 的 extension 工厂退出 0；含完整 initialize/tools/list/tools/call 请求与实际 JSON-RPC 响应，以及原生工具执行结果 |

完整 wire 保存在最后一个 JSON 的 stdout 字段，未只保留截断的终端预览。受控 tools/list 返回 echo 及对象 schema（必需 text:string）；tools/call 收到 `{name:"echo",arguments:{text:"suite-through-native-bridge"}}`，返回 content 文本 `suite-through-native-bridge`；原生 MCPTool 返回 details.serverName=`suite-selected`、mcpToolName=`echo`。

## 1. 原生名单无法被 --config 或 provider 排除覆盖

源码证据（checkout 与安装包 mcp/config.ts 的相关实现一致）：

- `packages/coding-agent/src/mcp/config.ts:15-24`：选项只有 enableProjectConfig、filterExa、filterBrowser、extensionRoots，没有 config-path/list override。
- 同文件 `103-152`：先 `getMCPConfigPath("user", cwd)` 和 `readMCPConfigFile`，构造 disabledServers / forcedEnabled，再向 loadCapability 传 suppress。denylist 最高优先；forcedEnabled 可覆盖源的 enabled:false。
- `packages/utils/src/dirs.ts:1190-1194`：用户 mcp.json 固定为 `getAgentDir()/mcp.json`，没有只改 MCP 的环境变量。
- `packages/coding-agent/src/mcp/settings.ts`：原生设置包含 enableProjectConfig、startupTimeoutMs、renderMarkdownResults、notifications 等，没有用户 enabled/disabled 名单替换项。
- `docs/extensions.md:296-312`：runtime override 面向已注册 setting handle；不是覆盖任意文件字段。用它改 disabledProviders 可以控制来源，不能改变上述独立 JSON 读取。

实际运行：禁用 native、mcp-json 等 provider，保留 suite package 的 omp-plugins，并将 enableProjectConfig=false：

| 隔离 user mcp.json 名单 | 最终 configs |
| --- | --- |
| 空名单 | suite-selected |
| disabledServers=[suite-selected] | 空 |
| enabledServers=[suite-force] | suite-force、suite-selected（suite-force 在 package 源中 enabled:false） |

user-default/project-default 均未进入 configs，但名单仍改变 suite 结果。因此旧路径不能声称自包含。

## 2. 公开 manager / 工具桥路线

### 公开导出与 extension import 实测

- 安装包 `package.json:52-64`：根导出指向 src/index.ts；公开 `"./*"` 映射到 `"./src/*.ts"`，允许 mcp/manager 子路径。
- `packages/coding-agent/src/mcp/index.ts:16-22` 公开导出 loader、manager、tool-bridge。
- `mcp/manager.ts:65-66,354-359`：`MCPConfigLoader` 类型和 `new MCPManager(cwd, cache, loadConfigs)`；自定义 loader 是构造器参数，不是覆盖内部方法。
- `manager.ts:646-665`：discoverAndConnect 使用传入 loadConfigs；`777-783` connectServers 接受明确 configs；`1178` getTools；`1427` disconnectAll。
- `mcp/tool-bridge.ts:711-755,776-805`：原生 MCPTool 暴露 schema、来源元数据、approval、strict，并执行真实 MCP 调用。
- `extensibility/extensions/types.ts:971-1023`：公开 ToolDefinition 与 registerTool 所需 execute 形状；注意参数顺序与旧 CustomTool 不同。
- `extensibility/extensions/loader.ts:534-543`：真实 loadExtensions 导入并绑定普通工厂。

**运行使用的扩展中直接写的是 `import { MCPManager } from "@oh-my-pi/pi-coding-agent/mcp/manager"`，不是 monkey patch，也不是只证明绝对源码路径可导入。** 该工厂在隔离临时目录中由已安装 OMP loadExtensions 实际加载，成功注册 `mcp__suite_selected_echo`。

最小路线：

```ts
const manager = new MCPManager(cwd, null, async () => ({
  configs: suiteServers, sources: suiteSources, exaApiKeys: [],
}));
const loaded = await manager.discoverAndConnect({ startupTimeoutMs: 5000 });
for (const tool of loaded.tools) {
  pi.registerTool({
    name: tool.name, label: tool.label, description: tool.description,
    parameters: tool.parameters, approval: tool.approval, strict: tool.strict,
    mcpServerName: tool.mcpServerName, mcpToolName: tool.mcpToolName,
    execute: (id, args, signal, update, ctx) =>
      tool.execute(id, args, update, ctx, signal),
  });
}
pi.on("session_shutdown", () => manager.disconnectAll());
```

这只是报告中的已验证调用形状，实际产品应保留所需 native 元数据（包括 legacyName、mcpReadOnlyHint）、初始化失败清理、有限等待与动态工具清单同步；本次未进行产品实现。这里没有自行实现 JSON-RPC、协议握手、工具 schema 转换或远端调用重连。

### 原生默认来源是否可能先连接

需要**启动前**把 provider 排除列表放入进程 `--config` overlay，不能等 adapter 的 session_start 才关默认来源。

源码顺序：

1. `main.ts:1956-1957` 通过 `Settings.init({cwd,configFiles:parsedArgs.config})` 加载 overlay；`2024-2025` 绑定 initializeWithSettings。
2. `sdk.ts:1948-1949` session 同样先绑定 settings。
3. `capability/index.ts:306-318,325-342` 在调用 provider loader 前过滤 disabledProviders。
4. `sdk.ts:2804-2843,2888-2894` 才构造/发现原生 manager；非 UI 路径可能在 extension runtime 绑定前连接；UI deferred 也不能靠 session_start 保证阻止。

当前 MCP provider 集合为 native、mcp-json、claude、claude-plugins、codex、cursor、gemini、opencode、vscode、windsurf、agent-plugins、omp-plugins（见 discovery 下各文件注册 mcpCapability）。**全部禁用时原生 loadAllMCPConfigs 仍会读取 user mcp.json 的名单，但没有任何 server 可被名单强制复活，因此为空。** 工具层 adapter 则根本不调用该默认 config loader。

运行 bridge smoke 先禁用全部这些 providers，预置 user not-selected 与同名 suite-selected denylist，再在 extension 工厂加载前调用原生 loadAllMCPConfigs，精确断言 configs={}。随后 adapter 仅连接 suite-selected，真实 tools/call 成功。由此证明配置发现层无默认 server 可连接；不是把“读取用户名单”误说成“连接默认服务”。

**证据范围：本次直接使用实际 provider 开关 API 模拟 overlay 生效后的状态，未跑完整 CLI + --config + 模型会话。启动前 --config 的绑定时序有源码证据，完整 CLI 启动仍需父级验收。** adapter 单独不能保证前置隔离；丢失 overlay、漏禁 provider 或固定扩展自行连接外部资源时，不能宣称安全。

### 已确认的功能差异与成本

- SDK 有 `CreateAgentSessionOptions.mcpManager`（sdk.ts:817-818、2804-2808），可让宿主复用自定义 manager，但 extension API 没有此注入入口。本任务不改宿主，不把 SDK 参数冒充 extension 功能。
- 独立 manager 不自动成为宿主 `/mcp` 的 manager；`mcp://` 资源、MCP prompt slash command、通知桥接不能自动获得原生 session 集成。sdk.ts:5760-5780 显示这些回调由宿主持有 manager 绑定。
- 初始化工具集合已实测；tools/list_changed 后的新增/移除、extension reload、后台延迟完成的工具、认证型 HTTP MCP、取消/重连、子代理继承尚未实际验证。原生 manager 具有相关连接实现，但不能据此声称适配层所有生命周期已经验收。
- 可以复用 manager 与 MCPTool，不需要重写协议和调用桥。若要求完整保持 /mcp、资源、prompts，则需要额外 adapter 管理/命令/资源接入或公开宿主 seam；目前没有证明无需宿主改动即可完整等价。**该差异需用户明确接受；不能默默把全部 MCP 语义缩成工具。**

## 3. skills 精确选择与固定显式扩展

运行 `resources.ts` 实际创建隔离 user-default、project-default、suite-selected 三个技能，禁用默认技能 providers，customDirectories 仅指向带空格的 selected skills 目录。结果精确为：

```json
[{"name":"suite-selected","source":"custom:user"}]
```

同次调用 loadExtensions 显式载入 fixed.ts 与 suite package/index.ts；fixed.ts 真实注册 `fixed-retained` 命令且 errors=[]。provider 禁用不阻止显式入口的工厂加载。该结果针对受控固定扩展，不声称已执行用户真实 connect-bot/codebase-tools 或验证 manager/node 角色筛选。

源码：skills.ts:304-360 先做 capability 发现；485-510 独立扫描 customDirectories。capability/index.ts:306-318 只过滤 provider，所以 custom 扫描仍有效。新 manager adapter 路线还可禁用 omp-plugins，减少显式扩展 package 的旁路 skill/MCP 贡献；精确 TS 入口继续用 loadExtensions 加载。

禁用 providers 是跨资源的，会连带同来源的 rules/context 等；这是既有已接受范围，不是细粒度沙箱。所选/固定扩展也可主动注册资源或发起连接，仍须审查；这些参数不是对不受信任扩展的安全隔离。

## 最终判定

- **通过**：已安装 OMP 可在隔离环境启动 version/help、导入实际模块；customDirectories+provider 排除只得到所选 skill；显式固定扩展保留；原生 package MCP 的名单缺口被真实复现；公开 manager 自定义 loader + 原生工具桥可绕开用户名单并完成受控 MCP 工具调用。
- **源码成立、未端到端验收**：使用 --config 在原生 MCP 初始化之前禁用全部 MCP providers，因此适配扩展加载前默认来源无 server 可连接；保留实际原 agentDir 的产品启动装配。
- **不声称通过**：checkout 源码运行、真实模型工具声明、全 CLI 初始化、真实 fixed 扩展/角色、动态 MCP 工具更新、mcp:// 与 /mcp 等完整体验、真实 symlink、认证服务、并发 Chat。
- 建议向用户提供明确选择：接受 OMP suite 由公开 adapter 管理 MCP 工具、原生 /mcp/资源/prompts 不自动覆盖的差异；若必须完整等价，则当前公开 extension 接口尚有 seam 缺口。不能再把用户名单冲突当作 suite 必须继承的业务规则。
