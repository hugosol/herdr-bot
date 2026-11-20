# Map 脚本日志预览：OMP / Pi 能力调查

2026-11-19。本文保留最初源码调查与已确认边界；随后已在 herdr-bot 和 my-agent-toolkit 实现，未修改 OMP／Pi 宿主。验证结果见末节。

## 已确认需求

用户确认：在 manager 的原生终端展示脚本运行日志，包含 console 输出和普通 stdout/stderr，不是节点交付文件 `outputs`。

## 结论

两种宿主都支持在异步等待期间通过工具 `onUpdate` 提交结果快照并原地刷新，不需修改宿主内核。用户已确认嵌入正在等待的 `collect` 工具调用输出区，接受随对话滚动，不使用固定 widget，也不更新已返回的 dispatch 调用。后端现已提供运行中的日志快照，manager 扩展已接入。collect 采用直接顶层调用，避免宿主内建包装器不转发嵌套工具更新的限制。并行布局、行宽与结束清理策略已逐项确认并实现，见下文。

### OMP

源码根：`E:/Developer/myhub/oh-my-pi`。

- `packages/coding-agent/src/extensibility/extensions/types.ts:275-290,418-442` 声明 widget；`packages/coding-agent/src/modes/controllers/extension-ui-controller.ts:425-475` 同 key 替换、undefined 清除并请求重绘。字符串数组最多 10 项，五行可容纳，但长行可能折行。
- `packages/coding-agent/src/extensibility/extensions/types.ts:1018-1097` 支持工具 onUpdate、ANSI renderResult 与 TSP describeResult；`packages/agent/src/agent-loop.ts:3731-3753` 在执行尚未结束时发工具更新。工具卡受折叠和视口影响，不等同固定常驻区域。
- `packages/tui/src/tools/mcp.ts:419-455` 将 MCP progress message 的换行压成一行；`packages/coding-agent/src/mcp/tool-bridge.ts:379-433` 合并高频更新。默认 MCP progress 展示不是原样五行日志通道。

### Pi

源码根：`E:/Developer/myhub/pi`。

- `packages/coding-agent/src/modes/interactive/interactive-mode.ts:2543-2583,2630-2641,2789-2800` 支持同 key widget 替换、清除与请求重绘，默认编辑框上方；数组最多 10 项。
- `packages/coding-agent/src/core/extensions/types.ts:480-485,670-688` 声明 onUpdate 与 renderResult/isPartial；`packages/coding-agent/src/modes/interactive/interactive-mode.ts:3737-3780` 在工具等待期间更新对应行。
- `packages/coding-agent/src/core/nested-tool-calls.ts:221-230` 的嵌套工具更新需要可选 onUpdate 接收；内置 codemode 的调用在 `packages/coding-agent/src/extensions/codemode/execute.ts:515-517,532-549` 只传 signal。不能假定嵌套 collect 会自动拥有可见的独立工具结果区。

## 实现前的业务链路缺口（调查记录）

- [scripts.rs](../../backend/src/runs/scripts.rs):87-149：后端独立启动 Bun，stdout/stderr 各由线程读取，日志只保留有界尾部，在进程结束后 join 汇总；没有在途发布。该文件索引 freshness 为 metadata_changed，结论来自磁盘源码。
- [scripts.rs](../../backend/src/runs/scripts.rs):190：日志仅附于 failure；不能依赖成功报告获得运行日志。
- [script-runner.ts](../../backend/src/runs/script-runner.ts):5-16：stdout 含带前缀的宿主协议，console.log/info/debug/warn/error 重定向到 stderr。预览须过滤协议、同时消费普通 stdout/stderr；不能直接把 stdout 当全部日志。
- `E:/Developer/myhub/my-agent-toolkit/extensions/workflow-chat/manager-chat/shared-logic/tools/index.ts:75-82` 的 collect 仅等待最终返回；`shared-logic/actions.ts:33-38` 调用现有 HTTP collect，没有实时日志展示。
- [ADR 0019](../adr/0019-rust-script-bots.md) 要求 dispatch 后立即返回、后端持有执行生命周期，manager 显式交接。日志预览不应将 dispatch 改成长时间同步执行，也不应以日志替代报告或 complete。

## 建议与尚待决定的边界

建议保持 collect 的结果及取消语义不变，另提供运行日志尾部快照/订阅，由 manager 扩展异步消费，通过 onUpdate 更新 collect 工具输出区；OMP 的 ANSI/TSP 渲染入口分别适配 renderResult/describeResult，Pi 使用 renderResult。按 Run/node/execution 隔离，排除协议消息；清理终端控制字符、正确处理 UTF-8 跨块与未完成行，限制单行及缓冲大小。stdout/stderr 合并只能承诺接收顺序，不能恢复两个管道的严格原始写入顺序。

