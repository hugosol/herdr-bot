import { fireEvent, render, screen, within } from '@testing-library/react'
import { act } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'
import { createSseSessionFeed } from '../src/sseSessionFeed'
import { FakeSessionEventSource, resetFakeEventSources } from './fakeSessionEventSource'

/**
 * 04 号票的 S1 覆盖：跨前端恢复与传输故障（P24、P27、P28、P29、P34、P46、P47、P49、P50）。
 *
 * 观察接缝仍是批准契约里的 S1——**真实主界面**：这里渲染真 `App`，数据源是真实 SSE 接线
 * （`createSseSessionFeed`，用假 `EventSource` 驱动同一段帧映射代码），命令走记录型 `fetch` 替身。
 * 因此用例看到的是「页面消费后端帧」的实际结果，而不是私有状态、选择器或请求 adapter 的内部。
 *
 * 出发点：关机安排由**后端进程**持有（ADR 0005）。多个前端／刷新／重连都只是重新读同一份权威状态；
 * 传输故障只让某次命令变成「未确认」，既不冒充拒绝，也不触发重发或补偿命令。
 */

/** 服务端时钟的一个固定取值：用例不依赖真实时间，也不从实现公式算期望。 */
const SERVER_TIME = 1_700_000_000_000

/** wire 形状的会话视图。 */
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

/** wire 形状的关机状态：还差 `seconds` 秒到期（`deadline` 与服务端时钟对齐）。 */
function countingDown(seconds: number) {
  return {
    phase: 'counting-down',
    arrangementId: 'shutdown-7',
    deadline: SERVER_TIME + seconds * 1000,
    serverTime: SERVER_TIME,
    monitoredSessionIds: [] as string[],
  }
}

/** 没有安排的权威状态。 */
function idle() {
  return { phase: 'none', serverTime: SERVER_TIME, monitoredSessionIds: [] as string[] }
}

/** 一个终局的权威状态。 */
function terminal(phase: 'cancelled' | 'issued' | 'failed') {
  return {
    phase,
    arrangementId: 'shutdown-7',
    serverTime: SERVER_TIME,
    monitoredSessionIds: [] as string[],
  }
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
 * 挂载**一个**前端页面：每个页面各有自己的 SSE 连接，就像真实浏览器里各开一个标签页。
 * 返回它那条连接的假实现与卸载函数（卸载＝关闭这个页面）。
 */
function mountPage(index: number) {
  const feed = createSseSessionFeed({
    eventSourceFactory: (url) => new FakeSessionEventSource(url),
  })
  const view = render(
    <div data-testid={`page-${index}`}>
      <App sessionFeed={feed} />
    </div>,
  )
  return { source: FakeSessionEventSource.instances.at(-1)!, unmount: view.unmount }
}

/** 某个页面范围内的查询入口。 */
function page(index: number) {
  return within(screen.getByTestId(`page-${index}`))
}

beforeEach(() => {
  resetFakeEventSources()
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

describe('跨前端恢复：打开、刷新与重连看到同一安排（P24、P29）', () => {
  it('两个前端、刷新与重连都看到当前安排与实际剩余时间，且不发送任何命令', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date(SERVER_TIME))
    const calls = recordingFetch(() => jsonResponse(200, {}))

    const first = mountPage(0)
    const second = mountPage(1)
    act(() => {
      first.source.emit('snapshot', { sessions: [], shutdown: countingDown(30) })
      second.source.emit('snapshot', { sessions: [], shutdown: countingDown(30) })
    })

    // 两个前端各自从**同一份**权威状态显示同一剩余时间。
    expect(page(0).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '30')
    expect(page(1).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '30')

    // 时间往前走：两边按服务端的截止时间一起递减。
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000)
    })
    expect(page(0).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '28')
    expect(page(1).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '28')

    // 刷新第一个页面：卸载后重新挂载＝重新建立连接并取得首帧。安排没有被重新开始或延长——
    // 首帧给出的仍是同一个安排（剩余 28 秒，而不是 30 秒），而且刷新没有发出任何命令。
    first.unmount()
    const refreshed = mountPage(2)
    act(() => {
      refreshed.source.emit('snapshot', { sessions: [], shutdown: countingDown(28) })
    })
    expect(page(2).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '28')

    // 断线后浏览器自动重连：首帧快照即收敛，仍然没有任何命令。
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_000)
    })
    act(() => second.source.fail())
    expect(page(1).getByTestId('shutdown-stale')).toHaveTextContent('可能已经过期')
    act(() => {
      second.source.open()
      second.source.emit('snapshot', { sessions: [], shutdown: countingDown(27) })
    })
    expect(page(1).queryByTestId('shutdown-stale')).toBeNull()
    expect(page(1).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '27')

    expect(calls).toEqual([])
  })
})

