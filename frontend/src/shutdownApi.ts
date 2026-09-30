/**
 * 关机接口客户端：页面与接口同源，请求地址直接使用相对路径。
 *
 * 请求面是封闭的（票 04）：只发空 `POST /api/shutdown`，不带查询串、不带请求体；
 * 关机的动作、提权与错误细节都在服务端，客户端不提交任何参数。
 */

import { SHUTDOWN_RESPONSE_TIMEOUT_MS } from './config'

/**
 * 一次关机请求在页面一侧的归宿，三者互斥。
 *
 * - `sent`：服务端接受了这次关机（HTTP 200），关机指令已发出。
 * - `failed`：服务端明确回报关机没做成（HTTP 500 `shutdown-failed`），电脑不会关。
 * - `unconfirmed`：请求没得到响应（连不上、连接被中断、超过等待上限），结果无法确认；
 *   它既不是成功也不是失败，页面不得据此重发请求。
 *
 * 三条出路都是同一次请求的观察结果，任何一条都不会引出第二次请求。
 */
export type ShutdownOutcome = 'sent' | 'failed' | 'unconfirmed'

/**
 * 发出本流程唯一的一次关机请求，并把答复归一成三类归宿。
 *
 * 请求没得到响应不是失败：这里不抛异常，也不重发——超时只是给“没有响应”定性，
 * 中止的是同一次请求。
 */
export async function requestShutdown(): Promise<ShutdownOutcome> {
  const abort = new AbortController()
  const timer = window.setTimeout(() => abort.abort(), SHUTDOWN_RESPONSE_TIMEOUT_MS)

  try {
    const response = await fetch('/api/shutdown', { method: 'POST', signal: abort.signal })
    // 收到回应就说明服务端答复过：200 是“已发出”；其余状态码（含 500 `shutdown-failed`）
    // 都是明确没有发出关机指令。
    return response.ok ? 'sent' : 'failed'
  } catch {
    // 连接失败、连接被中断或因超时被中止：没有响应可用，只能如实说“无法确认”。
    return 'unconfirmed'
  } finally {
    window.clearTimeout(timer)
  }
}
