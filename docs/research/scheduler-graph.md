# Scheduler 只读依赖图：成熟交互与 React 组件调研

调研日期：2026-10-26。本文仅支持设计访谈与方案选择，不实施 UI、不安装依赖。除明确标为“适配建议”的内容外，产品/组件能力均来自下列官方文档；未做本项目交互验收。

## 结论与当前边界

- **推荐借鉴四个参考：React Flow（画布交互）、Airflow（结构与详情）、Dagre（简洁分层）、ELK/elkjs（复杂布局）。** 后两者是布局引擎，不是完整 UI 产品。
- 当前是静态 Map 定义浏览器：箭头只表示 `blocked_by` 前置依赖，不是数据流，也不是正在运行的任务链。建议图例写成“前置节点 → 依赖它的节点”。节点颜色只能表达类型、选中、关系等已有语义，不能凭空表示成功、失败、运行中。
- 据主代理本轮源码调查，`frontend/src/MapGraph.tsx` 当前是自绘 SVG + `foreignObject`、Kahn 分层列布局及单击选中，没有实际 zoom/pan/fit；依赖仅 React/React DOM。仓库 `desktop-graph.png` 上的画布按钮不能证明当前源码具有这些能力。
- **不要把“采用成熟交互”直接等同于“引入 React Flow”，也不要把“引入 React Flow”直接等同于“自动布局”。** React Flow 官方明确尚未实现自有布局方案，节点布局及边路由需要另选算法。[R4]

## 四个参考：事实、适配与代价

### 1. React Flow：视口与可访问交互

**来源事实**

- 默认交互类似地图：指针拖动平移、滚轮或 pinch 缩放；也提供设计工具式配置：滚动平移、修饰键 + 滚动缩放等。[R1]
- `Controls` 包含放大、缩小、Fit View、交互锁定；Fit View 默认调整视口以显示所有节点。`showInteractive` 可隐藏锁定按钮。[R2]
- 默认节点/边支持 Tab 聚焦、Enter/Space 选择、Escape 清除；聚焦节点时自动平移使其可见。可通过 `ariaLabelConfig` 本地化提示。方向键移动节点的前提包括节点可拖动。[R3]
- API 提供 `onNodeClick`、`onSelectionChange`、`nodesDraggable`、`nodesConnectable`、`edgesReconnectable`、`deleteKeyCode`、`zoomOnPinch`、`panOnDrag`、`preventScrolling` 等。`zoomOnPinch` 默认为 true；`preventScrolling` 默认 true，意味着嵌入页面时要有意识处理页面滚动冲突。[R5]

**适配建议（不是官方默认成品）**

- 保留“− / 缩放百分比 / + / 适应全部”工具组；图初次显示或切换 Map 时 fit，用户选节点/打开详情后不要每次强制重新 fit。
- 只读不等于不可交互：保留选中、详情、平移、缩放；禁用拖动节点、连线、重连和删除快捷键。若采用 React Flow，可从 `nodesDraggable={false}`、`nodesConnectable={false}`、`edgesReconnectable={false}`、`deleteKeyCode={null}` 开始；同时检查自定义节点是否覆盖了全局设置。[R5]
- 隐藏 `Controls` 的交互锁切换，避免点击后重新打开编辑能力。无新增节点按钮、连接端口操作提示、运行按钮或动画数据流。
- 默认 ARIA 文案可能包含移动/删除说明，必须改成真实只读行为的中文提示；不要通过关闭全部键盘辅助来达成只读。节点、关系和详情应有可读标签。[R3]

**代价**：需迁移节点/边数据与样式、维持详情选择状态、调整容器尺寸、处理滚动与焦点；库提供基础设施而不替业务决定详情面板、上游语义和布局。官方可访问能力不是本项目已达到 WCAG 的证明。[R3][R5]

### 2. Airflow：结构图与节点详情，而不是执行控制台

**来源事实**

Airflow 3.3.2 的 Graph View 展示任务依赖逻辑结构，节点表示任务、边表示依赖；单击任务可查看元数据及最近运行历史。图还可选择运行实例并显示实例状态。官方页面分别展示未选择运行实例的结构图，以及运行相关视图。[A1]

**适配建议**

- 借鉴“图回答依赖关系，详情回答节点是什么”的分工。点击/键盘选择节点后，在桌面右侧详情区展示现有定义字段、直接前置列表、直接后继列表；点击列表项可定位并选中对应节点。
- 详情侧栏是本项目的适配方案，**不声称上述官方页面证明 Airflow 的 Graph View 使用同样侧栏布局**。小屏可改成底部详情或独立详情页，保留返回图时的选中与视口。
- 节点卡片只放名称、类型等短信息；详细描述放详情，避免所有节点都膨胀。选中使用明确边框与文字，不以“成功绿/失败红”作装饰。
- 不照搬运行选择器、日志、历史、重试、清除、触发、暂停/恢复、耗时或状态热图；这些属于 Airflow 的执行观测能力，不属于当前 Map 浏览器。[A1]

