import { beforeEach, describe, expect, it, vi } from 'vitest'

import { createSseSessionFeed } from '../src/sseSessionFeed'
import { FakeSessionEventSource, resetFakeEventSources } from './fakeSessionEventSource'

/** 各字段齐全的在线会话视图（wire 形状）。 */
function wireSession(sessionId: string, overrides: Record<string, unknown> = {}) {
  return {
    sessionId,
    sessionName: `会话 ${sessionId}`,
    cwd: 'E:\\Developer\\myhub\\herdr-bot',
    model: 'claude-sonnet-4-5',
    online: true,
    state: 'working',
    ...overrides,
  }
}

/** 一帧 `snapshot` 的载荷：会话快照 + **同一帧**里的权威关机状态（真实后端两半都带）。 */
function snapshot(payload: { sessions: unknown[] }) {
  return { ...payload, shutdown: { phase: 'none', serverTime: 0, monitoredSessionIds: [] } }
}

/** 建立一条订阅并返回新开的假连接（页面的真实路径：订阅即连线）。 */
function subscribed() {
  const feed = createSseSessionFeed({
    eventSourceFactory: (url) => new FakeSessionEventSource(url),
  })
  const unsubscribe = feed.subscribe(() => {})
  const source = FakeSessionEventSource.instances.at(-1)!
  return { feed, source, unsubscribe }
}

beforeEach(() => {
  resetFakeEventSources()
})

