# Map Run 运行说明

## 发起与观察

在 scheduler 选中 Map，打开「+ 发起 Run」，选择 manager 类型的 omp Task 与已登记 zone。Workspace 名称来自管理者 zone 的目录名，不退回默认或当前 Workspace。可先校验配置；「启动 Run」会重新执行同一整图预检，然后创建独立 Run。三份 Bot 文档允许空文本，不是业务质量门禁。

启动前请确认角色 Task 的固定扩展入口已在部署的 my-agent-toolkit 中交付并可加载、omp 未开启 autoResume，且管理者能按绝对路径读取 Map、本说明及运行记录。运行期间保持定义与 Task 配置不变。本服务不增加路径可读性探针，也不修改原生会话目录。缺失扩展入口会明确拒绝启动，不用空扩展代替。

管理者 Task 的固定 prompt 定义职能与限制；初始 Skill、model/thinking 和固定 prompt 按 Task 原配置生效。本次输入只提供 Run ID、Map 路径、本说明及三份记录的路径，最后以独立段落附上非空本次要求的原文。空要求不附段落。Bot 文档、完整 Map 正文与 node.input 不隐式注入管理者。

取得 Run ID 后自动进入 workflow 的基础只读详情，链接形如 `#workflow/<Run ID>`。刷新链接、切换入口后返回、手动刷新只读取事实，不创建 Run 或 Chat。本次要求不可编辑；详情中的 Run/node 状态不是 Chat 在线或轮次状态。此入口没有启动进度页、状态操作按钮或日志浏览器。

## 记录及配置

沿用服务 TOML 的 `[scheduler]`：

```toml
[scheduler]
map_directory = 'D:\service-data\maps'
bot_directory = 'D:\service-data\bots'
run_directory = 'D:\service-data\runs'
run_instructions = 'D:\herdr-bot\docs\workflow-runtime.md'
```

相对目录在服务启动时按服务工作目录解析一次；向 Chat 提供绝对路径，不按 Chat 的 zone/cwd 重新推导。默认 Run 根为 `.data/workflow-runs`。部署时应显式设置稳定的 `run_instructions`，不要依赖编译机器保留源码；默认指向随源码构建的本说明。

每个 Run 目录只有三份业务记录：

- `run.json`：当前已提交事实，包含 Map 引用、时间、管理者 Task/zone/Workspace、本次要求、管理者真实 Chat 关联及节点状态。
- `history.jsonl`：追加执行/状态历史；新 Run 初始为空。
- `messages.jsonl`：追加报告；新 Run 初始为空。

正常顺序为整图预检 → 三份记录全部初始化 → Workspace/Tab/agent 启动 → 获取原生 session_id → 持久保存 Run 的真实关联 → 提交初始指示。单文件更新使用同目录临时文件后替换，不先删除正式文件。不承诺三文件事务、断电恢复、多进程写入协议或外部 Chat 回滚。

## 基础管理者状态协议

`POST /api/workflow/manager/status` 的传输正文仅为 `{"sessionId":"<当前真实原生身份>"}`，返回 `snapshot`，内容来自当前已提交的 `run.json`，不保存第二套 ready 状态。角色扩展必须通过公共 Session 能力自动取得身份；这不是模型业务参数，模型不提供 Run ID、角色或归属。

后端先校验同机来源，再通过已保存的管理者原生身份解析唯一的当前后端 Run 归属；没有关联、重复关联、非管理者均拒绝，不按名称、Task、Tab 或 cwd 猜测。session_id 不是秘密认证凭据：接受同机进程冒用风险，不构成强认证或沙箱。重启后旧 Run 只读，普通 Chat Resume 不代表接管 Run。

manager-chat 需要在独立 toolkit 中通过公共 `PromptDeclaration` 的 `first-enabled-run` 提供工具操作指引，并将无业务参数的 status 工具接到上述协议。后端协议和页面不是该外部扩展已交付或真实注入已验证的证据。

## 启动失败

初始化失败不启动 Chat。原生身份或关联保存失败不提前提交指示。已启动的 Tab/agent 不会被假报回滚；初始指示提交失败也不代表外部端绝无接收效果。详情列出可得的 Workspace、Tab、pane、agent 及原生身份，说明哪些步骤已确认完成。先核实这些引用，再处理现场；不要盲目再次发起，异常窗口不承诺完整幂等。

管理者启动错误只保存在当前后端内存，刷新仍能查询。诊断输出包含 Run、失败阶段及已发生效果，不重复本次要求正文。错误不写入业务历史；后端重启后不重建页面错误，只保留持久事实与既有诊断。没有自动重试、自动关闭 Chat 或自动派发节点。
