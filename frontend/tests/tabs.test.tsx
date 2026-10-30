import { fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/app/App'

beforeEach(() => {
  window.history.replaceState(null, '', '/')
})

describe('App navigation', () => {
  it('returns to the Run list after leaving a detail for another tab and coming back', async () => {
    window.history.replaceState(null, '', '/#workflow/history-run')
    const user = userEvent.setup()
    render(<App />)
    await screen.findByRole('link', { name: '返回全部 Run' })
    await user.click(screen.getByRole('tab', { name: 'chat' }))
    await user.click(screen.getByRole('tab', { name: 'workflow' }))
    expect(screen.getByRole('link', { name: '返回全部 Run' })).toBeVisible()
    await user.click(screen.getByRole('link', { name: '返回全部 Run' }))
    expect(await screen.findByRole('searchbox')).toBeVisible()
    expect(screen.queryByRole('link', { name: '返回全部 Run' })).not.toBeInTheDocument()
  })

  it('defaults to Chat with only work entries visible and one active page', () => {
    render(<App />)
    expect(within(screen.getByRole('tablist')).getAllByRole('tab').map(tab => tab.textContent)).toEqual(['chat', 'zone', 'workflow'])
    expect(screen.getByRole('tab', { name: 'chat' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getAllByRole('tabpanel')).toHaveLength(1)
    expect(screen.getByTestId('panel-sessions')).toBeVisible()
    expect(screen.getByTestId('panel-scheduler')).not.toBeVisible()
  })

  it('retains native keyboard activation and wraps through visible entries', async () => {
    const user = userEvent.setup()
    render(<App />)
    screen.getByRole('tab', { name: 'chat' }).focus()
    await user.tab()
    expect(screen.getByRole('tab', { name: 'zone' })).toHaveFocus()
    await user.keyboard(' ')
    expect(screen.getByTestId('panel-files')).toBeVisible()
    await user.keyboard('{ArrowRight}')
    expect(screen.getByRole('tab', { name: 'workflow' })).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(screen.getByRole('tab', { name: 'chat' })).toHaveFocus()
    await user.keyboard('{ArrowLeft}')
    expect(screen.getByRole('tab', { name: 'workflow' })).toHaveFocus()
    await user.keyboard('{Home}')
    expect(screen.getByRole('tab', { name: 'chat' })).toHaveAttribute('aria-selected', 'true')
    await user.keyboard('{End}')
    expect(screen.getByRole('tab', { name: 'workflow' })).toHaveAttribute('aria-selected', 'true')
  })

  it('switches groups only from a deliberate navigation-strip swipe and remembers each group selection', async () => {
    const user = userEvent.setup()
    render(<App />)
    const nav = screen.getByRole('navigation', { name: '主导航' })
    Object.defineProperty(nav.querySelector('.layout-nav-viewport'), 'clientWidth', { value: 300 })
    const swipe = (target: HTMLElement, from: number, to: number) => {
      fireEvent.touchStart(target, { touches: [{ identifier: 1, clientX: from, clientY: 20 }] })
      fireEvent.touchMove(target, { touches: [{ identifier: 1, clientX: to, clientY: 20 }] })
      fireEvent.touchEnd(target, { touches: [], changedTouches: [{ identifier: 1, clientX: to, clientY: 20 }] })
    }
    await user.click(screen.getByRole('tab', { name: 'zone' }))
    swipe(screen.getByTestId('panel-files'), 20, 200)
    swipe(nav, 280, 181)
    expect(screen.getByRole('tab', { name: 'zone' })).toHaveAttribute('aria-selected', 'true')
    swipe(nav, 280, 190)
    expect(screen.getByRole('tab', { name: 'zone' })).toHaveAttribute('aria-selected', 'true')
    fireEvent.touchStart(nav, { touches: [{ identifier: 1, clientX: 20, clientY: 20 }] })
    fireEvent.touchCancel(nav)
    fireEvent.touchEnd(nav, { touches: [], changedTouches: [{ identifier: 1, clientX: 200, clientY: 20 }] })
    expect(screen.queryByRole('tab', { name: 'task' })).toBeNull()
    swipe(nav, 280, 180)
    await user.click(screen.getByRole('tab', { name: 'bot' }))
    swipe(nav, 20, 120)
    expect(screen.getByRole('tab', { name: 'zone' })).toHaveAttribute('aria-selected', 'true')
    swipe(nav, 280, 180)
    expect(screen.getByRole('tab', { name: 'bot' })).toHaveAttribute('aria-selected', 'true')
  })

  it('persists only layout in the URL without replacing mounted page instances or query/hash data', async () => {
    window.history.replaceState(null, '', '/?other=keep#anchor')
    const user = userEvent.setup()
    const first = render(<App />)
    const chat = screen.getByTestId('panel-sessions')
    await user.click(screen.getByRole('button', { name: '展开导航' }))
    expect(screen.getByRole('tab', { name: 'bot' })).toBeVisible()
    expect(screen.getByTestId('panel-sessions')).toBe(chat)
    expect(window.location.search).toContain('other=keep')
    expect(window.location.search).toContain('layout=expanded')
    expect(window.location.hash).toBe('#anchor')
    await user.click(screen.getByRole('tab', { name: 'scheduler' }))
    first.unmount()
    render(<App />)
    expect(screen.getByRole('tab', { name: 'chat' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('button', { name: '收起导航' })).toBeVisible()
  })

  it('ignores an existing Run hash on reload without reactivating it on layout changes', async () => {
    window.history.replaceState(null, '', '/?layout=expanded#workflow/old-run')
    const timing = Object.getOwnPropertyDescriptor(performance, 'getEntriesByType')
    Object.defineProperty(performance, 'getEntriesByType', { configurable: true, value: vi.fn(() => [{ type: 'reload' }]) })
    try {
      const user = userEvent.setup()
      render(<App />)
      expect(screen.getByRole('tab', { name: 'chat' })).toHaveAttribute('aria-selected', 'true')
      await user.click(screen.getByRole('button', { name: '收起导航' }))
      expect(screen.getByRole('tab', { name: 'chat' })).toHaveAttribute('aria-selected', 'true')
      expect(window.location.hash).toBe('#workflow')
    } finally {
      if (timing) Object.defineProperty(performance, 'getEntriesByType', timing)
      else Reflect.deleteProperty(performance, 'getEntriesByType')
    }
  })

  it('returns from Server to the selected scheduler subview and restores focus', async () => {
    window.history.replaceState(null, '', '/?layout=expanded')
    const user = userEvent.setup()
    render(<App />)
    await user.click(screen.getByRole('tab', { name: 'bot' }))
    await user.click(screen.getByTestId('server-button'))
    expect(screen.getByTestId('server-page')).toBeVisible()
    expect(screen.queryByRole('tablist')).toBeNull()
    expect(screen.getByTestId('start-button')).toBeEnabled()
    expect(screen.getByTestId('stop-button')).toBeEnabled()
    expect(screen.getByTestId('server-back')).toHaveAccessibleName('返回')
    expect(screen.queryByTestId('server-button')).toBeNull()
    await user.click(screen.getByTestId('server-back'))
    expect(screen.getByTestId('server-button')).toHaveTextContent('server')
    expect(screen.getByTestId('panel-scheduler')).toBeVisible()
    expect(screen.getByRole('tab', { name: 'bot' })).toHaveFocus()
    expect(screen.getByRole('region', { name: '共享 Bot 列表' })).toBeVisible()
  })

  it('keeps the application heading', () => {
    render(<App />)
    expect(screen.getByRole('heading', { level: 1, name: 'herdr-bot' })).toBeVisible()
  })
})
