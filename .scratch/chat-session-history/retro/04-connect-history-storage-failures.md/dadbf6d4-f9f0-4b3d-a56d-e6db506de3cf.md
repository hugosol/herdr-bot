# Retro — 04-connect-history-storage-failures.md

## Provenance

```json
{
  "feature": "chat-session-history",
  "ticket": "04-connect-history-storage-failures.md",
  "run_id": "dadbf6d4-f9f0-4b3d-a56d-e6db506de3cf",
  "agent": "tdd",
  "agent_id": "04-connect-history-storage-failuresmd-bc3ff705-3",
  "session_id": "01a16475-23bd-7635-bf49-642637c1365c",
  "session_file": "E:\\Developer\\myhub\\herdr-bot\\.data\\bot-sessions\\bc-51d7c89c6a97\\2026-10-22T16-09-08-811Z_01a1645f-97cb-7019-986a-1886f5962fcf\\04-connect-history-storage-failuresmd-bc3ff705-3.jsonl",
  "parent_session_id": "01a1645f-97cb-7019-986a-1886f5962fcf",
  "parent_session_file": "E:\\Developer\\myhub\\herdr-bot\\.data\\bot-sessions\\bc-51d7c89c6a97\\2026-10-22T16-09-08-811Z_01a1645f-97cb-7019-986a-1886f5962fcf.jsonl",
  "workspace": "E:\\Developer\\myhub\\herdr-bot",
  "execution_cwd": "C:\\Users\\hugos\\AppData\\Local\\Temp\\hyper-ticket-0zCjWi",
  "tdd_status": "completed",
  "started_at": "2026-10-22T16:32:40.861Z",
  "tdd_finished_at": "2026-10-22T16:52:42.195Z",
  "retro_finished_at": "2026-10-22T16:55:49.712Z"
}
```

## Retrospective

# TDD 执行复盘建议

本复盘仅依据指定 ticket 的执行 JSONL 与其中的工具结果，不引用父 session 或其他执行结果。以下按对验收可信度和执行效率的影响排序；本轮只提出建议，未修改文件或重跑验收。

## 1. 高：区分“应用重建”与“后端进程重启”的验收证据

**事实**：P11 的测试 `explicit_register_recovers_new_and_existing_records_without_rewriting_first_time` 使用预定服务端时刻，最后执行 `drop(server)`，再通过 `TestServer::start_app(...)` 构造新应用并 HTTP 读回。该测试证明了独立应用实例的持久读回，但测试运行进程没有因此退出。故障保护测试也采用了这一重建方式。最终交付正文准确使用了“重建”表述，却同时勾选了包含“跨重启读回”的全部验收项，并报告本票无未完成项。

**证据边界**：不能据此认定持久化实现有缺陷；本轮没有展示“故障恢复＋预定首次时间”这一完整场景的真实跨进程读回。其他测试中一般性的重启覆盖，也不能直接补足该组合场景。

**建议**：让验收汇总明确区分应用重建、独立进程读取和实际进程重启，由审查代理逐项核对票据要求的运行边界。复用既有安全进程夹具，让新进程通过公开 HTTP 读回相同隔离存储中的预定字段；尚未覆盖的边界保留为未验证，而不是由测试名称中的 `restart` 推导完成。

**完成标准**：涉及重启的每项验收，都能指向实际建立相应进程边界的测试及结果。

## 2. 高：将“有效 red”作为进入实现的门槛，而不只是一次失败退出

**事实**：操作者警告测试首次因缺少 `yaml` 无法加载；依赖修复后的失败，又是用对象形状 `{message, level}` 匹配实际的 `string[]` 通知。代理随后修正测试，改读 `notifications`、`notificationLevels` 和 `widgets`，但在再次运行修正后的测试之前，已经修改了生产 receipt 解析与反馈逻辑。下一次运行直接得到 green。因此，日志没有展示修正后观察面上的行为 red。

相比之下，时间测试在补齐接口后再次运行，实际得到“真实当前时间不等于预定时间”的行为失败；无自主写入测试还经过临时定时重试 mutation，确实在目标断言失败，恢复实现后重新通过。这两段证据更强。

**建议**：复用现有 TDD skill 已有的 red 判定规则，不再增加同义禁令。让执行和审查的门槛落到一个可检查条件：**修正夹具或观察形状后，修正后的测试曾在生产行为修改前因目标违约失败**。已有满足的行为仍按 skill 允许直接核实，不制造假 red。对于“没有重试”等负向承诺，保留本轮受控时间＋针对性 mutation 的做法。

**完成标准**：red 记录能指出已到达的目标条件及具体违约断言；导入、类型或夹具失败单独归类，不充当该行为的 red。

## 3. 中：前置现有类型检查，而不是新增人工 API 禁用规则

