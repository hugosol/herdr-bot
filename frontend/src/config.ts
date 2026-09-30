/**
 * 关机的构建期常量。
 *
 * 关机的全部配置都在这里（ADR 0002）：它们是前端构建期常量，改动后需要重新
 * `npm --prefix frontend run build` 才会生效。后端没有对应的 `[shutdown]` 配置段，
 * 也不提供运行时开关——`POST /api/shutdown` 永远注册，下面的开关只控制按钮的显示。
 */

/** 关机的安全窗口：确认后到倒计时归零之间的秒数。 */
export const SHUTDOWN_COUNTDOWN_SECONDS = 10

/** 是否在页面上显示“关机”按钮；只影响显示，不关闭后端端点。 */
export const SHUTDOWN_BUTTON_VISIBLE = true

/**
 * 等待 `POST /api/shutdown` 响应的上限（毫秒）。
 *
 * 超过它说明这次请求没得到响应：页面按“结果无法确认”反馈，既不当成失败，也不会
 * 重发——超时只是给“没有响应”定性，不是重试的起点。
 */
export const SHUTDOWN_RESPONSE_TIMEOUT_MS = 10000
