/**
 * 后端权威的关机状态：观察流与命令回执里的同一个形状（ADR 0005）。
 *
 * 关机安排由后端进程持有并到期执行，前端只展示与提交意图。因此这里没有任何倒计时时长常量：
 * 剩余时间由服务端的**截止时间**与服务端**当前时间**算出，本机时钟偏差只影响「收到帧之后经过了多久」，
 * 不决定后端实际到期执行（P49）。
 */

/** 关机阶段：与服务端下发的封闭取值一一对应。 */
export type ShutdownPhase = 'none' | 'counting-down' | 'executing' | 'cancelled' | 'issued' | 'failed'

/**
 * 后端权威的完整关机状态。
 *
 * `deadline` 与 `serverTime` 都是**服务端时钟的 Unix 毫秒**；`receivedAt` 是收到这一帧时的本机时间，
 * 用来把「服务端剩余时间」换算成本地经过时间。终局没有截止时间，因此 `deadline` 缺省即省略。
 */
export type ShutdownView = {
  phase: ShutdownPhase
  /** 相关安排标识；从未有过安排时省略。 */
  arrangementId?: string
  /** 截止时间（服务端时钟的 Unix 毫秒）；只有倒计时中与执行中可观察。 */
  deadline?: number
  /** 服务端当前时间（服务端时钟的 Unix 毫秒）。 */
  serverTime: number
  /** 当前完成后关机监听集合；没有监听时就是权威的空集合。 */
  monitoredSessionIds: string[]
  /** 收到这一帧时的本机时间（Unix 毫秒），由数据源填入。 */
  receivedAt: number
}

/**
 * 还没有收到任何后端状态时的空视图。
 *
 * 它不冒充「没有安排」之外的任何结论：`serverTime` 与 `receivedAt` 都是 0，因此没有剩余时间可言；
 * 连接建立后的首个快照会立刻取代它。
 */
export const NO_SHUTDOWN: ShutdownView = {
  phase: 'none',
  serverTime: 0,
  monitoredSessionIds: [],
  receivedAt: 0,
}

/** 封闭取值：会话与服务端都用同一组阶段词，未知取值整份丢弃。 */
const PHASES: Record<string, true> = {
  none: true,
  'counting-down': true,
  executing: true,
  cancelled: true,
  issued: true,
  failed: true,
}

/**
 * 把 wire 上的关机状态收进视图。
 *
 * `phase` 与 `serverTime` 缺一不可：缺任一个就整份丢弃（返回 `null`），由调用方决定是保留上一份状态
 * 还是当作没有状态——渲染半份关机状态会让提醒与服务端真实安排不一致。
 */
export function toShutdownView(value: unknown, receivedAt: number): ShutdownView | null {
  if (typeof value !== 'object' || value === null) return null
  const record = value as Record<string, unknown>

  const phase = record.phase
  if (typeof phase !== 'string' || PHASES[phase] !== true) return null
  const serverTime = record.serverTime
  if (typeof serverTime !== 'number' || !Number.isFinite(serverTime)) return null

  const arrangementId = typeof record.arrangementId === 'string' ? record.arrangementId : undefined
  const deadline = typeof record.deadline === 'number' ? record.deadline : undefined
  const monitored = Array.isArray(record.monitoredSessionIds)
    ? record.monitoredSessionIds.filter((id): id is string => typeof id === 'string')
    : []

  return {
    phase: phase as ShutdownPhase,
    serverTime,
    monitoredSessionIds: monitored,
    receivedAt,
    ...(arrangementId === undefined ? {} : { arrangementId }),
    ...(deadline === undefined ? {} : { deadline }),
  }
}

/**
 * 距离截止时间还剩多少毫秒（按服务端时钟）。
 *
 * 本机时钟偏差不参与这个差值：经过的时间只由「收到帧之后过了多久」算出，而截止时间是服务端的取值。
 * 没有截止时间（没有安排，或已经是终局）时返回 `null`。
 */
export function remainingMs(view: ShutdownView, nowMs: number): number | null {
  if (view.deadline === undefined) return null
  // 经过的时间不会为负：显示用的“现在”可能比收到这一帧的时刻更早（例如刚收到帧、还没刷新）。
  const sinceReceipt = Math.max(0, nowMs - view.receivedAt)
  return view.deadline - (view.serverTime + sinceReceipt)
}

/** 还剩多少整秒（向上取整）；已经归零时为 0。 */
export function remainingSeconds(view: ShutdownView, nowMs: number): number {
  const remaining = remainingMs(view, nowMs)
  if (remaining === null) return 0
  return Math.max(0, Math.ceil(remaining / 1000))
}