**代价**：需定义节点选择与详情关闭的焦点规则，并决定长名称、空字段及长依赖列表的呈现；不需要为仿照截图引入运行模型。

### 3. Dagre：适合平面有向图的自动分层候选

**来源事实**

Dagre 官方将自身定义为客户端有向图布局库，强调速度与渲染无关；需要节点宽高，计算后输出节点中心坐标及边路径点。支持 `rankdir` 的 TB/BT/LR/RL 和 `nodesep`、`ranksep` 等间距配置；自身不渲染图。[D1]

React Flow 官方将 Dagre 归为较简单的布局选项，但提示子图中节点与图外节点连接时存在布局限制。其集成比较表也不把 Dagre 当作通用边路由方案；因此“有边路径点”不等于任意场景下可避障路由。[R4]

**适配建议**

- 若现有列布局的问题主要是交叉线多、汇聚/分叉不易读、节点间距粗糙，可先比较“保留 SVG，只替换布局”为 Dagre，而不是同时更换渲染与布局。
- 优先稳定的左到右依赖方向；支持上下布局可作为访谈选项，不默认添加切换器。应将所有定义节点输入布局，而非从边端点反推节点集，以保留孤立节点。
- 节点尺寸必须与实际卡片一致；Dagre 中心坐标和 SVG/组件左上角坐标的转换需明确，不能直接互换。[D1]

**代价**：新增布局依赖与数据转换；仍要自行提供视口、键盘、触摸、选中与详情。未来若出现跨层级复合子图，不应默认认为它继续适用。[R4]

### 4. ELK / elkjs：复杂拓扑与边路由的升级候选

**来源事实**

- elkjs 官方说明它只计算布局位置，不提供图渲染或样式；支持异步 `layout()` 和 Web Worker，Worker 是可选部署方式，而非“返回 Promise 就自动离开主线程”。[E1]
- ELK Layered 通过分层强调边方向并减少交叉，支持直线、正交、样条路由、ports 及启用对应选项后的跨层级复合图。[E2]
- `separateConnectedComponents` 指定是否分别处理各连通分量；ELK Layered 的该选项默认 true，并提供分量间距配置。[E2][E3]

**适配建议**

- 只有当真实 Map 出现复杂汇合、密集交叉、异尺寸节点、端口约束或分组跨层级关系，而简单布局持续无法读清时，再评估 ELK。
- 可让孤立节点作为单节点分量参与布局，但是否单独放在“无依赖节点”区域属于本产品决策，布局引擎不会自动提供这个业务区块。
- 不引入移动、抖动的力导向画面：静态定义浏览应让节点位置可预测。按定义/尺寸改变计算布局，选择和高亮不应触发布局重排。

**代价**：配置和调试成本高于 Dagre；需处理异步结果、尺寸测量、渲染坐标及路由映射。若用 Worker，还需处理构建产物路径与部署。不能仅凭“算法更强”就承诺更流畅或更好读，本文没有本项目性能基准。[E1][R4]

## 交互清单：借鉴能力不等于照搬编辑器

以下均为适配建议；事实依据列对应官方能力，不表示已实现。

| 场景 | 建议交互 | 依据与边界 |
| --- | --- | --- |
| Zoom / pan / fit | 桌面空白拖动平移；提供始终可见的 +/−/适应全部；明确滚轮行为，避免画布无提示吞掉页面滚动 | React Flow 的两种 viewport 模式及交互 props 可作参考。[R1][R2][R5] |
| 选择与详情 | 单击/Enter/Space 均到达同一选中详情；Escape 清除选中或关闭最上层详情；关闭详情后焦点回原节点 | React Flow 提供选择基础；Airflow 提供点击任务看元数据的产品参考；具体侧栏及焦点管理由项目实现。[R3][A1] |
| 上下游高亮 | 选中节点强调其直接前置/后继与连线，其他元素降低强调但不消失；详情显示“前置依赖”“依赖此节点”，不只写含混的上下游 | `getIncomers` / `getOutgoers` 可取得一跳邻居；自动高亮与详情不是这些 util 自带的完整功能。[R6][R7] |
| 传递关系 | 若提供“全部前置/全部后继”，明确区别于一跳关系；需自己遍历并用 visited 去重。高亮是结构关系，不是正在传输数据 | 官方 util 描述的是直接相连节点；不应把一次调用当作传递闭包。[R6][R7] |
| 孤立节点 | 在适应全部和节点总数中保留；标记“无前置依赖 / 无后继”而不是“未运行/异常”；不能为好看伪造连接到根节点 | ELK 有连通分量处理；呈现语义为本项目建议。[E2][E3] |
| 移动端 | 图区域内拖动平移、双指 pinch 缩放；单击选择，不能依赖 hover；详情避免压缩图至不可用；+ / − / fit 是手势替代入口 | pinch 与 touch pan 有官方 API 支持；移动布局及页面滚动策略仍需真实设备验证。[R1][R5] |
| 键盘与读屏 | Tab 可到节点与工具按钮；可见焦点；读出名称、类型和关系；提供等价的依赖文本列表；自定义提示删除所有编辑指令 | React Flow 支持焦点、选择和 ARIA；文本列表及产品焦点流需项目实现，不默认假设方向键会沿依赖边导航。[R3] |
| 防止伪装编辑器 | 不允许节点位置修改、连线/重连、删除；不出现“运行”“重试”、运行状态色和运动粒子 | 可用 React Flow 交互 props 限制，但只读约束必须覆盖自定义节点与全部入口。[R5] |