describe('重连首帧覆盖过期视图并保留增量语义（P29、P46）', () => {
  it('断线期间的会话与安排变化由首帧完整快照覆盖，之后仍是会话增量', () => {
    const calls = recordingFetch(() => jsonResponse(200, {}))
    const { source } = mountPage(0)

    act(() => {
      source.emit('snapshot', {
        sessions: [wireSession('sess-a')],
        shutdown: countingDown(30),
      })
    })
    expect(page(0).getByTestId('shutdown-countdown')).toBeVisible()

    // 断线：只降级、不丢最后一份视图。
    act(() => source.fail())
    expect(page(0).getByTestId('shutdown-stale')).toBeVisible()

    // 断线期间另一个调用者取消了安排、并改变了会话状态；重连的首帧是**完整快照**，
    // 过期视图被它整个替换（旧倒计时消失、会话那一半也收敛）。
    act(() => {
      source.open()
      source.emit('snapshot', {
        sessions: [wireSession('sess-a', { state: 'awaiting-input' }), wireSession('sess-b')],
        shutdown: terminal('cancelled'),
      })
    })
    expect(page(0).queryByTestId('shutdown-countdown')).toBeNull()
    expect(page(0).queryByTestId('shutdown-stale')).toBeNull()
    expect(page(0).getByTestId('shutdown-result')).toHaveAttribute('data-outcome', 'cancelled')
    expect(
      within(page(0).getByTestId('waiting-group')).getByTestId('session-name'),
    ).toHaveTextContent('会话 sess-a')

    // 首帧之后仍是既有的会话增量语义：upsert 就地更新那一行，不需要再来一帧快照。
    act(() => {
      source.emit('upsert', { session: wireSession('sess-a', { state: 'working' }) })
    })
    expect(page(0).queryByTestId('waiting-group')).toBeNull()
    expect(page(0).getAllByTestId('session-card')).toHaveLength(2)

    expect(calls).toEqual([])
  })
})

