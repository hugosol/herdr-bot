# OMP 项目与 suite 同时原生加载 MCP：CLI 实测

## 结论

**两份声明不会必然创建两条连接。** 安装 OMP 18.16.0 的原生发现会先按来源优先级取胜者，再压制同名或异名但连接配置等价的声明。

- `cwd/.omp/mcp.json` 与显式 `-e <suite包根>` 的 `suite/mcp.json` 同时存在：本次同名同配置、同名不同 args、异名同配置均只有项目服务器实际启动、initialize 和 tools/call 各一次；不是两个进程启动后仅隐藏一个工具。
- 项目 `.omp/mcp.json` 的同名项 `enabled:false` 会压制 suite 的默认启用项：两边均不启动，没有工具。
- **不能泛化为“项目总优先”。** 项目根 `cwd/mcp.json` 属于低优先级 fallback；本次根配置同名不同 args 时 suite 胜出，只有 suite 的 B fixture 被实际读取；根配置 `enabled:false` 也不能压制 suite 的启用声明。
- suite 走原生 extension package 发现，原生 session 创建和拥有 manager。补充的异名不同 args 场景中，项目 A 与 suite B 各有一次独立启动/initialize/call，且两者都有原生资源及 prompts 能力。**无需另建 manager、注册替代工具或放弃原生能力。**

对于当前仓库实际 `.omp/mcp.json` 与 suite 复制同 ID、同 command/args 的情况，本 smoke 对应结果是 **保留仓库原生声明的一条连接，suite 那份被去重**，不是双开。未访问或启动当前仓库声明的真实 MCP，更未触碰其现有端口/daemon。

## 环境、入口及分层

- 安装包：`D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent`；本 runner 从安装 `package.json` 实读版本 **18.16.0**。
- 最终成功入口为该安装包的 **`src/cli.ts` 原生 CLI**，不是 SDK 手工拼装 manager，也不是 checkout 构建产物。
- CLI 按正常参数解析 `-e <suite package>`，原生加载 suite `package.json` 的 `omp.extensions:['./index.ts']`，并由 `omp-plugins` 从 suite 根发现 `mcp.json`。没有调用 `setDisabledProviders`、没有定制 MCP loader、没有 `new MCPManager`、没有 `registerTool`。
- suite 的验证扩展只在原生 `session_start` 后取 `MCPManager.instance()`（原生 session 安装的实例），调用公开方法并记录结果，最后 `ctx.shutdown()`。工具调用的是该实例已生成的 `MCPTool.execute`，并额外记录 `pi.getAllTools()` 证明工具确已进入原生 session registry。
- 选择 source CLI 是为了让扩展绝对源码 import 与 CLI 使用同一模块实例。没有修改安装源码。`dist/cli.js` 前期探针存在扩展调用错误，未据其退出 0 宣称验收成功；详见下方失败记录。**最终成功范围明确为已安装包的 source CLI，不宣称 dist CLI 的 singleton 取用路径已验收。**
- `E:/Developer/myhub/oh-my-pi` checkout 缺 Windows `pi_natives.win32-x64-modern.node`，不能将本结果说成 checkout 可运行。这个既有失败及安装 CLI `--version` 输出复用 `../omp/probe-output.json`，未重复执行 checkout 失败检查。

## 安全边界与复现命令

先阅读了本仓库 `docs/agents/testing.md`、前次 `../omp/report.md`、`run-resources.py` 和 `resources.ts`，复用其隔离方式，但本次**没有复用其独立 manager 工具桥方案**。

运行命令（仓库根）：

```text
python .scratch/suite-cli-research/verification/omp-native-duplicate/run.py
python .scratch/suite-cli-research/verification/omp-native-duplicate/run.py --root
```

成功命令均返回 **0**。每个场景的完整绝对路径 argv、cwd、CLI stdout/stderr、最终来源、原始 MCP JSON-RPC request/response、服务器 PID、期望哈希和实际工具结果分别保存在 `evidence.json`（五场景）与 `evidence-root.json`（两场景）。不是仅保留终端截断预览。

每个 CLI 的完整命令形式（每次真实绝对 Temp 路径见 evidence 的 `cases[].command`）：

```text
bun D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent/src/cli.ts --mode rpc --no-session --no-tools --no-skills --model owned-local/smoke -e <owned Temp>/<case>/suite package
```

