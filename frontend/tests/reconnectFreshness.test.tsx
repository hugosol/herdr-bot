/**
 * 08 号票的 S2 覆盖：断线风险保持至有效首帧，页面观察不写业务（P44、P45、P46、P47、P49）。
 *
 * 观察接缝仍是批准契约里的 S2——**真实主界面**：这里渲染真 `App`，数据源是真实 SSE 接线
 * （`createSseSessionFeed`，用假 `EventSource` 驱动同一段帧映射代码），关机命令走记录型 `fetch` 替身。
 * 因此用例看到的是「浏览器观察连接断开、重连时页面消费真实帧」的实际结果，而不是私有状态或选择器。
 *
 * 出发点：浏览器断线只是**本页读不到后端**，不是会话离线、也不是安排被取消。恢复实况只有一条路径——
 * 一份完整、读得懂的首帧（会话与关机两部分都在）：`open` 事件与任何增量都不算恢复证据。
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { act } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'
import { createSseSessionFeed } from '../src/sseSessionFeed'
import { FakeSessionEventSource, resetFakeEventSources } from './fakeSessionEventSource'

/** 服务端时钟的一个固定取值：用例不依赖真实时间，也不从实现公式算期望。 */
const SERVER_TIME = 1_700_000_000_000

/** wire 形状的会话视图：缺省是在线、工作中的会话。 */
function wireSession(sessionId: string, overrides: Record<string, unknown> = {}) {
  return {
    sessionId,
    sessionName: `会话 ${sessionId}`,
    cwd: `E:\\work\\${sessionId}`,
    online: true,
    state: 'working',
    ...overrides,
  }
}

/** 一帧 `snapshot` 的载荷：会话快照与**同一帧**里的权威关机状态（真实后端两半都带）。 */
function snapshot(sessions: unknown[], shutdown: unknown) {
  return { sessions, shutdown }
}

/** 没有安排的权威关机状态。 */
function idle(monitoredSessionIds: string[] = []) {
  return { phase: 'none', serverTime: SERVER_TIME, monitoredSessionIds }
}

/** 还差 `seconds` 秒到期的权威安排。 */
function countingDown(seconds: number, monitoredSessionIds: string[] = []) {
  return {
    phase: 'counting-down',
    arrangementId: 'shutdown-8',
    deadline: SERVER_TIME + seconds * 1000,
    serverTime: SERVER_TIME,
    monitoredSessionIds,
  }
}

/** 一个终局的权威关机状态。 */
function terminal(phase: 'cancelled' | 'issued' | 'failed', monitoredSessionIds: string[] = []) {
  return { phase, arrangementId: 'shutdown-8', serverTime: SERVER_TIME, monitoredSessionIds }
}

/** 关机命令的传输替身：记录每次请求的地址，并按用例给定的答复返回。 */
function recordingFetch(reply: (url: string) => Promise<Response> | Response) {
  const calls: string[] = []
  const fetchDouble = vi.fn(async (input: RequestInfo | URL) => {
    const url = String(input)
    calls.push(url)
    return reply(url)
  })
  vi.stubGlobal('fetch', fetchDouble)
  return calls
}

/** 一个正常的 JSON 答复：只提供客户端真正用到的三个成员。 */
function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response
}

/** 让在途的 Promise 链跑完（不依赖真实时钟）。 */
async function flush() {
  await act(async () => {
    for (let index = 0; index < 8; index += 1) await Promise.resolve()
  })
}

/**
 * 打开**一个**页面：真实观察数据源 + 它自己那条假 `EventSource`，并切到「会话列表」tab，
 * 让会话列表与降级标注都处在可见状态。返回该页连接与卸载函数（卸载＝关闭这个页面）。
 */
