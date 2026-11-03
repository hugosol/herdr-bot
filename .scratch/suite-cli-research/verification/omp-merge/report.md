# OMP 原生项目 / suite MCP 配置合并矩阵

## 结论

**两边同时原生加载，不等于两个同名服务器同时连接。** 本次实际调用安装版 `loadAllMCPConfigs`，未禁用任何 provider：

- `cwd/.omp/mcp.json` 属于 `native`（priority **100**），胜过 suite explicit extension package 根的 `mcp.json`（`omp-plugins`，priority **90**）。
- `cwd/mcp.json` 属于 `mcp-json`（priority **5**），反而败给 suite。不能概括成“项目永远优先”。
- 同 ID 先按名称 first-wins；即使 command、args 或 URL 不同，也不会合并字段或创建两个连接。
- 不同 ID 但 command/args/env/cwd 相同，会再按 connection equivalence 去重，本次只启动一个进程。
- 高优先级 `enabled:false` 仍占同名 key，不会回退到低优先级 enabled 的定义。
- 独立 user `disabledServers:["duplicate"]` 会压制两边的同名定义；不是 suite 自包含的豁免名单。

## 运行环境 / 安全范围

执行安装包 `D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent/src`，版本 **18.16.0**，Bun **1.4.4**。不是声称 checkout 已可运行：`E:/Developer/myhub/oh-my-pi` 的 Windows pi_natives 缺失已由既有 `../omp/report.md` / `probe-output.json` 记录，本次不重复触发已知失败。此次涉及的六个实现文件，安装包与 checkout **SHA256 全部相同**，完整哈希见 `source-hashes.json`。

每例独占系统 Temp 根，隔离 HOME / USERPROFILE / APPDATA / LOCALAPPDATA / XDG_* / PI_CODING_AGENT_DIR / cwd；过滤继承的 PI_/OMP_/HERDR_ 和名称含 API_KEY/TOKEN/SECRET/PASSWORD 的变量。user MCP 的服务器映射为空，只有单独 denylist 场景增加 disabledServers。不读取实际认证，不使用真实项目 MCP，不连接 9749、已有 daemon 或 Herdr，不启动模型。所有配置/stdio server/随机 binary fixture 均自造；产品代码、用户配置及他人脚本未修改。

URL 变体只验证 loader 选择，**没有发出 HTTP 请求**；这些配置用回环占位 URL，未声称 HTTP transport 可连接。其余 stdio 案例由 `new MCPManager(cwd,null)` 的默认原生 loader + `discoverAndConnect(options)` 实际发现、连接并执行原生 MCPTool，**没有 custom loader / 工具桥 / provider 禁用**。

## suite 正常入口

Temp suite 的 `package.json`：`{name:"owned-suite",version:"1.0.0",omp:{extensions:["./index.ts"]}}`；index.ts 是普通 noop extension，MCP 在 suite 根 `mcp.json`，不塞入 extension 的自建 manager。

传给真实 loader 和 manager 的共同选项：

```ts
{
  enableProjectConfig: true, // 仅 project-off 对照为 false
  filterExa: false,
  filterBrowser: false,
  extensionRoots: {
    explicit: [suiteRoot], mode: "explicit-only",
    configured: [], configuredLevel: "user"
  }
}
```

这里 `explicit-only` 选择显式 extension package 根，不是禁用 native / mcp-json / 其他 MCP providers。suite source.level 实测为 user；项目来源为 project。完整 source.path/provider/providerName/level 原样保存在 evidence。

## 实测矩阵（user mcpServers={}，无 denylist）

`P` = 所测项目文件，`S` = suite 根 mcp.json。enabled 未写表示原生默认启用（即表中 true）；只在 false 变体显式写 false。两者同时声明时每例均实际运行 loader。

| 变体 | cwd/.omp/mcp.json 结果 | cwd/mcp.json 结果 | stdio 启动数（两位置） |
| --- | --- | --- | --- |
| 同 ID、完全相同定义 | duplicate，P/native | duplicate，S/omp-plugins | 1 / 1 |
| 同 ID、不同 args | duplicate，P/native，project args | duplicate，S/omp-plugins，suite args | 1 / 1 |
| 同 ID、不同 command | duplicate，P/native，裸 `bun` | duplicate，S/omp-plugins，绝对 Bun command | 1 / 1 |
| 同 ID、不同 URL | duplicate，P 的 project URL | duplicate，S 的 suite URL | 未连接 / 未连接 |
| 不同 ID、同 command/args/env/cwd | project-alias，P/native | suite-alias，S/omp-plugins | 1 / 1 |
| 同 ID，P false / S true | configs={} | duplicate，S/omp-plugins | 0 / 1 |
| 同 ID，S false / P true | duplicate，P/native | configs={} | 1 / 0 |
| enableProjectConfig=false 对照 | duplicate，S/omp-plugins | duplicate，S/omp-plugins | 1 / 1 |

完全相同 stdio 定义使用绝对 Bun command、同一 server.ts、args=[server,"shared"]、env={OWNED_MARKER:"fixture"}、同一个绝对 cwd。不同 args 仅项目改为 [server,"project"]；不同 command 仅项目改为裸 bun。URL 变体分别为 `http://127.0.0.1:1/project-owned-no-connect` 和 `.../suite-owned-no-connect`。因此并非只比较名称而未真正制造参数差异。

