# 05: OMP／Pi 成果批准工具与自由报告

**What to build:** 两宿主 node-chat 通过统一业务语义提交成果批准，人工模式只认原生 Yes；节点报告随时发送，正常成果交接由执行者先批准再报告。

**Delivers:** enabling: unblocks P2、P4、P5、P6、P7、P8、P9、P10、P11、P12、P13、P14、P16、P17、P18、P19、P20、P23、P24、P28、P30、P31、P34、P35、P37、P38

**Test owner for:** P38（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** 01：普通工作 node 的批准与显式完成闭环

**Repositories:** toolkit

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 沿用公共 Session／HTTP／Prompt 和现有 adapter，不造第二套客户端或持久批准权威；采用 01 已交付的实际公开协议。模型不传执行身份、报告引用、说明、产物或摘要。
- [ ] 从当前真实角色查询冻结模式，查询或协议错误不能默认自动；自动模式调用后端批准，人工模式只在原生显式 Yes 后提交。确认原文严格为“当前会话的成果是否已达到预期，可以交接并推进后续流程？”。
- [ ] 拒绝、取消、超时和无 UI 明确反馈但不批准、不将执行标为 failed、不自动重试；用户后续主动请求才再次确认。确认等待不持有阻塞 Run 的工作资源。
- [ ] 请求开始时绑定执行上下文，确认后提交仍针对原执行；不能在旧 Yes 返回后重新解析并批准 retry 新执行。真实身份和后台门禁仍由后端检查。
- [ ] node report 删除旧发送确认，保留参数校验、保存／通知错误和执行隔离；approve 不伪造报告、通知或推进。持久化失败原样反馈，不声称成功。
- [ ] 更新双宿主 node／manager 双角色指引与首次注入：正常成果先 approve 再 report，进度／失败／问题可先报告；批准不等于 manager 验收。结束节点无 node 批准职责，chief 委派与报告确认不变。
- [ ] 删除 toolkit 自己的旧 node report auto/manual 审批测试；确定性回归覆盖新工具、拒绝／异常、身份绑定及 chief 边界，不重钉旧字段。更新实际工具和运维说明（P38）。
- [ ] 本票只交付可运行工具及确定性回归；不得把受控 provider、配置存在、工具描述或单宿主通过写成原生／真实模型承诺已完成，原生测试由 10–15 按分区唯一拥有。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P38 | toolkit 旧 node report 确认及角色指引断言 | 05 |

本票还拥有工具协议与受控 adapter 的开发回归；它们是 enabling 证明，不另外认领 10–15 的原生或真实模型覆盖项。

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 本仓检查

从本票 toolkit worktree 根使用已有 Bun 1.4.4 和 frozen-lock 依赖，完成后依次运行 `bun run typecheck`、`bun run test`、`bun run check:integrations`，并实际执行受影响 workflow／node／chief 原生 dock 场景，记录精确选择器和证据。需要配对后端的 dock 将 `HERDR_BOT_BINARY` 指向参与 herdr-bot worktree 本次构建的 `safe_shutdown_shell.exe`，`HERDR_BOT_METADATA` 指向同一 worktree 元数据，不能用生产后端执行关机回归。

原生最终验收归 10–15；本票受控 provider、adapter 回归或工具加载存在不能替代这些承诺。
