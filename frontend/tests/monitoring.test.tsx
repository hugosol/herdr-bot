import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { act } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import App from '../src/app/App'
import type { SessionFeed, SessionFeedSnapshot, SessionView } from '../src/chats/sessionFeed'
import { NO_SHUTDOWN, type ShutdownView } from '../src/server/shutdown/shutdownView'

/** 服务端时钟的一个固定取值：用例不依赖真实时间。 */
const SERVER_TIME = 1_700_000_000_000

/** 权威关机状态：本次只关心监听集合与阶段。 */
function shutdownWith(monitored: string[], phase: ShutdownView['phase'] = 'none'): ShutdownView {
  return {
    phase,
    serverTime: SERVER_TIME,
    monitoredSessionIds: monitored,
    receivedAt: Date.now(),
  }
}

/** 可以推送新帧的假数据源：会话列表与关机状态同一条观察流。 */
function feedOf(sessions: SessionView[], shutdown: ShutdownView = NO_SHUTDOWN) {
  let snapshot: SessionFeedSnapshot = { sessions, connection: 'live', shutdown }
  const listeners = new Set<() => void>()
  const feed: SessionFeed = {
    subscribe: (onChange) => {
      listeners.add(onChange)
      return () => listeners.delete(onChange)
    },
    getSnapshot: () => snapshot,
  }
  const push = async (next: ShutdownView) => {
    snapshot = { ...snapshot, shutdown: next }
    await act(async () => listeners.forEach((notify) => notify()))
  }
  return { feed, push }
}

/** 一次被记录的监听／取消命令：地址与请求体。 */
type RecordedCall = { url: string; body: string | null }

/** 关机命令的传输替身：记录每次请求的地址与请求体，并按用例给定的答复返回。 */
function recordingFetch(reply: (url: string, body: string | null) => Promise<Response> | Response) {
  const calls: RecordedCall[] = []
  const fetchDouble = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    const body = typeof init?.body === 'string' ? init.body : null
    calls.push({ url, body })
    return reply(url, body)
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
  const promise = new Promise<Response>((resolve) => {
    settle = resolve
  })
  return { promise, settle }
}

/** 在线、agent 工作中的会话。 */
const WORKING: SessionView = {
  sessionId: 'sess-1',
  sessionName: '工作中会话',
  cwd: 'E:\\Developer\\myhub\\herdr-bot',
  online: true,
  state: 'working',
}

/** 在线、等待用户输入的会话。 */
const WAITING: SessionView = { ...WORKING, sessionId: 'sess-2', sessionName: '等待会话', state: 'awaiting-input' }

/** 离线（上报过期但登记仍在）、最后上报为工作中的会话。 */
const OFFLINE: SessionView = {
  ...WORKING,
  sessionId: 'sess-offline',
  sessionName: '离线会话',
  online: false,
}

/** 离线、未被监听的会话：用作「离线不能新增」的对照。 */
const OFFLINE_UNMONITORED: SessionView = {
  ...OFFLINE,
  sessionId: 'sess-offline-plain',
  sessionName: '离线未监听',
}

/** 打开页面并切到「会话列表」tab。 */
async function showSessions(sessions: SessionView[], shutdown: ShutdownView = NO_SHUTDOWN) {
  const { feed, push } = feedOf(sessions, shutdown)
  const user = userEvent.setup()
  render(<App sessionFeed={feed} />)
  await user.click(screen.getByTestId('tab-sessions'))
  return { user, push }
}

/** 某个会话卡片上的监听按钮。 */
function monitorButton(sessionName: string) {
  const card = screen
    .getAllByTestId('session-card')
    .find((node) => within(node).getByTestId('session-name').textContent === sessionName)
  if (card === undefined) throw new Error(`没有名为 ${sessionName} 的卡片`)
  return within(card)
}

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