describe('未确认的取消与传输故障（P28、P50）', () => {
  it('取消无回应且观察流中断时保留提醒与过期警告，不显示取消成功、不自动重发', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date(SERVER_TIME))
    const calls = recordingFetch((url) =>
      url === '/api/shutdown/cancel'
        ? Promise.reject(new Error('连接中断'))
        : jsonResponse(200, {}),
    )
    const { source } = mountPage(0)
    act(() => {
      source.emit('snapshot', { sessions: [], shutdown: countingDown(20) })
    })

    fireEvent.click(page(0).getByTestId('shutdown-cancel'))
    await flush()

    // 观察连接也在同一时间断了：提醒仍然留着，未确认与过期两句话同时可见。
    act(() => source.fail())

    expect(page(0).getByTestId('shutdown-countdown')).toBeVisible()
    expect(page(0).getByTestId('shutdown-warning')).toHaveTextContent('取消未确认，电脑仍可能关机')
    expect(page(0).getByTestId('shutdown-stale')).toHaveTextContent('可能已经过期')
    expect(page(0).queryByTestId('shutdown-result')).toBeNull()
    expect(page(0).queryByText(/取消成功/)).toBeNull()

    // 越过响应等待上限也不重发：无回应不是重试的起点。
    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000)
    })
    expect(calls).toEqual(['/api/shutdown/cancel'])
  })

  it('恢复权威状态不发送补偿命令，观察连接自动重连也不重发业务命令（P50）', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date(SERVER_TIME))
    const calls = recordingFetch((url) =>
      url === '/api/shutdown/start'
        ? Promise.reject(new Error('连接中断'))
        : url === '/api/shutdown/cancel'
          ? Promise.reject(new Error('连接中断'))
          : jsonResponse(200, {}),
    )
    const { source } = mountPage(0)

    // 手动入口只在控制面板可见；页面默认停在会话列表。
    fireEvent.click(page(0).getByTestId('tab-control'))

    // 开始意图没有回应：只标记未确认，一次请求。
    fireEvent.click(page(0).getByTestId('shutdown-button'))
    fireEvent.click(page(0).getByTestId('shutdown-confirm-button'))
    await flush()
    expect(calls).toEqual(['/api/shutdown/start'])
    expect(page(0).getByTestId('shutdown-start-unconfirmed')).toBeVisible()

    // 观察流随后给出权威状态：这次开始其实已经成立。页面据此显示倒计时，**不再补发**开始命令。
    act(() => {
      source.emit('snapshot', { sessions: [], shutdown: countingDown(20) })
    })
    await flush()
    expect(page(0).getByTestId('shutdown-countdown')).toBeVisible()
    expect(calls).toEqual(['/api/shutdown/start'])

    // 取消同样没有回应；恢复后观察流给出「取消已生效」的权威证据，页面才显示取消成功，
    // 而且这里也没有任何补偿命令（第二次请求之后不再增加）。
    fireEvent.click(page(0).getByTestId('shutdown-cancel'))
    await flush()
    expect(calls).toEqual(['/api/shutdown/start', '/api/shutdown/cancel'])
    expect(page(0).getByTestId('shutdown-warning')).toHaveTextContent('取消未确认，电脑仍可能关机')

    act(() => {
      source.emit('shutdown', { shutdown: terminal('cancelled') })
    })
    expect(page(0).getByTestId('shutdown-result')).toHaveAttribute('data-outcome', 'cancelled')
    expect(page(0).getByTestId('shutdown-result')).toHaveTextContent('取消成功')
    expect(calls).toEqual(['/api/shutdown/start', '/api/shutdown/cancel'])

    // 观察连接自动重连（浏览器 EventSource 自己的行为）同样只是重新读权威状态，不重发业务命令。
    act(() => source.fail())
    act(() => {
      source.open()
      source.emit('snapshot', { sessions: [], shutdown: terminal('cancelled') })
    })
    expect(calls).toEqual(['/api/shutdown/start', '/api/shutdown/cancel'])
  })
})

describe('连接中断不作为关机成功的证据（P34）', () => {
  it('本地归零且连接中断时显示等待与无法确认，重连后才按权威结果收敛', () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date(SERVER_TIME))
    const calls = recordingFetch(() => jsonResponse(200, {}))
    const { source } = mountPage(0)

    act(() => {
      // 已经到点，但后端结果还没到；同时观察连接断了。
      source.emit('snapshot', { sessions: [], shutdown: countingDown(0) })
    })
    act(() => source.fail())

    expect(page(0).getByTestId('shutdown-waiting')).toHaveTextContent('等待服务端确认')
    expect(page(0).getByTestId('shutdown-stale')).toHaveTextContent('连接已中断')
    expect(page(0).queryByText(/关机指令已发出/)).toBeNull()
    expect(calls).toEqual([])

    // 重连后的首帧给出后端真实结果：这才是唯一可以显示「已发出」的依据。
    act(() => {
      source.open()
      source.emit('snapshot', { sessions: [], shutdown: terminal('issued') })
    })
    expect(page(0).getByTestId('shutdown-result')).toHaveAttribute('data-outcome', 'issued')
    expect(calls).toEqual([])
  })
})

