# 01: 明确脚本执行与业务材料契约，解除必要的 Chat 耦合

**What to build:** 为无 Chat 的固定 Rust 能力提供明确、可校验的执行与材料表示，使后续业务能复用现有定义、预检、派发和报告 interface；本票不宣称脚本业务已可运行。

**Delivers:** enabling: unblocks P1, P5, P20, P24

**Blocked by:** None (can start immediately).

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 按已批准 contract 的 S1/S2/S3 明确固定 Rust 能力标识的字段、两个固定值、目标互斥规则和未知能力拒绝行为；不新增任意代码、解释器或独立业务 exe interface。
- [ ] 明确无 Chat 的 execution、报告来源、执行开始/结束/失败与节点交接状态的序列化表达。保留真实 execution 归属，不要求虚假 Task、agent、session 或 Chat；不改变现有 Task/Map 历史可读要求。
- [ ] 明确创建分支 request、baseline、必要 handoff、saved 的字段、类型、必填项、目标类型、授权和确认范围、来源/字段对应关系、真实 revision 与完整配置表示。纯正文仍为原样 UTF-8 text，handoff 不复制完整正文或配置，不承担隐式寻址。
- [ ] 明确非法格式、缺项、歧义、身份/目标/revision 冲突、写入失败、读回失败及保存后材料失败的可校验反馈和已发生效果表达；不增设自动恢复、回滚或内容相同即成功规则。
- [ ] 为上述公开表示给出独立字面合法/非法实例，供 02/03/10 消费；预期不由实现算法生成。材料格式只收紧创建分支，不能改变修订分支。
- [ ] 仅重整实现这些表示所必需的既有内部目标/身份边界，保持 Task/Map 行为可运行；不建设通用脚本框架、第二份运行权威或仅能保存却不能运行的占位能力。不为后续票预先添加无业务实现的成功路径。
- [ ] 更新现有定义、运行及业务材料文档，明确字段和例子不表示两个能力已经交付。若具体表示导致新的行为或兼容取舍，先修订 contract 并取得批准。

## Verification and ownership

本票只拥有表示细化和必要 prefactor 的回归，不认领 P1/P5/P20/P24 的最终行为测试。02 拥有创建与共享执行通路，03 拥有合并保存业务，10 消费此处的公开表示。

使用本 worktree 的独立绝对 CARGO_TARGET_DIR，先构建本 worktree 静态页，再运行 `node scripts/test-backend.mjs`。按测试指引保持真实文件、进程和网络隔离；不添加源码文本、接线或提示词快照测试冒充行为验证。
