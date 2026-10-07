import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { act } from 'react'
import { describe, expect, it } from 'vitest'

import App from '../src/App'
import type { SessionFeed, SessionFeedSnapshot, SessionView } from '../src/sessionFeed'
import { NO_SHUTDOWN } from '../src/shutdownView'

/**
 * 假数据源：会话列表的组件测试只通过注入的数据源驱动，不接触全局 EventSource——
 * 页面在 jsdom 里没有任何 EventSource，组件若直接依赖它就会当场失败。
 */
function feedOf(sessions: SessionView[]): SessionFeed {
  const snapshot: SessionFeedSnapshot = { sessions, connection: 'live', shutdown: NO_SHUTDOWN }
  return { subscribe: () => () => {}, getSnapshot: () => snapshot }
}

/** 打开页面并切到「会话列表」tab。 */
async function showSessions(sessions: SessionView[]) {
  const user = userEvent.setup()
  render(<App sessionFeed={feedOf(sessions)} />)
  await user.click(screen.getByTestId('tab-sessions'))
}

/** 在线、agent 工作中、各展示字段齐全的常规会话。 */
const WORKING: SessionView = {
  sessionId: '3f9c1a72-aa10-4d0e-9b11-7c2e5f8a0011',
  sessionName: '会话桥契约梳理',
  cwd: 'E:\\Developer\\myhub\\omp-bot',
  model: 'claude-sonnet-4-5',
  online: true,
  state: 'working',
}

