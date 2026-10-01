import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { act } from 'react'
import { describe, expect, it } from 'vitest'

import App from '../src/App'
import type { ConnectionStatus, SessionFeed, SessionFeedSnapshot, SessionView } from '../src/sessionFeed'

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

/**
 * 可变数据源：既能推送新的会话集合，也能推送连接状态，用来观察
 * 真实迁移（working → awaiting-input）、同值帧、重连与断线降级。
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
  const push = async (next: SessionView[], connection: ConnectionStatus = 'live') => {
    snapshot = { sessions: next, connection }
    await act(async () => listeners.forEach((notify) => notify()))
  }
  return { feed, push }
}

/** 打开页面并切到「会话列表」tab。 */
async function showSessions(feed: SessionFeed) {
  const user = userEvent.setup()
  render(<App sessionFeed={feed} />)
  await user.click(screen.getByTestId('tab-sessions'))
  return user
}

/** 被高亮的「刚结束一轮」卡片——组头 N 必须与它的张数一致。 */
function highlightedCards(): HTMLElement[] {
  return screen.getAllByTestId('session-card').filter((card) => card.dataset['justEnded'] === 'true')
}

describe('轮次结束提示与组头 N', () => {
  it('提示只由 working → awaiting-input 的真实迁移派生一次：首帧、同值帧与「知道了」之后都不提示', async () => {
    const { feed, push } = liveFeed([session('a', '会话 A', { state: 'working' })])
    const user = await showSessions(feed)

    // 首帧只建立基线：还没有观察到迁移，就没有提示。
    expect(screen.queryByTestId('waiting-group')).toBeNull()

    // 同值帧（同值心跳）不推进任何迁移。
    await push([session('a', '会话 A', { state: 'working' })])
    expect(screen.queryByTestId('waiting-group-note')).toBeNull()

    // 真实迁移：工作 → 等待用户输入，派生出一次提示。
    await push([session('a', '会话 A', { state: 'awaiting-input' })])
    expect(screen.getByTestId('waiting-group-note')).toHaveTextContent('1 个刚结束轮次')

    // 这个状态再来多少帧都只是同一件已经提示过的事，不重复提示、不补发。
    await push([session('a', '会话 A', { state: 'awaiting-input' })])
    expect(screen.getByTestId('waiting-group-note')).toHaveTextContent('1 个刚结束轮次')
    expect(highlightedCards()).toHaveLength(1)

    await user.click(screen.getByTestId('waiting-group-dismiss'))
    expect(screen.queryByTestId('waiting-group-note')).toBeNull()

    await push([session('a', '会话 A', { state: 'awaiting-input' })])
    expect(screen.queryByTestId('waiting-group-note')).toBeNull()
    expect(screen.queryByTestId('waiting-group-dismiss')).toBeNull()
    expect(highlightedCards()).toHaveLength(0)
  })

  it('多个会话同时结束汇总进同一个等待组，组头 N 按会话去重', async () => {
    const { feed, push } = liveFeed([
      session('a', '会话 A', { state: 'working' }),
      session('b', '会话 B', { state: 'working' }),
    ])
    await showSessions(feed)

    // 两个会话同时结束：合成一个等待组（不是两条提示），N=2。
    await push([
      session('a', '会话 A', { state: 'awaiting-input' }),
      session('b', '会话 B', { state: 'awaiting-input' }),
    ])
    expect(screen.getAllByTestId('waiting-group')).toHaveLength(1)
    expect(screen.getByTestId('waiting-group-note')).toHaveTextContent('2 个刚结束轮次')
    expect(highlightedCards()).toHaveLength(2)

    // A 重新开跑：它离开等待组，N 随之只数还在等待的 B。
    await push([
      session('a', '会话 A', { state: 'working' }),
      session('b', '会话 B', { state: 'awaiting-input' }),
    ])
    expect(screen.getByTestId('waiting-group-note')).toHaveTextContent('1 个刚结束轮次')

    // A 第二次结束：N 数的是会话而不是事件，A 仍然只算一次。
    await push([
      session('a', '会话 A', { state: 'awaiting-input' }),
      session('b', '会话 B', { state: 'awaiting-input' }),
    ])
    expect(screen.getByTestId('waiting-group-note')).toHaveTextContent('2 个刚结束轮次')
  })

  it('组头 N 只数在线会话：结束者在等待中转为离线后移出计数与高亮', async () => {
    const { feed, push } = liveFeed([
      session('a', '在线 A', { state: 'working' }),
      session('b', '在线 B', { state: 'working' }),
      session('c', '在线 C', { state: 'working' }),
    ])
    await showSessions(feed)

    await push([
      session('a', '在线 A', { state: 'awaiting-input' }),
      session('b', '在线 B', { state: 'awaiting-input' }),
      session('c', '在线 C', { state: 'awaiting-input' }),
    ])
    expect(screen.getByTestId('waiting-group-note')).toHaveTextContent('3 个刚结束轮次')

    // C 在等待中转为离线：它退出等待组，也就不再计入 N、不再高亮、也不再是「刚结束」。
    await push([
      session('a', '在线 A', { state: 'awaiting-input' }),
      session('b', '在线 B', { state: 'awaiting-input' }),
      session('c', '在线 C', { online: false, state: 'awaiting-input' }),
    ])
    expect(screen.getByTestId('waiting-group-note')).toHaveTextContent('2 个刚结束轮次')
    expect(highlightedCards()).toHaveLength(2)

    const offline = screen.getAllByTestId('session-card').find((card) => card.dataset['keepalive'] === 'offline')
    expect(offline).toBeDefined()
    expect(offline).toHaveAttribute('data-state', 'awaiting-input')
    expect(offline).not.toHaveAttribute('data-just-ended', 'true')
    expect(offline).not.toHaveAttribute('data-waiting', 'true')
  })

  it('点「知道了」只清 N 与高亮：等待组与 awaiting-input 状态保留', async () => {
    const { feed, push } = liveFeed([
      session('a', '会话 A', { state: 'working' }),
      session('b', '会话 B', { state: 'working' }),
    ])
    const user = await showSessions(feed)

    await push([
      session('a', '会话 A', { state: 'awaiting-input' }),
      session('b', '会话 B', { state: 'awaiting-input' }),
    ])
    expect(screen.getByTestId('waiting-group-note')).toHaveTextContent('2 个刚结束轮次')

    await user.click(screen.getByTestId('waiting-group-dismiss'))

    // 强调（N 与高亮）清掉了，等待组本身回到普通组头。
    expect(screen.queryByTestId('waiting-group-note')).toBeNull()
    expect(screen.queryByTestId('waiting-group-dismiss')).toBeNull()
    expect(highlightedCards()).toHaveLength(0)

    const group = screen.getByTestId('waiting-group')
    expect(group).toBeVisible()
    expect(within(group).getByTestId('waiting-group-heading')).toHaveTextContent('等待用户输入')

    // 会话确实还在等：卡片、状态与等待组高亮都保留。
    const grouped = within(group).getAllByTestId('session-card')
    expect(grouped).toHaveLength(2)
    for (const card of grouped) {
      expect(card).toHaveAttribute('data-state', 'awaiting-input')
      expect(card).toHaveAttribute('data-waiting', 'true')
    }

    // 等待组仍非空，跨 tab 圆点也不因为「知道了」而清零。
    expect(screen.getByTestId('tab-sessions-indicator')).toBeVisible()
  })

  it('重连补进来的 awaiting-input 不计 N、不提示', async () => {
    const { feed, push } = liveFeed([session('a', '会话 A', { state: 'working' })])
    await showSessions(feed)

    // 断线：没有观察到迁移。
    await push([session('a', '会话 A', { state: 'working' })], 'interrupted')
    expect(screen.getByTestId('connection-banner')).toBeVisible()

    // 重连后的快照里这个会话已经在等待输入：它属于快照收敛，不是前端观察到的轮次结束。
    await push([session('a', '会话 A', { state: 'awaiting-input' })], 'live')
    expect(screen.queryByTestId('connection-banner')).toBeNull()
    expect(screen.getByTestId('waiting-group')).toBeVisible()
    expect(screen.queryByTestId('waiting-group-note')).toBeNull()
    expect(highlightedCards()).toHaveLength(0)

    // 恢复实况之后，真实的迁移照常提示。
    await push([session('a', '会话 A', { state: 'working' })], 'live')
    await push([session('a', '会话 A', { state: 'awaiting-input' })], 'live')
    expect(screen.getByTestId('waiting-group-note')).toHaveTextContent('1 个刚结束轮次')
  })
})

