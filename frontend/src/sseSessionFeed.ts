/**
 * 「会话列表」与后端关机状态的真实数据源：浏览器端 SSE 长连接（读路径，接缝 S1）。
 *
 * 服务端每次（重）连的第一帧都是全量 `snapshot`（会话 + 关机状态），之后只推 `upsert`/`remove` 会话
 * 增量与 `shutdown` 关机状态事件；断线期间错过的事件不补，重连即收敛。因此这里**没有任何轮询、时钟或
 * 退避逻辑**：重连交给浏览器 `EventSource` 自己的重试（服务端给了 `retry: 2s`），前端只负责把收到的帧
 * 映射成视图。
 *
 * 断线只降级、不丢数据：最后一帧原样保留并在页面上标注可能过期，恢复**只**由一份完整的 `snapshot`
 * 首帧完成（`open` 事件与任何增量都不算恢复证据，见 `handleFrame`）。
 *
 * 组件不直接接触全局 `EventSource`——它们只消费 [`SessionFeed`] 契约；真实接线在本模块，由
 * `main.tsx` 注入给页面。`EventSource` 的构造因此是可注入的：jsdom 里没有它，接线测试用假实现驱动
 * 同一段帧映射代码。
 */

import { NO_SHUTDOWN, toShutdownView, type ShutdownView } from './shutdownView'
import type { AgentState, SessionFeed, SessionFeedSnapshot, SessionView } from './sessionFeed'

/** 一帧命名事件的载荷：SSE 的 `data:` 字段。 */
export type SessionFrameEvent = { data: string }

/**
 * 本模块用到的 `EventSource` 子集：订阅命名事件与关闭。
 *
 * 只声明实际用到的部分，真实 `EventSource` 与测试假实现都能满足它；`readyState`/`onmessage` 用不上——
 * 帧都是命名事件（`snapshot`/`upsert`/`remove`），重连由浏览器自己完成。
 */
export type SessionEventSource = {
  addEventListener(type: string, listener: (event: SessionFrameEvent) => void): void
  close(): void
}

/** 建立一条连接；默认真实 `EventSource`，测试注入假实现。 */
export type SessionEventSourceFactory = (url: string) => SessionEventSource

/**
 * 默认端点：与页面同源。
 *
 * 交付运行时由 Axum 同时提供页面与该接口，开发时由 Vite 的 `/api` 代理转发到本机后端，
 * 所以两种情形下相对路径都指向同一个 omp-bot 实例。
 */
export const SESSION_EVENTS_URL = '/api/sessions/events'

const defaultEventSourceFactory: SessionEventSourceFactory = (url) => new EventSource(url)

/** 尚未收到任何帧时的快照：无会话、连接正常、还没有后端关机状态。 */
const EMPTY_SNAPSHOT: SessionFeedSnapshot = {
  sessions: [],
  connection: 'live',
  shutdown: NO_SHUTDOWN,
}

/** agent 状态的封闭取值：未知取值都当作「尚未同步到状态」。 */
const STATE_VALUES: Record<string, true> = {
  idle: true,
  working: true,
  'awaiting-input': true,
}

/** 把 wire 上的 agent 状态收进视图的封闭三值；缺省或未知取值都当作「尚未同步到状态」。 */
function toAgentState(value: unknown): AgentState | undefined {
  return typeof value === 'string' && STATE_VALUES[value] === true ? (value as AgentState) : undefined
}

/**
 * 把 wire 上的一个会话视图映射成界面用的视图。
 *
 * `sessionId`/`cwd` 是必需字段，缺任一个就整帧丢弃（返回 `null`）——渲染半张卡比保留上一份快照更糟。
 * `state` 缺省即未知（界面显示中性占位），不发明 `unknown` 哨兵；`online` 只认 `true`。
 */
function toSessionView(value: unknown): SessionView | null {
  if (typeof value !== 'object' || value === null) return null
  const record = value as Record<string, unknown>
  const sessionId = record.sessionId
  const cwd = record.cwd
  if (typeof sessionId !== 'string' || sessionId === '' || typeof cwd !== 'string') return null

  const sessionName = typeof record.sessionName === 'string' ? record.sessionName : undefined
  const model = typeof record.model === 'string' ? record.model : undefined
  const state = toAgentState(record.state)

  return {
    sessionId,
    cwd,
    online: record.online === true,
    ...(sessionName === undefined ? {} : { sessionName }),
    ...(model === undefined ? {} : { model }),
    ...(state === undefined ? {} : { state }),
  }
}

/**
 * 把一帧 `snapshot` 的 `sessions` 收成视图；缺任一行读不懂的会返回 `null`（整帧不认）。
 *
 * 一份快照是**整份**视图：它既要能整体替换手上的展示，也要作为「这是当前实况」的证据，
 * 因此不接受半份数据（见 `handleFrame` 的 `snapshot` 分支）。
 */
function toSessionViews(raw: unknown): SessionView[] | null {
  if (!Array.isArray(raw)) return null
  const sessions: SessionView[] = []
  for (const item of raw) {
    const session = toSessionView(item)
    if (session === null) return null
    sessions.push(session)
  }
  return sessions
}

/** 解析一帧的 JSON 载荷；解析不了（或不是对象）即当这一帧没来过。 */
function parsePayload(data: string): Record<string, unknown> | null {
  try {
    const parsed: unknown = JSON.parse(data)
    return typeof parsed === 'object' && parsed !== null ? (parsed as Record<string, unknown>) : null
  } catch {
    return null
  }
}

