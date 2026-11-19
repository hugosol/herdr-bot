# 01: framework 发布会话级 ready

**What to build:** framework 订阅者能够区分会话的启动输入初始化、扩展初始化和 agent 工作状态；connect-bot 将当前启动的真实原生身份及 ready 上报，陈旧 owner 失去发布资格。依据[合同](../contract.md)的 S2 与[规格](../spec.md)。

**Delivers:** P16, P17, P18, P19

**Test owner for:** P16–P18 的 framework／安装宿主组合，以及 P19 的发布归属与旧 owner 失效；05 拥有真实 TUI/App 生产链路，03 拥有接收边界。

**Blocked by:** None (can start immediately)

**Repositories:** toolkit

**Status:** ready-for-agent

- [ ] 通过公开订阅验证按会话发布 ready，与扩展 startup/initialization、登记、在线与 idle 无关，既有事件语义不变。
- [ ] 真实 OMP 启动 hook 挂起时，无 ready；闸门打开后才通知，不替换 editor/handler、不阻塞 session_start 等待自身解除。
- [ ] 真实 Pi 首启及原生恢复均以输入初始化为证据；后续启动模态作为反例，不能从早期 OSC idle 单独推出 ready 或永久可输入。
- [ ] 生产 connect-bot 上报关联本次启动、真实原生会话和目标；更换／终止 owner 后旧证据不再有效，独立普通登记仍可用。与 03 采用同一上报语义，不新增公开产品 seam。
- [ ] toolkit typecheck、framework、integration 检查及受影响的原生生命周期 dock 通过；记录安装版本与受控依赖，不把本票当作配对验收。
