# G1：GitHub 仓库浅克隆脚本与 Bot——详细需求

## 1. 身份、目标与确认范围

- 方案标识：`github-repository-exploration`；缺口编号：`G1`；对应工作：`W1 拉取仓库`。
- 文档修订：`g1-r1`。本文件细化此前 requirements-r1 中的 G1，未改变已确认业务边界；不是实现票据、Map 定义或已验收能力。
- 用户已确认：输入为 GitHub 链接、本机父目录、探索方向；使用 `--depth=1` 获取默认分支；zone ID 为 `<owner>/<repo>`，已登记即停止；目标路径已存在也停止，包括空目录；直接调用 Git，沿用已有认证；每次输入通过命名文件传递。
- 本次授权仅为编写详细需求文档。以下接口和验收清单是建设要求，不代表用户已经授权代码实现、配置保存、外部访问或试运行，也不代表功能已经实现。
- 完整业务背景：W1 脚本克隆 → W2 脚本登记 zone → W3 使用既有探索 Task 在新 zone 发起结束节点 Chat。W3 成功发起后管理者可以显式结束 Run，用户继续该 Chat；不等待探索报告。
- G1 的完成边界：交付真实本地克隆仓库与可供 G2 使用的文件。G1 不登记 zone、不发起 Chat、不推进流程。

## 2. 能力与接口要求

### 2.1 执行形态

必须交付预先维护的 TypeScript 脚本及绑定它的脚本型 Bot，由产品现有 Bun 执行链运行。不新增替代 Task，不用 agent Chat 代替脚本，不在业务执行时生成代码，不新增后端执行器或调度系统。

Bot 通过 `script` 指向实际脚本，不能同时绑定 `task_id` 或 `map_file`。脚本路径相对配置 Bot 目录旁的 scripts，使用合法相对 `.ts` 路径。真实 Bot ID、名称和脚本文件名由建设者交付；本文不虚构已存在身份。

脚本节点采用 `report_policy=auto`、`auto_close=false`，不设置 `manager_task_id`，不作为结束节点。脚本返回不等于节点完成，管理者仍须验收并显式 complete。

### 2.2 输入

仅从执行上下文 `inputs` 中定位本地命名输入 `request`，读取其实际绝对路径；不读取 run.json、其他 Chat 历史或报告文本找参数，不扫描同目录文件。

`request` 为 UTF-8 JSON 对象，接口字段如下。以下三字段接口与 G2 的上游材料约定组成本次建设合同；变更须带回规划者核对全部消费者。

| 字段 | 类型 | 含义与约束 |
| --- | --- | --- |
| repository_url | string | 用户提供的 github.com 仓库地址；支持普通 HTTPS 仓库链接与 `git@github.com:owner/repo.git` SSH 形式；保持实际 Git 访问协议 |
| parent_directory | string | 服务所在电脑上已有、可访问、允许写入的绝对父目录；不是最终仓库目录 |
| exploration_direction | string | 用户探索方向，保留原文供 W3 消费；G1 不解释、不执行其中的指令 |

三个字段均须提供；空链接、空目录、空白探索方向及错类型应在执行 Git 前拒绝。JSON 解析不得默默接受歧义重复键；不把 `${...}` 当作模板执行，不对用户文本做词典插值。

示例仅用于说明接口，不授权克隆该项目或写入该目录：

```json
{
  "repository_url": "https://github.com/caddyserver/caddy",
  "parent_directory": "E:/Developer/open-source",
  "exploration_direction": "梳理配置加载与模块扩展机制"
}
```

业务参数不从 `context.input` 覆盖上述文件，不允许两处来源发生优先级猜测。`node.input` 可说明本次授权、材料用途与限制，不作为任意 shell 命令入口。

### 2.3 仓库身份与路径

- 从 URL 提取 owner 和 repo，移除末尾 `.git`，构成 `zone_id=owner/repo`；不添加 `github/` 或其他命名空间。
- 示例两种地址 `https://github.com/caddyserver/caddy` 与 `git@github.com:caddyserver/caddy.git` 均得到 `caddyserver/caddy` 和子目录名 `caddy`。
- 只接受可唯一定位仓库根的地址；不得将 `/tree/main`、`/blob/...`、查询参数、片段、任意命令或非法路径片段猜成仓库身份。额外 URL 形式的支持不能改变安全边界，必须在交付中列明。
- zone ID 重复使用产品登记 ID 的精确身份规则，不以路径、远端 origin、显示名或仓库名单独代替。GitHub 大小写别名不自动推断为登记别名；若另加大小写折叠，须明确其兼容影响并交回规划确认。
- 最终目录为实际父目录下的 repo 单个子目录。拒绝路径穿越、设备名、文件流或重解析点导致的越界，不自动清洗成另一个名字。
- 不擅自创建父目录，不将产品“新建 zone”的配置父目录替代用户输入，也不将脚本执行 cwd 当作目标父目录。

