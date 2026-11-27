# Retro — 05-inputs-and-call-context.md

## Provenance

```json
{
  "feature": "bot-map-contract-refactor",
  "ticket": "05-inputs-and-call-context.md",
  "run_id": "f5184a9a-00d8-4e6d-a8cb-6f4fba6cdafe",
  "agent": "tdd",
  "agent_id": "05-inputs-and-call-contextmd-01cc2c17-95eb-4a26-",
  "session_id": "01a21bb6-bf68-70ed-82ec-7982cdc45bef",
  "session_file": "C:\\Users\\hugos\\.omp\\agent\\sessions\\--E--Developer-myhub-herdr-bot--\\2026-11-26T21-28-40-789Z_01a219c2-b655-73fe-b79a-9d1f4fbbf05e\\05-inputs-and-call-contextmd-01cc2c17-95eb-4a26-.jsonl",
  "parent_session_id": "01a219c2-b655-73fe-b79a-9d1f4fbbf05e",
  "parent_session_file": "C:\\Users\\hugos\\.omp\\agent\\sessions\\--E--Developer-myhub-herdr-bot--\\2026-11-26T21-28-40-789Z_01a219c2-b655-73fe-b79a-9d1f4fbbf05e.jsonl",
  "workspace": "E:/Developer/myhub/herdr-bot",
  "execution_cwd": "C:\\Users\\hugos\\AppData\\Local\\Temp\\hyper-ticket-ehiDbC",
  "tdd_status": "completed",
  "started_at": "2026-11-27T06:34:51.017Z",
  "tdd_finished_at": "2026-11-27T06:50:30.005Z",
  "retro_finished_at": "2026-11-27T06:51:02.913Z"
}
```

## Retrospective

## 票据 05：TDD 执行复盘

### 高｜包管理：统一 Windows 上的冻结安装方式

**事实：** `bun install --frozen-lockfile` 报告安装成功，但 Node 运行 Vite/PostCSS 时从 Bun 链接缓存加载依赖，找不到 `picocolors`。浏览器用例尚未启动。改用同一锁文件执行 `bun install --frozen-lockfile --force --linker=hoisted --backend=copyfile` 后，原浏览器构建与所选 13 个用例通过。仓库已有 `test:browser:build`，覆盖前端编译、生产产物及 Playwright；缺口是安装与构建使用的解析方式不一致，而不是缺少浏览器检查。

**建议：** 在 Windows 的独立 checkout 中验证冻结锁文件的 hoisted/copyfile 安装，再把可复现的方式集中到前端既有安装入口。由现有构建检查消费这份安装结果，不新增一套 lint 或在代理指令里复制命令。**验收：** 干净 checkout 按唯一入口安装后，既有构建可在 Node 下解析 PostCSS 依赖，锁文件不变，并能进入所选 Playwright 用例。**未验证：** CI 是否使用相同链接器、其他 checkout 是否普遍受影响，以及两个安装参数中哪个是必要条件。

### 中｜定位：把测试指引的双重编号和入口分开

**事实：** 任务要求定位 `p42` 恢复用例，但本功能的 P42 是迁移提示词；会话里读取的 `docs/agents/testing.md` 则把 P42 用作既有 Run Chat 恢复编号。首读返回 957 行文件中的前 300 行，混入多段与本票无关的真实安装、音频和交接专项；随后又用大纲、搜索及局部读取定位实际测试名。最终确实跑到了 `p42` 和 `app_delegated_map_binds_manager_before_approved_input`，不是恢复覆盖缺失。

**建议：** 修改 `AGENTS.md` 现有 Testing 指针，按「产品定向回归／真实宿主专项」分流。在测试指引开头用测试目标和选择器索引产品路径；保留原选择器，标明 `p42` 是历史用例名，不把当前功能的 promise 编号作为查找键。真实宿主安装和取证细节留在专项引用下。**验收：** 下一张 workflow 票能从入口直接定位初始提交、输入准备、角色恢复和页面发起四组命令，无需先载入原生安装说明。

### 中｜红灯：先证明移除门禁后真的会发起业务

**事实：** `mapInput` 的首个红灯直接显示公开快照缺字段；chief 的首个红灯直接显示缺材料时预览被错误地返回 409。这两处都指向预期的违约。必需文件消失的派发红灯仅显示 `failed` 而不是 `rejected`；当时用例尚未把宿主切到有效的新节点身份，后续才把该配置移到拒绝断言前。不能用原红灯证明无门禁的路径原本可成功发起。最终用例核对了拒绝、持久字节不变、无新宿主调用，以及恢复该文件后成功派发。

**建议：** 在 TDD 技能现有「Design before red」里加入一个针对**拒绝和零副作用断言**的反事实：先把身份、路径和外部回执配置到正常成功，只破坏目标门禁所检查的一项；若红灯落在另一阶段，修正夹具再继续。测试从同一公开入口读取持久结果和外部调用，避免只凭状态文字判定。**未验证：** 用最终夹具移除门禁后会得到怎样的精确响应；本轮没有做该故障注入。

### 低｜精确选择：验证记录应列实际命中的用例

**事实：** 一次后端命令指定了多个 target，但筛选器只命中其中一个，另外两个打印 `running 0 tests`；一次浏览器补测的筛选器没有命中列出的嵌套用例，后续用准确标题重新执行了该用例。最终报告区分了各轮通过数，但工具的聚合成功不足以让读者判断每项是否执行。

**建议：** 在现有验收记录约定中要求每个必验场景提供至少一个实际命中的用例名，允许多个 promise 引用同一份证据。使用选择器时，以已运行的用例清单为准；零匹配标为未执行，不依赖总退出码。若现有定向 runner 可获知预期的选择器，则可让零匹配直接失败，无需新建 runner。**验收：** 缺少一个必验筛选项时，即使同一命令中的其他测试全绿，记录也不会把它列为通过。

本复盘仅依据票据 05 的实现对话及工具结果。受控宿主和浏览器结果不证明真实扩展加载、模型行为或现场切换；没有重跑验收或修改仓库。