- 每场景独占 Temp cwd、suite、HOME/USERPROFILE/APPDATA/LOCALAPPDATA/XDG_* 与 `PI_CODING_AGENT_DIR`。过滤继承的 PI_/OMP_/HERDR_/XDG_ 和 API_KEY/TOKEN/SECRET/PASSWORD/PROXY 变量，不读真实认证，不读真实用户 settings。
- 用隔离 `models.json` 提供专用 `owned-local/smoke`，端点 `http://127.0.0.1:1/v1`，占位 key 仅在隔离文件中。**没有输入 prompt，没有模型 turn，没有受控模型响应、更没有真实模型调用。** RPC stdin 为 EOF；全部行为在 session_start 内完成。
- 仅启动自己生成的 Bun stdio MCP，不连接真实 MCP、Herdr、现有 daemon。没有监听真实 MCP 端口；没有 build/lint/formatter/永久 tests。
- 保留原生集成不等于 TUI UI 验收：本次不是交互 `/mcp` UI 测试，也不声称验证 UI 绘制、交互选择或热刷新。

## 夹具与行为证明

每场景 Python 在受控进程启动前创建两个不同文件，独立 `hashlib.sha256(read_bytes())` 记录期望值：

- A：项目配置的目标文件，309 bytes。
- B：suite 不同 args 配置的目标文件，343 bytes。

配置形状：

```json
{"mcpServers":{"duplicate":{"command":"bun","args":["<controlled-mcp.mjs>","<fixture-A.bin>","<A-events.jsonl>"]}}}
```

suite 同配置场景完全复用 A 的 command/args；不同 args 场景将 fixture 与日志参数换成 B；异名场景把 suite ID 改为 `suite-other`。`enabled:false` 仅放在项目同名项，suite 项不设置该字段（原生默认为启用）。

`sha256_file` 工具参数只有 `{nonce:'actual-file-read'}`，不传路径、内容或预期 digest。服务器从自身配置 argv 得到文件路径，真实 `fs.readFileSync` 并 `crypto.createHash('sha256')`；返回实际读取字节数/路径/digest。独立检查了原生工具返回文本的 digest 和长度与 Python 期望均一致。每个活动服务器还通过 resource read 再次真实读取文件计算哈希。完整 wire 内含实际 request/response，不是 echo 或仅检查最终字典。

## 原生 CLI 结果矩阵

表内 `start/init/call` 都是受控服务器自己的日志计数；等于 0 表示没有启动日志，不是“连接后没调用”。同配置异名场景共享 A 参数，因此只应出现一个 A 进程而非 B。

| 场景 | 项目来源 | 原生最终 source | A start/init/call | B start/init/call | 原生工具数 |
|---|---|---|---|---|---|
| 同 ID 同配置 | `.omp/mcp.json` | native / 项目文件 | 1/1/1 | 0/0/0 | 1 |
| 同 ID 不同 args | `.omp/mcp.json` | native / 项目文件 | 1/1/1 | 0/0/0 | 1 |
| 不同 ID 同配置 | `.omp/mcp.json` | native / 项目文件 | 1/1/1 | 0/0/0 | 1 |
| 项目 enabled:false | `.omp/mcp.json` | 无活动服务器 | 0/0/0 | 0/0/0 | 0 |
| 不同 ID 不同 args（正对照） | `.omp/mcp.json` | native 项目 + omp-plugins suite | 1/1/1 | 1/1/1 | 2 |
| 同 ID 不同 args | 根 `mcp.json` | omp-plugins / suite 文件 | 0/0/0 | 1/1/1 | 1 |
| 项目 enabled:false（同配置） | 根 `mcp.json` | omp-plugins / suite 文件 | 1/1/1 | 0/0/0 | 1 |

最后一行活动进程虽读取 A，但其**配置来源明确为 suite 的 `mcp.json`**；suite 启用声明与项目禁用声明有相同 args，因此不能仅用 fixture 名推断来源。来源从原生 manager `getSource` 实读。

实际原生工具名：`mcp__duplicate_sha256_file`；正对照额外有 `mcp__suite_other_sha256_file`。正对照 manager 记录 suite source 为 `{provider:'omp-plugins',providerName:'OMP Extension Packages',path:'<suite package>/mcp.json',level:'user'}`；项目为 `{provider:'native',providerName:'OMP',path:'<project>/.omp/mcp.json',level:'project'}`。

