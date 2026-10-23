# 01: 新建 Bot 回归 agent 原生会话存储

**What to build:** 从 zone 新建 Pi／OMP Bot 时，遵循 agent 自身正常的原生会话存储配置，不再覆盖或预分配专属原生会话目录。用户可通过原生发现机制找到这些对话，既有配置和旧会话文件不受本次功能改动影响。

**Delivers:** P23、P24；P25 的原生存储修正与旧文件保护分区。

**Test owner for:** P25 的存储修正前后旧主文件及附属文件保护；本票不单独完成 P25。

**Blocked by:** None (can start immediately).

**Status:** in-progress

- [x] 移除 herdr-bot 对新 Bot 原生会话目录的覆盖、预分配及仅为该目录服务的创建前置条件；不保留旧策略的兼容路径，不引入替代性的强制新会话策略。
- [x] Pi／OMP 均遵循自身有效存储配置；不启用、关闭或修改 autoResume、原生目录或其他 agent 全局配置，不把新建 Herdr Tab 宣称为一定产生新对话的保证。
- [x] 受控 adapter 覆盖启动选择；分别运行隔离的真实 Pi／OMP，独立观察原生文件实际位置、引用及原生发现能力。启动参数回显、帮助文本、此前文件迁移结果均不替代真实落盘和发现证据。
- [x] 使用隔离配置验证非固定默认位置的有效配置确实生效；独立比较运行前后配置，不修改用户真实全局文件或向已有用户对话提交测试输入。
- [x] 隔离环境预放旧原生主文件与附属记录，比较改动流程运行前后的字节和目录清单，确认没有移动、复制、转换、删除或改写。此项为本票独占的 P25 存储修正覆盖；显式 register 与精确引用读取分别归 02、05、06。
- [x] 同步修正此前以独立原生目录规避 autoResume 的运行说明，准确说明行为由 agent 配置决定。
- [x] 准备 P23-H1 两种 agent 的有效配置说明、创建对象定位、实际原生引用、目录／header 证据与原生发现最小核对步骤。H1 只由用户在实际部署确认，不以受控 adapter 或隔离运行标为通过。

## 验证边界

依据已批准 contract 的 S4 验证外部行为。代理完成本票全部实现、自动检查及真实运行证据后再交付；H1 不暂停其余可执行工作。原生文件不是本功能的迁移或清理对象。

## Comments

- 本票实现与负责的验收已完成，待独立集成代理接管；未自行合入集成分支。依项目规则，未合入不能标 `closed`，当前保留 `in-progress`。P23-H1 仅已准备材料，实际部署验收尚未获用户确认。
- P23/P24：`bot_identity::each_agent_keeps_its_native_storage_selection_without_allocating_directories` 首次 red 明确发现旧 `--session-dir`，移除参数、目录状态、预分配前置条件后 green。没有替代性的强制新会话策略。
- P25：首次启动的 `runtime_data.rs` 原有布局切换还会递归删除旧根目录 `bot-sessions`。新增原生主文件／附属记录保护 red 实际失败后，删除该清理；zone／模板切换保持原有行为。默认及随附配置下两次创建、两次真实后端重启比较旧文件字节与目录清单；旧位置被普通文件占用也不再阻断创建。
- 实际命令 `cargo test --test bot_identity --test bot_creation --test bot_creation_detached --test runtime_data`（backend）：最终 49 passed，0 failed。此前运行暴露的并发目录隔离旧断言已按撤销的存储策略迁移为 Herdr 对象独立性断言。`npm ci --ignore-scripts`、`npm run build`（frontend）及 `cargo build --bins`（backend）均成功。
- 实际命令 `npx playwright test templateExtensionLaunch.spec.ts staleSelectionRecovery.spec.ts`（frontend）：29 passed、2 failed。负责的六组合完整启动参数、同进程映射、修复后模型参数检查通过；两项失败是已有 directory-entrypoint 用例期待 500，而当前 `catalog::check_entrypoints` 明确允许可读目录并返回 202。此不属于原生存储票据，不改生产目录支持、不弱化原验收期望。启动前另发现 browser fixture 只支持 toolkit 路径，已改为按扩展稳定 ID 隔离，并补齐真实元数据的官方固定项字面预期，避免依赖用户安装路径。
- 真实隔离命令：`python scripts/test-native-session-storage.py --pi-package D:/nvm4w/nodejs/node_modules/@earendil-works/pi-coding-agent --omp-package D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent`。退出 0：Pi 1.4.0／OMP 18.12.6 各两次真实 CLI 落盘，非固定默认配置生效，原生发现列出精确 ID／引用，设置文件与旧原生主文件／附属目录字节清单前后相同。模型 endpoint 为隔离 loopback 替身，不调用用户模型／旧对话，不冒充真实 Herdr 部署。
- 证据、实际路径／header、有效配置说明与 H1 最小核对见 `docs/native-session-storage-acceptance.md` 及 `docs/evidence/native-session-storage/isolated-real-agents.json`。P25 的 register 与精确引用读取分区仍由其他票负责，不把本票作为 P25 整体通过。
- 集成接管：在原开发分支解决 a33b597 的 `support.ts` 冲突，保留按扩展 ID 隔离入口的实现；同步当前交付元数据的浏览器字面预期，将已支持的目录入口迁移为成功覆盖，重启磁盘清单纳入持久历史。另定位 Windows 自动分配的 6566／6667 被 Fetch 拒绝（`Error: bad port`），浏览器夹具改为探测 49152–65535 安全动态端口，不修改生产配置。
- 本轮全局验收最终结果：backend `cargo test` 为 383 passed、3 ignored、0 failed；frontend `npm test` 为 27 文件、205 passed；frontend `npm run test:browser` 为 59 passed、0 failed。浏览器前置产物 `npm run build` 与 backend `cargo build --bins` 均成功。未重跑复杂脚本、真实 agent smoke 或跨票据独立门禁。修复及反向合并提交在当前开发分支，等待宿主排队合入；依规则尚不标 closed，H1 仍未确认。