describe('会话列表（卡片流）', () => {
  it('每个会话一张卡，展示 keepAlive 灯、会话名、agent 状态、cwd 与 model', async () => {
    await showSessions([WORKING])

    const card = screen.getByTestId('session-card')
    expect(within(card).getByTestId('keepalive-dot')).toHaveAttribute('aria-label', '在线')
    expect(within(card).getByTestId('session-name')).toHaveTextContent('会话桥契约梳理')
    expect(within(card).getByTestId('session-state')).toHaveTextContent('工作中')
    expect(within(card).getByTestId('session-cwd')).toHaveTextContent('E:\\Developer\\myhub\\omp-bot')
    expect(within(card).getByTestId('session-model')).toHaveTextContent('claude-sonnet-4-5')

    // 不显示最后活动时间：卡片的可见文本恰是这几项字段加上「完成后关机」按钮，多出任何一项都会让这条断言失败。
    expect(card.textContent?.replace(/\s+/g, '')).toBe(
      '会话桥契约梳理工作中E:\\Developer\\myhub\\omp-botclaude-sonnet-4-5完成后关机',
    )
  })

  it('会话名为空时回退为「<cwd 目录名> · <sessionId 前 8 位>」', async () => {
    const unnamed: SessionView = { ...WORKING, sessionName: undefined }
    const blank: SessionView = {
      ...WORKING,
      sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      sessionName: '   ',
      cwd: 'E:\\Developer\\myhub\\my-omp-toolkit\\',
    }

    await showSessions([unnamed, blank])

    const names = screen.getAllByTestId('session-name').map((node) => node.textContent)
    expect(names).toEqual(['omp-bot · 3f9c1a72', 'my-omp-toolkit · aaaaaaaa'])
  })

  it('model 缺省或为空时隐藏该字段', async () => {
    const missing: SessionView = { ...WORKING, model: undefined }
    const empty: SessionView = { ...WORKING, sessionId: 'bbbbbbbb-1111-2222-3333-444455556666', model: '' }

    await showSessions([missing, empty])

    for (const card of screen.getAllByTestId('session-card')) {
      expect(within(card).queryByTestId('session-model')).toBeNull()
    }
  })

  it('keepAlive 与 agent 状态分开显示：在线行无离线标记，离线行仍保留 agent 状态并标「已离线」', async () => {
    const offlineWaiting: SessionView = {
      ...WORKING,
      sessionId: '8b2d4e60-1f33-4a5c-9d77-0e6a1b3c5544',
      online: false,
      state: 'awaiting-input',
    }

    await showSessions([WORKING, offlineWaiting])

    const [online, offline] = screen.getAllByTestId('session-card')
    expect(online).toHaveAttribute('data-keepalive', 'online')
    expect(within(online).getByTestId('keepalive-dot')).toHaveAttribute('aria-label', '在线')
    expect(within(online).queryByTestId('offline-tag')).toBeNull()

    // keepAlive 是独立一步：离线只加标记，不改写 agent 状态。
    expect(offline).toHaveAttribute('data-keepalive', 'offline')
    expect(offline).toHaveAttribute('data-state', 'awaiting-input')
    expect(within(offline).getByTestId('keepalive-dot')).toHaveAttribute('aria-label', '离线')
    expect(within(offline).getByTestId('offline-tag')).toHaveTextContent('已离线')
    expect(within(offline).getByTestId('session-state')).toHaveTextContent('等待用户输入')
  })

  it('state 缺省时显示中性占位「状态同步中…」，不预设成任一具体状态', async () => {
    const unsynced: SessionView = {
      sessionId: 'cccccccc-1111-2222-3333-444455556666',
      cwd: 'E:\\Developer\\myhub\\omp-bot',
      online: true,
    }

    await showSessions([unsynced])

    const card = screen.getByTestId('session-card')
    expect(card).toHaveAttribute('data-state', 'unknown')
    const state = within(card).getByTestId('session-state')
    expect(state).toHaveTextContent('状态同步中…')
    expect(state).not.toHaveTextContent('空闲')
  })

  it('工作中、等待用户输入、离线三种会话同时在场时，agent 状态、keepAlive、会话名与身份字段仍可查看', async () => {
    const waiting: SessionView = {
      ...WORKING,
      sessionId: 'waiting-1',
      sessionName: '等待会话',
      state: 'awaiting-input',
    }
    const offline: SessionView = { ...WORKING, sessionId: 'offline-1', sessionName: '离线会话', online: false }

    await showSessions([WORKING, waiting, offline])

    const cards = screen.getAllByTestId('session-card')
    // 展示顺序：等待置顶 → 工作中 → 离线沉底。
    expect(cards.map((card) => within(card).getByTestId('session-name').textContent)).toEqual([
      '等待会话',
      '会话桥契约梳理',
      '离线会话',
    ])
    // agent 状态与 keepAlive 是两条独立信息，三类会话各自如实呈现。
    expect(cards.map((card) => card.dataset['state'])).toEqual(['awaiting-input', 'working', 'working'])
    expect(cards.map((card) => card.dataset['keepalive'])).toEqual(['online', 'online', 'offline'])
    expect(cards.map((card) => within(card).getByTestId('session-state').textContent)).toEqual([
      '等待用户输入',
      '工作中',
      '工作中',
    ])
    expect(cards.map((card) => within(card).getByTestId('keepalive-dot').getAttribute('aria-label'))).toEqual([
      '在线',
      '在线',
      '离线',
    ])
    expect(within(cards[2]).getByTestId('offline-tag')).toHaveTextContent('已离线')

    // 会话身份与其他普通字段一张都不少。
    for (const card of cards) {
      expect(within(card).getByTestId('session-cwd')).toBeVisible()
      expect(within(card).getByTestId('session-model')).toBeVisible()
    }
  })

  it('无会话时显示引导空态：面板已就绪、尚无会话接入，且不渲染任何卡片', async () => {
    await showSessions([])

    const empty = screen.getByTestId('sessions-empty')
    expect(empty).toBeVisible()
    expect(empty).toHaveTextContent('面板已就绪')
    expect(empty).toHaveTextContent('尚无会话接入')
    expect(screen.queryAllByTestId('session-card')).toHaveLength(0)
  })

  it('数据源推送新快照时卡片流随之更新', async () => {
    let snapshot: SessionFeedSnapshot = { sessions: [], connection: 'live', shutdown: NO_SHUTDOWN }
    const listeners = new Set<() => void>()
    const feed: SessionFeed = {
      subscribe: (onChange) => {
        listeners.add(onChange)
        return () => listeners.delete(onChange)
      },
      getSnapshot: () => snapshot,
    }

    const user = userEvent.setup()
    render(<App sessionFeed={feed} />)
    await user.click(screen.getByTestId('tab-sessions'))
    expect(screen.getByTestId('sessions-empty')).toBeVisible()

    snapshot = { sessions: [WORKING], connection: 'live', shutdown: NO_SHUTDOWN }
    await act(async () => listeners.forEach((notify) => notify()))
    expect(screen.getByTestId('session-card')).toBeVisible()
    expect(screen.queryByTestId('sessions-empty')).toBeNull()

    snapshot = { sessions: [], connection: 'interrupted', shutdown: NO_SHUTDOWN }
    await act(async () => listeners.forEach((notify) => notify()))
    expect(screen.getByTestId('sessions-empty')).toBeVisible()
    expect(screen.queryAllByTestId('session-card')).toHaveLength(0)
  })
})