## 保留现有实现 vs 引入库：选择边界

| 路径 | 适用边界（设计判断，非规模基准） | 获得 / 仍需负责 |
| --- | --- | --- |
| 保留 SVG + 现有分层 | 实际定义图较简单，主要诉求是样式、选中与依赖详情；团队愿意维护少量画布交互 | 无新增依赖；仍须自己实现 zoom/pan/fit、手势冲突、焦点与 ARIA。当前不能把这些当作已有功能 |
| 保留 SVG + Dagre | 问题主要在排列与交叉，而非画布交互；平面图布局足够 | 只替布局；保留已有节点展示。不会顺便获得 Controls、详情或可访问导航。[D1] |
| React Flow + 现有布局 | 画布交互、焦点、节点定制的长期维护已超过自绘收益，但现有排列可读 | 获得视口/Controls/选择基础；位置仍由现有算法提供；必须关闭编辑入口并迁移样式。[R2][R4][R5] |
| React Flow + Dagre | 同时需要成熟视口和更好的常规分层排列 | 两个独立职责/依赖，不是一键 DAG 查看器。[R4][D1] |
| SVG 或 React Flow + ELK | 真实图的复杂路由、复合分组或约束足以证明引入成本合理 | 复杂自动布局；渲染、详情、交互与只读语义仍属项目责任。[E1][E2] |

**本轮推荐**：先确定只读交互契约与真实拓扑痛点，不默认新增依赖。若要做下一轮方案验证，使用项目已有 Map 中的链、分叉、汇聚和孤立节点案例比较现有布局与 Dagre；只有复杂约束真实存在再评估 ELK。是否实施以及采用何库仍待设计讨论，不在此次调研中决定。

## 已有观察与证据限制

- 主代理本轮浏览器观察了 [React Flow 官方 Overview 示例](https://reactflow.dev/examples/overview) 的[嵌入示例](https://example-apps.xyflow.com/react/examples/misc/overview/index.html)：点击 Zoom In 后 viewport scale 从约 0.677616 变为 0.813139；随后点击 Fit View，截图可见左下角工具组与右下角 MiniMap。这是官方示例观察，不是本项目功能验证，也不是移动端/键盘测试。
- 本调研读取官方文本，未验证所有官方 demo 行为，未运行本项目测试、build、lint 或格式化。没有据此声称任何路线达到某节点规模的性能目标。
- 尚待设计访谈决定：图是否主要用来追溯一个节点的依赖，还是总览整个 Map；直接关系是否够用；小屏优先图还是依赖列表。这些选择影响布局与信息密度，但不改变只读边界。

## 一手来源

以下页面均已在本轮读取；Airflow stable 当次显示 3.3.2，后续 stable 可能变化。

- [R1] React Flow — [Panning and Zooming](https://reactflow.dev/learn/concepts/the-viewport)
- [R2] React Flow — [Controls](https://reactflow.dev/api-reference/components/controls)
- [R3] React Flow — [Accessibility](https://reactflow.dev/learn/advanced-use/accessibility)
- [R4] React Flow — [Layouting overview](https://reactflow.dev/learn/layouting/layouting)
- [R5] React Flow — [ReactFlow component API](https://reactflow.dev/api-reference/react-flow)
- [R6] React Flow — [getIncomers](https://reactflow.dev/api-reference/utils/get-incomers)
- [R7] React Flow — [getOutgoers](https://reactflow.dev/api-reference/utils/get-outgoers)
- [A1] Apache Airflow — [UI Overview / Graph View](https://airflow.apache.org/docs/apache-airflow/stable/ui.html#graph-view)
- [D1] Dagre 官方 — [Wiki：Design Priorities、Rendering、Layout configuration](https://github.com/dagrejs/dagre/wiki)
- [E1] elkjs 官方 — [README：职责、异步 API 与 Worker](https://github.com/kieler/elkjs)
- [E2] Eclipse ELK — [ELK Layered](https://eclipse.dev/elk/reference/algorithms/org-eclipse-elk-layered.html)
- [E3] Eclipse ELK — [Separate Connected Components](https://eclipse.dev/elk/reference/options/org-eclipse-elk-separateConnectedComponents.html)
