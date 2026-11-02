# 02: report 完成声明与单一来源输出登记

**What to build:** 节点用一次 report 提交简短 body、必填 boolean can_complete 和可选命名 outputs。后端从真实 Session 关联确定报告身份，将输出随报告保存后再通知；调用者可以查询报告和分阶段结果。登记不读取或操作业务文件，旧报告及重启后的运行事实仍可读取。

**Delivers:** P10, P12, P25；P2, P8, P9, P11, P13, P26 的下列覆盖项。

**Blocked by:** None (can start immediately).

**Repositories:** herdr-bot

**Status:** ready-for-agent

## 契约与覆盖归属

依据 [contract](../contract.md) 与 [spec](../spec.md)，经 S1、S3 验证。报告记录是输出登记唯一权威来源；不建立 registry、状态索引、背景副本或新身份映射。

| Promise | 本票覆盖项 | Test owner | 其余覆盖归属 |
|---|---|---|---|
| P2 | 输出绝对地址、同／异卷和 zone 外地址；非法类型／相对地址不登记 | 02 | 直接输入：01 |
| P8 | 后端身份归属、非法调用者、两个 Run、保留旧 execution 与真实落盘 | 02 | 实际工具 Session 注入：07 |
| P9 | 同 execution 按名更新、省略保留、新 execution 隔离的登记事实 | 02 | 下游获得 A=a2/B=b1：03 |
| P10 | 不存在文件可登记；不检查文件或生成／复制／移动／发布；无输出 true 合法 | 02 | 无 |
| P11 | 真实持久记录和目录无第二套输出权威来源 | 02 | 派发解析一致性：03 |
| P12 | 保存故障无登记、待办、通知或恢复后候选事实 | 02 | 无 |
| P13 | 保存成功通知失败仍可查询、分阶段结果、有界无自主重投 | 02 | 失败通知后的下游解析：03 |
| P25 | 独立历史样例及真实重启后读取；旧 Run 不恢复管理／写入资格 | 02 | 历史缺字段不能放行 complete：04 的 P14 |
| P26 | 缺 can_complete 的新请求明确拒绝且不保存，与历史读取区分 | 02 | toolkit 配套实际调用：07 |

## Acceptance criteria

- [ ] 新请求要求 body 与严格 boolean can_complete，可省略 outputs；缺字段、错误类型、非法地址和额外运行身份字段明确拒绝且不追加报告。沿用既有基本参数与同机／角色限制。
- [ ] 后端只根据调用 Session 的真实执行关联决定 Run、node、execution、source 和 target。覆盖管理者、无关联、歧义、两个 Run 及保留旧 execution；旧调用不能改写当前执行事实。
- [ ] 报告持久保存新增字段。历史报告缺字段可读，但不补出 true；不得为兼容旧形状新请求提供默认放行。
- [ ] 同 execution 先登记 A=a1、B=b1，再只登记 A=a2，再省略 outputs，记录足以推导 A=a2/B=b1；新 execution 不继承旧集合。不增加删除输出协议。
- [ ] 接受同卷、异卷、zone 外的合法绝对输出地址。不存在文件也登记成功；不检查存在、可读性、格式、数量或完整性，不修改源文件字节，不创建登记目标。无 outputs 的 true 报告合法。
- [ ] 报告记录保留唯一权威登记事实；实际持久目录、Run 当前事实和背景无新登记副本。仅从记录派生的查询结果不得变成独立权威存储。
- [ ] 在测试拥有的存储中制造真实保存故障，核对错误、原记录、待办、宿主调用及恢复读取；失败候选不得被观察为已提交。
- [ ] 保存成功、通知失败时报告可立即查询，返回保存／通知分阶段结果；不回滚、自动重报或重投。保留至少 300 ms 的有界观察及全部宿主调用，不声称无限期性质。
- [ ] 使用独立历史样例并实际重启后端，保留报告和 Run 事实可读证据；旧 manager/node 身份仍不能写入或接管旧 Run。
- [ ] 本票同步迁移 herdr-bot 内受影响的新 report 请求与测试夹具，按各场景明确选择 true／false，不一律补 true；同步长期协议文档。完成门禁属于 04，实际 toolkit 迁移属于 05。

## Verification

在本票 worktree 根运行 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture` 及受影响的历史记录检查，再运行 `node scripts/test-backend.mjs`。经真实 HTTP 和隔离文件记录冒烟，提交正常、非法、保存失败及通知失败报告并查询；实际执行重启读取。受控 Herdr adapter 只证明确定性故障，不替代 07 的真实宿主证据。文件操作遵循既有 files module 和协调范围，锁不得跨宿主通知，不新增存储抽象或自动恢复。
