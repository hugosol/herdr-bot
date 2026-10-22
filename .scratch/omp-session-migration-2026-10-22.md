# OMP 原生会话一次性迁移

用户授权：先尝试移动可确认的 OMP 原生会话；不包含产品功能实现或停止运行中的 agent。

## 已执行

- 通过 OMP 前置 title slot 识别 11 个候选；其中 `bc-1923163e47c3` 被运行中 OMP PID 38756 使用，原位保留。
- 其余 10 个主会话按 header cwd 迁入 `C:/Users/hugos/.omp/agent/sessions/` 下正常 cwd bucket；主文件及同名附属目录合计 42 个文件、13073599 字节。
- 目标 bucket：`--E--Developer-bot-playground-test--`、`--E--Developer-myhub-herdr-bot--`、`--E--Developer-myhub-my-agent-toolkit--`。本次 cwd 均为 E:/Developer 下非 home/temp 的路径，使用已核对的 OMP 绝对路径编码规则。
- 复制前重新枚举进程，并为全部源文件持有 Windows 禁止并发写入的读取句柄；目标不存在才写入，不覆盖。
- 源与目标全部文件逐一 SHA-256 相等后才移除源文件；保持文件字节不变。原创建目录未主动清理。
- 逐文件源/目标路径、字节数及 SHA-256 见 `omp-session-migration-2026-10-22.json`。

## 实际验证

使用本机安装的 OMP v18.12.6 原生 `listSessionsReadOnly` 扫描三个目标 bucket，并使用其 `filterSessionsForPicker` 判定：10/10 主会话被发现，10/10 符合选择器展示条件。未运行交互式 `/resume`，未加载或继续执行用户任务。

## 保持不变与边界

- 正在使用的 1 个 OMP 会话及 6 个不能仅由 header 确认类型的主会话原位保留；未停止任何进程。
- 未改 herdr-bot 新 Bot 的启动参数或其他产品代码，所以新创建目前仍会使用自定义 session-dir。
- 未把迁移记录导入将来的历史列表。
- 未修改 Herdr 持久化状态、原生会话内容中的旧绝对路径或 extension 外部缓存。若这些位置引用迁移前路径，仍需针对实际引用单独处理；本次验证仅证明目标文件完整性及 OMP 原生发现能力，不承诺所有旧路径引用自动跟随迁移。