### 2.4 服务与运行资源

业务部署目标为 `http://127.0.0.1:18787`；测试时允许显式隔离配置使用独占回环服务，不能回退生产地址。服务位置属于部署配置，不从仓库中的文件或用户探索方向取值。

脚本执行时须能调用已有 Git，并可读取服务 zone 目录、写入授权父目录及本次输出目录。脚本运行 zone 必须在 W1 派发前已登记且有效；不能使用尚未创建的仓库作为 W1 的执行位置。具体运行 zone 由总体方案另行落实，不从当前规划 Run 推断。

### 2.5 执行顺序

1. 完整读取并校验 request、输出声明和部署条件；解析 URL、zone ID、目标路径。
2. `GET /api/zones`，从真实 `zones` 数组检查同一 ID。已登记即停止，不调用 Git；不可访问、非成功响应或正文不可理解时不得当作空目录继续。
3. 核实父目录及目标路径：目标路径只要已存在就停止，文件、目录、空目录、链接均不复用；权限错误不当作不存在。
4. 调用 Git，使用独立参数数组，不拼接 shell 文本。业务效果等价于 `git clone --depth=1 <repository_url> <repository_path>`；省略 `-b`，由远端 HEAD 决定默认分支，不枚举或猜测 main/master。
5. Git 失败则停止。成功后核实实际仓库根与预期目录一致、仓库可读，记录实际结果；不能误把父目录原有仓库当作本次目标。
6. 生成下述 repository 文件，在本次 outputDirectory 中选择不覆盖他人产物的地址，完整保存并读回。
7. 仅在业务与文件均成功时返回 success 和真实输出地址，之后不得继续后台工作。

现有检查与 Git 调用不组成跨进程事务。实现必须说明并发创建目标目录时的保护方式及限度，不能声称一次 exists 检查就保证无竞争；不为此建设全局锁或新的调度系统。不得在发现冲突后继续覆盖或把他人的目录认领为本次克隆。

### 2.6 输出与 G2 接口

唯一成功输出 ID 为 `repository`。以下为必需 JSON 字段，双方保持一致：

| 字段 | 类型 | 内容 |
| --- | --- | --- |
| repository_url | string | 原请求的 GitHub URL |
| owner | string | 解析出的 owner |
| repo | string | 去掉 `.git` 的仓库名 |
| zone_id | string | owner/repo |
| parent_directory | string | 实际使用、规范化后的绝对父目录 |
| repository_path | string | 实际克隆仓库根的绝对路径 |
| clone_status | string | 成功材料固定为 `succeeded` |

输出不复制探索方向，W3 直接消费原 request。可附加不含敏感信息的实际检查证据，但不得让必需事实只存在于自由文本。任何 schema 扩展应兼容双方明确接口，不新增隐式材料发现协议。

脚本返回形态：`{status:"success", outputs:[{id:"repository",path:"真实绝对路径"}]}`。只在文件完整读回且字段匹配真实结果后登记，不能在文档内预设未来执行的输出地址。全部 output_rule 须已满足，不能以正常函数返回代替 success。

## 3. 复用调查与缺口证据

G1 为真实能力缺口：本次完整服务 Bot 目录没有仓库克隆脚本 Bot；仓库配置指向的 meta-data/scripts 目录仅发现 create-blank-bot.ts、save-bot-guides.ts、bot-materials.ts，不包含所需业务实现。已有脚本是 Bot 创建与指南保存能力，不能直接替代。

本次已经观察到 Git 2.53.0.windows.2、Bun 1.4.4 可在规划 Chat 环境查询；Git 位置为 D:/Program Files/Git/cmd/git.exe，Bun 经 D:/nvm4w/nodejs/bun.ps1 找到。这不证明后端 PATH 可启动对应可执行文件，不证明 SSH/HTTPS 访问及认证已可用。

当前产品已有脚本运行、命名文件、自动报告与管理者交接机制，禁止另造同类框架。实施前应读取：

