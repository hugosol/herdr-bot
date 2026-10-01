import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { act } from 'react'
import { describe, expect, it } from 'vitest'

import App from '../src/App'
import type { SessionFeed, SessionFeedSnapshot, SessionView } from '../src/sessionFeed'

/** 造一个会话；只写这条用例关心的字段，其余走缺省值。 */
function session(sessionId: string, sessionName: string, rest: Partial<SessionView> = {}): SessionView {
  return {
    sessionId,
    sessionName,
    cwd: 'E:\\Developer\\myhub\\omp-bot',
    online: true,
    state: 'idle',
    ...rest,
  }
}

/** 不可变数据源：一次渲染到底。 */
function staticFeed(sessions: SessionView[]): SessionFeed {
  const snapshot: SessionFeedSnapshot = { sessions, connection: 'live' }
  return { subscribe: () => () => {}, getSnapshot: () => snapshot }
}

/**
 * 可变数据源：可以在用例中途推送新快照，用来观察迁移（working → awaiting-input、会话转离线）引起的重排与圆点变化。
 */
function liveFeed(sessions: SessionView[]) {
  let snapshot: SessionFeedSnapshot = { sessions, connection: 'live' }
  const listeners = new Set<() => void>()
  const feed: SessionFeed = {
    subscribe: (onChange) => {
      listeners.add(onChange)
      return () => listeners.delete(onChange)
    },
    getSnapshot: () => snapshot,
  }
  const push = async (next: SessionView[]) => {
    snapshot = { sessions: next, connection: 'live' }
    await act(async () => listeners.forEach((notify) => notify()))
  }
  return { feed, push }
}

/** 打开页面并切到「会话列表」tab。 */
async function showSessions(sessions: SessionView[]) {
  const user = userEvent.setup()
  render(<App sessionFeed={staticFeed(sessions)} />)
  await user.click(screen.getByTestId('tab-sessions'))
}

describe('等待组与 tier 排序', () => {
  it('按 tier 排序：在线等待输入 → 在线工作中 → 空闲 → 状态未知 → 离线，同 tier 保持注册顺序', async () => {
    // 注册顺序（数据源给出的顺序）故意与展示顺序无关：同 tier 的两张空闲卡相对顺序必须保持不变。
    await showSessions([
      session('registration-1', '工作中', { state: 'working' }),
      session('registration-2', '等待输入', { state: 'awaiting-input' }),
      session('registration-3', '空闲一'),
      session('registration-4', '离线工作中', { online: false, state: 'working' }),
      session('registration-5', '状态未知', { state: undefined }),
      session('registration-6', '离线等待输入', { online: false, state: 'awaiting-input' }),
      session('registration-7', '空闲二'),
    ])

    expect(screen.getAllByTestId('session-name').map((node) => node.textContent)).toEqual([
      '等待输入',
      '工作中',
      '空闲一',
      '空闲二',
      '状态未知',
      '离线工作中',
      '离线等待输入',
    ])
  })

  it('在线的等待输入会话聚成置顶「等待用户输入」组并高亮，组内只有在线会话', async () => {
    await showSessions([
      session('working-1', '工作中', { state: 'working' }),
      session('waiting-online', '在线等待输入', { state: 'awaiting-input' }),
      session('waiting-offline', '离线等待输入', { online: false, state: 'awaiting-input' }),
    ])

    const board = screen.getByTestId('session-board')
    const group = screen.getByTestId('waiting-group')

    // 置顶：等待组是卡片流里的第一个子块。
    expect(board.firstElementChild).toBe(group)
    expect(within(group).getByTestId('waiting-group-heading')).toHaveTextContent('等待用户输入')

    // 组内只有在线的等待者，且被高亮标记。
    const grouped = within(group).getAllByTestId('session-card')
    expect(grouped).toHaveLength(1)
    expect(within(grouped[0]).getByTestId('session-name')).toHaveTextContent('在线等待输入')
    expect(grouped[0]).toHaveAttribute('data-waiting', 'true')

    // 组外两张卡都不带高亮：离线等待者虽保留 awaiting-input 状态，仍不属于等待组。
    for (const card of screen.getAllByTestId('session-card')) {
      if (card === grouped[0]) continue
      expect(card).not.toHaveAttribute('data-waiting', 'true')
    }
  })

  it('刚发生 working → awaiting-input 的会话额外高亮；首次快照就是等待输入的会话只进等待组', async () => {
    const { feed, push } = liveFeed([
      session('just-ended', '刚结束轮次', { state: 'working' }),
      session('already-waiting', '早已等待', { state: 'awaiting-input' }),
    ])
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)
    await user.click(screen.getByTestId('tab-sessions'))
    expect(screen.getAllByTestId('session-card').map((card) => card.dataset['justEnded'])).toEqual([
      undefined,
      undefined,
    ])

    await push([
      session('just-ended', '刚结束轮次', { state: 'awaiting-input' }),
      session('already-waiting', '早已等待', { state: 'awaiting-input' }),
    ])

    const group = screen.getByTestId('waiting-group')
    const [justEnded, alreadyWaiting] = within(group).getAllByTestId('session-card')
    expect(within(justEnded).getByTestId('session-name')).toHaveTextContent('刚结束轮次')
    expect(within(alreadyWaiting).getByTestId('session-name')).toHaveTextContent('早已等待')
    expect(justEnded).toHaveAttribute('data-just-ended', 'true')
    expect(alreadyWaiting).not.toHaveAttribute('data-just-ended', 'true')
  })

  it('等待中的会话转为离线后下沉末位、标「已离线」并保留最后已知状态，同时退出等待组', async () => {
    const { feed, push } = liveFeed([
      session('still-working', '仍在工作', { state: 'working' }),
      session('waiting', '等待输入', { state: 'awaiting-input' }),
    ])
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)
    await user.click(screen.getByTestId('tab-sessions'))
    expect(screen.getByTestId('waiting-group')).toBeVisible()

    await push([
      session('still-working', '仍在工作', { state: 'working' }),
      session('waiting', '等待输入', { online: false, state: 'awaiting-input' }),
    ])

    // 等待组只含在线会话：唯一的等待者离线后组消失，且它沉到「工作中」之后。
    expect(screen.queryByTestId('waiting-group')).toBeNull()
    expect(screen.getAllByTestId('session-name').map((node) => node.textContent)).toEqual(['仍在工作', '等待输入'])

    const offline = screen.getAllByTestId('session-card')[1]
    expect(offline).toHaveAttribute('data-keepalive', 'offline')
    expect(within(offline).getByTestId('offline-tag')).toHaveTextContent('已离线')
    // 离线只改 keepAlive 与排序 tier，不改写 agent 状态。
    expect(within(offline).getByTestId('session-state')).toHaveTextContent('等待用户输入')
    expect(offline).not.toHaveAttribute('data-waiting', 'true')
  })
})

