# 12: 延期 tester：定义、发起、输入与角色上下文验收

**What to build:** 后续 tester 专项手动领取本票，在真实安装及公开黑盒入口核实定义、独立 Task、Map 管理者准备、初始 Skill/输入和资料供给；不把产品受控回归当作原生通过。

**Delivers:** P1, P2, P3, P4, P9, P10, P33, P34, P35, P36, P37, P42, P46, P59

**Blocked by:** 08 落地创建map资产、唯一资料与选择性迁移交接; 11 手动：两仓工具协议与角色接入联调

**Status:** ready-for-human

**执行方式：** 延期至 tester 专项，不纳入本轮 TDD 执行队列；仅手动选择执行。

**Target project:** `E:/Developer/myhub/herdr-bot-tester`（独立专项，不是当前自动仓库清单成员）。

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [ ] 延期至 tester 专项，不纳入本轮 TDD 执行队列。本票目标项目为 herdr-bot-tester，可修改该项目的相关场景并执行；它不是当前 repositories.json 的第三个开发仓，开始前由操作者另行准备其合规 worktree/分支和所需授权，不阻塞 01–10。
- [ ] 拥有 check-http-basics 的 bot-definition 新契约 case：独立新四字段字面对象、草稿/目标规则、旧字段拒绝与拒绝后持久不变；旧脚本通过不能视为新覆盖。
- [ ] 复用 accept-paired-task-launch 等发起入口，补齐独立/节点对照、实际 Skill 加载、初始输入、Bot 索引不注入与实现资料供给；直接 Map 在无 Bot/必需材料未齐时确实启动管理者，准备/派发工具按新规则运行。
- [ ] 真实节点取得背景、委托、实际材料/输出要求与条件式业务资料入口；词典每消息同快照且不任意改写用户输入/路径/报告。创建map 独立 Task 与节点实际资料入口可达，实际业务遵从仍交用户，不以存在文件证明已阅读。
- [ ] 本票仅拥有首次发起/加载格的 P34/P35/P37；恢复及嵌套/chief 工具格归 13。OMP/Pi 和要求的宿主组合分别取证；未选择、native-only、缺安装/权限/认证、清理失败和原始失败保持未验证，不用产品绿补格。
- [ ] 保存 tester 源码/锁文件身份、产品/toolkit worktree及产物来源、安装版本、原生会话/实际工具回执、公开读回、文件和所属进程退出。回填 P59 对应专项结果，保留 13 及人工/现场未验项。

## 检查与证据

从操作者准备的 tester worktree 执行，先按其 README 和场景 `--help` 确认参数。统一显式绑定 `$BOT`、`$TOOLKIT`、本轮构建后端与新的证据目录，不推测邻接 checkout。

先执行 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" check-http-basics --help`，按该入口的实际参数要求绑定本轮独占后端/fixture 和新证据目录，再执行新 Bot case。配对入口为 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-paired-task-launch --toolkit-worktree "$TOOLKIT" --backend "$BACKEND" --isolated-config "$CONFIG" --combinations all --cases all --evidence-dir "$NEW_EVIDENCE"`。所有变量均由操作者显式设置为本专项拥有的绝对路径；不得使用默认生产数据/会话，发布本票不执行这些命令。

场景补丁的定向设施自测、各仓适用产品/工具包检查与原生场景结果分列；设施自测零退出不代表原生覆盖。沿用既有资源隔离、数值回环、原生确认、所拥有进程退出及原始错误保留约定。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 12 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。