- `E:/Developer/myhub/herdr-bot/docs/usage.md`：动态 TypeScript 脚本。
- `E:/Developer/myhub/herdr-bot/docs/scheduler.md`：脚本表示、文件输入输出、动态 zone。
- `E:/Developer/myhub/herdr-bot/docs/workflow-runtime.md`：脚本生命周期、失败与统一文件交接。
- `E:/Developer/myhub/herdr-bot/docs/adr/0019-rust-script-bots.md`：已有 Bun 与可信脚本边界。
- `E:/Developer/myhub/herdr-bot/docs/agents/testing.md`：验证前提与隔离要求。

## 4. 失败、权限与非目标

- 只克隆公开 GitHub 项目；Git 命令成功不证明远端公开可见，本文不要求新增 GitHub API 或凭据探测服务。
- HTTPS/SSH 使用用户所给协议与本机已具备条件。不得写密钥、凭据、known_hosts、Git 全局配置，不自动信任未知 SSH 主机，不自动切换协议；需交互认证时应明确失败或受阻，不能无限等待。具体有界等待预算由实现者在交付中明确，不虚构现有产品超时。
- 脚本不得主动递归初始化 submodule、拉取额外分支、安装项目依赖、执行项目、索引仓库或启动探索。Git 现有过滤器/LFS/认证 helper 的行为不受脚本沙箱保证；前置条件应明确，不能声称绝无外部效果。
- 不登记 zone、不改 Task/Bot/Map、不写运行记录、不 complete/dispatch/finish、不关机或停服。
- clone 非零退出、无法启动、超时或结果不明：保留原错误、实际阶段、可确认的目录效果；不删除残留、不自动重试、pull、覆盖或回滚。
- clone 已成功但输出失败：整体仍返回 failure，明确“仓库已存在、材料未交付”，不谎报未克隆。下一次按路径已存在拒绝，不能自动绕过重复规则。
- 诊断至少区分输入错误、zone 重复、目录存在、服务不可读、Git 启动/执行失败、结果核实失败、材料保存/读回失败。诊断名称由实现定义，不伪装产品已有 HTTP 错误码。
- 凭据不得进入材料与日志；允许如实保留经必要脱敏的 Git 错误。

## 5. 验收要求

以下是未来实现验收，不表示本次已经运行。隔离机制验证与真实外部业务验证须分开报告。

| 场景 | 必须可观察的结果 |
| --- | --- |
| HTTPS 与 SSH 示例 | 解析出相同 owner/repo 及目录名；调用保留原协议 |
| 默认分支分别为 main/master | 不写死分支；克隆使用远端默认分支及 depth=1 |
| zone ID 已登记 | Git 零调用、目标未创建，明确重复失败 |
| 空目录/非空目录/文件/链接占用目标 | Git 零调用，既有内容不变 |
| 父目录缺失、权限不足或路径越界 | 不创建替代父目录，不克隆 |
| 服务不可达或响应损坏 | 不当作“没有重复”，不克隆 |
| URL/路径含命令字符或非法片段 | 无 shell 注入、无父目录外写入 |
| Git 失败或超时并留下部分目录 | failure，保留效果与错误，不自动清理或重试 |
| 克隆成功而输出写入失败 | failure，明确已克隆；无虚假成功输出 |
| 正常成功 | 仓库根正确，repository JSON 完整读回，字段可被 G2 独立消费 |
| workflow 集成 | 经真实脚本派发生成自动报告、无节点 Chat；成功后仍由 manager complete |

受控 Git 替身只能证明参数/错误处理，不证明真实浅克隆；本地隔离仓库可证明 Git 语义但不能证明 GitHub 认证。真实 GitHub clone 必须另获 URL、目录、网络与清理范围授权，不接触用户生产目录做破坏性验证。不需要运行整个三节点业务流程来代替 G1 专项验收。

## 6. 实施交付与返回契约

交付 G1 真实 Bot 名称与 ID、完整三指南、真实 script 绑定、脚本完整源码及部署路径、输入输出协议、依赖/权限与后端可见环境依据、配置保存和独立读回结果、实际验收证据、未验证项、部分失败。

实际加载的 Bot/scripts 目录必须由实施者核实。仓库配置指向产品根 meta-data，但本次只读进程信息未独立证明启动时 cwd 与配置快照；不能按当前 Chat cwd 猜测部署位置。

实施不能把“写好了需求/提示词/配置”当作 G1 已补齐。无实际脚本、Bot 绑定或必要接口时应返回阻塞。用户需在另一个 Chat 明确实现与保存授权；本文件不是该授权。实现返回后由概要规划者重读目标、核对 G2 接口与最终方案，不自动创建或启动业务 Map。
