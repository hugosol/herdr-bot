# Ticket 14：发布 V1 原生验收交接

日期：2026-11-18。用户要求本轮收尾，后续会话继续。**未完成；不认证成功、不集成、不关闭，未成功执行 hyper_auto_coding finish（工具因三票未集成而拒绝结束）。** 不再新增模型请求、测试或修复，保留原始错误和原 worktree。

工作流最终已 collect：无 active／integrating，14为带未完成error的 `ready`，不是验收通过；总体 `finished=false`。见 `evidence/handoff-workflow-state.json`。下次获准继续时用原工作流 `status`／`dispatch` 恢复实现，不用 `retry_integration`，不重建票据。

## 恢复位置

- 原票：`implementation/14-published-v1-native-acceptance.md`；负责 P31、P34、P35 的 V1／其他非V2发布场景分区。
- herdr-bot：`C:/Users/hugos/AppData/Local/Temp/hyper-ticket-KirY7B`，分支 `hyper/node-result-approval/14-published-v1-native-acceptanc-50f74d-349645ddb346`。
- toolkit：`C:/Users/hugos/AppData/Local/Temp/hyper-ticket-gEpwuE`，分支 `hyper/node-result-approval/14-published-v1-native-acceptanc-50f74d-3a3cc4cfa470`。
- 最新已核对场景源码：herdr-bot `2c910b98421508828cf7844f0c130fe8f98285fb`，toolkit `15b11cfe6827398e597606efa508120a6e1e3458`。更早提交含 `b160035`、`f5fe897`、`9f20497`、公共 observer 移植 `5aaa8c6`、候选误门禁／旧Run污染修正 `2938042`。
- 最终收尾 ledger：`t14-closeout-ledger.json`。存在**一处未提交、未测试修改**：`scripts/node_approval_v1_acceptance.py`，移除investigation继承的额外候选／proposal要求；停止前已编辑，不能按已验证代码使用。已另存协调仓 `evidence/ticket-14-handoff/uncommitted/node_approval_v1_acceptance.py`，原worktree保留。
- 最新逐场景 ledger：`t14-isolated-03/outcome-ledger.json`。其 `passed=false`、`finalIntegrationEligible=false`，48项内部覆盖仍为0 verified／48 unverified。

## 最新五个 OMP 独立场景

以下只描述真实观察，不把局部成功提升为整例或整票通过。

| Case | 已观察的实际业务 | 最早阻碍／未完成 |
|---|---|---|
| v1，Run `bc-194f5c08496d` | Bun 创建空白 Bot／baseline；create_blank 自动批准且 manager complete；write_plan_guide 真实原生 Yes、manual批准及ready报告 | 105个gpt-6-luna成功回执后一次timeout（HTTP200，requests1）；三字段保存未到。manager对write_plan_guide的complete曾返回isError=true，应读原响应，不假定成功 |
| outer，Run `bc-2055f6386cf1` | prepare_inputs成功，inputsReady=true、ready=[w1] | 7成功回执后独立HTTP400；未派发节点 |
| revision，Run `bc-2db444610b45` | 命名request准备成功，inputsReady=true、ready=[w1] | 5成功回执后独立HTTP400；未派发节点 |
| planning，Run `bc-227e01a8e411` | 实际run status／只读检查 | 5成功回执后独立HTTP400；未到prepare_inputs或派发 |
| investigation，Run `bc-250c05c8e999` | 两节点真实auto批准，manager complete两节点并finish，Run completed；48个真实身份均正确 | responsibility/truth/handoff=true，但scope=false：通用fixture额外要求Chat完整候选／proposal，导致拒绝；页面节点事实未验证 |

该轮174次adapter调用：170份实际 `serverModel=gpt-6-luna`，4次失败无成功身份回执。不是174次全部成功，也不是完整48项通过。五个case原 `scenarioExit` 均为1。

## 保留的早期失败

- `t14-native-01/outcome-ledger.json`：首次双宿主真实运行146次模型调用，48项未认证。OMP在真实Yes前错误要求所有同字节proposal均被native read，并扫描已失败Run，污染其他case。Pi真实模型写的request.target缺type，真实Bun以missing-field拒绝；没有代模型补字段或重试该决定。
- `t14-omp-recovery-02/outcome-ledger.json`：112个真实成功回执后，1个 `provider-error/httpStatus=200/requests=1`；不把HTTP200当业务／SDK成功。后续共享terminal传播的502不代表独立业务失败。
- 最新五个case已经独立transport；outer/revision/planning的400不再是共享terminal传播。原native OpenAI消息call/result集合匹配，**不证明SDK转换后的Responses payload合法，也不说明provider拒绝原因**。

