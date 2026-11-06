# 为既有目标包装完整 Bot

公开能力为 `create-bot-wrapper` Bot，唯一目标是 `create-bot-wrapper.json` Map。使用现有 scheduler 的定义读取、Run 预检与发起入口，或由外层 Map 的节点引用这个 Bot；不增加产品入口。它与旧的“创建 Task＋Bot”流程不同：目标必须已经存在，不能由本流程新建、修改或执行。旧流程和其他 Map 保持原样；本文件不表示用户部署已迁移。

## 调用接口

| Map 命名文件输入 | 必填 | 内容 |
| --- | --- | --- |
| `request` | 是 | UTF-8 业务委托：真实 normal Task ID 或 Map 文件名（二选一）、明确创建要求、业务需求、资源／权限／非目标、材料写入范围，以及可选的 Bot 名称／ID 和正式交付位置 |
| `materials` | 否 | 可读参考材料及其用途；没有参考材料时不提供此绑定，不制造空路径。提供的文件不可读则失败，不视为未提供 |

`request` 无须含完整 Task／Map 配置，核实者自行经真实服务查询。`userInput` 提供本次运行背景，不替代 `request`。嵌套调用时，父节点通过 `input_rule` 以同名输入显式绑定 request／materials，并填写自己的 `manager_task_id`、zone 和自包含业务 input；管理者配置不是 Bot 属性。父节点还应声明 `saved` 和 `handoff` 输出，要求子管理者在完成整个子 Run 后将 `save_bot` 的同名交付直接报告给父层；地址登记不是业务验收，父管理者仍须显式接受交接。Map 不声明不存在的全图输出接口。

最终 `saved` 是公开接口完整读回的真实 Bot 对象，`handoff` 只记真实身份、normal Task／Map 唯一绑定、确认／授权依据、三字段来源、保存和非目标保持结论、已发生效果及未验证项。父层必须核对真实身份和绑定，不根据“完成”文字推断新能力已验证。必要时显式要求额外交付 baseline，但不通过 handoff 隐式寻址。

## 执行与授权

材料角色与安全保存只遵循[共同业务材料契约](business-materials.md)，不另立协议：

1. `create_blank` 直接消费 request 和可选 materials，自行核实真实目标及 Bot 目录。创建委托授权新建唯一绑定的空白 Bot；三字段严格为空，名称／ID 冲突就停止，不复用、覆盖或改名绕过。完整读回并确认目标未变，交付单份 baseline 和必要 handoff。
2. 三个 `write_*` 分支各自直接消费同一 baseline，独立编写、确认所属全文并交付唯一 text＋必要 handoff；不写 Bot。每个分支的 `report_policy: manual` 要求另取得报告发送确认，不能用全文确认、保存授权或 `can_complete=true` 替代。
3. 三个 `optimize_*` 分支各自直接消费同一 baseline、所属 text＋handoff，只优化表达。语义变化导致确认失效时重新确认受影响语义，不自行扩权；不写 Bot。
4. `save_bot` 直接消费 baseline 与三组最终 text＋handoff，依真实批准来源核实整个替换批次的授权，不因流程走到此处或 `can_complete=true` 推断用户批准。重读 Bot、真实 revision 及完整目标；缺材料、未确认、冲突或运行禁写时整批不开始。合法时一次保存三个字段，保持身份、绑定及全部非目标，公开完整读回 Bot 并重读目标确认未变。失败或不明先核实效果，不盲目重试、回滚或吞掉错误。

仅使用六个既有共享 Task，稳定 ID 和独立／workflow 配对归属见共同契约。本能力没有新增专属 Task。不会自动运行新能力、安装工具、补建目标、扩权或改写自身运行配置。配置交付不证明目标工具／Skill／扩展加载、模型遵从或业务效果。

## 验证入口与证据边界

使用本 worktree 独立 `CARGO_TARGET_DIR`，先执行 `npm --prefix frontend ci`、`npm --prefix frontend run build`；定向入口为：

```powershell
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch create_bot_wrapper -- --nocapture
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch shared_business -- --nocapture
node scripts/test-backend.mjs
```

wrapper 回归通过既有公开 Bot 保存／读取、Map 输入与节点保存／读取、真实文件预检检查八节点完整接线、可选材料缺席合法和错误材料拒绝，不固定提示词措辞。发布检查还实际运行本 worktree 构建的 `debug/examples/safe_shutdown_shell.exe --config panel.toml`，复用 `scripts/check-workflow-file-consumption.py` 的隔离 Smoke.prepare 配置和日志消费设施，随后经 HTTP 保存／读取本 Map 八节点与包装 Bot；真实 request 文件、参考材料缺席／可读、Windows 独占读锁拒绝均已观察。语义上未声明的 Run 输入被现有接口拒绝为 `map-read-failed`／HTTP 500（不是本烟测最初预期的 400）；保留原错误后重启同一隔离后端，公开完整读回核实原定义未变。拒绝不以特定 4xx 为充分条件，必须核对无定义副作用；这不是新增的错误状态契约。全程无 Run 或外部宿主调用，成功失败均终止并等待所属安全后端与日志线程。测试先核对发布的六个共享 Bot／Task 稳定绑定；受控宿主预检使用测试拥有的 Task 环境和 zone，不能作为部署 Suite 或原生角色加载证据。shared_business 回归独立检查 Task／Map 两种绑定三字段一次保存、真实 stale revision 拒绝及完整非目标保持。

最小真实共享 Task 调用使用[测试指引](agents/testing.md#共享-bot-指南材料的启用检查)中的隔离 Pi 入口，保留原生材料 read／write、授权结果、实际文件及完整读回。本能力发布时实际使用安装版 Pi、`task-18e31d34797f1464-3` 的原始 prompt 和公开 CompletionTransport 运行独立 Chat：真实读取源文件和技能、写入唯一 text／handoff 并完整读回，保留源文件与未授权哨兵。首次 handoff 的来源路径被原生生成的 Python 字符串转义为 TAB；保留该失败材料，在同一原生 Chat 中明确授权仅新建修正 handoff，再实际读回核对准确来源和旧文件不变。这是带显式错误修正的窄烟测，不是首次自主交付完全成功或六 Task 配对通过的证据。路径必须核对实际字符，进程零退出与模型自称完成均不足以证明材料准确。该烟测不是完整八节点自主业务执行；P39–P45 的两类目标、有／无材料、真实批准来源和六 Task 配对由 21 唯一负责，外层真实调用归 24。不能以配置存在或受控回归通过宣称这些 contract 验收通过。缺真实宿主、模型授权或能力时必须如实保留未验证。
