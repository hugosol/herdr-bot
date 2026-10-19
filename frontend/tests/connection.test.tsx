import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { act } from 'react'
import { describe, expect, it } from 'vitest'

import App from '../src/App'
import type { ConnectionStatus, SessionFeed, SessionFeedSnapshot, SessionView } from '../src/sessionFeed'
import { NO_SHUTDOWN } from '../src/shutdownView'

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
 * 可变数据源：既能推送新的会话集合，也能推送连接状态，用来观察断线降级与重连收敛。
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
  const push = async (next: SessionView[], connection: ConnectionStatus = 'live') => {
    snapshot = { sessions: next, connection, shutdown: NO_SHUTDOWN }
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

describe('断线降级与重连', () => {
  it('连接中断保留最后快照并在会话列表内标「可能已经过期」，降级不跨 tab', async () => {
    const sessions = [
      session('working', '仍在工作', { state: 'working' }),
      session('waiting', '等待输入', { state: 'awaiting-input' }),
    ]
    const { feed, push } = liveFeed(sessions)
    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)

    // 先切到控制面板再断线：降级只属于会话列表，这里不呈现。
    await user.click(screen.getByTestId('tab-control'))
    await push(sessions, 'interrupted')
    const banner = screen.getByTestId('connection-banner')
    expect(banner).not.toBeVisible()
    expect(within(screen.getByTestId('panel-control')).queryByTestId('connection-banner')).toBeNull()

    await user.click(screen.getByTestId('tab-sessions'))
    expect(screen.getByTestId('connection-banner')).toBeVisible()
    expect(screen.getByTestId('connection-banner')).toHaveTextContent('可能已经过期')

    // 保留最后快照：卡片一张不少、状态照旧。
    expect(screen.getAllByTestId('session-card')).toHaveLength(2)
    const group = screen.getByTestId('waiting-group')
    expect(within(group).getByTestId('session-name')).toHaveTextContent('等待输入')

    // 恢复实况后降级文案消失，快照照旧。
    await push(sessions, 'live')
    expect(screen.queryByTestId('connection-banner')).toBeNull()
    expect(screen.getAllByTestId('session-card')).toHaveLength(2)
  })

  it('轮次结束与浏览器重连后都不显示「知道了」按钮，等待组照常保留', async () => {
    const { feed, push } = liveFeed([session('a', '会话 A', { state: 'working' })])
    await showSessions(feed)
    expect(screen.queryByTestId('waiting-group')).toBeNull()

    // 轮次结束：会话从工作中转入等待用户输入。
    await push([session('a', '会话 A', { state: 'awaiting-input' })])
    expect(screen.getByTestId('waiting-group')).toBeVisible()
    expect(screen.queryByTestId('waiting-group-dismiss')).toBeNull()
    expect(screen.queryByRole('button', { name: '知道了' })).toBeNull()

    // 断线：会话仍在等待；重连后的首帧快照仍是同一个等待者。
    await push([session('a', '会话 A', { state: 'awaiting-input' })], 'interrupted')
    expect(screen.getByTestId('connection-banner')).toBeVisible()
    await push([session('a', '会话 A', { state: 'awaiting-input' })], 'live')
    expect(screen.queryByTestId('connection-banner')).toBeNull()

    // 重连后同样没有确认按钮：等待者与等待组照常，只是没有任何轮次结束提示。
    expect(screen.queryByTestId('waiting-group-dismiss')).toBeNull()
    expect(screen.queryByRole('button', { name: '知道了' })).toBeNull()
    const group = screen.getByTestId('waiting-group')
    expect(within(group).getByTestId('session-name')).toHaveTextContent('会话 A')
    expect(within(group).getByTestId('session-card')).toHaveAttribute('data-state', 'awaiting-input')
  })
})