describe('SSE 数据源接线', () => {
  it('订阅即建立同源长连接，首帧 snapshot 重置全部会话', () => {
    const { feed, source, unsubscribe } = subscribed()

    expect(source.url).toBe('/api/sessions/events')
    expect(feed.getSnapshot().sessions).toEqual([])

    source.emit('snapshot', snapshot({ sessions: [wireSession('a'), wireSession('b')] }))
    expect(feed.getSnapshot().sessions.map((session) => session.sessionId)).toEqual(['a', 'b'])

    // 重连后的快照是「重置」而不是合并：断线期间注销的会话不能留在列表里。
    source.emit('snapshot', snapshot({ sessions: [wireSession('b')] }))
    expect(feed.getSnapshot().sessions.map((session) => session.sessionId)).toEqual(['b'])

    unsubscribe()
    expect(source.closed).toBe(true)
  })

  it('upsert 就地更新已存在的会话、追加未知的会话，并保持注册顺序', () => {
    const { feed, source } = subscribed()
    source.emit('snapshot', snapshot({ sessions: [wireSession('a'), wireSession('b', { state: 'idle' })] }))

    // 已存在的会话就地在原位更新（服务端按注册顺序下发，增量不该把行挪位）。
    source.emit('upsert', { session: wireSession('a', { state: 'awaiting-input' }) })
    expect(feed.getSnapshot().sessions.map((session) => [session.sessionId, session.state])).toEqual([
      ['a', 'awaiting-input'],
      ['b', 'idle'],
    ])

    // 未知会话追加到末位——新注册的会话正是最后一行。
    source.emit('upsert', { session: wireSession('c', { online: false }) })
    expect(feed.getSnapshot().sessions.map((session) => session.sessionId)).toEqual(['a', 'b', 'c'])
  })

  it('snapshot 与 upsert 只认原生 pi/omp，缺省、未来值和非法类型归一化为未知', () => {
    const { feed, source } = subscribed()
    source.emit('snapshot', snapshot({ sessions: [
      wireSession('pi', { agentType: 'pi' }),
      wireSession('omp', { agentType: 'omp' }),
      wireSession('legacy'),
      wireSession('future', { agentType: 'future-harness' }),
      wireSession('invalid', { agentType: 42 }),
      wireSession('prototype', { agentType: '__proto__' }),
      wireSession('uppercase', { agentType: 'PI' }),
    ] }))
    expect(feed.getSnapshot().sessions.map((session) => session.agentType)).toEqual([
      'pi', 'omp', undefined, undefined, undefined, undefined, undefined,
    ])
    source.emit('upsert', { session: wireSession('pi', { agentType: 'omp' }) })
    expect(feed.getSnapshot().sessions[0].agentType).toBe('omp')
    source.emit('upsert', { session: wireSession('pi', { agentType: 'future-harness' }) })
    expect(feed.getSnapshot().sessions[0]).not.toHaveProperty('agentType')
    expect(feed.getSnapshot().connection).toBe('live')
  })

  it('remove 按 sessionId 只删掉那一行', () => {
    const { feed, source } = subscribed()
    source.emit('snapshot', snapshot({ sessions: [wireSession('a'), wireSession('b'), wireSession('c')] }))

    source.emit('remove', { sessionId: 'b' })
    expect(feed.getSnapshot().sessions.map((session) => session.sessionId)).toEqual(['a', 'c'])

    // 未知 sessionId 的 remove 不动列表（重复注销不该清掉别人的行）。
    source.emit('remove', { sessionId: 'nope' })
    expect(feed.getSnapshot().sessions.map((session) => session.sessionId)).toEqual(['a', 'c'])
  })

  it('视图字段按契约映射：state 缺省或未知即「尚未同步」，online=false 即离线', () => {
    const { feed, source } = subscribed()

    source.emit(
      'snapshot',
      snapshot({
        sessions: [
          // state 缺省：视图里就没有 state（界面据此显示中性占位「状态同步中…」）。
          { sessionId: 'a', cwd: 'E:\\a', online: true },
          // online 缺省按离线处理；非法状态值当作未知，不写进视图。
          { sessionId: 'b', cwd: 'E:\\b', state: 'paused' },
          // 可空字段原样带过（空白会话名由界面回退显示，数据源不做展示层加工）。
          { sessionId: 'c', cwd: 'E:\\c', online: false, sessionName: '  ', model: 'claude-sonnet-4-5', state: 'idle' },
        ],
      }),
    )

    expect(feed.getSnapshot().sessions).toEqual([
      { sessionId: 'a', cwd: 'E:\\a', online: true },
      { sessionId: 'b', cwd: 'E:\\b', online: false },
      {
        sessionId: 'c',
        cwd: 'E:\\c',
        online: false,
        sessionName: '  ',
        model: 'claude-sonnet-4-5',
        state: 'idle',
      },
    ])
  })

  it('坏帧（解析失败或视图缺必需字段）被忽略，不清空也不污染已有视图', () => {
    const { feed, source } = subscribed()
    source.emit('snapshot', snapshot({ sessions: [wireSession('a')] }))
    const before = feed.getSnapshot()

    source.emitRaw('snapshot', '{ 这不是 JSON')
    // 视图缺必需字段：整帧不认（这里补上合法的关机状态，让被拒的原因确实是这一行会话）。
    source.emit('snapshot', snapshot({ sessions: [wireSession('b', { cwd: undefined })] }))
    source.emitRaw('upsert', 'null')
    source.emit('remove', {})

    expect(feed.getSnapshot()).toBe(before)
  })

  it('断线保留最后一份快照并标记过期，重连靠 snapshot 收敛；客户端不自行重连、不排定时器', () => {
    vi.useFakeTimers()
    try {
      const { feed, source } = subscribed()
      source.emit('snapshot', snapshot({ sessions: [wireSession('a')] }))

      source.fail()
      const interrupted = feed.getSnapshot()
      expect(interrupted.connection).toBe('interrupted')
      // 事件不是「清空列表」，而是保留最后一帧并标注可能过期（降级标记由会话列表呈现）。
      expect(interrupted.sessions.map((session) => session.sessionId)).toEqual(['a'])

      // 前端零轮询、零时钟逻辑：不管等多久都不会冒出第二条连接或新的帧，快照引用也不变。
      vi.runAllTimers()
      expect(FakeSessionEventSource.instances).toHaveLength(1)
      expect(source.closed).toBe(false)
      expect(feed.getSnapshot()).toBe(interrupted)

      // 浏览器按服务端给的 retry 自行重连：`open` 只说这条连接又通了，读数仍是断线前那一份，
      // 因此它**不是**恢复证据，连接健康度与展示都不动。
      source.open()
      expect(feed.getSnapshot()).toBe(interrupted)

      // 一份完整的首帧快照才完成收敛。
      source.emit('snapshot', snapshot({ sessions: [wireSession('a'), wireSession('b')] }))
      expect(feed.getSnapshot().connection).toBe('live')
      expect(feed.getSnapshot().sessions.map((session) => session.sessionId)).toEqual(['a', 'b'])
    } finally {
      vi.useRealTimers()
    }
  })

  it('快照引用只在确有一帧改变视图时才更换（React 订阅的前提）', () => {
    const { feed, source } = subscribed()

    const initial = feed.getSnapshot()
    expect(feed.getSnapshot()).toBe(initial)

    source.emit('snapshot', snapshot({ sessions: [wireSession('a')] }))
    const afterFrame = feed.getSnapshot()
    expect(afterFrame).not.toBe(initial)
    expect(feed.getSnapshot()).toBe(afterFrame)
  })
})
