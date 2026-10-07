/**
 * 关机命令客户端：页面与接口同源，请求地址直接使用相对路径。
 *
 * 两条命令都不直接关机（ADR 0005）：开始意图只让**后端**建立权威倒计时，取消是全局清空意图。
 * 请求面是封闭的——只发空 `POST`，不带查询串、不带请求体；倒计时时长、到期执行与裁定都在服务端，
 * 客户端不提交任何参数。
 *
 * 三条归宿互不混淆（P47）：
 *
 * - **接受**：服务端接受了这次意图，回执里带它自己的权威关机状态。
 * - **明确拒绝**：服务端给出可机器读取的类别码（例如执行已开始时的 `shutdown-not-cancellable`）。
 * - **无回应**：请求没得到响应（连不上、连接被中断、超过等待上限）——它既不是接受也不是拒绝，
 *   调用者不得据此重发（P50），也不得把它显示成取消成功或关机已发生（P27、P34）。
 */

import { SHUTDOWN_RESPONSE_TIMEOUT_MS } from './config'
import { NO_SHUTDOWN, toShutdownView, type ShutdownView } from './shutdownView'

/** 开始意图的请求路径。 */
export const START_SHUTDOWN_URL = '/api/shutdown/start'

/** 全局取消的请求路径。 */
export const CANCEL_SHUTDOWN_URL = '/api/shutdown/cancel'

/** 完成后关机监听的请求路径：提交明确的目标选中值。 */
export const MONITOR_SHUTDOWN_URL = '/api/shutdown/monitor'

/** 取消被拒绝的类别码：关机执行已经开始。 */
export const NOT_CANCELLABLE = 'shutdown-not-cancellable'

/** 监听被拒绝的类别码：该会话当前不满足新增监听的条件。 */
export const NOT_ELIGIBLE = 'session-not-eligible'

/** 监听被拒绝的类别码：已有活动倒计时。 */
export const COUNTDOWN_ACTIVE = 'shutdown-in-progress'

/** 监听被拒绝的类别码：该会话尚未注册。 */
export const UNKNOWN_SESSION = 'unknown-session'

/** 读不到类别码时的占位：它不冒充任何具体类别。 */
const UNKNOWN_CODE = 'unknown'

/** 一次开始意图的归宿。 */
export type StartResult =
  | { kind: 'accepted'; shutdown: ShutdownView }
  | { kind: 'rejected'; code: string }
  | { kind: 'unconfirmed' }

/** 一次全局取消的归宿。 */
export type CancelResult =
  | { kind: 'cancelled'; shutdown: ShutdownView }
  | { kind: 'refused'; code: string }
  | { kind: 'unconfirmed' }

/** 一次监听命令的归宿：目标选中值是否已经成立。 */
export type MonitoringResult =
  | { kind: 'accepted'; selected: boolean; shutdown: ShutdownView }
  | { kind: 'rejected'; code: string }
  | { kind: 'unconfirmed' }

/**
 * 发出一次开始意图。
 *
 * 回执里的关机状态由服务端给出，因此调用者立刻就有了权威依据；没有回执时只能说「未确认」，
 * 不重发、不推断。
 */
export async function startShutdown(): Promise<StartResult> {
  const response = await send(START_SHUTDOWN_URL)
  if (response === null) return { kind: 'unconfirmed' }

  const body = await bodyOf(response)
  if (!response.ok) {
    return { kind: 'rejected', code: typeof body.code === 'string' ? body.code : UNKNOWN_CODE }
  }
  // 回执形状不符时回退成「没有安排」，不编造一份；观察流随后给出权威状态。
  return { kind: 'accepted', shutdown: toShutdownView(body.shutdown, Date.now()) ?? NO_SHUTDOWN }
}

/**
 * 发出一次全局取消。
 *
 * 只有服务端明确回报 200 才算取消生效（P27）：回执本身就是「取消已生效」的后端证据，并带上它自己的
 * 权威关机状态。执行已经开始时服务端会明确拒绝，那不是取消成功。
 */
export async function cancelShutdown(): Promise<CancelResult> {
  const response = await send(CANCEL_SHUTDOWN_URL)
  if (response === null) return { kind: 'unconfirmed' }

  const body = await bodyOf(response)
  if (response.status === 200) {
    // 回执读不出关机状态时按「取消已生效」补一份：这是服务端刚确认过的结果，不是本地推断。
    const shutdown = toShutdownView(body.shutdown, Date.now()) ?? {
      ...NO_SHUTDOWN,
      phase: 'cancelled',
    }
    return { kind: 'cancelled', shutdown }
  }
  return { kind: 'refused', code: typeof body.code === 'string' ? body.code : UNKNOWN_CODE }
}

/**
 * 设置或撤销某个会话的完成后关机监听。
 *
 * 提交的是**明确的目标选中值**（选中或撤销），不是「翻转」：重复提交同一个目标值不会反向改变集合。
 * 三条归宿与另外两条命令一致——接受（回执带服务端自己的权威集合）、明确拒绝（可机器读取的类别码）、
 * 无回应（未确认，不重发；P50）。
 */
export async function setMonitoring(
  sessionId: string,
  selected: boolean,
): Promise<MonitoringResult> {
  const response = await send(MONITOR_SHUTDOWN_URL, { sessionId, selected })
  if (response === null) return { kind: 'unconfirmed' }

  const body = await bodyOf(response)
  if (!response.ok) {
    return { kind: 'rejected', code: typeof body.code === 'string' ? body.code : UNKNOWN_CODE }
  }
  // 回执里的类别是服务端给出的目标选中值；读不出来时按提交过的目标值显示，绝不凭本地翻转。
  const outcome = body.outcome === 'selected' ? true : body.outcome === 'unselected' ? false : selected
  return {
    kind: 'accepted',
    selected: outcome,
    shutdown: toShutdownView(body.shutdown, Date.now()) ?? NO_SHUTDOWN,
  }
}

/**
 * 发一次空 POST；没有响应（连接失败、被中断、超时）时返回 `null`。
 *
 * 超时只是给「没有响应」定性：中止的是这一次请求，不是重试的起点。
 */
async function send(url: string, body?: unknown): Promise<Response | null> {
  const abort = new AbortController()
  const timer = window.setTimeout(() => abort.abort(), SHUTDOWN_RESPONSE_TIMEOUT_MS)
  try {
    return await fetch(url, {
      method: 'POST',
      signal: abort.signal,
      // 只有携带命令内容的那一条命令才有请求体：目标会话与目标选中值只来自请求体。
      ...(body === undefined
        ? {}
        : { headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) }),
    })
  } catch {
    return null
  } finally {
    window.clearTimeout(timer)
  }
}

/** 响应体的 JSON 对象；读不出来时按空对象处理（类别码与关机状态都按缺省走）。 */
async function bodyOf(response: Response): Promise<Record<string, unknown>> {
  const body: unknown = await response.json().catch(() => null)
  return typeof body === 'object' && body !== null ? (body as Record<string, unknown>) : {}
}
