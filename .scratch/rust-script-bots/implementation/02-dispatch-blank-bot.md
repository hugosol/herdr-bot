# 02: 通过真实 Map 异步创建空白 Bot 并交接报告

**What to build:** manager 使用原派发动作启动 Rust 空白创建能力，立即取得派发结果；后端通过命名文件真实创建 Bot、交付 baseline/handoff 并自动报告，manager 显式 complete 后才放行下游。

**Delivers:** P1, P3, P5, P6, P7, P8, P9, P10, P12, P20, P21, P32, P33（本票覆盖分区见下）

**Blocked by:** 01 — 明确脚本执行与业务材料契约，解除必要的 Chat 耦合。

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 以 01 的明确表示保存/读回固定 Rust 目标；与 Task/Map 互斥，未知能力不能成为可运行目标；拒绝请求不覆盖原定义。能力直接编入后端，不引入业务 exe 或解释器。
- [ ] 通过公开 HTTP 创建隔离真实 Map/Run，使用既有 manager dispatch 入口异步执行。受控业务仍在途时，独立观察派发调用已返回，不能以函数结束或最终都返回替代证明。
- [ ] 派发冻结 input_rule、output_rule 和本次解析 inputs，按已有命名来源读取文件；对照修改 node.input 不改变业务判断，不依赖隐藏参数或未声明材料。
- [ ] 仅接受真实存在的普通 Task 或 Map 作为新 Bot 目标；拒绝脚本目标、非法/缺失/歧义目标和身份/唯一绑定冲突，不创建、修改或运行目标本身，不覆盖已有 Bot。
- [ ] 实际创建唯一绑定的新 Bot，when、plan_guide、node_guide 均严格为 ""；完整读回 Bot 和目标，以服务真实 revision 形成完整 baseline，交付独立 handoff。由一个真实下游消费者按命名文件读取材料，交叉核对完整配置和 revision。
- [ ] 创建请求非法格式/缺项/冲突/歧义分别拒绝，独立读回完整对象及真实文件，确认无对应业务误写或成功声明。创建写入失败、保存后读回失败均不能提交 canComplete=true；如实记录可得错误与实际效果。
- [ ] 业务只有检查、真实写入、完整读回和所需材料交付均成功，才自动保存 body、canComplete=true、outputs。报告输出只归所属 execution；未声明或同份重复 ID 拒绝，不另建输出权威。
- [ ] 报告保存接入原 manager 通知及 collect；公开详情、持久报告和 outputs 可读。报告前、报告后但 complete 前、合法 complete 后分别观察下游资格；无当前 true 报告仍拒绝首次 complete，不自动派发下游。
- [ ] 外部调用记录与执行读回证明脚本没有节点 Chat/Tab、伪造 Task/agent session、manual 报告等待或 Chat 关闭调用；自动资源结束不等于 complete。
- [ ] 使用真实运行中受保护定义验证创建能力的禁写，原对象不变并生成不可完成的错误报告；复用合法保存和现有文件安全边界，不直接绕过底层存储。
- [ ] 更新现有定义、运行及业务材料使用说明，记录真实成功/拒绝实例，不提前迁移正式创建流程定义。

## Coverage partition and ownership

本票拥有 P1/P6/P9/P20/P21 全部自动化，以及 P3 的在途返回、P5 的后端身份/外部调用、P10 的保存/通知/collect、P12 的完成门禁、P32 的后端自动报告/无关闭调用。

P7/P8/P33：本票只拥有创建能力用例，03 拥有合并保存用例。P5 页面归 09，P32 定义页面归 08、运行页面归 09。P3/P5/P10/P12 的真实 S2 人工 H1 由 11 准备、用户确认。P2 的两个能力构建运行联合证明和 P29 跨仓闭环归 11，不在此重复认领。

## Verification

沿 S1 的现有公开 HTTP/文件 interface 验证真实 Map、实际配置/材料和报告，不以直接 Rust 函数或模拟业务写入代替。独立字面输入来自 01 和 contract；使用独占定义、Run、zone、临时文件、数值回环服务及记录型外部宿主，明确其非真实 manager 证据。本 worktree 独立绝对 CARGO_TARGET_DIR，先构建静态页，完成检查运行 `node scripts/test-backend.mjs`；真实进程场景按测试指引安全退出并确认后清理。