用户已确认：最多五个日志行，长行截断，未换行内容也刷新；collect 结束后在工具结果区保留最后日志和原有结果。用户希望用日志观察脚本活动，但静默不代表停止，有日志也不保证业务有进展。日志预览仅供 UI，不加入模型可见的工具 content；最终尾部应放入 UI details 并由 renderer 呈现。用户已确认：同一次 collect 的输出区按节点分区，每个节点最多五行，不额外创建 collect 调用；每区标题显示执行状态及距最后输出的时间，无输出时明确显示尚无输出，不生成假日志。用户已确认：取消 collect 或日志连接断开时停止刷新，保留快照并标注已停止观测，不取消脚本、不自动重连；再次 collect 先显示当前 execution 的已有尾部，再继续刷新，重跑不混入旧执行日志。后端有界尾部保留至 Run 结束，不增加完整日志归档或后端重启恢复。用户已确认匹配范围：沿用现有 collect 的后端身份归属，公共 Session 身份解析至当前管理 Run，再以 runId/nodeId/executionId 定位同层当前脚本执行，而非按 manager 曾触发过哪些工作匹配；子 Run 不递归汇入父 Run。用户已确认整体方案并授权实现。工具调用结束时 final result 会替换 partial，已显式保留最终展示数据，不改变 collect 的业务结果。本次不包含完整日志归档、重启恢复或自动重试。

## 实现与验证

后端新增 `POST /api/workflow/manager/script-output`，以当前 manager 身份读取有界快照；toolkit 在 collect 在途时每次成功请求后间隔 500ms 采样，失败停止。普通 stdout/stderr 在后端持续捕获，协议消息过滤，五行尾部每行最多 512 字节。

`github-clone.ts` 将克隆子进程的 stdout/stderr 实时转发到脚本 stderr，并用 `--progress` 启用非 TTY 进度。仅转发副本将 Git 的回车刷新转换成换行，原始诊断尾部保持不变；校验命令继续只捕获结果，不显示为日志。不改后端采集或报告协议。`bun test scripts/github-clone.test.ts` 已通过（1 个用例、19 项断言），覆盖隔离子进程的在途输出、协议 stdout、校验静默与失败诊断，无外部网络请求。

宿主内建 codemode／eval 包装器不会自动把嵌套扩展更新转给外层工具区。因此 collect 明确作为顶层原生工具调用；Pi 使用公开的 `model-only` exposure，在 Suite only 下保持直接可调用且不供嵌套调用。不改变 Suite mode，不增加 widget，不修改宿主。详情见 [运行说明](../workflow-runtime.md#脚本日志尾部读取)。

OMP 扩展层现为 Collect 提供在途动画和静态结束／错误提示；ANSI 定时显示更新随工具结束、异常或取消清理，不依赖日志是否持续产生。ANSI／TSP 隐藏最终原始 JSON，保留最后日志；模型收到的结构化正文与后端逻辑不变，Pi 显示不变。管理者扩展在完整的 `Workflow report` 输入开头添加一次 `📋`，保留原始普通消息及正文，不拦截重发。用户已确认前缀同时进入历史和模型上下文，且不修改 OMP 源码。

本次显示验证：定向三文件首轮 13 passed／1 failed；失败为报告测试误将宿主注入的职责指示当成报告正文，并使用了目标 TS lib 不支持的 `.at`。改为按实际 provider 文本分块核对报告后，该用例单独通过，验证历史正文、模型输入、单次请求、重复前缀及不完整报告边界。`bun run typecheck` 与 `bun run check:integrations --extension workflow-chat` 通过。PTY 中调用实际注册的 renderer，观察到连续不同动画帧、静态结束／错误状态、保留中文日志且无 JSON；临时烟测文件已删除。这是 renderer 可视证据及隔离原生会话协议验证，不是生产管理者完整 TUI 截图，也不是全仓通过声明。

本次执行的检查：

- `npm --prefix frontend run build` 成功，为后端测试准备当前静态页面；Vite 有现有大 chunk 警告。
- 独立 `CARGO_TARGET_DIR` 下 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch typescript_ -- --nocapture`：6 passed，包含真实 Bun 与 HTTP 的在途读取、隔离和生命周期检查。
- toolkit `bun run typecheck`（含 authoring）通过；`bun run check:integrations --extension workflow-chat` 全部 COMPLETE。
- toolkit workflow-chat 与相关原生 dock 定向集：首轮 86 passed／3 failed；修正测试对固定工具调用 ID、Pi 隐藏工具代理的错误假设后，仅重跑失败的 3 项，全部通过。不是全仓测试通过声明。
- 已安装 OMP/Pi 原生宿主专项验证执行中更新、正常结束、失败停止、取消保留、Pi Suite only 顶层暴露，并检查后续 provider 请求不含预览日志。provider 与 HTTP peer 为受控测试设施，不代表真实模型自主选择或生产部署验收。
- 终端 PTY 烟测调用实际注册的 OMP ANSI／Pi renderer，观察运行中与成功结束两帧、中文长行截断和最终业务结果；OMP TSP 语义结果的五行数据及 `wrap:false` 亦已检查。临时脚本已删除。未在真实用户会话内操作，也未取得完整原生 TUI 会话截图。