function mountPage(index = 0) {
  const feed = createSseSessionFeed({
    eventSourceFactory: (url) => new FakeSessionEventSource(url),
  })
  const view = render(
    <div data-testid={`page-${index}`}>
      <App sessionFeed={feed} />
    </div>,
  )
  const scope = within(screen.getByTestId(`page-${index}`))
  fireEvent.click(scope.getByTestId('tab-sessions'))
  return { source: FakeSessionEventSource.instances.at(-1)!, unmount: view.unmount, scope }
}

/** 某个页面范围内的查询入口。 */
function page(index: number) {
  return within(screen.getByTestId(`page-${index}`))
}

/** 当前展示的会话名（按展示次序）。 */
function shownNames(index: number): (string | null)[] {
  return page(index)
    .getAllByTestId('session-name')
    .map((node) => node.textContent)
}

beforeEach(() => {
  resetFakeEventSources()
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

describe('断线后只有一份完整首帧能恢复实况（P45）', () => {
  it('浏览器重连只发生 open、还没有首帧时，过期标注保持、展示不动', () => {
    const calls = recordingFetch(() => jsonResponse(200, {}))
    const { source } = mountPage()
    act(() => {
      source.emit('snapshot', snapshot([wireSession('a')], countingDown(30)))
    })
    expect(page(0).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '30')

    act(() => source.fail())
    expect(page(0).getByTestId('connection-banner')).toBeVisible()
    expect(page(0).getByTestId('shutdown-stale')).toBeVisible()

    // 只是这条连接又通了：手上那份快照仍可能过期，不能凭 open 宣称实况。
    act(() => source.open())
    expect(page(0).getByTestId('connection-banner')).toBeVisible()
    expect(page(0).getByTestId('shutdown-stale')).toBeVisible()
    expect(page(0).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '30')

    // 一份完整首帧到达之后才恢复实况，并且整体替换成首帧里的当前事实。
    act(() => {
      source.emit('snapshot', snapshot([wireSession('a', { state: 'awaiting-input' })], countingDown(30)))
    })
    expect(page(0).queryByTestId('connection-banner')).toBeNull()
    expect(page(0).queryByTestId('shutdown-stale')).toBeNull()
    expect(calls).toEqual([])
  })

  it('无效首帧既不清除过期标注，也不替换展示；有效首帧才恢复', () => {
    const { source } = mountPage()
    act(() => {
      source.emit('snapshot', snapshot([wireSession('a')], countingDown(30)))
    })
    act(() => source.fail())
    act(() => source.open())

    // 解析不了的一帧：当它没来过。
    act(() => source.emitRaw('snapshot', '{ 这不是 JSON'))
    expect(page(0).getByTestId('connection-banner')).toBeVisible()
    expect(page(0).getByTestId('shutdown-stale')).toBeVisible()

    // 会话那一半读得懂、关机那一半没有：缺一半的帧不是「当前实况」，
    // 不能拿上一份关机状态冒充整体实况，也不能只替换一半展示。
    act(() => source.emit('snapshot', { sessions: [wireSession('a'), wireSession('b')] }))
    expect(page(0).getByTestId('connection-banner')).toBeVisible()
    expect(page(0).getAllByTestId('session-card')).toHaveLength(1)
    expect(shownNames(0)).toEqual(['会话 a'])
    expect(page(0).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '30')

    // 关机阶段取值不在封闭集合里：同样不认这一帧。
    act(() => {
      source.emit(
        'snapshot',
        snapshot([wireSession('a'), wireSession('b')], {
          phase: 'paused',
          serverTime: SERVER_TIME,
          monitoredSessionIds: [],
        }),
      )
    })
    expect(page(0).getAllByTestId('session-card')).toHaveLength(1)
    expect(page(0).getByTestId('connection-banner')).toBeVisible()

    // 一份完整首帧才恢复实况，并整体替换成它自己的当前事实。
    act(() => {
      source.emit('snapshot', snapshot([wireSession('b')], idle()))
    })
    expect(page(0).queryByTestId('connection-banner')).toBeNull()
    expect(shownNames(0)).toEqual(['会话 b'])
  })

  it('首帧之前的增量不替代完整恢复证据：展示冻结到一份完整首帧为止', () => {
    const { source } = mountPage()
    act(() => {
      source.emit('snapshot', snapshot([wireSession('a')], countingDown(30)))
    })
    act(() => source.fail())
    act(() => source.open())

    // 断线期间错过的变化以增量到达：它们各只覆盖一个域，都不能冒充「完整恢复」。
    act(() => {
      source.emit('upsert', { session: wireSession('b') })
      source.emit('remove', { sessionId: 'a' })
      source.emit('shutdown', { shutdown: terminal('cancelled') })
    })
    expect(page(0).getByTestId('connection-banner')).toBeVisible()
    expect(page(0).getByTestId('shutdown-stale')).toBeVisible()
    expect(page(0).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '30')
    expect(page(0).queryByTestId('shutdown-result')).toBeNull()
    expect(shownNames(0)).toEqual(['会话 a'])

    // 完整首帧到达：它就是当前事实，旧卡片与旧安排一起退出展示。
    act(() => {
      source.emit('snapshot', snapshot([wireSession('b')], terminal('cancelled')))
    })
    expect(page(0).queryByTestId('connection-banner')).toBeNull()
    expect(shownNames(0)).toEqual(['会话 b'])
    expect(page(0).getByTestId('shutdown-result')).toHaveAttribute('data-outcome', 'cancelled')
  })
})

describe('断线保留最后会话与已有安排（P44）', () => {
  it('浏览器断线不清空卡片、不改写 keepAlive、不把已有安排显示成已取消', () => {
    const calls = recordingFetch(() => jsonResponse(200, {}))
    const { source } = mountPage()
    act(() => {
      source.emit(
        'snapshot',
        snapshot(
          [wireSession('a', { state: 'working' }), wireSession('b', { state: 'awaiting-input' })],
          countingDown(30, ['a']),
        ),
      )
    })
    expect(page(0).getAllByTestId('session-card')).toHaveLength(2)
    expect(page(0).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '30')
    expect(page(0).queryByTestId('connection-banner')).toBeNull()

    act(() => source.fail())

    // 观察断开是「本页读不到后端」，不是无会话：卡片一张不少，等待组照常。
    expect(page(0).getByTestId('connection-banner')).toBeVisible()
    expect(page(0).getByTestId('connection-banner')).toHaveTextContent('可能已经过期')
    expect(page(0).getAllByTestId('session-card')).toHaveLength(2)
    expect(page(0).queryByTestId('sessions-empty')).toBeNull()
    expect(page(0).getByTestId('waiting-group')).toBeVisible()

    // 也不是「会话离线」：keepAlive 与最后上报状态都照旧。
    for (const card of page(0).getAllByTestId('session-card')) {
      expect(card).toHaveAttribute('data-keepalive', 'online')
    }

    // 已有安排照旧：倒计时还在、监听集合照旧，只是明说读数可能过期；没有变成「取消成功」。
    expect(page(0).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '30')
    expect(page(0).getByTestId('shutdown-stale')).toHaveTextContent('可能已经过期')
    expect(page(0).getByTestId('monitoring-summary')).toHaveTextContent('1 个')
    expect(page(0).queryByTestId('shutdown-result')).toBeNull()
    expect(calls).toEqual([])
  })
})

describe('重连首帧替换断线期间陈旧的展示（P46）', () => {
  it('断线期间的登记删除、监听集合变化与安排取消由完整首帧一次性替换', () => {
    const { source } = mountPage()
    act(() => {
      source.emit(
        'snapshot',
        snapshot([wireSession('a'), wireSession('b')], countingDown(30, ['a', 'b'])),
      )
    })
    expect(page(0).getByTestId('monitoring-summary')).toHaveTextContent('2 个')
    expect(page(0).getByTestId('shutdown-countdown')).toBeVisible()

    // 断线期间后端清除了登记 b、把 c 加进监听集合、并撤销了安排——这些变化本页一条都没收到。
    act(() => source.fail())
    act(() => {
      source.emit(
        'snapshot',
        snapshot([wireSession('a', { state: 'awaiting-input' }), wireSession('c')], terminal('cancelled', ['c'])),
      )
    })

    // 首帧自己就是当前事实：旧卡片、旧集合、旧安排一起退出展示。
    expect(page(0).queryByTestId('connection-banner')).toBeNull()
    expect(shownNames(0)).toEqual(['会话 a', '会话 c'])
    expect(page(0).getByTestId('monitoring-summary')).toHaveTextContent('1 个')
    expect(page(0).queryByTestId('shutdown-countdown')).toBeNull()
    expect(page(0).queryByTestId('shutdown-stale')).toBeNull()
    expect(page(0).getByTestId('shutdown-result')).toHaveAttribute('data-outcome', 'cancelled')
  })
})

describe('打开、刷新、关闭与断线都只读（P47）', () => {
  it('这些交互只产生读取／观察，没有会话写入、补偿命令或隐式取消', () => {
    const calls = recordingFetch(() => jsonResponse(200, {}))
    const first = mountPage(0)
    act(() => {
      first.source.emit('snapshot', snapshot([wireSession('a')], countingDown(30, ['a'])))
    })

    // 刷新页面：卸载（关闭那条连接）后重新打开，另起一条观察连接。
    first.unmount()
    const second = mountPage(1)
    act(() => {
      second.source.emit('snapshot', snapshot([wireSession('a')], countingDown(30, ['a'])))
    })

    // 观察连接断线后再重连：仍然只是重新读权威状态。
    act(() => second.source.fail())
    act(() => second.source.open())
    act(() => {
      second.source.emit('snapshot', snapshot([wireSession('a')], idle(['a'])))
    })

    // 关闭页面即关闭长连接。
    second.unmount()

    // 全程没有任何写请求：没有会话写入、没有补偿命令、没有隐式取消，也就不可能刷新 last_seen。
    expect(calls).toEqual([])
    expect(FakeSessionEventSource.instances.map((instance) => instance.url)).toEqual([
      '/api/sessions/events',
      '/api/sessions/events',
    ])
  })
})

describe('页面只提交用户明确的关机意图（P49）', () => {
  it('在线、离线到清除都由后端观察驱动；前端时间流逝不删除记录，也不自己发写请求', async () => {
    vi.useFakeTimers()
    const calls = recordingFetch(() => jsonResponse(200, {}))

    const { source } = mountPage()
    act(() => {
      source.emit('snapshot', snapshot([wireSession('a')], idle()))
    })
    expect(shownNames(0)).toEqual(['会话 a'])

    // 时间流逝：前端既不清除登记，也不发任何请求——登记生命周期只由后端维护决定。
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10 * 60_000)
    })
    expect(shownNames(0)).toEqual(['会话 a'])
    expect(calls).toEqual([])

    // 后端观察给出离线：卡片保留，只是标注为离线与最后上报值。
    act(() => source.emit('upsert', { session: wireSession('a', { online: false }) }))
    expect(page(0).getByTestId('session-card')).toHaveAttribute('data-keepalive', 'offline')
    expect(page(0).getByTestId('last-reported-tag')).toBeVisible()
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10 * 60_000)
    })
    expect(shownNames(0)).toEqual(['会话 a'])
    expect(calls).toEqual([])

    // 清除同样只在后端观察到达时发生。
    act(() => source.emit('remove', { sessionId: 'a' }))
    expect(page(0).getByTestId('sessions-empty')).toBeVisible()
    expect(calls).toEqual([])

    // 唯一的业务写意图是用户明确提交的关机操作（手动入口只在控制面板上）。
    fireEvent.click(page(0).getByTestId('tab-control'))
    fireEvent.click(page(0).getByTestId('shutdown-button'))
    fireEvent.click(page(0).getByTestId('shutdown-confirm-button'))
    await flush()
    expect(calls).toEqual(['/api/shutdown/start'])
  })
})
