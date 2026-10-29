import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { RunDetails, selectedRunId } from '../src/RunDetails'

const initialScroll = Object.getOwnPropertyDescriptor(window, 'scrollY')!
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  Object.defineProperty(window, 'scrollY', initialScroll)
  window.history.replaceState(null, '', '/')
})

describe('Run detail navigation', () => {
  it('clears selection on the explicit list route without losing filters or the prior list scroll', async () => {
    // Only HTTP and viewport APIs are substituted: the real refresh hook and
    // mounted RunList handle the route changes, not a navigation test double.
    vi.stubGlobal('fetch', vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url === '/api/workflow/runs') return new Response(JSON.stringify({
        pollIntervalSeconds: 86400,
        runs: [
          { id: 'alpha-run', managed: true, readError: null, run: { map: 'alpha.json', status: 'running', createdAt: 2000, nodes: {} } },
          { id: 'beta-run', managed: false, readError: 'broken saved record', run: null },
        ],
      }))
      if (url === '/api/workflow/runs/beta-run') return new Response(JSON.stringify({
        id: 'beta-run', managed: false, run: null, readError: 'broken saved record', startupError: null, reports: [], pendingReports: {},
      }))
      throw new Error(`Unexpected request: ${url}`)
    }))
    const scrollTo = vi.spyOn(window, 'scrollTo').mockImplementation(() => {})
    window.history.replaceState(null, '', '/#workflow')
    expect(selectedRunId()).toBeNull()
    const { rerender } = render(<RunDetails id={selectedRunId()} active />)
    await screen.findByRole('link', { name: /beta-run/ })
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'beta' } })
    Object.defineProperty(window, 'scrollY', { configurable: true, value: 480 })
    fireEvent.scroll(window)
    const link = screen.getByRole('link', { name: /beta-run/ })
    fireEvent.click(link)
    window.history.replaceState(null, '', '/#workflow/beta-run')
    expect(selectedRunId()).toBe('beta-run')
    rerender(<RunDetails id={selectedRunId()} active />)
    await screen.findByRole('link', { name: '返回全部 Run' })
    expect(await screen.findByRole('alert')).toHaveTextContent('broken saved record')
    expect(screen.queryByRole('searchbox')).not.toBeInTheDocument()
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    Object.defineProperty(window, 'scrollY', { configurable: true, value: 0 })
    window.history.replaceState(null, '', '/#workflow')
    rerender(<RunDetails id={selectedRunId()} active />)
    expect(screen.getByRole('searchbox')).toHaveValue('beta')
    expect(screen.getByRole('link', { name: /beta-run/ })).toBeVisible()
    expect(screen.queryByRole('link', { name: /alpha-run/ })).not.toBeInTheDocument()
    expect(scrollTo).toHaveBeenLastCalledWith(0, 480)
  })
})
