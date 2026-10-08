import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { act } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'
import type { ConnectionStatus, SessionFeed, SessionFeedSnapshot } from '../src/sessionFeed'
import { NO_SHUTDOWN, remainingSeconds, type ShutdownView } from '../src/shutdownView'

/** 服务端时钟的一个固定取值：用例不依赖真实时间，也不断言实现里的计算公式。 */
const SERVER_TIME = 1_700_000_000_000

/** 一个倒计时中的权威状态；`receivedAt` 是本机收到它的时刻。 */
function countingDown(seconds: number, receivedAt: number): ShutdownView {
  return {
    phase: 'counting-down',
    arrangementId: 'shutdown-1',
    deadline: SERVER_TIME + seconds * 1000,
    serverTime: SERVER_TIME,
    monitoredSessionIds: [],
    receivedAt,
  }
}

/** 一个「取消已生效」的权威状态。 */
function cancelled(receivedAt: number): ShutdownView {
  return {
    phase: 'cancelled',
    arrangementId: 'shutdown-1',
    serverTime: SERVER_TIME,
    monitoredSessionIds: [],
    receivedAt,
  }
}

/**
 * 只驱动关机状态的假数据源：与会话列表同一份契约，因此页面拿到的就是观察流会给出的形状。
 */
function shutdownFeed(initial: ShutdownView = NO_SHUTDOWN) {
  let snapshot: SessionFeedSnapshot = { sessions: [], connection: 'live', shutdown: initial }
  const listeners = new Set<() => void>()
  const feed: SessionFeed = {
    subscribe: (onChange) => {
      listeners.add(onChange)
      return () => listeners.delete(onChange)
    },
    getSnapshot: () => snapshot,
  }
  const push = async (shutdown: ShutdownView, connection: ConnectionStatus = 'live') => {
    snapshot = { sessions: [], connection, shutdown }
    await act(async () => listeners.forEach((notify) => notify()))
  }
  return { feed, push }
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

/** 一个可以稍后再答复的请求：用来观察「答复到达之前」的界面。 */
function deferredReply() {
  let settle: (response: Response) => void = () => {}
  let fail: (error: unknown) => void = () => {}
  const promise = new Promise<Response>((resolve, reject) => {
    settle = resolve
    fail = reject
  })
  return { promise, settle, fail }
}

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

describe('手动关机走后端权威倒计时', () => {
  it('确认页返回不发任何命令；确认之后只提交一次开始意图，前端归零也不发实际关机请求', async () => {
    const calls = recordingFetch(() =>
      jsonResponse(200, {
        outcome: 'counting-down',
        shutdown: {
          monitoredSessionIds: [],
          arrangementId: 'shutdown-1',
          phase: 'counting-down',
          deadline: SERVER_TIME + 10_000,
          serverTime: SERVER_TIME,
        },
      }),
    )
    const { feed } = shutdownFeed()
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)

    // 确认页：返回是纯前端事件，不形成安排，也不发送开始／取消／清结果命令（P19、P42）。
    await user.click(screen.getByTestId('shutdown-button'))
    expect(screen.getByTestId('shutdown-confirm')).toBeVisible()
    await user.click(screen.getByTestId('shutdown-back'))
    expect(screen.queryByTestId('shutdown-takeover')).toBeNull()
    expect(calls).toEqual([])
    // 返回没有变成「取消成功」，也没有留下任何提醒。
    expect(screen.queryByTestId('shutdown-result')).toBeNull()

    // 确认：只提交一次开始意图，地址是后端的开始命令（旧的即时关机路径不存在）（P37）。
    await user.click(screen.getByTestId('shutdown-button'))
    await user.click(screen.getByTestId('shutdown-confirm-button'))
    expect(calls).toEqual(['/api/shutdown/start'])

    // 回执被接受：页面只反馈这次开始的归宿；倒计时必须等观察流给出（P42、P43）。
    expect(screen.getByTestId('shutdown-start-accepted')).toBeVisible()
    expect(screen.queryByTestId('shutdown-countdown')).toBeNull()
  })

  it('打开页面就能看到后端已有的安排与剩余时间，本地归零不发请求也不声称已关机', async () => {
    const calls = recordingFetch(() => jsonResponse(200, {}))
    // 安排已经存在（例如另一个设备上发起的）。
    const { feed, push } = shutdownFeed(countingDown(5, Date.now()))
    render(<App sessionFeed={feed} />)

    expect(screen.getByTestId('shutdown-countdown')).toBeVisible()
    expect(screen.getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '5')

    // 后端给出新的剩余时间（另一帧）：显示跟着权威状态走。
    await push(countingDown(2, Date.now()))
    expect(screen.getByTestId('shutdown-countdown')).toHaveAttribute('data-seconds', '2')

    // 本地归零而后端结果还没到：显示等待／无法确认，绝不声称电脑已经关闭，也不发任何请求。
    await push({ ...countingDown(0, Date.now() - 60_000), deadline: SERVER_TIME })
    expect(screen.getByTestId('shutdown-waiting')).toBeVisible()
    expect(screen.getByTestId('shutdown-waiting')).toHaveTextContent('等待服务端确认')
    expect(screen.queryByText(/关机指令已发出/)).toBeNull()
    expect(calls).toEqual([])
  })

  it('连接中断时归零显示无法确认，不显示关机成功', async () => {
    const calls = recordingFetch(() => jsonResponse(200, {}))
    const { feed, push } = shutdownFeed(countingDown(1, Date.now()))
    render(<App sessionFeed={feed} />)

    await push({ ...countingDown(0, Date.now() - 60_000), deadline: SERVER_TIME }, 'interrupted')
    expect(screen.getByTestId('shutdown-waiting')).toHaveTextContent('连接已中断')
    expect(screen.queryByText(/关机指令已发出/)).toBeNull()
    expect(calls).toEqual([])
  })

  it('取消被服务端确认后只反馈归宿；「取消成功」要等观察流给出', async () => {
    const cancel = deferredReply()
    recordingFetch((url) =>
      url === '/api/shutdown/cancel'
        ? cancel.promise
        : jsonResponse(200, { outcome: 'counting-down', shutdown: countingDown(30, Date.now()) }),
    )
    const { feed, push } = shutdownFeed(countingDown(30, Date.now()))
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)

    await user.click(screen.getByTestId('shutdown-cancel'))
    // 请求在途：没有归宿说明，也没有任何成功。
    expect(screen.queryByTestId('shutdown-result')).toBeNull()
    expect(screen.queryByTestId('shutdown-accepted')).toBeNull()

    // 服务端确认取消生效（回执 200）：只反馈这次操作的归宿，安排仍按观察流显示倒计时。
    await act(async () => {
      cancel.settle(
        jsonResponse(200, {
          outcome: 'cancelled',
          shutdown: cancelled(Date.now()),
        }),
      )
    })
    expect(screen.getByTestId('shutdown-accepted')).toHaveTextContent('已确认这次取消')
    expect(screen.getByTestId('shutdown-countdown')).toBeVisible()
    expect(screen.queryByTestId('shutdown-result')).toBeNull()

    // 观察流随后给出「取消已生效」：这才是显示「取消成功」的事实来源。
    await push(cancelled(Date.now()))
    expect(screen.getByTestId('shutdown-result')).toHaveAttribute('data-outcome', 'cancelled')
    expect(screen.getByTestId('shutdown-result')).toHaveTextContent('取消成功')
  })

  it('后端拒绝取消时不显示取消成功，并说明执行已经开始', async () => {
    const calls = recordingFetch((url) =>
      url === '/api/shutdown/cancel'
        ? jsonResponse(409, { code: 'shutdown-not-cancellable', message: '关机执行已经开始' })
        : jsonResponse(200, {}),
    )
    // 取消请求与到期执行擦肩而过：发出时还在倒计时，服务端收到时执行已经开始。
    const { feed } = shutdownFeed(countingDown(3, Date.now()))
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)

    await user.click(screen.getByTestId('shutdown-cancel'))
    expect(calls).toEqual(['/api/shutdown/cancel'])
    expect(screen.getByTestId('shutdown-refusal')).toHaveTextContent('取消没有生效')
    expect(screen.queryByTestId('shutdown-result')).toBeNull()
  })

  it('执行已经开始时界面不再提供取消，也不显示任何成功结果', async () => {
    const calls = recordingFetch(() => jsonResponse(200, {}))
    // 执行已经开始的权威状态：期限已过，后端已经开始实际关机。
    const executing: ShutdownView = {
      ...countingDown(0, Date.now() - 30_000),
      phase: 'executing',
      deadline: SERVER_TIME,
    }
    const { feed } = shutdownFeed(executing)
    render(<App sessionFeed={feed} />)

    expect(screen.getByTestId('shutdown-executing')).toBeVisible()
    expect(screen.queryByTestId('shutdown-cancel')).toBeNull()
    expect(screen.queryByTestId('shutdown-result')).toBeNull()
    expect(calls).toEqual([])
  })

  it('取消没有回应时保留提醒与“电脑仍可能关机”的警告，且不自动重发', async () => {
    vi.useFakeTimers()
    const calls = recordingFetch((url) =>
      url === '/api/shutdown/cancel'
        ? Promise.reject(new Error('连接中断'))
        : jsonResponse(200, {}),
    )
    const { feed } = shutdownFeed(countingDown(20, Date.now()))
    render(<App sessionFeed={feed} />)

    fireEvent.click(screen.getByTestId('shutdown-cancel'))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(calls).toEqual(['/api/shutdown/cancel'])
    const warning = screen.getByTestId('shutdown-warning')
    expect(warning).toHaveTextContent('取消未确认，电脑仍可能关机')

    // 超过响应等待上限也不重发：无回应不是重试的起点（P50），也不会变成取消成功。
    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000)
    })
    expect(calls).toEqual(['/api/shutdown/cancel'])
    expect(screen.queryByTestId('shutdown-result')).toBeNull()
  })

  it('开始意图没有回应时标记未确认、给出电脑仍可能关机的警告，且不自动重发', async () => {
    vi.useFakeTimers()
    const calls = recordingFetch(() => Promise.reject(new Error('连接中断')))
    const { feed } = shutdownFeed()
    render(<App sessionFeed={feed} />)

    fireEvent.click(screen.getByTestId('shutdown-button'))
    fireEvent.click(screen.getByTestId('shutdown-confirm-button'))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(calls).toEqual(['/api/shutdown/start'])
    expect(screen.getByTestId('shutdown-start-unconfirmed')).toBeVisible()
    expect(screen.getByTestId('shutdown-warning')).toHaveTextContent('电脑仍可能关机')

    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000)
    })
    expect(calls).toEqual(['/api/shutdown/start'])
  })

  it('结果页的返回不发命令，也不影响后端的最近结果', async () => {
    const calls = recordingFetch(() => jsonResponse(200, {}))
    const issued: ShutdownView = {
      phase: 'issued',
      arrangementId: 'shutdown-9',
      serverTime: SERVER_TIME,
      monitoredSessionIds: [],
      receivedAt: Date.now(),
    }
    const { feed } = shutdownFeed(issued)
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)

    expect(screen.getByTestId('shutdown-result')).toHaveAttribute('data-outcome', 'issued')
    await user.click(screen.getByTestId('shutdown-result-back'))
    expect(screen.queryByTestId('shutdown-takeover')).toBeNull()
    expect(calls).toEqual([])
  })

  it('剩余时间按服务端时钟算：本机时钟偏差不改变显示的秒数', async () => {
    vi.useFakeTimers()
    // 本机时钟被设到与后端相差很远的位置：只有「收到帧之后经过的时间」参与计算。
    vi.setSystemTime(new Date(9_000_000_000_000))
    const { feed } = shutdownFeed(countingDown(5, Date.now()))
    render(<App sessionFeed={feed} />)

    expect(screen.getByTestId('shutdown-count')).toHaveTextContent('5')

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000)
    })
    expect(screen.getByTestId('shutdown-count')).toHaveTextContent('3')
  })
})

