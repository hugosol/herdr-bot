import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { RunDetail } from '../src/workflow/workflowApi'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { RunDetails, selectedRunId } from '../src/workflow/RunDetails'

const initialScroll = Object.getOwnPropertyDescriptor(window, 'scrollY')!
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  Object.defineProperty(window, 'scrollY', initialScroll)
  window.history.replaceState(null, '', '/')
})

function savedDetail(id: string, name?: string): RunDetail {
  return { id, managed: false, readError: null, startupError: null, reports: [], pendingReports: {}, run: {
    id, name, map: 'alpha.json', createdAt: 2000, managerTaskId: 'manager', zone: 'zone', workspaceName: 'workspace', userInput: 'request', status: 'completed', manager: null, nodes: {},
  } }
}

function renameRequests(patch: (id: string, name: string) => Promise<Response>) {
  const records: Record<string, RunDetail> = { first: savedDetail('first'), second: savedDetail('second', 'Second name') }
  const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    if (init?.method === 'PATCH') {
      const id = url.split('/').at(-1)!
      const response = await patch(id, JSON.parse(String(init.body)).name)
      if (response.ok) records[id] = await response.clone().json()
      return response
    }
    if (url === '/api/workflow/runs') return new Response(JSON.stringify({ pollIntervalSeconds: 86400, runs: Object.values(records) }))
    if (url.startsWith('/api/workflow/runs/')) return new Response(JSON.stringify(records[url.split('/').at(-1)!]))
    if (url.startsWith('/api/scheduler/maps/')) return new Response(JSON.stringify({ title: 'Map title', description: '', nodes: {} }))
    return new Response(JSON.stringify({ message: 'Catalog unavailable' }), { status: 500 })
  })
  vi.stubGlobal('fetch', fetcher)
  vi.spyOn(window, 'scrollTo').mockImplementation(() => {})
  return fetcher
}

describe('Run rename', () => {
  it('cancels without a write, rejects blank names and retains a failed draft before saving a trimmed duplicate name', async () => {
    const patch = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify({ message: 'Disk unavailable' }), { status: 500 }))
      .mockImplementation(async (id: string, name: string) => new Response(JSON.stringify(savedDetail(id, name))))
    renameRequests(patch)
    const { rerender } = render(<RunDetails id="first" active />)
    await screen.findByRole('heading', { level: 1, name: 'Map title' })
    fireEvent.click(screen.getByRole('button', { name: '重命名' }))
    expect(screen.getByRole('textbox', { name: 'Run 名称' })).toHaveValue('Map title')
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Discard me' } })
    fireEvent.click(screen.getByRole('button', { name: '取消' }))
    expect(patch).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: '重命名' }))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: '   ' } })
    fireEvent.click(screen.getByRole('button', { name: '保存' }))
    expect(screen.getByText('名称不能为空。')).toHaveAttribute('role', 'alert')
    expect(patch).not.toHaveBeenCalled()
    fireEvent.change(screen.getByRole('textbox'), { target: { value: '  Second name  ' } })
    fireEvent.click(screen.getByRole('button', { name: '保存' }))
    expect(await screen.findByText('Disk unavailable')).toHaveAttribute('role', 'alert')
    expect(screen.getByRole('textbox')).toHaveValue('  Second name  ')
    fireEvent.click(screen.getByRole('button', { name: '保存' }))
    await screen.findByRole('heading', { level: 1, name: 'Second name' })
    expect(patch).toHaveBeenLastCalledWith('first', 'Second name')
    rerender(<RunDetails id={null} active />)
    await screen.findByRole('link', { name: /Second name.*first/ })
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
  })

  it('ignores a late rename result after the selected Run changes', async () => {
    let resolve!: (response: Response) => void
    const patch = vi.fn(() => new Promise<Response>(done => { resolve = done }))
    renameRequests(patch)
    const { rerender } = render(<RunDetails id="first" active />)
    await screen.findByRole('button', { name: '重命名' })
    fireEvent.click(screen.getByRole('button', { name: '重命名' }))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Late first name' } })
    fireEvent.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() => expect(patch).toHaveBeenCalledOnce())
    rerender(<RunDetails id="second" active />)
    await screen.findByRole('heading', { level: 1, name: 'Second name' })
    fireEvent.click(screen.getByRole('button', { name: '重命名' }))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Second draft' } })
    await act(async () => { resolve(new Response(JSON.stringify(savedDetail('first', 'Late first name')))) })
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Second name')
    expect(screen.getByRole('textbox')).toHaveValue('Second draft')
  })
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