**事实**：会话已经读取到 toolkit 的 `typecheck` 脚本，内容是 TypeScript 检查加 `check:authoring`。但新测试先经 Bun 运行，直到后段执行 `tsc --noEmit`，才发现七处 `.at()` 不被当前目标库支持；修复后又发现 `PanelSessionView` 缺少 `agentType`。Bun 的测试成功并不等于这些测试通过项目静态检查。最终直接 TypeScript 检查通过，但日志没有显示完整 `bun run typecheck` 中的 authoring 检查被执行。

另外，真实 interop 回归先后因 receipt 新增 `historySaved` 和既有快照中的 `agentType` 不匹配而失败；单次相关命令约 39–40 秒。

**建议**：优先将已有类型检查接入快速验证入口，并使耗时 interop 阶段依赖其成功；若现有 CI 或提交钩子尚未接入，则复用该命令补齐接线，不另写“.at() 禁用”等机械规则。协议变更时先核对已知 receipt／view 类型与断言，再进入真实时钟回归。汇总中区分直接 `tsc` 与完整 `typecheck`，避免将前者报告成后者。

**未验证**：本轮日志不足以判断仓库是否已有 CI 或提交钩子，也不能认定这些检查从未接入。

## 4. 中：把原生宿主生命周期和 wire 形状放到测试入口附近

**事实**：真实 connect interop 最初猜用了 `monitored: true`、顶层 `phase` 和 `countdown`，而现有接口分别使用 `selected: true`、`data.shutdown.phase` 和 `counting-down`。后续即使补发了 `startRun`，仍因结束上下文持续报告 `idle: false` 而没有形成完成事件；读取已有 `state-machine.test.ts` 的完整场景后，改用实际空闲的结束上下文才通过。期间出现一次 30 秒超时和两次约 5 秒的等待失败。

**建议**：在现有 connect 测试支撑入口提供一个短导航指针，分别指向公开 monitor/SSE 示例和完整 round 生命周期示例。对确实重复的宿主驱动、事件解码和带帧输出的等待逻辑，可复用窄测试 helper；业务期望仍由各测试独立书写，避免 helper 同时生成输入和期望而形成自证。

保留现有“超时列出实际帧”的诊断能力。失败反馈若先展示最后观察到的状态及正在等待的转换，会比仅等待总超时更容易定位夹具问题。

**推断**：这种入口可降低后续票据重复猜测协议和宿主状态的概率；本轮没有量化该收益。

## 5. 中：为隔离 worktree 提供可复用的依赖准备与跨仓库入口

**事实**：外部 caller worktree 第一次运行缺少 `yaml`；前端第一次构建缺少 `tsc`。Windows 下直接执行未妥善转义的 `cmd /c mklink`，创建了错误路径的 junction；第二次引用尝试仍失败，随后用 PowerShell 成功创建。错误 junction 在提交前被找到并单独删除。外部测试还需要显式传入安全后端二进制及 metadata 路径。

**建议**：在既有 worktree 准备设施中统一处理锁定依赖安装或受控复用、Windows 路径引用和 junction 清理边界，并将准备状态与测试结果分开报告。跨仓库入口只记录难以自行推导的关系：真实 caller 所属仓库、隔离工作约束、安全后端要求及外部提交独立交付；命令和默认路径继续从脚本、配置读取，避免复制成易过期文档。

若需要常驻指针，只在相关分支触发，例如“跨仓库 connect-bot 验收时，读取现有接入说明”，不把整套操作加入全局 AGENTS.md。

**保留的做法**：本轮外部修改在独立 worktree 提交，最终明确区分了本仓库提交与尚待单独合入的外部提交；这比将两者合称“已集成”更可靠。

## 6. 低：语义搜索遇到地区性不可用时尽早降级

**事实**：首次 `find` 的 26 个 judge 请求全部返回地区限制 451；工具报告枚举 201 项、读取 20 个文件，却没有得到有效判断。代理随后报告问题并改用图搜索与 grep，没有把失败当作不存在。

**建议**：在搜索工具层对同类不可恢复错误增加快速降级：首次确认地区限制后，停止本轮同后端的继续调用，返回明确的能力不可用状态及可用的替代搜索方式。此类问题适合工具实现处理，不需要增加常驻代理指令。

**完成标准**：一次已确定不可恢复的服务拒绝，不再扩展成整批相同失败；结果仍明确区分“未搜索成功”和“未找到”。

## 应保留的有效实践与结论边界

- 通过公开 HTTP、实际浏览器和真实 caller 命令观察行为；文件系统故障注入使用隔离真实存储，而不是仅替换私有保存函数。
- 无自主重试测试使用受控时间，并以实际 mutation 验证检测能力，值得继续复用。
- 关机相关 interop 使用安全外壳；本轮日志未显示真实关机执行。
- 最终日志显示后端专项 4 例通过、浏览器 1 例通过、外部相关 48 例通过，且直接 TypeScript 检查通过；这些结果不能外推为全仓库、真实宿主部署或所有重启边界均已验证。
- 本复盘没有验证外部提交后续是否合入或部署，也没有检查当前 CI、钩子或文件系统状态；这些未知项不作为已存在缺陷陈述。
