import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('App（omp-bot 主界面）', () => {
  it('渲染标题、两个固定操作与初始反馈', () => {
    render(<App />)

    expect(screen.getByRole('heading', { name: 'omp-bot' })).toBeInTheDocument()
    expect(screen.getByTestId('start-button')).toBeEnabled()
    expect(screen.getByTestId('stop-button')).toBeEnabled()
    expect(screen.getByTestId('operation-result')).toHaveTextContent('尚未提交操作。')
  })

  it('点击「启动」提交请求并显示执行完成反馈', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({
        operation: 'start',
        ended: true,
        started: true,
        exitCode: 0,
        outcome: 'completed',
      }),
    })
    vi.stubGlobal('fetch', fetchMock)

    const user = userEvent.setup()
    render(<App />)

    // 页面默认停在会话列表；固定操作在控制面板。
    await user.click(screen.getByTestId('tab-control'))
    await user.click(screen.getByTestId('start-button'))

    expect(await screen.findByTestId('operation-result')).toHaveTextContent(
      '启动脚本调用已结束：脚本执行完成（退出码 0）。',
    )
    expect(fetchMock).toHaveBeenCalledWith('/api/operations/start', { method: 'POST' })
  })
})
