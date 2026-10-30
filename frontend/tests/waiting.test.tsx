import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { act } from 'react'
import { describe, expect, it } from 'vitest'

import App from '../src/app/App'
import type { SessionFeed, SessionFeedSnapshot, SessionView } from '../src/chats/sessionFeed'
import { NO_SHUTDOWN } from '../src/server/shutdown/shutdownView'

/** 造一个会话；只写这条用例关心的字段，其余走缺省值。 */
function session(sessionId: string, sessionName: string, rest: Partial<SessionView> = {}): SessionView {
  return {
    sessionId,
    sessionName,
    cwd: 'E:\\Developer\\myhub\\herdr-bot',
    online: true,
    state: 'idle',
    ...rest,
  }
}

/** 不可变数据源：一次渲染到底。 */
function staticFeed(sessions: SessionView[]): SessionFeed {
  const snapshot: SessionFeedSnapshot = { sessions, connection: 'live', shutdown: NO_SHUTDOWN }
  return { subscribe: () => () => {}, getSnapshot: () => snapshot }
}

/**
 * 可变数据源：可以在用例中途推送新快照，用来观察迁移（working → awaiting-input、会话转离线）引起的重排与分组变化。
 */
function liveFeed(sessions: SessionView[]) {
  let snapshot: SessionFeedSnapshot = { sessions, connection: 'live', shutdown: NO_SHUTDOWN }
  const listeners = new Set<() => void>()
  const feed: SessionFeed = {
    subscribe: (onChange) => {
      listeners.add(onChange)
      return () => listeners.delete(onChange)
    },
    getSnapshot: () => snapshot,
  }
  const push = async (next: SessionView[]) => {
    snapshot = { sessions: next, connection: 'live', shutdown: NO_SHUTDOWN }
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

  it('在线的等待输入会话聚成置顶「等待用户输入」组，组内只有在线会话', async () => {
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

    // 组内只有在线的等待者，且带普通等待状态标记。
    const grouped = within(group).getAllByTestId('session-card')
    expect(grouped).toHaveLength(1)
    expect(within(grouped[0]).getByTestId('session-name')).toHaveTextContent('在线等待输入')
    expect(grouped[0]).toHaveAttribute('data-waiting', 'true')

    // 组外两张卡都不带等待标记：离线等待者虽保留 awaiting-input 状态，仍不属于等待组。
    for (const card of screen.getAllByTestId('session-card')) {
      if (card === grouped[0]) continue
      expect(card).not.toHaveAttribute('data-waiting', 'true')
    }
  })

  it('在线等待、在线工作中、在线空闲、离线四类示例各一张时，等待会话置顶成组，其余按工作中 → 空闲 → 离线排列', async () => {
    await showSessions([
      session('offline', '离线会话', { online: false, state: 'working' }),
      session('idle', '空闲会话'),
      session('working', '工作中会话', { state: 'working' }),
      session('waiting', '等待会话', { state: 'awaiting-input' }),
    ])

    // 注册顺序故意与展示顺序不同：展示顺序完全由 tier 决定。
    expect(screen.getAllByTestId('session-name').map((node) => node.textContent)).toEqual([
      '等待会话',
      '工作中会话',
      '空闲会话',
      '离线会话',
    ])

    // 等待会话单独在置顶组里，其余三张都在组外。
    const group = screen.getByTestId('waiting-group')
    expect(screen.getByTestId('session-board').firstElementChild).toBe(group)
    expect(within(group).getAllByTestId('session-name').map((node) => node.textContent)).toEqual(['等待会话'])
    expect(within(group).getAllByTestId('session-card')).toHaveLength(1)
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

describe('轮次结束不产生强调', () => {
  it('工作中转为等待用户输入后，等待组不显示轮次计数与「知道了」，卡片也没有刚结束轮次专属标记', async () => {
    const { feed, push } = liveFeed([
      session('ends', '转为等待的会话', { state: 'working' }),
      session('still-working', '仍在工作', { state: 'working' }),
    ])
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)
    await user.click(screen.getByTestId('tab-sessions'))

    await push([
      session('ends', '转为等待的会话', { state: 'awaiting-input' }),
      session('still-working', '仍在工作', { state: 'working' }),
    ])

    // 组里只有标题这一行：没有「N 个刚结束轮次」，也没有「知道了」。
    const group = screen.getByTestId('waiting-group')
    expect(within(group).getByTestId('waiting-group-heading')).toHaveTextContent('等待用户输入')
    expect(within(group).queryByTestId('waiting-group-note')).toBeNull()
    expect(screen.queryByTestId('waiting-group-dismiss')).toBeNull()
    expect(screen.queryByRole('button', { name: '知道了' })).toBeNull()
    expect(within(group).queryAllByRole('button')).toHaveLength(0)
    // 改名保留旧强调也会被这条挡住：组头一段文字里不该出现轮次计数或确认按钮的文案。
    expect(group.textContent).not.toContain('个刚结束轮次')
    expect(group.textContent).not.toContain('知道了')

    // 卡片没有任何「刚结束轮次」专属标记；普通等待状态样式与状态文案保留。
    const ended = within(group).getByTestId('session-card')
    expect(ended).not.toHaveAttribute('data-just-ended')
    expect(ended.className).not.toMatch(/just-ended/)
    expect(ended).toHaveAttribute('data-waiting', 'true')
    expect(ended.className).toMatch(/is-waiting/)
    expect(within(ended).getByTestId('session-state')).toHaveTextContent('等待用户输入')

    // 另一张仍工作中的卡片照常留在等待组之外。
    const working = screen.getAllByTestId('session-card').find((card) => card.dataset['state'] === 'working')
    expect(working).toBeDefined()
    expect(within(working!).getByTestId('session-name')).toHaveTextContent('仍在工作')
  })
})

describe('会话列表 tab 不显示等待提示', () => {
  it('存在在线等待用户输入会话时，tab 仍只有文字：没有黄点、没有提示元素，等待组照常出现', async () => {
    const { feed, push } = liveFeed([
      session('working', '工作中', { state: 'working' }),
      session('offline-waiting', '离线等待输入', { online: false, state: 'awaiting-input' }),
    ])
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)

    // 基线：只有离线等待者，tab 没有任何提示。
    const sessionsTab = screen.getByTestId('tab-sessions')
    expect(sessionsTab.textContent).toBe('chat')

    // 等待者转为在线等待输入：旧行为会在这里点亮 tab 黄点。
    await push([
      session('working', '工作中', { state: 'working' }),
      session('waiting', '等待输入', { state: 'awaiting-input' }),
    ])

    // tab 仍然只有标签文字：没有黄点、没有计数，也没有任何挂在 tab 上的提示元素。
    expect(sessionsTab.textContent).toBe('chat')
    expect(screen.queryByTestId('tab-sessions-indicator')).toBeNull()
    // 装饰图标不改变无障碍名；等待状态不附加黄点、计数或提示。
    expect(screen.getByRole('tab', { name: 'chat' })).toBe(sessionsTab)
    expect(screen.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')

    // 场景确实到达了条件：会话列表里的等待组非空，等待者就在里面。
    await user.click(sessionsTab)
    expect(within(screen.getByTestId('waiting-group')).getByTestId('session-name')).toHaveTextContent('等待输入')
  })
})