export type SseSessionFeedOptions = {
  /** 端点；缺省与页面同源（见 [`SESSION_EVENTS_URL`]）。 */
  url?: string
  /** 连接的构造方式；缺省用真实 `EventSource`，测试注入假实现。 */
  eventSourceFactory?: SessionEventSourceFactory
}

/**
 * 建立真实的数据源。
 *
 * 连接在**第一个订阅者出现时**打开、最后一个退订时关闭：页面挂载即连线，卸载即断开。
 * `getSnapshot` 返回缓存住的快照对象（同一份数据必须同一引用，否则 React 会判为无限循环）；
 * 只有帧真正改变视图时才换新对象并通知订阅者。
 */
export function createSseSessionFeed(options: SseSessionFeedOptions = {}): SessionFeed {
  const url = options.url ?? SESSION_EVENTS_URL
  const createEventSource = options.eventSourceFactory ?? defaultEventSourceFactory

  let snapshot: SessionFeedSnapshot = EMPTY_SNAPSHOT
  let source: SessionEventSource | null = null
  const subscribers = new Set<() => void>()

  function publish(next: SessionFeedSnapshot) {
    snapshot = next
    for (const notify of subscribers) notify()
  }

  /**
   * 一次连接错误：会话列表明明还在手上，断线不该把它丢掉（既不空列表闪烁，也不假装实况）。
   * 关机状态同理照旧保留。状态没变就不发布——同一份数据必须保持同一引用。
   *
   * 恢复实况**不**在这里：`open` 只说这条连接又通了，手上的读数仍可能是断线前的旧数据。
   * 唯一的恢复证据是一份完整的 `snapshot` 首帧（见 `handleFrame`）。
   */
  function noteInterrupted() {
    if (snapshot.connection === 'interrupted') return
    publish({ sessions: snapshot.sessions, connection: 'interrupted', shutdown: snapshot.shutdown })
  }

  /** 只改关机状态：会话那一半照旧，因此同一条流上的两类变化互不覆盖。 */
  function publishShutdown(shutdown: ShutdownView) {
    publish({ sessions: snapshot.sessions, connection: snapshot.connection, shutdown })
  }

  function handleFrame(name: string, data: string) {
    const payload = parsePayload(data)
    if (payload === null) return

    if (name === 'snapshot') {
      // 一份**完整**快照是唯一能给出「当前实况」的帧：会话与关机两部分都要读得懂。缺一半就整帧不认——
      // 半份快照既会凭空少行，也会把上一份好数据换成「新会话 + 旧关机状态」这种谁也不是的状态；
      // 它同时是断线后恢复实况的**唯一**证据（P45）。
      const sessions = toSessionViews(payload.sessions)
      if (sessions === null) return
      const shutdown = toShutdownView(payload.shutdown, Date.now())
      if (shutdown === null) return
      publish({ sessions, connection: 'live', shutdown })
      return
    }

    // 断线期间除了上面的完整首帧，任何帧都不改变展示：手上那份快照原样保留并继续标注可能过期。
    // 增量各只覆盖一个域，都不能替代一次完整恢复（P45）。
    if (snapshot.connection === 'interrupted') return

    if (name === 'upsert') {
      const session = toSessionView(payload.session)
      if (session === null) return
      const at = snapshot.sessions.findIndex((known) => known.sessionId === session.sessionId)
      // 已存在的行就地在原位更新（增量不该挪位），未知的行追加到末位——新注册的正是最后一行。
      const sessions =
        at === -1
          ? [...snapshot.sessions, session]
          : snapshot.sessions.map((known, index) => (index === at ? session : known))
      publish({ sessions, connection: snapshot.connection, shutdown: snapshot.shutdown })
      return
    }

    if (name === 'remove') {
      const sessionId = payload.sessionId
      if (typeof sessionId !== 'string') return
      const sessions = snapshot.sessions.filter((known) => known.sessionId !== sessionId)
      if (sessions.length === snapshot.sessions.length) return
      publish({ sessions, connection: snapshot.connection, shutdown: snapshot.shutdown })
      return
    }

    if (name === 'shutdown') {
      // 关机变化携带**完整**关机状态，因此这里只做「收下这份状态」这一件事。
      const shutdown = toShutdownView(payload.shutdown, Date.now())
      if (shutdown === null) return
      publishShutdown(shutdown)
    }
  }

  function connect() {
    const opened = createEventSource(url)
    source = opened
    opened.addEventListener('snapshot', (event) => handleFrame('snapshot', event.data))
    opened.addEventListener('upsert', (event) => handleFrame('upsert', event.data))
    opened.addEventListener('remove', (event) => handleFrame('remove', event.data))
    opened.addEventListener('shutdown', (event) => handleFrame('shutdown', event.data))
    // 连接健康度：一次错误即降级（浏览器随后自行重连），一次完整的 `snapshot` 首帧即恢复实况。
    // 这里不重连、不退避、不排定时器，也不监听 `open`——重连后的收敛由服务端的完整首帧负责。
    opened.addEventListener('error', () => noteInterrupted())
  }

  function disconnect() {
    source?.close()
    source = null
  }

  return {
    subscribe(onChange) {
      subscribers.add(onChange)
      if (subscribers.size === 1) connect()
      return () => {
        subscribers.delete(onChange)
        if (subscribers.size === 0) disconnect()
      }
    },
    getSnapshot: () => snapshot,
  }
}