describe('未确认与明确拒绝互不混淆（P47）', () => {
  it('开始意图无回应是“未确认”，不冒充后端拒绝或失败', async () => {
    const calls = recordingFetch(() => Promise.reject(new Error('传输中断')))
    mountPage(0)

    fireEvent.click(page(0).getByTestId('tab-control'))
    fireEvent.click(page(0).getByTestId('shutdown-button'))
    fireEvent.click(page(0).getByTestId('shutdown-confirm-button'))
    await flush()

    const unconfirmed = page(0).getByTestId('shutdown-start-unconfirmed')
    expect(unconfirmed).toHaveAttribute('data-outcome', 'unconfirmed')
    expect(unconfirmed.getAttribute('data-code')).toBeNull()
    expect(page(0).queryByTestId('shutdown-start-refused')).toBeNull()
    expect(calls).toEqual(['/api/shutdown/start'])
  })

  it('取消被明确拒绝时给出可机器读取的类别码，与无回应的未确认分得开', async () => {
    recordingFetch(() =>
      jsonResponse(409, { code: 'shutdown-not-cancellable', message: '关机执行已经开始' }),
    )
    const { source } = mountPage(0)
    act(() => {
      source.emit('snapshot', { sessions: [], shutdown: countingDown(10) })
    })

    fireEvent.click(page(0).getByTestId('shutdown-cancel'))
    await flush()

    expect(page(0).getByTestId('shutdown-refusal')).toHaveTextContent('取消没有生效')
    expect(page(0).queryByTestId('shutdown-warning')).toBeNull()
    expect(page(0).queryByTestId('shutdown-result')).toBeNull()
  })

  it('监听命令无回应是“未确认”，公开文案不夹带系统详情', async () => {
    const calls = recordingFetch(() => Promise.reject(new Error('传输中断')))
    const { source } = mountPage(0)
    act(() => {
      source.emit('snapshot', { sessions: [wireSession('sess-a')], shutdown: idle() })
    })

    fireEvent.click(page(0).getByTestId('monitor-button'))
    await flush()

    expect(calls).toEqual(['/api/shutdown/monitor'])
    const status = page(0).getByTestId('monitor-status')
    expect(status).toHaveAttribute('data-outcome', 'unconfirmed')
    expect(status.getAttribute('data-code')).toBeNull()
    expect(status).toHaveTextContent('未确认')
    expect(status).not.toHaveTextContent(/Error|error|连接中断/)
  })
})

describe('前端时钟偏差不改变显示对齐（P49）', () => {
  it('本机时钟相差数小时的两个页面按服务端时间显示同一剩余时间，且显示不发命令', async () => {
    vi.useFakeTimers()
    const calls = recordingFetch(() => jsonResponse(200, {}))

    // 页面 0 的本机时钟落后一小时。
    vi.setSystemTime(new Date(SERVER_TIME - 3_600_000))
    const lagging = mountPage(0)
    act(() => {
      lagging.source.emit('snapshot', { sessions: [], shutdown: countingDown(10) })
    })
    expect(page(0).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '10')
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3_000)
    })
    expect(page(0).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '7')
    lagging.unmount()

    // 换一台时钟超前两小时的设备打开同一份安排：显示的仍是同样的服务端剩余时间。
    vi.setSystemTime(new Date(SERVER_TIME + 7_200_000))
    const ahead = mountPage(1)
    act(() => {
      ahead.source.emit('snapshot', { sessions: [], shutdown: countingDown(10) })
    })
    expect(page(1).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '10')
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3_000)
    })
    expect(page(1).getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '7')

    expect(calls).toEqual([])
  })
})
