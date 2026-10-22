# 06: OMP 成功创建后独立收录真实原生对话

**What to build:** 从 zone 成功创建 OMP Bot 后，无论是否勾选 connect-bot，都在不影响创建结果的独立任务中取得本次实际原生身份并收录历史。身份获取最多 30 秒；失败仅记可关联的服务端警告，不干扰 agent 或当前会话状态。

**Delivers:** P01、P02、P05、P17、P30 的 OMP 分区；P19、P22、P25、P26 的 OMP zone 分区。

**Test owner for:** P01、P02、P05、P17、P30 的全部 OMP 覆盖；P19 的 OMP 服务端诊断；P22 的 OMP 有界任务；P25 的 OMP 精确引用读取；P26 的 OMP zone 状态独立性。对应 Pi 覆盖归 05，各 promise 须全部分区完成。

**Blocked by:** 01: 新建 Bot 回归 agent 原生会话存储；02: 成功 register 形成可持久读取的历史视图。

**Status:** in-progress

- [x] 沿用创建成功定义：OMP 可交互且合成初始指示提交已获确认，不等待模型轮次或任务结束；未确认成功的启动／提交不以 zone 来源入历史。同一原生 ID 独立成功 register 仍可按接入规则收录。
- [x] 不勾选 connect-bot 仍可收录。取得与本次 OMP 创建精确关联的完整原生 sessionId、cwd、Bot 名称及可获得的原生引用；不以创建 ID、Tab／Pane、别名、cwd 或“最近文件”猜身份。
- [x] 本次独立任务不进入创建成功响应关键路径，不改变创建结果或增加历史专用页面轮询；页面关闭不取消任务。不关闭 Tab、不停止 agent、不重发初始指示。
- [x] 用尚未完成的受控身份读取观察创建已经成功，同时其他创建、register 和当前观察仍能推进，不为身份等待持有这些共享状态；关闭页面后提供身份，独立读回正确历史。
- [x] 身份获取总预算最多 30 秒，覆盖期限前成功、到期仍不可得、精确文件 header 暂不可读、页面关闭及保存失败。取得身份后仅尝试保存一次；超时／失败即结束，不猜身份、不在期限后继续获取或重试写入。不将预算理解为模型轮次时限或任意内核磁盘操作的强制取消保证。
- [x] 精确文件引用只读取必要身份元数据；明确覆盖 OMP 的 title 元数据行可位于 session header 前，不只取首个物理行，也不因格式相近猜 agent 类型。不读取／导入正文或扫描旧目录。P25 本票覆盖 OMP 收录路径不改写旧原生文件；存储策略保护和普通 register 覆盖分别归 01、02。
- [x] 身份缺失、超时和保存失败只产生能关联本次创建及原因的服务端警告；实际收取交付运行诊断。创建页面无历史状态或历史失败提示。
- [x] 实际浏览器核对：未 connect 时仅出现在历史，不建立当前登记；connect 后两视图可同时存在，注销当前登记不删除历史。
- [x] P26 本票覆盖 OMP zone 正常收录、身份等待／失败及保存失败，保持当前 keepAlive、登记、完成后关机监听和倒计时原有语义，不持久化临时意图、不执行真实关机。
- [x] 受控 adapter 覆盖上述时序和故障；另用隔离真实 OMP／Herdr 交互取得精确原生引用与 header 证据，独立比对历史 ID。受控 adapter 结果不冒充真实外部运行；缺失外部前提须明确报告，不能降级成 connect-only。
- [x] 准备 P01-H1 的 OMP 创建对象定位、原生 ID／header、历史读取结果及最小实际部署步骤，不提交用户模型任务。P23-H1 的存储与发现证据归 01，最终 H1 确认仍由用户完成。
- [x] 同步更新 OMP 收录与诊断说明。复用现有生产／受控 adapter 和合理的共享实现，不增加只为测试存在的外部 interface；本票不依赖 05，独立完成后 OMP 即可演示收录。

## 验证边界

依据已批准 contract 的 S1、S2、S4。P04／P09／P29 的跨来源和并发测试归 07，但本票仍须维护已交付的唯一性及名称优先级，不能将已知错误留到最后一票。H1 不暂停代理可完成的实现与检查。

## Comments

- 代理实现与本票自动／真实隔离验收完成，尚待独立集成合并；按项目规则合并后才设 `closed`。
- `cargo test --test omp_zone_history --test session_history -- --nocapture`：11 项通过（OMP 7、既有历史 4）。OMP title 首行即终止的临时 mutation 已观察 red，恢复后全部 green；生产代码不保留 mutation。
- `npm run build`、`cargo build` 成功；最终 `npx playwright test ompZoneHistory.spec.ts` 6 项全部通过（2.1 分钟），实际 Chrome 页面／交付进程验证页面关闭、两个视图独立、30 秒界限、失败服务端诊断及无后台重试。
- 真实 Herdr 0.9.5 + OMP 18.12.6 的独立隔离运行，经公开 zone 创建取得官方精确引用、title 后 session header，独立历史 ID 为 `01a1648d-9a3a-7422-969b-1d2ee0a984e7`，当前登记为空。只使用 loopback 合成模型，不提交用户账号／用户模型任务。
- 逐项证据与 H1 步骤：`docs/omp-zone-native-history-acceptance.md`；真实原生记录：`docs/evidence/omp-zone-native-history.json`；实际交付 warning 原文：`docs/evidence/omp-zone-history-diagnostics.json`；注销后历史截图：`docs/evidence/omp-zone-history-after-unregister.png`。
- 用户实际部署 P01-H1 仍未验证，不以受控 adapter 或隔离真实运行代替；本票已完成该项所需的证据和最小操作准备。