describe('断线降级', () => {
  it('连接中断保留最后快照并在会话列表内标「可能已经过期」，降级不跨 tab', async () => {
    const sessions = [
      session('working', '仍在工作', { state: 'working' }),
      session('waiting', '等待输入', { state: 'awaiting-input' }),
    ]
    const { feed, push } = liveFeed(sessions)
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)

    // 停在控制面板时断线：降级只属于会话列表，这里不呈现。
    await push(sessions, 'interrupted')
    const banner = screen.getByTestId('connection-banner')
    expect(banner).not.toBeVisible()
    expect(within(screen.getByTestId('panel-control')).queryByTestId('connection-banner')).toBeNull()

    await user.click(screen.getByTestId('tab-sessions'))
    expect(screen.getByTestId('connection-banner')).toBeVisible()
    expect(screen.getByTestId('connection-banner')).toHaveTextContent('可能已经过期')

    // 保留最后快照：卡片一张不少、状态照旧；首帧就是等待输入的会话也不算「刚结束」。
    expect(screen.getAllByTestId('session-card')).toHaveLength(2)
    const group = screen.getByTestId('waiting-group')
    expect(within(group).getByTestId('session-name')).toHaveTextContent('等待输入')
    expect(screen.queryByTestId('waiting-group-note')).toBeNull()

    // 恢复实况后降级文案消失，快照照旧。
    await push(sessions, 'live')
    expect(screen.queryByTestId('connection-banner')).toBeNull()
    expect(screen.getAllByTestId('session-card')).toHaveLength(2)
  })
})
