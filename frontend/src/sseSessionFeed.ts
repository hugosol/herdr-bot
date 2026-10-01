/**
 * 「会话列表」的真实数据源：浏览器端 SSE 长连接（读路径，接缝 S1）。
 *
 * 服务端每次（重）连的第一帧都是全量 `snapshot`，之后只推 `upsert`/`remove` 增量；断线期间错过的事件
 * 不补，重连即收敛。因此这里**没有任何轮询、时钟或退避逻辑**：重连交给浏览器 `EventSource` 自己的
 * 重试（服务端给了 `retry: 2s`），前端只负责把收到的帧映射成视图。
 *
 * 组件不直接接触全局 `EventSource`——它们只消费 [`SessionFeed`] 契约；真实接线在本模块，由
 * `main.tsx` 注入给页面。`EventSource` 的构造因此是可注入的：jsdom 里没有它，接线测试用假实现驱动
 * 同一段帧映射代码。
 */

import type { AgentState, ConnectionStatus, SessionFeed, SessionFeedSnapshot, SessionView } from './sessionFeed'

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

/** 尚未收到任何帧时的快照：无会话、连接正常。 */
const EMPTY_SNAPSHOT: SessionFeedSnapshot = { sessions: [], connection: 'live' }

const STATE_VALUES = new Set<string>(['idle', 'working', 'awaiting-input'])

/** 把 wire 上的 agent 状态收进视图的封闭三值；缺省或未知取值都当作「尚未同步到状态」。 */
function toAgentState(value: unknown): AgentState | undefined {
  return typeof value === 'string' && STATE_VALUES.has(value) ? (value as AgentState) : undefined
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
   * 只改连接健康度：会话列表明明还在手上，断线不该把它丢掉（既不空列表闪烁，也不假装实况）。
   * 状态没变就不发布——同一份数据必须保持同一引用。
   */
  function setConnection(connection: ConnectionStatus) {
    if (snapshot.connection === connection) return
    publish({ sessions: snapshot.sessions, connection })
  }

  function handleFrame(name: string, data: string) {
    const payload = parsePayload(data)
    if (payload === null) return

    if (name === 'snapshot') {
      const raw = payload.sessions
      if (!Array.isArray(raw)) return
      const sessions: SessionView[] = []
      for (const item of raw) {
        const session = toSessionView(item)
        // 整帧不完整就整帧不认：半份快照既会凭空少行、也会把上一份好数据换掉，比「当作没收到」糟。
        if (session === null) return
        sessions.push(session)
      }
      publish({ sessions, connection: 'live' })
      return
    }

    if (name === 'upsert') {
      const session = toSessionView(payload.session)
      if (session === null) return
      const at = snapshot.sessions.findIndex((known) => known.sessionId === session.sessionId)
      // 已存在的行就地在原位更新（增量不该挪位），未知的行追加到末位——新注册的正是最后一行。
      const sessions =
        at === -1
          ? [...snapshot.sessions, session]
          : snapshot.sessions.map((known, index) => (index === at ? session : known))
      publish({ sessions, connection: 'live' })
      return
    }

    if (name === 'remove') {
      const sessionId = payload.sessionId
      if (typeof sessionId !== 'string') return
      const sessions = snapshot.sessions.filter((known) => known.sessionId !== sessionId)
      if (sessions.length === snapshot.sessions.length) return
      publish({ sessions, connection: 'live' })
    }
  }

  function connect() {
    const opened = createEventSource(url)
    source = opened
    opened.addEventListener('snapshot', (event) => handleFrame('snapshot', event.data))
    opened.addEventListener('upsert', (event) => handleFrame('upsert', event.data))
    opened.addEventListener('remove', (event) => handleFrame('remove', event.data))
    // 连接健康度只有两件事：一次错误即降级（浏览器随后自行重连），一次成功即恢复实况。
    // 这里不重连、不退避、不排定时器——重连后的收敛由服务端的首帧快照负责。
    opened.addEventListener('error', () => setConnection('interrupted'))
    opened.addEventListener('open', () => setConnection('live'))
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
