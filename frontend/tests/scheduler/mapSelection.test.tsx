import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { SchedulerPanel } from '../../src/scheduler/SchedulerPanel'

const map = (title: string) => ({ title, description: '', nodes: {} })
const response = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status })
afterEach(() => vi.unstubAllGlobals())

it('keeps Map navigation and shared definitions stable while loading, ignores stale responses, and still refreshes explicitly', async () => {
  const pending = new Map<string, (response: Response) => void>()
  const fetcher = vi.fn((url: string) => {
    if (url === '/api/scheduler/maps') return Promise.resolve(response({ files: ['first.json', 'second.json', 'third.json'] }))
    if (url === '/api/scheduler/bots') return Promise.resolve(response({ bots: [] }))
    if (url === '/api/scheduler/maps/first.json') return Promise.resolve(response(map('First map')))
    return new Promise<Response>(resolve => { pending.set(url, resolve) })
  })
  vi.stubGlobal('fetch', fetcher)
  render(<SchedulerPanel active view="map" onViewChange={vi.fn()} onTask={vi.fn()} onZones={vi.fn()} />)
  const first = await screen.findByRole('button', { name: 'first.json' })
  await waitFor(() => expect(screen.queryByText('正在读取工作定义……')).not.toBeInTheDocument())
  fireEvent.click(first)
  await screen.findByRole('heading', { name: 'First map' })
  const filter = screen.getByRole('searchbox', { name: '筛选 Map' })
  fireEvent.change(filter, { target: { value: '.json' } })
  fireEvent.click(screen.getByRole('button', { name: 'second.json' }))
  expect(screen.getByText('正在读取 Map……')).toHaveAttribute('role', 'status')
  expect(screen.queryByText('正在读取工作定义……')).not.toBeInTheDocument()
  expect(screen.queryByText('正在读取共享 Bot 目录……')).not.toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'first.json' })).toBe(first)
  expect(filter).toHaveValue('.json')
  expect(fetcher.mock.calls.filter(([url]) => url === '/api/scheduler/bots')).toHaveLength(1)
  expect(fetcher.mock.calls.filter(([url]) => url === '/api/scheduler/maps')).toHaveLength(1)
  fireEvent.click(screen.getByRole('button', { name: 'third.json' }))
  await act(async () => pending.get('/api/scheduler/maps/third.json')!(response(map('Third map'))))
  await screen.findByRole('heading', { name: 'Third map' })
  await act(async () => pending.get('/api/scheduler/maps/second.json')!(response(map('Second map'))))
  expect(screen.queryByRole('heading', { name: 'Second map' })).not.toBeInTheDocument()
  expect(screen.getByRole('heading', { name: 'Third map' })).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: '刷新全部' }))
  await act(async () => pending.get('/api/scheduler/maps/third.json')!(response(map('Updated third map'))))
  await screen.findByRole('heading', { name: 'Updated third map' })
  expect(fetcher.mock.calls.filter(([url]) => url === '/api/scheduler/bots')).toHaveLength(2)
  expect(fetcher.mock.calls.filter(([url]) => url === '/api/scheduler/maps')).toHaveLength(2)
})

it('recovers an unfinished Bot catalog when selecting a Map cancels initial loading', async () => {
  let finishBots!: (response: Response) => void
  const fetcher = vi.fn((url: string) => {
    if (url === '/api/scheduler/maps') return Promise.resolve(response({ files: ['first.json'] }))
    if (url === '/api/scheduler/bots') {
      return new Promise<Response>(resolve => { finishBots = resolve })
    }
    return Promise.resolve(response(map('First map')))
  })
  vi.stubGlobal('fetch', fetcher)
  render(<SchedulerPanel active view="map" onViewChange={vi.fn()} onTask={vi.fn()} onZones={vi.fn()} />)
  fireEvent.click(await screen.findByRole('button', { name: 'first.json' }))
  await act(async () => finishBots(response({ bots: [] })))
  await screen.findByRole('heading', { name: 'First map' })
  expect(screen.queryByText('正在读取共享 Bot 目录……')).not.toBeInTheDocument()
  expect(screen.getByRole('button', { name: '刷新全部' })).toBeEnabled()
})