describe('命令回执只反馈归宿，业务展示只随观察流更新（P42、P43）', () => {
  it('观察已给出取消结果后，迟到的合法开始回执不把关机安排拉回倒计时', async () => {
    const start = deferredReply()
    recordingFetch(() => start.promise)
    const { feed, push } = shutdownFeed()
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)

    // 提交开始意图；它的回执被拖住，观察流先给出更晚的事实：这次安排已经被取消。
    await user.click(screen.getByTestId('shutdown-button'))
    await user.click(screen.getByTestId('shutdown-confirm-button'))
    await push(cancelled(Date.now()))
    expect(screen.getByTestId('shutdown-result')).toHaveAttribute('data-outcome', 'cancelled')

    // 迟到的旧回执到达：它只说明那次开始意图的归宿，不得把关机安排拉回倒计时。
    // 回执里的时刻更早（serverTime 与观察流不构成严格递增版本），页面也不靠它排序。
    await act(async () => {
      start.settle(
        jsonResponse(200, {
          outcome: 'counting-down',
          shutdown: { ...countingDown(30, Date.now()), serverTime: SERVER_TIME - 120_000 },
        }),
      )
    })
    expect(screen.getByTestId('shutdown-result')).toHaveAttribute('data-outcome', 'cancelled')
    expect(screen.queryByTestId('shutdown-countdown')).toBeNull()
  })

  it('回执先到、观察尚未更新时业务展示不翻转，只反馈这次开始已被接受', async () => {
    recordingFetch(() =>
      jsonResponse(200, { outcome: 'counting-down', shutdown: countingDown(30, Date.now()) }),
    )
    const { feed } = shutdownFeed()
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)

    await user.click(screen.getByTestId('shutdown-button'))
    await user.click(screen.getByTestId('shutdown-confirm-button'))

    // 回执自称安排已经在倒计时，但观察流还没给出它：不得据此显示倒计时或取消入口。
    expect(screen.queryByTestId('shutdown-countdown')).toBeNull()
    expect(screen.queryByTestId('shutdown-cancel')).toBeNull()

    // 反馈只说明这次操作的归宿：服务端已接受。
    const accepted = screen.getByTestId('shutdown-accepted')
    expect(accepted).toHaveAttribute('data-outcome', 'accepted')
    expect(accepted).toHaveTextContent('已接受这次开始意图')
  })

  it('断线期间被确认的取消只反馈归宿，最后的安排与过期标注都保留', async () => {
    const cancel = deferredReply()
    recordingFetch((url) =>
      url === '/api/shutdown/cancel' ? cancel.promise : jsonResponse(200, {}),
    )
    const { feed, push } = shutdownFeed(countingDown(20, Date.now()))
    render(<App sessionFeed={feed} />)

    // 观察连接中断：保留最后一份安排，并标注它可能已经过期。
    await push(countingDown(20, Date.now()), 'interrupted')
    expect(screen.getByTestId('shutdown-stale')).toBeVisible()

    // 命令照样发得出去，服务端也确认了取消生效。
    fireEvent.click(screen.getByTestId('shutdown-cancel'))
    await act(async () => {
      cancel.settle(jsonResponse(200, { outcome: 'cancelled', shutdown: cancelled(Date.now()) }))
    })

    // 业务展示仍是断线前最后观察到的那份安排，过期标注也还在；回执只反馈这次取消的归宿。
    expect(screen.getByTestId('shutdown-countdown')).toBeVisible()
    expect(screen.getByTestId('shutdown-stale')).toBeVisible()
    expect(screen.getByTestId('shutdown-accepted')).toHaveTextContent('已确认这次取消')
    expect(screen.queryByTestId('shutdown-result')).toBeNull()
  })
})

describe('剩余时间的计算依据', () => {
  it('只用服务端时间与截止时间：与「服务端收到的时刻」不再等价时也照实递减', () => {
    // 期望值来自独立算出的差值：截止时间比服务端时间晚 5 秒，收到 2 秒之后还剩 3 秒。
    expect(remainingSeconds(countingDown(5, 1_000), 3_000)).toBe(3)
    // 本机时钟整体偏移（这里把“现在”与收到时刻一起平移）不改变剩余时间。
    expect(remainingSeconds(countingDown(5, 1_000_000), 1_002_000)).toBe(3)
    // 越过截止时间即 0，不会出现负数。
    expect(remainingSeconds(countingDown(5, 0), 9_999)).toBe(0)
  })
})
