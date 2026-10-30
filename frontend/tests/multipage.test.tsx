/**
 * 多页面收敛（契约 P48，S2：主界面交互 interface）。
 *
 * 两个页面同时打开、各自持有**自己的**观察连接：真实的共享观察数据源（`createSseSessionFeed`，与运行时
 * 同一段帧映射代码）各自接一条独立的假 `EventSource`，因此每一页的展示只由**它自己那条流**驱动。
 *
 * 被验收的正是这一点：一页设置／撤销监听、离线维护与登记清除都要经另一页**自己的**观察连接收敛，
 * 不依赖命令回执送到另一页，也不依赖两页共享任何本地状态。
 */

import { render, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { act } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/app/App'
import { createSseSessionFeed } from '../src/chats/sseSessionFeed'
import { FakeSessionEventSource, resetFakeEventSources } from './fakeSessionEventSource'

/** 服务端时钟的一个固定取值：用例不依赖真实时间。 */
const SERVER_TIME = 1_700_000_000_000

/** wire 形状的会话视图：缺省是在线、工作中的会话。 */
function wireSession(sessionId: string, sessionName: string, overrides: Record<string, unknown> = {}) {
  return {
    sessionId,
    sessionName,
    cwd: `E:\\work\\${sessionId}`,
    online: true,
    state: 'working',
    ...overrides,
  }
}

/** 一帧 `snapshot` / `shutdown` 里的关机状态。 */
function shutdownFrame(monitored: string[]) {
  return { phase: 'none', monitoredSessionIds: monitored, serverTime: SERVER_TIME }
}

/** 一个已打开的页面：它的容器，以及它自己那条观察连接的假 `EventSource`。 */
interface Page {
  container: HTMLElement
  source: FakeSessionEventSource
}

/** 一个页面：自己的观察连接（真实数据源 + 独立假 `EventSource`）与自己的容器。 */
function openPage(): Page {
  const feed = createSseSessionFeed({
    eventSourceFactory: (url) => new FakeSessionEventSource(url),
  })
  const view = render(<App sessionFeed={feed} />)
  return { container: view.container, source: FakeSessionEventSource.instances.at(-1)! }
}

/** 页面里某个会话的卡片。 */
function sessionCard(page: Page, sessionName: string): HTMLElement {
  const found = within(page.container)
    .getAllByTestId('session-card')
    .find((node) => within(node).queryByTestId('session-name')?.textContent === sessionName)
  if (found === undefined) throw new Error(`页面里没有名为 ${sessionName} 的卡片`)
  return found
}

/** 某个会话卡片内部的查询域。 */
function card(page: Page, sessionName: string) {
  return within(sessionCard(page, sessionName))
}

beforeEach(() => {
  resetFakeEventSources()
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('多页面经由各自的观察连接收敛', () => {
  it('一页设置／撤销监听与离线维护、登记清除都反映于两页，另一页不需要收到命令回执', async () => {
    const calls: { url: string; body: string | null }[] = []
    // 命令传输替身：按请求体里**明确的目标值**如实回报；回执里的 shutdown 是故意与展示无关的内容，
    // 用来证明两页的选中态都不由回执驱动（选中态只随各自观察流走）。
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const body = typeof init?.body === 'string' ? init.body : null
        calls.push({ url: String(input), body })
        const target = body === null ? { selected: false } : (JSON.parse(body) as { selected: boolean })
        return {
          ok: true,
          status: 200,
          json: async () => ({
            outcome: target.selected ? 'selected' : 'unselected',
            shutdown: shutdownFrame(['不该来自回执']),
          }),
        } as unknown as Response
      }),
    )

    const user = userEvent.setup()
    const pageA = openPage()
    const pageB = openPage()
    await user.click(within(pageA.container).getByTestId('tab-sessions'))
    await user.click(within(pageB.container).getByTestId('tab-sessions'))

    // 两页各自的观察连接各收到首帧快照。
    const snapshot = {
      sessions: [wireSession('a', '会话甲'), wireSession('b', '会话乙')],
      shutdown: shutdownFrame([]),
    }
    act(() => pageA.source.emit('snapshot', snapshot))
    act(() => pageB.source.emit('snapshot', snapshot))
    expect(within(pageA.container).getAllByTestId('session-card')).toHaveLength(2)
    expect(within(pageB.container).getAllByTestId('session-card')).toHaveLength(2)

    // 一页设置监听：明确目标值到达服务端；回执只回到这一页。
    await user.click(card(pageA, '会话甲').getByTestId('monitor-button'))
    expect(calls).toHaveLength(1)
    expect(calls[0].url).toBe('/api/shutdown/monitor')
    expect(calls[0].body).toBe(JSON.stringify({ sessionId: 'a', selected: true }))
    // 回执不参与业务展示：两页都还没变色，另一页也没有任何命令回执。
    expect(card(pageA, '会话甲').getByTestId('monitor-button')).toHaveAttribute('data-selected', 'false')
    expect(card(pageB, '会话甲').getByTestId('monitor-button')).toHaveAttribute('data-selected', 'false')
    expect(within(pageB.container).queryByTestId('monitoring-bar')).toBeNull()

    // 另一页经**自己的**观察连接收敛：它没有命令回执，只收到自己那条流的 `shutdown`。
    act(() => pageB.source.emit('shutdown', { shutdown: shutdownFrame(['a']) }))
    expect(card(pageB, '会话甲').getByTestId('monitor-button')).toHaveAttribute('data-selected', 'true')
    expect(within(pageB.container).getByTestId('monitoring-summary')).toHaveTextContent('1 个')
    // 第一页仍停在自己的上一帧：两页各由自己的连接驱动，不共享本地状态。
    expect(card(pageA, '会话甲').getByTestId('monitor-button')).toHaveAttribute('data-selected', 'false')
    act(() => pageA.source.emit('shutdown', { shutdown: shutdownFrame(['a']) }))
    expect(card(pageA, '会话甲').getByTestId('monitor-button')).toHaveAttribute('data-selected', 'true')

    // 第二页也设置一个监听：命令由第二页发出，两页仍各自经自己的观察连接收敛同一份集合。
    await user.click(card(pageB, '会话乙').getByTestId('monitor-button'))
    expect(calls).toHaveLength(2)
    expect(calls[1].body).toBe(JSON.stringify({ sessionId: 'b', selected: true }))
    act(() => {
      pageA.source.emit('shutdown', { shutdown: shutdownFrame(['a', 'b']) })
      pageB.source.emit('shutdown', { shutdown: shutdownFrame(['a', 'b']) })
    })
    for (const page of [pageA, pageB]) {
      expect(within(page.container).getByTestId('monitoring-summary')).toHaveTextContent('2 个')
      expect(card(page, '会话乙').getByTestId('monitor-button')).toHaveAttribute('data-selected', 'true')
    }

    // 离线维护：两页都把这条登记显示为离线，监听意图仍在（离线成员保留可撤销的入口）。
    const offline = wireSession('a', '会话甲', { online: false })
    act(() => {
      pageA.source.emit('upsert', { session: offline })
      pageB.source.emit('upsert', { session: offline })
    })
    for (const page of [pageA, pageB]) {
      expect(sessionCard(page, '会话甲')).toHaveAttribute('data-keepalive', 'offline')
      expect(card(page, '会话甲').getByTestId('monitor-button')).toHaveAttribute('data-selected', 'true')
      expect(within(page.container).getByTestId('monitoring-summary')).toHaveTextContent('2 个')
    }

    // 一页撤销监听（撤销的正是那个已离线的成员）：回执回到这一页，两页仍各自收敛。
    await user.click(card(pageA, '会话甲').getByTestId('monitor-button'))
    expect(calls).toHaveLength(3)
    expect(calls[2].body).toBe(JSON.stringify({ sessionId: 'a', selected: false }))
    act(() => pageA.source.emit('shutdown', { shutdown: shutdownFrame(['b']) }))
    expect(within(pageA.container).getByTestId('monitoring-summary')).toHaveTextContent('1 个')
    expect(card(pageA, '会话甲').queryByTestId('monitor-button')).toBeNull()
    // 另一页还没收到撤销帧：仍是两个成员与选中态。
    expect(within(pageB.container).getByTestId('monitoring-summary')).toHaveTextContent('2 个')
    act(() => pageB.source.emit('shutdown', { shutdown: shutdownFrame(['b']) }))
    expect(within(pageB.container).getByTestId('monitoring-summary')).toHaveTextContent('1 个')
    expect(card(pageB, '会话甲').queryByTestId('monitor-button')).toBeNull()

    // 登记清除：移除登记，并把不再存在的成员移出集合同步反映于两页。
    act(() => {
      pageA.source.emit('remove', { sessionId: 'a' })
      pageB.source.emit('remove', { sessionId: 'a' })
    })
    for (const page of [pageA, pageB]) {
      expect(within(page.container).getAllByTestId('session-name').map((node) => node.textContent)).toEqual([
        '会话乙',
      ])
      expect(within(page.container).getByTestId('monitoring-summary')).toHaveTextContent('1 个')
    }

    // 另一页始终没有发出任何命令：它的每一次收敛都来自自己的观察连接。
    expect(calls.map((call) => call.url)).toEqual([
      '/api/shutdown/monitor',
      '/api/shutdown/monitor',
      '/api/shutdown/monitor',
    ])
  })
})