describe('完成后关机监听的选择与撤销', () => {
  it('观察已给出较新的集合后，迟到的合法监听回执不让选中展示回退', async () => {
    const select = deferredReply()
    recordingFetch(() => select.promise)
    const { user, push } = await showSessions([WORKING])

    // 提交选中；回执被拖住，观察流先给出更晚的事实：倒计时已经开始，监听集合被消耗为空。
    await user.click(monitorButton('工作中会话').getByTestId('monitor-button'))
    await push(shutdownWith([], 'counting-down'))
    expect(screen.queryByTestId('monitor-button')).toBeNull()

    // 迟到的旧回执到达：它只说明那次命令的归宿，不得让监听展示回退到倒计时之前。
    await act(async () => {
      select.settle(
        jsonResponse(200, {
          outcome: 'selected',
          shutdown: { monitoredSessionIds: ['sess-1'], phase: 'none', serverTime: SERVER_TIME },
        }),
      )
    })
    expect(screen.queryByTestId('monitor-button')).toBeNull()
    expect(screen.queryByTestId('monitoring-bar')).toBeNull()
  })

  it('监听回执先到、观察尚未更新时按钮不乐观变色，只反馈这次命令的归宿', async () => {
    recordingFetch(() =>
      jsonResponse(200, {
        outcome: 'selected',
        shutdown: { monitoredSessionIds: ['sess-1'], phase: 'none', serverTime: SERVER_TIME },
      }),
    )
    const { user } = await showSessions([WORKING])

    await user.click(screen.getByTestId('monitor-button'))

    // 观察流还没给出新集合：按钮不得按回执变色。
    expect(screen.getByTestId('monitor-button')).toHaveAttribute('data-selected', 'false')
    expect(screen.queryByTestId('monitoring-bar')).toBeNull()
    // 反馈只说明这次命令已被接受。
    const status = screen.getByTestId('monitor-status')
    expect(status).toHaveAttribute('data-outcome', 'accepted')
    expect(status).toHaveTextContent('已接受这次监听命令')
  })
  it('选中与撤销都提交明确目标值，并且只依观察流的确认变色', async () => {
    const select = deferredReply()
    const calls = recordingFetch(() => select.promise)
    const { user, push } = await showSessions([WORKING])

    const button = monitorButton('工作中会话').getByTestId('monitor-button')
    expect(button).toHaveAttribute('data-selected', 'false')

    // 提交选中：请求体是明确的目标值，而不是「翻转」这种含糊意图。
    await user.click(button)
    expect(calls).toHaveLength(1)
    expect(calls[0].url).toBe('/api/shutdown/monitor')
    expect(calls[0].body).toBe(JSON.stringify({ sessionId: 'sess-1', selected: true }))

    // 答复到达之前不做本地乐观切换：按钮仍然未选中。
    expect(monitorButton('工作中会话').getByTestId('monitor-button')).toHaveAttribute(
      'data-selected',
      'false',
    )

    // 命令被接受：回执只反馈归宿，回执里的集合不参与展示。
    await act(async () => {
      select.settle(
        jsonResponse(200, {
          outcome: 'selected',
          shutdown: {
            monitoredSessionIds: ['sess-1'],
            phase: 'none',
            serverTime: SERVER_TIME,
          },
        }),
      )
    })
    expect(monitorButton('工作中会话').getByTestId('monitor-button')).toHaveAttribute(
      'data-selected',
      'false',
    )

    // 观察流给出权威集合之后才变色。
    await push(shutdownWith(['sess-1']))
    expect(monitorButton('工作中会话').getByTestId('monitor-button')).toHaveAttribute(
      'data-selected',
      'true',
    )

    // 再次点击提交明确的「未选中」目标值。
    const revoke = deferredReply()
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        calls.push({ url: String(input), body: typeof init?.body === 'string' ? init.body : null })
        return revoke.promise
      }),
    )
    await user.click(monitorButton('工作中会话').getByTestId('monitor-button'))
    expect(calls[1].body).toBe(JSON.stringify({ sessionId: 'sess-1', selected: false }))
    expect(monitorButton('工作中会话').getByTestId('monitor-button')).toHaveAttribute(
      'data-selected',
      'true',
    )

    await act(async () => {
      revoke.settle(
        jsonResponse(200, {
          outcome: 'unselected',
          shutdown: {
            monitoredSessionIds: [],
            phase: 'none',
            serverTime: SERVER_TIME,
          },
        }),
      )
    })
    // 观察流给出清空后的集合：这个会话已经不再监听，而它仍在工作，按钮回到未选中的可新增状态。
    await push(shutdownWith([]))
    expect(monitorButton('工作中会话').getByTestId('monitor-button')).toHaveAttribute(
      'data-selected',
      'false',
    )
  })

  it('只有在线工作中的未监听会话才有新增入口；监听不改动会话状态与身份信息', async () => {
    await showSessions([
      WORKING,
      WAITING,
      { ...WORKING, sessionId: 'sess-3', sessionName: '空闲会话', state: 'idle' },
      { ...WORKING, sessionId: 'sess-4', sessionName: '离线会话', online: false },
      { ...WORKING, sessionId: 'sess-5', sessionName: '状态未知会话', state: undefined },
    ])

    expect(monitorButton('工作中会话').queryByTestId('monitor-button')).not.toBeNull()
    for (const name of ['等待会话', '空闲会话', '离线会话', '状态未知会话']) {
      expect(monitorButton(name).queryByTestId('monitor-button')).toBeNull()
    }

    // 会话状态与身份信息照旧可查看（监听控件不替换它们）。
    const working = monitorButton('工作中会话')
    expect(working.getByTestId('session-state')).toHaveTextContent('工作中')
    expect(working.getByTestId('keepalive-dot')).toHaveAttribute('aria-label', '在线')
    expect(working.getByTestId('session-cwd')).toHaveTextContent('E:\\Developer\\myhub\\herdr-bot')
    expect(working.getByTestId('session-name')).toHaveTextContent('工作中会话')
  })

  it('A 已等待、B 仍工作时：A 的已选按钮仍可撤销，并说明在等其他被监听会话', async () => {
    const select = deferredReply()
    recordingFetch(() => select.promise)
    const { user, push } = await showSessions(
      [WORKING, WAITING],
      shutdownWith(['sess-1', 'sess-2']),
    )

    const waitingCard = monitorButton('等待会话')
    const waitingButton = waitingCard.getByTestId('monitor-button')
    expect(waitingButton).toHaveAttribute('data-selected', 'true')
    expect(waitingCard.getByTestId('monitor-note')).toHaveTextContent('正在等待其他被监听会话')

    // B 仍在工作：它没有「等待其他会话」的说明。
    expect(monitorButton('工作中会话').queryByTestId('monitor-note')).toBeNull()

    // 等待中的已选按钮仍然可以撤销。
    await user.click(waitingButton)
    await act(async () => {
      select.settle(
        jsonResponse(200, {
          outcome: 'unselected',
          shutdown: {
            monitoredSessionIds: ['sess-1'],
            phase: 'none',
            serverTime: SERVER_TIME,
          },
        }),
      )
    })
    // 回执被接受并不改变展示：撤销后的集合仍要由观察流给出。
    expect(waitingCard.getByTestId('monitor-button')).toHaveAttribute('data-selected', 'true')

    // 观察流给出撤销后的权威集合：这个会话既不在集合里、又不是工作中，入口与说明一起消失。
    await push(shutdownWith(['sess-1']))
    expect(monitorButton('等待会话').queryByTestId('monitor-button')).toBeNull()
    expect(monitorButton('等待会话').queryByTestId('monitor-note')).toBeNull()
  })

  it('观察流推送新的权威集合时，按钮的选中态跟着它走', async () => {
    const { push } = await showSessions([WORKING])
    expect(monitorButton('工作中会话').getByTestId('monitor-button')).toHaveAttribute(
      'data-selected',
      'false',
    )

    await push(shutdownWith(['sess-1']))
    expect(monitorButton('工作中会话').getByTestId('monitor-button')).toHaveAttribute(
      'data-selected',
      'true',
    )

    // 后端开始倒计时即消耗整组监听：界面回到未选中，并且不再提供新增入口。
    await push(shutdownWith([], 'counting-down'))
    expect(screen.queryByTestId('monitor-button')).toBeNull()
    expect(screen.queryByTestId('monitoring-bar')).toBeNull()
  })

  it('倒计时期间界面不提供新增监听入口，也不显示权威空集合之外的任何选中态', async () => {
    await showSessions([WORKING, WAITING], shutdownWith([], 'counting-down'))

    expect(screen.queryAllByTestId('monitor-button')).toHaveLength(0)
    expect(screen.queryByTestId('monitoring-bar')).toBeNull()
    expect(screen.queryByTestId('monitor-note')).toBeNull()
  })

  it('离线但仍被监听的成员保留显式撤销入口：只反馈归宿，选中态与剩余集合随观察更新', async () => {
    const revoke = deferredReply()
    const calls = recordingFetch(() => revoke.promise)
    // 两个已监听成员（一个在线工作中、一个离线），外加一个离线但未监听的对照。
    const { user, push } = await showSessions(
      [WORKING, OFFLINE, OFFLINE_UNMONITORED],
      shutdownWith(['sess-1', 'sess-offline']),
    )

    // 离线已监听成员：撤销入口可用且显示为已选中 —— 不能因离线隐藏撤销能力。
    const offlineCard = monitorButton('离线会话')
    const offlineButton = offlineCard.getByTestId('monitor-button')
    expect(offlineButton).toBeVisible()
    expect(offlineButton).toBeEnabled()
    expect(offlineButton).toHaveAttribute('data-selected', 'true')

    // 对照：离线但未被监听的成员没有新增入口（离线不满足新增资格）。
    expect(monitorButton('离线未监听').queryByTestId('monitor-button')).toBeNull()

    // 剩余集合的展示：两个成员。
    expect(screen.getByTestId('monitoring-summary')).toHaveTextContent('2 个')

    // 提交明确的「未选中」目标值。
    await user.click(offlineButton)
    expect(calls).toHaveLength(1)
    expect(calls[0].url).toBe('/api/shutdown/monitor')
    expect(calls[0].body).toBe(JSON.stringify({ sessionId: 'sess-offline', selected: false }))

    // 回执只反馈本次命令的归宿，不凭本地翻转选中态。
    await act(async () => {
      revoke.settle(
        jsonResponse(200, {
          outcome: 'unselected',
          shutdown: { monitoredSessionIds: ['sess-1'], phase: 'none', serverTime: SERVER_TIME },
        }),
      )
    })
    expect(monitorButton('离线会话').getByTestId('monitor-button')).toHaveAttribute(
      'data-selected',
      'true',
    )
    expect(monitorButton('离线会话').getByTestId('monitor-status')).toHaveAttribute(
      'data-outcome',
      'accepted',
    )

    // 观察流给出撤销后的权威集合：离线成员的选择态与剩余集合展示一起改变。
    await push(shutdownWith(['sess-1']))
    expect(monitorButton('离线会话').queryByTestId('monitor-button')).toBeNull()
    expect(screen.getByTestId('monitoring-summary')).toHaveTextContent('1 个')
  })
})

