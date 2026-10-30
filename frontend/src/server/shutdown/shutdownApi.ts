/**
 * 关机命令客户端：页面与接口同源，请求地址直接使用相对路径。
 *
 * 两条命令都不直接关机（ADR 0005）：开始意图只让**后端**建立权威倒计时，取消是全局清空意图。
 * 请求面是封闭的——只发空 `POST`，不带查询串、不带请求体；倒计时时长、到期执行与裁定都在服务端，
 * 客户端不提交任何参数。
 *
 * 三条归宿互不混淆（P47），而且**只有归宿**：
 *
 * - **接受**：服务端接受了这次意图。
 * - **明确拒绝**：服务端给出可机器读取的类别码（例如执行已开始时的 `shutdown-not-cancellable`）。
 * - **无回应**：请求没得到响应（连不上、连接被中断、超过等待上限），或者回执里没有可确认的数据——
 *   它既不是接受也不是拒绝，调用者不得据此重发（P50），也不得把它显示成取消成功或关机已发生
 *   （P27、P34）。
 *
 * 归宿里没有业务状态：关机安排与监听集合只随共享观察流更新，因此这里既不回传、也不构造关机状态
 * （P42、P43）——独立通道的迟到回执无法覆盖较新的观察结果。
 */

import { SHUTDOWN_RESPONSE_TIMEOUT_MS } from './config'

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
  | { kind: 'accepted' }
  | { kind: 'rejected'; code: string }
  | { kind: 'unconfirmed' }

/** 一次全局取消的归宿。 */
export type CancelResult =
  | { kind: 'cancelled' }
  | { kind: 'refused'; code: string }
  | { kind: 'unconfirmed' }

/** 一次监听命令的归宿：服务端是否确认了这次命令。 */
export type MonitoringResult =
  | { kind: 'accepted' }
  | { kind: 'rejected'; code: string }
  | { kind: 'unconfirmed' }

/**
 * 发出一次开始意图。
 *
 * 2xx 就是服务端接受了这次意图——这次操作的归宿是「接受」。回执里的关机状态不参与展示：安排只随
 * 观察流更新。没有回执时只能说「未确认」，不重发、不推断。
 */
export async function startShutdown(): Promise<StartResult> {
  const response = await send(START_SHUTDOWN_URL)
  if (response === null) return { kind: 'unconfirmed' }

  const body = await bodyOf(response)
  if (!response.ok) return { kind: 'rejected', code: codeOf(body) }
  return { kind: 'accepted' }
}

/**
 * 发出一次全局取消。
 *
 * 只有服务端明确回报 200 才算取消生效（P27）：这一次操作的归宿是「已取消」。执行已经开始时服务端会
 * 明确拒绝，那不是取消成功；取消的实际结果（安排是否已经撤销）仍由观察流给出。
 */
export async function cancelShutdown(): Promise<CancelResult> {
  const response = await send(CANCEL_SHUTDOWN_URL)
  if (response === null) return { kind: 'unconfirmed' }

  const body = await bodyOf(response)
  if (response.status !== 200) return { kind: 'refused', code: codeOf(body) }
  return { kind: 'cancelled' }
}

/**
 * 设置或撤销某个会话的完成后关机监听。
 *
 * 提交的是**明确的目标选中值**（选中或撤销），不是「翻转」：重复提交同一个目标值不会反向改变集合。
 * 归宿与另外两条命令一致——接受、明确拒绝（可机器读取的类别码）、未确认（无回应或回执缺少可确认的
 * 类别，不重发；P50）。回执里的关机状态与提交过的目标值都不拿来填补展示：选中态只随观察流更新。
 */
export async function setMonitoring(
  sessionId: string,
  selected: boolean,
): Promise<MonitoringResult> {
  const response = await send(MONITOR_SHUTDOWN_URL, { sessionId, selected })
  if (response === null) return { kind: 'unconfirmed' }

  const body = await bodyOf(response)
  if (!response.ok) return { kind: 'rejected', code: codeOf(body) }
  // 可确认的数据是服务端给出的类别；读不出来时不拿提交过的目标值充数，也不宣称成功。
  if (body.outcome !== 'selected' && body.outcome !== 'unselected') return { kind: 'unconfirmed' }
  return { kind: 'accepted' }
}

/** 回执里的拒绝类别码；读不出来时用不冒充任何具体类别的占位。 */
function codeOf(body: Record<string, unknown>): string {
  return typeof body.code === 'string' ? body.code : UNKNOWN_CODE
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
