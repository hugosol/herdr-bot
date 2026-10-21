# HTML 阅读模式只支持静态内容并沿用 workspace 资源边界

> 术语更新（2026-10-21）：本文的 workspace 现称 zone；原因与硬切换范围见 [ADR 0009](./0009-rename-workspace-to-zone-for-herdr.md)。正文保留原文。

- 状态：accepted
- 日期：2026-10-21

本地 HTML 阅读模式不执行 JavaScript，也不允许表单提交或嵌入其他网页；它支持排版、CSS、内联样式、图片、
页内锚点与原生 `<details>`。内容在父页面净化后放入只允许同源、禁止脚本的 sandbox iframe，并同时带禁止脚本、
嵌入、连接、表单和外部资源自动加载的 CSP。`script`、iframe/object/embed、表单、meta refresh、内联事件处理器
以及 `javascript:`/`file:` 链接会被移除。

workspace 内的相对 CSS、图片与字体仍可加载，但根边界不变：HTML 与其资源通过
`/api/workspaces/reader/<workspace-hex>/<relative-path>` 读取，workspace 路径编码在路径段中，因此 HTML 的相对
URL 能自然解析回同一个 workspace；每次读取仍执行登记核对、路径分量检查与链接拒绝。没有选择直接让 iframe
指向原始 HTML 并开放脚本，因为那会让 workspace 文件获得与 omp-bot 主界面同源的脚本权限；也没有选择切到
workspace 父目录来加载资源，因为那会扩大用户登记的读取边界。

页内锚点保持默认；workspace 内的其他 HTML 在当前阅读模式中打开；`http`/`https` 普通链接在新标签页打开；
workspace 外的本地 `file:///…` 与其他 scheme 被拦截。外部图片、样式、字体等资源不会自动加载。阅读模式不要求
浏览器进入系统级全屏，右上角退出后恢复进入前的 workspace、目录、所选文件与滚动位置。