describe('全局清空监听', () => {
  it('清空入口与单会话撤销、结果页返回分开，归宿只说明这次命令被接受', async () => {
    const clearing = deferredReply()
    const calls = recordingFetch(() => clearing.promise)
    const { user, push } = await showSessions([WORKING, WAITING], shutdownWith(['sess-1', 'sess-2']))

    const bar = screen.getByTestId('monitoring-bar')
    expect(bar).toHaveTextContent('2 个')
    // 全局入口与每个会话的撤销按钮是两处不同的控件。
    expect(within(bar).getByTestId('monitoring-clear')).toBeVisible()
    expect(screen.queryAllByTestId('monitor-button')).toHaveLength(2)

    await user.click(within(bar).getByTestId('monitoring-clear'))
    expect(calls).toHaveLength(1)
    expect(calls[0].url).toBe('/api/shutdown/cancel')
    // 请求在途：还没有任何归宿说明。
    expect(screen.queryByTestId('monitoring-clear-status')).toBeNull()

    // 服务端确认取消生效（回执 200）：只反馈这次命令的归宿，集合仍按观察流显示。
    await act(async () => {
      clearing.settle(
        jsonResponse(200, {
          outcome: 'cancelled',
          shutdown: { monitoredSessionIds: [], phase: 'none', serverTime: SERVER_TIME },
        }),
      )
    })
    const status = screen.getByTestId('monitoring-clear-status')
    expect(status).toHaveAttribute('data-outcome', 'cleared')
    expect(status).toHaveTextContent('服务端已确认')
    // 归宿说明不冒充业务事实：集合仍按观察流显示那两个成员。
    expect(screen.getByTestId('monitoring-bar')).toHaveTextContent('2 个')

    // 观察流给出清空后的权威集合：清空入口随之收起，归宿说明保留。
    await push(shutdownWith([]))
    expect(screen.queryByTestId('monitoring-bar')).toBeNull()
    expect(screen.getByTestId('monitoring-clear-status')).toBeVisible()
  })

  it('清空没有回应时说明未确认，且不自动重发', async () => {
    vi.useFakeTimers()
    const calls = recordingFetch(() => Promise.reject(new Error('连接中断')))
    // 假定时器下不用 userEvent（它自己会等真实时钟）：直接渲染并派发原生 click。
    const { feed } = feedOf([WORKING], shutdownWith(['sess-1']))
    render(<App sessionFeed={feed} />)

    screen.getByTestId('monitoring-clear').click()
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(calls).toHaveLength(1)
    const status = screen.getByTestId('monitoring-clear-status')
    expect(status).toHaveAttribute('data-outcome', 'unconfirmed')
    expect(status).toHaveTextContent('清空未确认')

    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000)
    })
    // 无回应不是重试的起点：请求次数不变，也没有变成成功。
    expect(calls).toHaveLength(1)
    expect(screen.getByTestId('monitoring-clear-status')).toHaveAttribute(
      'data-outcome',
      'unconfirmed',
    )
  })
})

