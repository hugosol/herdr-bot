import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'

import App from '../src/App'

/**
 * 双 tab 骨架：结构、默认选中、键盘可达与整页可见标题。
 *
 * 键盘口径由票固定：两个 tab 是原生 button，Tab 聚焦 + Enter/Space 切换；
 * 明确不实现方向键与 roving tabindex（偏离 WAI-ARIA APG，依赖原生行为）。
 */
describe('App 双 tab 骨架', () => {
  it('整页恰两个 tab，默认停在「控制面板」', () => {
    render(<App />)

    const tabs = within(screen.getByRole('tablist')).getAllByRole('tab')
    expect(tabs).toHaveLength(2)
    expect(tabs.map((tab) => tab.textContent)).toEqual(['控制面板', '会话列表'])
    expect(tabs[0]).toHaveAttribute('aria-selected', 'true')
    expect(tabs[1]).toHaveAttribute('aria-selected', 'false')

    // 两个面板互不挤占：同一时刻只有一个 tabpanel 在可访问性树里可见。
    expect(screen.getAllByRole('tabpanel')).toHaveLength(1)
    expect(screen.getByTestId('panel-control')).toBeVisible()
    expect(screen.getByTestId('panel-sessions')).not.toBeVisible()
  })

  it('Tab 可聚焦两个 tab，Enter/Space 可切换且不再依赖方向键', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.tab()
    expect(screen.getByTestId('tab-control')).toHaveFocus()
    await user.tab()
    expect(screen.getByTestId('tab-sessions')).toHaveFocus()

    await user.keyboard('{Enter}')
    expect(screen.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('panel-sessions')).toBeVisible()
    expect(screen.getByTestId('panel-control')).not.toBeVisible()

    await user.tab({ shift: true })
    expect(screen.getByTestId('tab-control')).toHaveFocus()
    await user.keyboard(' ')
    expect(screen.getByTestId('tab-control')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('panel-control')).toBeVisible()
    expect(screen.getByTestId('panel-sessions')).not.toBeVisible()

    // 方向键不参与切换：两个 tab 都在 Tab 顺序里，不需要 roving tabindex。
    await user.click(screen.getByTestId('tab-sessions'))
    await user.keyboard('{ArrowLeft}')
    expect(screen.getByTestId('panel-sessions')).toBeVisible()
    expect(screen.getByTestId('panel-control')).not.toBeVisible()
  })

  it('点击 tab 可来回切换，会话列表挂载会话卡片流', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByTestId('tab-sessions'))
    expect(screen.getByTestId('panel-sessions')).toBeVisible()
    expect(screen.getByTestId('session-board')).toBeVisible()
    expect(screen.getByTestId('panel-control')).not.toBeVisible()

    await user.click(screen.getByTestId('tab-control'))
    expect(screen.getByTestId('panel-control')).toBeVisible()
    expect(screen.getByTestId('panel-sessions')).not.toBeVisible()

    // 控制面板的固定操作仍原样可用（两个 tab 互不挤占）。
    expect(screen.getByTestId('start-button')).toBeEnabled()
    expect(screen.getByTestId('stop-button')).toBeEnabled()
    expect(screen.getByTestId('operation-result')).toHaveTextContent('尚未提交操作。')
  })

  it('整页可见标题为 omp-bot', () => {
    render(<App />)

    expect(screen.getByRole('heading', { level: 1, name: 'omp-bot' })).toBeVisible()
  })
})
