# 06: 更新配置、原生存储与 Resume 的交付说明

**What to build:** 用户从当前交付说明和领域词汇能够准确区分发起 Chat、环境加载与原生会话恢复，不再把移除 session-dir 时留下的历史 autoResume 说明理解为本项目自动续接能力。说明与已批准合同一致，不虚构实现或验证状态。

**Delivers:** P21。P 编号引用本功能已批准合同。

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] 当前说明清楚区分 trigger config（初始消息、初始 Skill 调用、model、thinking）与 env config（当前扩展配置），说明发起应用两者、Resume 不重放 trigger config。
- [ ] 解释恢复读取来源 Bot 最新 env config 与当前 Bot 选项目录，不是历史快照或版本回放；来源缺失／Bot 删除使用 agent 原生默认发现，有 Bot 时逐项过滤且空集合仍关闭发现。
- [ ] 区分初始 Skill 调用与加载型 Skill、初始消息与系统提示词注入；未来加载型 skills 和系统提示词注入不描述为已经交付，不新增空实现或未来能力占位。
- [ ] 明确发起 Chat 不传 --session-dir，原生文件由 agent 管理；清理将历史 autoResume 说明描述成本项目自动续接功能或本次来源关联理由的措辞，不修改 agent 原生配置或新增续接功能。
- [ ] 使用 Bot、Chat、zone、Herdr Workspace 等现行领域词汇，尊重已接受 ADR；不改写历史 ADR、历史证据或父级 spec／contract 来消除历史差异。
- [ ] 解释已有目标只定位、Resume 不自动继续任务及降级不代表已确认成功；区分参数提交证据与真实扩展加载，说明 H-Pi／H-OMP 在用户确认前仍未验证。
- [ ] agent 审阅实际更新的交付说明及领域词汇与合同一致性，不新增源码或文案逐字匹配测试，不用未运行的功能描述冒充运行证据。本文档票不以运行时实现作为硬前置，也不宣告其他票已完成。