describe('监听命令的未确认与明确拒绝', () => {
  it('没有回应只标记未确认、不自动重发，也不把按钮标记成已选中', async () => {
    vi.useFakeTimers()
    const calls = recordingFetch(() => Promise.reject(new Error('连接中断')))
    const { feed } = feedOf([WORKING])
    render(<App sessionFeed={feed} />)

    screen.getByTestId('monitor-button').click()
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(calls).toHaveLength(1)
    const status = screen.getByTestId('monitor-status')
    expect(status).toHaveAttribute('data-outcome', 'unconfirmed')
    expect(status).toHaveTextContent('未确认')
    expect(screen.getByTestId('monitor-button')).toHaveAttribute('data-selected', 'false')

    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000)
    })
    expect(calls).toHaveLength(1)
    expect(screen.getByTestId('monitor-status')).toHaveAttribute('data-outcome', 'unconfirmed')
  })

  it('回执缺少可确认数据时不宣称成功，也不拿提交目标或回执里的集合充数', async () => {
    recordingFetch(() =>
      jsonResponse(200, {
        // 200，但正文里没有可确认的目标值；只有回执带来的集合与页面刚刚提交过的目标值。
        shutdown: { monitoredSessionIds: ['sess-1'], phase: 'none', serverTime: SERVER_TIME },
      }),
    )
    const { user } = await showSessions([WORKING])

    await user.click(screen.getByTestId('monitor-button'))

    const status = screen.getByTestId('monitor-status')
    expect(status).toHaveAttribute('data-outcome', 'unconfirmed')
    expect(status).toHaveTextContent('未确认')
    expect(screen.getByTestId('monitor-button')).toHaveAttribute('data-selected', 'false')
    expect(screen.queryByTestId('monitoring-bar')).toBeNull()
  })

  it('未注册的会话被拒绝时给出 404 的类别，与 409 类别互不混淆', async () => {
    recordingFetch(() =>
      jsonResponse(404, { code: 'unknown-session', message: '该会话尚未注册' }),
    )
    const { user } = await showSessions([WORKING])

    await user.click(screen.getByTestId('monitor-button'))

    const status = screen.getByTestId('monitor-status')
    expect(status).toHaveAttribute('data-outcome', 'refused')
    expect(status).toHaveAttribute('data-code', 'unknown-session')
    expect(status).toHaveTextContent('该会话尚未注册')
    expect(screen.getByTestId('monitor-button')).toHaveAttribute('data-selected', 'false')
  })

  it('服务端明确拒绝时给出可机器读取的类别，与未确认区分', async () => {
    recordingFetch(() =>
      jsonResponse(409, { code: 'session-not-eligible', message: '该会话当前不满足新增监听的条件' }),
    )
    const { user } = await showSessions([WORKING])

    await user.click(screen.getByTestId('monitor-button'))

    const status = screen.getByTestId('monitor-status')
    expect(status).toHaveAttribute('data-outcome', 'refused')
    expect(status).toHaveAttribute('data-code', 'session-not-eligible')
    expect(status).toHaveTextContent('服务端拒绝')
    expect(status).not.toHaveTextContent('未确认')
    expect(screen.getByTestId('monitor-button')).toHaveAttribute('data-selected', 'false')
  })

  it('倒计时期间的新增监听被服务端拒绝时，界面说明原因且不伪造选中', async () => {
    recordingFetch(() =>
      jsonResponse(409, { code: 'shutdown-in-progress', message: '已有活动倒计时' }),
    )
    // 倒计时尚未被前端观察到（例如刚发起）：按钮仍可见，但提交会被后端拒绝。
    const { user } = await showSessions([WORKING])

    await user.click(screen.getByTestId('monitor-button'))

    const status = screen.getByTestId('monitor-status')
    expect(status).toHaveAttribute('data-code', 'shutdown-in-progress')
    expect(status).toHaveTextContent('已有活动倒计时')
    expect(screen.getByTestId('monitor-button')).toHaveAttribute('data-selected', 'false')
  })
})
