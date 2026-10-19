import { fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'

import App from '../src/App'

/**
 * 三 tab 骨架：顺序、默认入口、键盘可达与“滑动不切 tab”。
 *
 * 顶层顺序固定为会话列表、文件列表、控制面板；每次打开或整页刷新都从会话列表开始，
 * 不恢复上次 tab。三个 tab 都是原生按钮：Tab 可逐个聚焦，Enter/Space 触发点击；
 * 方向键在三个入口之间循环切换并同步焦点。左右滑动不参与切换。
 */
describe('App 三 tab 骨架', () => {
  it('整页恰三个 tab，顺序为会话列表、文件列表、控制面板，默认停在会话列表', () => {
    render(<App />)

    const tabs = within(screen.getByRole('tablist')).getAllByRole('tab')
    expect(tabs.map((tab) => tab.textContent)).toEqual(['会话列表', '文件列表', '控制面板'])
    expect(tabs[0]).toHaveAttribute('aria-selected', 'true')
    expect(tabs[1]).toHaveAttribute('aria-selected', 'false')
    expect(tabs[2]).toHaveAttribute('aria-selected', 'false')

    // 同一时刻只有一个 tabpanel 在可访问性树里可见。
    expect(screen.getAllByRole('tabpanel')).toHaveLength(1)
    expect(screen.getByTestId('panel-sessions')).toBeVisible()
    expect(screen.getByTestId('panel-files')).not.toBeVisible()
    expect(screen.getByTestId('panel-control')).not.toBeVisible()
  })

  it('Tab 可聚焦三个 tab，Enter/Space 可切换', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.tab()
    expect(screen.getByTestId('tab-sessions')).toHaveFocus()
    await user.tab()
    expect(screen.getByTestId('tab-files')).toHaveFocus()
    await user.tab()
    expect(screen.getByTestId('tab-control')).toHaveFocus()

    // Space 激活当前聚焦的第三个入口。
    await user.keyboard(' ')
    expect(screen.getByTestId('tab-control')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('panel-control')).toBeVisible()
    expect(screen.getByTestId('panel-sessions')).not.toBeVisible()

    // Shift+Tab 回到文件列表，Enter 激活它。
    await user.tab({ shift: true })
    expect(screen.getByTestId('tab-files')).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(screen.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('panel-files')).toBeVisible()
    expect(screen.getByTestId('panel-control')).not.toBeVisible()
  })

  it('方向键在三个入口之间循环切换，并同步焦点', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByTestId('tab-sessions'))
    expect(screen.getByTestId('tab-sessions')).toHaveFocus()

    await user.keyboard('{ArrowRight}')
    expect(screen.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('tab-files')).toHaveFocus()
    expect(screen.getByTestId('panel-files')).toBeVisible()

    await user.keyboard('{ArrowRight}')
    expect(screen.getByTestId('tab-control')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('tab-control')).toHaveFocus()

    // 右方向在末尾回到第一个入口。
    await user.keyboard('{ArrowRight}')
    expect(screen.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('tab-sessions')).toHaveFocus()

    // 左方向在开头绕到最后一个入口。
    await user.keyboard('{ArrowLeft}')
    expect(screen.getByTestId('tab-control')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('tab-control')).toHaveFocus()

    // Home / End 直接跳到两端。
    await user.keyboard('{Home}')
    expect(screen.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')
    await user.keyboard('{End}')
    expect(screen.getByTestId('tab-control')).toHaveAttribute('aria-selected', 'true')
  })

  it('左右滑动不切换顶层 tab', async () => {
    render(<App />)
    const tablist = screen.getByRole('tablist')

    fireEvent.touchStart(tablist, {
      touches: [{ identifier: 1, clientX: 300, clientY: 20 }],
    })
    fireEvent.touchMove(tablist, {
      touches: [{ identifier: 1, clientX: 20, clientY: 20 }],
    })
    fireEvent.touchEnd(tablist, {
      changedTouches: [{ identifier: 1, clientX: 20, clientY: 20 }],
    })

    expect(screen.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('panel-sessions')).toBeVisible()
    expect(screen.getByTestId('panel-files')).not.toBeVisible()
    expect(screen.getByTestId('panel-control')).not.toBeVisible()
  })

  it('重新打开页面仍停在会话列表，不恢复上次 tab', async () => {
    const user = userEvent.setup()
    const first = render(<App />)

    await user.click(screen.getByTestId('tab-control'))
    expect(screen.getByTestId('tab-control')).toHaveAttribute('aria-selected', 'true')
    first.unmount()

    // 整页刷新等价于重新挂载：没有可恢复的持久化 tab 状态。
    render(<App />)
    expect(screen.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('panel-sessions')).toBeVisible()
    expect(screen.getByTestId('panel-control')).not.toBeVisible()
  })

  it('点击 tab 可来回切换，控制面板的固定操作仍原样可用', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByTestId('tab-sessions'))
    expect(screen.getByTestId('panel-sessions')).toBeVisible()
    expect(screen.getByTestId('session-board')).toBeVisible()

    await user.click(screen.getByTestId('tab-control'))
    expect(screen.getByTestId('panel-control')).toBeVisible()
    expect(screen.getByTestId('panel-sessions')).not.toBeVisible()

    expect(screen.getByTestId('start-button')).toBeEnabled()
    expect(screen.getByTestId('stop-button')).toBeEnabled()
    expect(screen.getByTestId('operation-result')).toHaveTextContent('尚未提交操作。')

    // 控制面板名称不变。
    expect(screen.getByTestId('tab-control')).toHaveTextContent('控制面板')
  })

  it('整页可见标题为 omp-bot', () => {
    render(<App />)

    expect(screen.getByRole('heading', { level: 1, name: 'omp-bot' })).toBeVisible()
  })
})