## 独立 user disabledServers 对照

在同样空 user mcpServers 下增加 `disabledServers:["duplicate"]`，以下 **14 例全部 configs={}、sources={}，零 server start**：

| 同 ID 变体 | cwd/.omp/mcp.json | cwd/mcp.json |
| --- | --- | --- |
| 完全相同 | 空 | 空 |
| 不同 args | 空 | 空 |
| 不同 command | 空 | 空 |
| 不同 URL | 空 | 空 |
| P false / S true | 空 | 空 |
| S false / P true | 空 | 空 |
| project config 关闭 | 空 | 空 |

前十例在 evidence.json；URL / project-off 四个补充例在 evidence-extra.json。没有为获得预期结果修改 provider 列表。

## 实际工具 / 进程证据

- 主矩阵 26 例，加补充 4 例，**30 个命令均 exit 0**；主矩阵 12 个 stdio winner 各启动一个进程、各执行一次 `sha256_file`，其余没有启动 stdio。每次连接的 initialize、tools/list、tools/call 请求和实际 response、启动 PID/argv/cwd 都在 `parsed.serverEvents`。
- MCPTool 收到的唯一业务参数是本例 owned fixture 的绝对路径。server 校验路径后实际 `fs.readFileSync`，计算 SHA256 和长度（每例 **345 bytes**）；不是 echo，也不把预期哈希给 server。
- Python 在 spawn 前独立读取随机 binary fixture 计算 expected；12 个原生工具返回的 digest 和长度均与独立 Python expected 完全相符。首例 digest：`c90a87b3c385916eb616cb7f597abbf632ed5e93756d93e5d0656e9b1a16710d`，原生 details.serverName=duplicate、provider=native。
- 真实 manager 每例 finally `disconnectAll()`，子命令退出后 runner 删除本例 Temp。服务器退出事件不是完整的存活证明：Bun 被终止时通常不会写 exit callback，所以另做 Windows OpenProcess 检查；12 个已记录 owned PID 全部返回 error 87（不存在），见 cleanup.json；没有清理其他进程。
- `loadAllMCPConfigs` 返回 configs/sources/exaApiKeys，不直接返回 warnings。为不伪造诊断，同例另调用真实 `loadCapability('mcps',{cwd,extensionRoots})`，原样保存 items/all/_shadowed/warnings/providers。**该 diagnostic 是未套用 loader 的 filter/suppress 的 discovery view**，所以 disabled / project-off 场景不能把其 items 当成最终连接配置；最终 winner 以 loaded.configs/sources 为准。30 例 warnings 均为空、stderr 均为空；同名覆盖与 alias 去重本身不产生 warning。

## 源码解释（安装 src 与 checkout 六文件逐字节一致）

1. `discovery/builtin.ts:42-45,239-268`：native priority=100，先读取 cwd/.omp/mcp.json（再 .mcp.json）然后 user agentDir；该路径不是仓库根 fallback。
2. `discovery/omp-plugins.ts:43-47,276,294-370`：priority=90；按原生 explicit roots 读取 .mcp.json / mcp.json，生成 omp-plugins source；相对 command/cwd 先按包根归一，比较的是处理后的定义。
3. `discovery/mcp-json.ts:171-190`：cwd/mcp.json 与 cwd/.mcp.json 为 project 级 fallback，priority=5。
4. `capability/index.ts:125-132,180-275`：provider 按 priority 高到低；遍历 allItems，先 `capability.key`（MCP name）first-wins，然后对不同名称执行 equivalent。普通重复记录只标 `_shadowed`，不会按字段 merge，也不发 collision warning。
5. `capability/mcp.ts:77-114`：key=server.name；equivalence 先比较 auth/oauth/requestIdFormat，再比较规范化 transport。stdio 比 command、args、env、有效 literal env keys、cwd；http/sse 比 URL、headers。name、enabled、timeout、instructions 不是 connection identity；不是按可执行文件最终解析路径、进程身份或服务器握手名字去重。
6. `mcp/config.ts:103-178`：读取用户 disabled/forced-enabled 名单；enableProjectConfig 在 dedupe **前**排除 project source。enabled:false / user deny 使用 suppress，仍占同名 key 但不输出且不对异名 enabled server 做 equivalence suppression。denylist 优先；本次不设置 enabledServers。
7. `mcp/manager.ts:646-675`：默认 discoverAndConnect 真调用 loadAllMCPConfigs，随后 connectServers(最终 configs,sources)。本次没有把预先构造的 configs 注入 manager。

## 复现命令及文件

从仓库根执行：

```text
python .scratch/suite-cli-research/verification/omp-merge/run.py
python .scratch/suite-cli-research/verification/omp-merge/run.py --extra
```

每例完整 Bun command / cwd / spec / 真实 stdout / stderr / exit code / fixture expected / configs / sources / diagnostics / manager status / tool result / server wire 见 `evidence.json`、`evidence-extra.json`；提炼主矩阵见 `summary.json`；源码哈希见 `source-hashes.json`；owned PID 结束核对见 `cleanup.json`。`probe.ts` 与 `run.py` 是本任务专属 throwaway smoke 复现资产，不是永久测试入口。不运行 build/lint/formatter/永久 tests。