### 真实工具返回 SHA256

| 场景/实际目标 | digest |
|---|---|
| 同 ID 同配置 / A | `c5d515dd4b660485b5746789e33668ed1b4db9faf78f62955d92e1e643076269` |
| 同 ID 不同 args / A | `6c0bc1dd4606d383b4c38310058bf839346a0a6b3358b4a1dec4349f907af79b` |
| 不同 ID 同配置 / A | `ae719381d9c3b8293fa010a1e6289b6f84203eca85e67c3d60434675bc36a77c` |
| 正对照 / A | `bc83f5fcb8df5f7ed8460e0a08fcf22328fc2792907ea6984e939876c8c04bd5` |
| 正对照 / B | `e7d4be0f267d3259bacca387545932b1139f54921221872688e88874a94c1aa9` |
| 根配置不同 args / B | `1386d79c3e218656d60655f0ff6424ba89c40dd74b944fdea931b741270a3615` |
| 根配置禁用 / suite 来源 A | `ec3eae727ffce92f87e98d7520fa8f3acc98f6662733c84ad7e88b0e2c50eaad` |

## 原生资源与 prompts 能力

所有七个实际活动服务器（五场景中五个、根配置两场景中两个）均在同一 session 原生 manager 上完成：

1. `ensureServerResources`，`getServerResources` 返回 `fixture://sha256`、mimeType `application/json`；wire 含 resources/list 和 resources/templates/list。
2. `readServerResource(name,'fixture://sha256')` 返回真实文件 digest/bytes，服务器有一次 `read-resource` 日志，Python 独立期望全部匹配。
3. `refreshServerPrompts`、`getServerPrompts` 返回 `fixture-prompt`；wire 含 prompts/list。
4. `executePrompt(name,'fixture-prompt',{})` 发送 prompts/get，返回受控 user text；**未把该 prompt 提交模型**。

尤其正对照 suite-B 与项目-A 都成功：suite 包根发现没有把 MCP 降级成仅 extension 自造工具，原生 resources/prompts/read 路径仍可用。

## 与 loader 矩阵交叉核对

与 OmpMergeMatrix 交换了 package 声明、实际原生优先级及 enabled:false 结果。其 `../omp-merge/evidence.json` 共 **26 例，全部 returncode=0**；其较广的 loader 矩阵与本 CLI 七例一致：native `.omp` > omp-plugins suite > 根 mcp-json fallback。同名不合并，异名等价连接也被去重。这里的结论不是单凭该矩阵推断；上面的 start/init/call 和 SHA256 为本任务独立 CLI 实测。

checkout `docs/mcp-config.md:483-504` 也明确这三个来源的顺序及 first-definition-wins，但文档仅用于解释，不替代运行证据。

## 前期失败与清理

保留失败，不把 CLI 退出码本身当通过：

- `attempt-unsupported-flag.json`：最初 dist CLI 命令带不存在的 `--no-prompt-templates`，五例退出 2，没有服务器启动。删除该误用参数后定向继续。
- `attempt-wrong-accessor.json`：第二轮 dist CLI 中把 `MCPManager.instance` 当属性，session_start 报 `waitForStartup is not a function`；CLI 虽退出 0，但每例 success=false，不算通过。改用已安装 source CLI 与正确 `instance()` 调用。
- `attempt-tool-shape.json`：第三轮 source CLI 已证实来源、资源、prompts，但误将 `getTools()` 元素当 wrapper 的 `.tool`，工具调用失败。改为其直接返回的原生 MCPTool。该轮 runner 清理探测还遇 Windows tasklist 本地编码错误；随后用 replacement decoding 单独清理，记录所有所属 PID 均已退出并删除该 Temp，原始结果/事件保留。
- 最终两个成功 runner 均正常退出；服务器日志都有 `stdin-eof` 与 `exit`。主五场景 PID `6532,7240,12012,25520,28624`、根配置 PID `5244,11660` 在 cleanup 时均已退出，未需强杀。两次 `cleanup.tempRemoved=true`，没有遗留 fixture 或所属服务器。

只写了本验证目录。没有改 herdr/OMP/Pi 源码、spec、他人的脚本或用户配置。永久测试、build、lint、formatter 均未运行。