describe('跨 tab 圆点', () => {
  it('存在在线等待输入会话时「会话列表」tab 显示圆点：无计数、不自动切换、点 tab 也不清零', async () => {
    const user = userEvent.setup()
    render(
      <App
        sessionFeed={staticFeed([
          session('waiting-online', '在线等待输入', { state: 'awaiting-input' }),
          session('working', '工作中', { state: 'working' }),
        ])}
      />,
    )

    const sessionsTab = screen.getByTestId('tab-sessions')
    const dot = within(sessionsTab).getByTestId('tab-sessions-indicator')
    expect(dot).toHaveAttribute('aria-label', '有会话等待输入')
    // 无计数：圆点自身不携带任何文本。
    expect(dot).toHaveTextContent('')
    expect(sessionsTab).toHaveTextContent('会话列表')

    // 不自动切换：圆点出现时页面仍停在「控制面板」。
    expect(screen.getByTestId('tab-control')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('panel-control')).toBeVisible()
    expect(screen.getByTestId('panel-sessions')).not.toBeVisible()

    // 点 tab 不清零：切进、切回，圆点都还在。
    await user.click(sessionsTab)
    expect(within(screen.getByTestId('tab-sessions')).getByTestId('tab-sessions-indicator')).toBeVisible()
    await user.click(screen.getByTestId('tab-control'))
    expect(within(screen.getByTestId('tab-sessions')).getByTestId('tab-sessions-indicator')).toBeVisible()
  })

  it('圆点只为在线等待输入会话亮起：无此类会话时不显示，离线等待者不算', async () => {
    const { feed, push } = liveFeed([
      session('working', '工作中', { state: 'working' }),
      session('offline-waiting', '离线等待输入', { online: false, state: 'awaiting-input' }),
    ])
    render(<App sessionFeed={feed} />)
    expect(screen.queryByTestId('tab-sessions-indicator')).toBeNull()

    // 停在控制面板也能看见圆点出现。
    await push([
      session('working', '工作中', { state: 'working' }),
      session('waiting', '等待输入', { state: 'awaiting-input' }),
    ])
    expect(screen.getByTestId('tab-sessions-indicator')).toBeVisible()
    expect(screen.getByTestId('tab-control')).toHaveAttribute('aria-selected', 'true')

    await push([
      session('working', '工作中', { state: 'working' }),
      session('waiting', '等待输入', { online: false, state: 'awaiting-input' }),
    ])
    expect(screen.queryByTestId('tab-sessions-indicator')).toBeNull()
  })
})
