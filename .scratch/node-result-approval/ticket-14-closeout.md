# Ticket 14：单格验收收尾交接

**验收未完成，不关闭 Ticket 14。** 十格均有过独立运行；截至本次收尾，只有两格 `investigation` 完整通过。已实现单格入口、隔离夹具及证据采集；以下八格不是未写入口，而是运行失败或尚未复验。未运行两仓完整回归，不能把单格、历史记录或部分 S3 当成整票通过。

原票：[14-published-v1-native-acceptance.md](implementation/14-published-v1-native-acceptance.md)。原始运行目录在 `C:/Users/hugos/AppData/Local/Temp/hyper-ticket-KirY7B/.scratch/node-result-approval/evidence/`，留在原 worktree，不进入 Git。每格先读 `<目录>/business/evidence.json` 的 `cells`，再读 `business/cells/<host>/<case>/native/<host>/evidence.json` 的 `originalErrors`、`publicFacts`、`coverage` 和 `cleanupErrors`；模型/代理问题对照 `completion-wire` 和格内的 `model-wire/index.json`。证据只绑定原运行的源码，不作为合入后复验。

| 宿主 / Case | 本次证据目录 | 已观察 / 下一步 |
|---|---|---|
| OMP / `investigation` | `oi-1794997421` | **4/4 通过**；51.344 秒静默观察，10 条合法 heartbeat 未误判成业务推进。 |
| Pi / `investigation` | `gi-1795008712` | **4/4 通过**；51.172 秒、10 条 heartbeat、无清理错误。只给出清理建议不是执行清理，判据已对齐发布 Map。 |
| OMP / `revision` | `xr-1794997990` | 隔离索引就绪、`w1` 已派发；模型流无终帧超时。用新版基线职责限定单跑；核验 `baseline`、`handoff`、三个修订者分别原生批准、真实替换保存和新 revision。 |
| Pi / `revision` | `ir-1795009561`、`jr-1795011418` | 已验证 `collect` HTTP 200 完整响应。此前 `w1` 误等候选确认；已限定候选要求只适用于编写者，但复验在派发前网络超时。先复验只读基线、再验证完整修订闭环。 |
| OMP / `outer` | `to-1795013054`、`oo-1795001719` | 命名 `materials` 已实证让 manager 确认输入并派发 `w1`。较早原生 Ask 内有完整 Task 提案，提取已用录制表单验证；最近 worker 只在 Chat 问范围后报告阻塞。新版委托明确既定范围并要求实际 Ask，尚未复验。仍缺真实 Task 创建/读回、W2 新 request、八节点子 Run、父子批准与最终 Task/Bot 保存。 |
| Pi / `outer` | `qo-1795002198` | 索引可查，`w1` 无报告、候选或实际 Ask，最终超时。新版命名材料、明确委托与原生 Ask 处理尚未在 Pi 复验；后续完整度同 OMP。 |
| OMP / `planning` | `xp-1795005415` | 两个隔离 Bot 公开读回成功；模型流在 `outline` 派发前无终帧超时。复验候选及原生决定、Map 保存/读回和 `inspect → summarize` 的 `findings` 绑定；不运行生成的 Map。 |
| Pi / `planning` | `rp-1795012862`、`fp-1795016334` | 已越过索引及错误的 Bot 详情路由；一轮把「先展示、之后再确认」误判为「批准当前候选」。已修正判据，不批准缺失候选。`fp` 已结束且失败：worker 最终真实读回两 Bot、revision 与 Task；仍将 Suite 实际资源/zone 视为未验证，未交付概要，manager 将 `outline` 判失败。下一次先提供获准的只读资源核验路径，再走候选、批准和保存；不得执行生成 Map 或宣称未核验的运行能力。 |
| OMP / `v1` | `ov-1795005870`、`sv-1795011473` | 早期真实 Bun 创建完成，三编写者并行冷启动时代理排队后断连；逐个派发版本在 `create_blank` 前遇网络超时。隔离索引、正确 Bot 路由、原生 Ask 修复未在该宿主复验；差三字段保存及完整原生/UI/manager 证据。 |
| Pi / `v1` | `pv-1795013304`、`lv-1795013819` | 已实证真实 manager 逐个派发，Bun 创建及部分 S3 已取证；Ask 值/标签比较已用录制表单验证。最近 `write_when` 因 Bot 查询 404、索引和业务确认停住；新版预检与委托未复验，最终保存未发生。 |

## 恢复顺序

1. 有授权模型和独占依赖时，先查 `fp` 最终证据，再复验 Pi `planning`/`revision`，验证已修正的材料和批准边界；随后逐格验两宿主 `outer` 与 `v1`，补 OMP `planning`/`revision`。每次一个宿主、一个 case、一个新证据目录；不重放原失败请求或旧 Run 副作用。
2. 区分三类失败：上游未给 HTTP 响应、HTTP 200 流无终帧、宿主在单槽排队时断连。单格内按真实 manager 逐个派发 V1 节点；不擅自增加模型容量，或修改产品超时/发布 Map/批准顺序。只看到节点报告或 UI 确认不等于保存和 `complete`/`finish`。
3. 全部单格恢复后，再按[原票](implementation/14-published-v1-native-acceptance.md)运行双仓检查及完整 92 项覆盖；不拼接跨版本证据认证整票。

```sh
python scripts/accept-workflow-codex.py --scenario node-approval-v1 \
  --hosts "$HOST" --cases "$CASE" --jobs 1 --native-only \
  --backend "$HERDR_BOT_BINARY" --toolkit "$TOOLKIT_WT" \
  --paths-file "$ISOLATED_PATHS" --pi-package "$PI_PACKAGE" \
  --auth-file "$AUTHORIZED_AUTH_FILE" --evidence-dir "$NEW_EVIDENCE_DIR"
```

参数用绝对路径，证据目录必须不存在，Windows 路径尽量短。可选 shell 用 `--shell-path` 显式指定。单格 `--native-only` 按设计不认证整票；看所选 `cells[].state`、coverage 和清理证据，不仅看外层退出码。不得提交凭据、模型原文或宿主运行目录。Toolkit 票分支 `9e337e4` 只含旧宿主验收快照，与其 `dev` 较新的快照冲突；不应以旧快照覆盖新记录。
