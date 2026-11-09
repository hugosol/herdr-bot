# 共享文件 → 双消费者 → 业务汇总的真实验收

本场景覆盖文件交接协议的 P23/P24：真实 Herdr、Pi 原生 Chat、实际模型和两个参与 worktree。受控 HTTP 回归、已有旧版 A/B→C 示例、准备好的文件或最终 completed 状态都不能替代本场景。角色工具与门禁另见[测试指引](agents/testing.md#文件交接真实角色自动验收)；本入口复用其隔离部署和真实模型传输，不复制旧例中“manager 默认读材料验收”的职责。

## 入口与依赖

从参与的 herdr-bot worktree 执行。先按测试指引安装依赖、构建该 worktree 前端静态资源和后端。不同源码副本必须使用独立 `CARGO_TARGET_DIR`，不可传入其他 checkout 的旧可执行文件。toolkit 必须指向参与 worktree 的绝对根；其实际角色入口及配置会复制到本次专用部署，而不是修改用户配置。

```powershell
$env:CARGO_TARGET_DIR = Join-Path $HERDR_BOT_WT 'backend/target'
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --example safe_shutdown_shell
python scripts/test-workflow-file-handoff.py `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --scenario fan-in --paths-file $ISOLATED_PATHS
```

`--paths-file` 必须是专用依赖根 TOML，不是用户业务或 agent 全局配置。所需根为 `herdr`、`pi_package`、`pi_agent`（只读加载 `extensions/herdr-agent-state.ts`）和 `workflow_model`（专用 `model.json`）。模型声明、命名凭据环境变量、可选 `--model-config` 以及公开 `completion` 薄传输的完整格式见上述测试指引。使用薄传输时将示例的 `--scenario roles` 改为 `--scenario fan-in`；真实模型自行选择工具，传输不替它决定读取、计算、登记或派发。该方式不宣称原生 OpenAI provider wire 合规，也不将 Pi 的真实模型结果外推为 OMP。

入口自动执行 report 参数/persistence 的 OMP/Pi dock；dock 中的受控 provider/Herdr 只验证该参数契约，不计作本场景真实业务模型证据。独立观察窗口默认 300 秒（`--timeout 1..900`），不重试业务或模型选择；超时、错误选择、缺依赖或清理失败都非零退出，不降级成替身。

## 固定字节和 Map

启动前固定两组可区分的字面输入及预期。全部是无 BOM UTF-8、LF，末尾 LF 必需；`\n` 表示一个 LF。预期只供验收器使用，不从实际文件、模型报告或被测解析逻辑重算，也不作为消费者初始化的答案。

| Run | 外部种子 → source 登记文件 | left 独立结果 | right 独立结果 | summary 最终文件 |
| --- | --- | --- | --- | --- |
| 0 | `item=amber-widget\nunits=7\nunit_price=6\nfee_per_unit=5\n` | `item=amber-widget\nsubtotal=42\n` | `item=amber-widget\nfee=35\n` | `item=amber-widget\nsubtotal=42\nfee=35\ntotal=77\n` |
| 1 | `item=violet-gear\nunits=9\nunit_price=4\nfee_per_unit=3\n` | `item=violet-gear\nsubtotal=36\n` | `item=violet-gear\nfee=27\n` | `item=violet-gear\nsubtotal=36\nfee=27\ntotal=63\n` |

两份 Map 均在任一 Run 启动之前保存：

- `source` 无依赖；其命名文件输入 seed 指向验收方固定的外部业务种子。真实 source Chat 用原生 read 读取种子、write 创建此前不存在的业务文件，然后用实际 `report` 登记 `outputs` 数组中 `id=result` 的地址。种子是已记录的验收方业务输入，不是伪造的运行记录或预先准备的最终产物；不依赖宿主 Bash 能否执行 Windows Python。
- `left`、`right` 都 `blocked_by: [source]`；两者的 `input_rule` 都包含 `{id: shared, type: node, node_id: source, output_id: result}`；各提供方的 `output_rule` 均预先声明 `id=result`。初始化只带原始登记地址，不带正文或答案。left 读取后计算 `subtotal=units*unit_price`；right 独立读取同一文件后计算 `fee=units*fee_per_unit`。分别写独立文件并登记各自的 `outputs` 数组中 `id=result` 的地址。
- `summary` 依赖 left/right，命名输入分别引用两者当前 execution 的 `result`。真实 summary Chat 必须实际读取两份文件，核对 item，再合并、相加并写最终文件。业务合并和内容检查属于这个明确安排的 node，不是框架动作。
- manager 只用真实 `run`/`collect` 编排：显式 dispatch source → true report 后 complete source → 显式 dispatch left/right → 两者各自 true 后 complete → 显式 dispatch summary → true 后 complete、finish。manager 不读业务正文做默认验收；验收器只观察，不替 manager 发动作或中途改运行记录。

## 目录和隔离

入口为一次执行创建独占 Temp 根；隔离 HOME/USERPROFILE、APPDATA、LOCALAPPDATA、XDG、Pi 存储、命名 Herdr server、后端持久目录和实际角色部署。后端只绑定动态数值回环地址，关闭 mDNS，不操作用户 Chat、Run、业务文件或真实关机。

manager zone、source/left/right/summary 四个 node zone 互不相同；每组业务产物位于 `business/0` 或 `business/1`，均在所有 zone 外。文件直接登记外部原地址，消费者原生读取这个地址，不复制或移动到 zone。两个 Run 的中间资料指引为 manager zone 下各自的 `.manager-zone/.workflow/<Run ID>`，从实际初始化观察其区别；它只是位置指引，不要求预分配目录、输出文件或强制业务落盘位置。

取得端口后持续消费 Herdr 和后端进程 stdout/stderr。正常或失败均停止并确认本次拥有的 server/进程；原始错误与清理错误分别保留。凭据配置退出时移除，不把密钥写入证据。临时证据保留供核对，不删除用户资源。

## 通过判据与证据

stdout 返回 `passed` 和 `evidence.json` 路径。失败仍保存已获得的真实证据，不能把准备完成写成通过。证据分别关联：

- **P23**：`runs[].nativeHistories` 的真实 Session、Run、execution 关联；`reads` 中以 toolCall ID 配对的原生 `read` 参数与成功 toolResult；两个消费者必须读取同一源地址，summary 必须读取两份独立结果。`files` 保留全部实际原始字节的 hex 和 SHA-256，逐字节比对固定预期。只有最终文件正确但缺任一实际读取，仍失败。
- **P24**：`runs[].zones` 和原生会话头的真实 cwd；Run 各自的 `materials` 指引；report HTTP 中原始外部地址、消费者初始化与原生读取地址一致；检查 zone 中没有同名业务产物。两个 Run 的输入与预期不同，避免把旧 Run 的文件冒充本次结果。
- **职责及可归因性**：保存运行前 Map/hash、所有验收方协调输入、参与源码 revision/status/diff、锁文件与后端哈希、依赖版本、实际模型请求/响应、原生历史、report 登记、manager complete/dispatch/finish 请求与时间顺序、只读运行记录副本。manager 原生轨迹只能含 run/collect；complete 的响应仍显示下游 pending，再有后续显式 dispatch，排除框架暗中派发。

常规检查须分别在两个参与源码根执行并记录命令、退出码和范围：

```powershell
# herdr-bot，使用该 worktree 独立 CARGO_TARGET_DIR
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture
node scripts/test-backend.mjs
npm --prefix frontend test
# runner 的缺依赖 fail-closed 回归，不代表真实业务通过
$env:WORKFLOW_HANDOFF_TOOLKIT = $TOOLKIT_WT
python scripts/test-workflow-file-handoff.test.py

# toolkit，临时 worktree 不共享默认 sibling 布局时显式指定安全壳及只读元数据
$env:HERDR_BOT_REPO = $HERDR_BOT_WT
$env:HERDR_BOT_BINARY = Join-Path $env:CARGO_TARGET_DIR 'debug/examples/safe_shutdown_shell.exe'
$env:HERDR_BOT_METADATA = Join-Path $HERDR_BOT_WT 'meta-data/task-options.toml'
bun test tests/workflow-chat
bun test tests/harness-dock/node-report-contract.test.ts tests/harness-dock/node-report.test.ts tests/harness-dock/manager-state-actions.test.ts tests/harness-dock/role-guidance-rehearsal.test.ts
bun run typecheck
# 修改 toolkit 源码/夹具/依赖/环境时还需其最终集成入口
bun run test
```

共享源码不边改边验收；重型检查按仓库测试规范协调独占窗口并顺序执行。既有结果只有对应源码、夹具、依赖和环境完全未变才可复用，不能借用另一 checkout 的结果。P23/P24 的真实读取、业务字节、隔离及两仓规定检查必须齐全才算本场景通过；缺少任一真实依赖或不能证明隔离，明确记录失败/阻塞，不以全局套件或旧场景成功顶替。

## 三个真实通知现场（P34.01）

历史工作流契约中的 P34.01 仍要求分别观察 manager 工作中、空闲和 collect 等待三个通知现场；本次 `fan-in` 自动入口不安排这三个现场，不能把其通过当作该历史覆盖完成。需要验证时，在独占真实部署中由专用业务 node 分阶段报告，通过已记录的外部协调文件分别释放报告；每个现场都关联报告 ID、报告保存回执、Herdr 输入提交结果、manager 原生实际收到的输入和独立 collect 结果。验收方的每次继续指示和释放均留证，不能根据最终模型动作反推已读、顺序或打断。通知只承诺输入提交阶段；详见[节点报告与独立通知](workflow-runtime.md#节点报告与独立通知)。manager 仍不默认读取业务正文。

## 显式交接、产物与活动顺序（P62.01/P63.01）

历史 P62.01 的核心观察是业务汇总 node 实际读双材料且字节符合事先独立预期；本次新登记协议的证据必须使用上文 P23 的原生读取、当前 execution 登记及原始字节，不能引用旧运行冒充。P63.01 中“交接后仍可收尾”的活动重叠另需带时间戳的实际活动证据：由前置业务 node 报告后在外部 gate 等待，再观察 manager complete → 显式 dispatch 下游 → 下游实际活动，最后释放前置收尾。不能把 running 状态当作进程仍活动，不能由验收器替 manager 派发。本次自动场景只观察显式 complete/dispatch/finish，不声称已覆盖这种收尾重叠；原有契约不因此放宽。

## H1 交接与证据完整性

H1 是用户对真实汇合推进、三类通知及全部干预的最终使用判断，不由脚本 `passed` 代替。向用户提供真实原生历史、业务材料字节、通知现场和所有人工/协调输入，并明确哪些现场未执行。P23/P24 自动通过不能被报告为 P34/P63 的额外现场通过或用户 H1 已认可；用户判断也不能替代缺失的真实读取、隔离或两仓检查。