## 已修驱动问题与待核对判据

### 已处理

1. 同字节额外proposal的native read误门禁：改为核对真实完整字面候选，不能强制读取全部等价副本。
2. 旧失败Run污染：只处理当前Run及其真实childRunId闭包。
3. 每case独立transport及现有入口具名selector：默认仍完整覆盖；部分选择仍保持未完整认证／非零，不能缩减48项来刷绿。
4. write_plan_guide真实Ask询问是否新增row-level evidence步骤。原批准全文只要求重复订单号、负金额、UTF-8报告及保留原值；Main明确业务偏好为no，不额外增加布局／证据步骤。新输入已写明，旧未答复失败保留；不能用默认checked代替用户选择。

### investigation 的额外scope要求

原common fixture让所有节点都“完整候选展示Chat／必要时写proposal”。对本票只读auto调查节点，这可能是额外批准前提，不是P34自身要求。Main在收尾前给出的边界：对照原contract和独立业务输入；若仅来自错误模板，删除该case不当继承，不改真实职责、权限、材料完整性或handoff；旧scope=false不改写。

**本轮不再继续修改或重新评估。** 下一会话先确认收尾记录是否已在停止指令前产生补充复核；若有，必须明确是对原证据的事后合同复核，不声称原场景全绿。不因为应用真实完成了两个节点就跳过S3页面证据。

## 复用事实及生产逻辑边界

复用 `accept-workflow-codex.py`、NativeCompletionTransport、OuterAcceptance、共享host隔离／cleanup、公共原生observer和workflowVisibility浏览器退出观察；未另建第二传输或产品CLI。票特有部分是发布副本22节点、真实Bun四输入／三字段、revision／目标／文件业务判据。

未修改backend、frontend/src或toolkit生产业务逻辑；涉及Python验收、JS页面／确认驱动、浏览器测试进程观察及文档。

14已有 `agent/config.yml` 显式shellPath；实际原生bash曾返回 `write: command not found`，是把工具设备write当shell命令的真实调用，不是证明shell未启动。OMP18.20普通bash使用嵌入式pi-shell，不能把“shellPath配置了Git Bash”直接等同普通bash工具一定生成外部Git Bash进程；15的配置接线问题不能套到14。

## 已运行验证

- `t14-checks/effective-evidence.json`：本仓五项及toolkit完整test/integrations/3具名dock成功；前端274，浏览器375通过／1既有skip，toolkit1932，具名dock10。
- toolkit初次typecheck因锁定TS7 win32 optional包缺失失败；独占cache frozen --force恢复后只重跑typecheck，exit0，authoring93文件零诊断。原失败／修复／复验分开保留，未改锁或编译器。
- CLI5、实际表单observer3 tests／28断言、SDK4 tests／21离线向量，以及selector窄检。见 `t14-selector-03.json`、`t14-recovery-offline-02.json`。
- 未因后续fixture变化重新跑已过九项全仓。以上不是P31/P34/P35完整业务通过。

## 下一会话最小步骤

1. 先读原票、contract、最新逐case ledger和investigation原定性判据；确认错误额外scope要求及待补页面证据。
2. 若要复用已完成investigation事实，单独记录合同复核和证据来源，不改旧失败／退出码。
3. 将一个实际provider失败最小化：复用15已提交的安全诊断，不再并行启动五个长case。HTTP200+SDK错误不被现有仅 `!response.ok` 的400诊断覆盖，不能猜根因或原样重发。
4. 再逐项推进V1三字段真实保存、其他发布场景与22节点页面交叉验证。不得代模型补request.target.type、伪造Yes或拿机械provider认证真实业务。

## 证据与收尾

协调仓副本：`evidence/ticket-14-handoff/`；原证据根为本worktree `.scratch/node-result-approval/evidence/`。原JSON绝对路径不改写，按副本相对路径映射。复制清单／SHA256完整性见副本 `preservation.json`；未复制的完整模型／原生存储仍保留在worktree。

最新逐case ledger确认55/55所属进程退出，`allChromeExited=true`、cleanup errors为空。无新增运行；不得清理源码、worktree、原始失败、报告／模型记录或归属不明文件。实际收尾后提交／未提交文件与资源状态以同目录最终stop记录补充为准。
