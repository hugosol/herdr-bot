# 06: 为历史 Chat 确定恢复位置并定位目标

**What to build:** Resume 在当前宿主定位已有目标或为新进程选择可靠位置；历史原容器 ID 保持不变，容器、zone 与恢复 cwd 不混用。依据[合同](../contract.md)及[规格](../spec.md)。

**Delivers:** P8, P10, P11, P12, P13, P14, P15, P41

**Test owner for:** 恢复位置的 S1 分支、公开历史／持久格式及已有目标定位分类；08 拥有真实跨端宿主的对应证据。

**Blocked by:** 02 — 选择 TUI 或 App，并核对目标实例

**Repositories:** herdr-bot

**Status:** ready-for-agent

- [ ] 原 ID 作为不透明 originalWorkspaceId 只向当前宿主查询；同端命中、另一端无匹配、人为同值命中单独检查。字段缺失、成功空结果、命令错误、畸形响应分别检查；只有前两类触发该 ID 回退。前后比较公开历史与既有持久格式，不补写或覆盖原 ID。
- [ ] 两个字面 workspace_name 都成为恢复兜底，不能硬编码、凭名称猜 Git 归属或自动创建容器。有效历史 cwd 优先；有独立 checkout/root 的可靠唯一证据才回退主工作树；歧义及缺证据均采用默认容器且不指定 cwd，公开目录降级及 App 限制。
- [ ] 通过公开 Resume 观察目标选择和选定目录；已有目标定位只聚焦关联的 native 目标，错误目标及聚焦失败不被报告为成功，也不承诺 Windows 窗口置前。
