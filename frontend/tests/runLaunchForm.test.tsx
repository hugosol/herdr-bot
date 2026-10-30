import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, expect, it, vi } from 'vitest'
import { RunLaunchForm } from '../src/RunLaunchForm'
import type { Task } from '../src/taskApi'

const options = { agents: ['omp', 'pi'], models: ['default'], thinking: {}, skills: [] }
const tasks: Task[] = (['omp', 'pi'] as const).flatMap(agent =>
  (['normal', 'manager'] as const).map(taskType => ({
    id: `${agent}-${taskType}`, name: `${agent} ${taskType}`, agent, taskType,
    chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: '', extensions: [],
  })))

function response(body: unknown): Response {
  return { ok: true, status: 200, json: async () => body } as Response
}

afterEach(() => { vi.unstubAllGlobals(); window.location.hash = '' })

it('lists both hosts’ manager Tasks only and sends the selected Pi manager unchanged to preflight and launch', async () => {
  const user = userEvent.setup()
  const calls: { url: string; body: unknown }[] = []
  vi.stubGlobal('fetch', vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    calls.push({ url, body: init?.body ? JSON.parse(String(init.body)) : null })
    if (url === '/api/tasks') return response({ tasks, options })
    if (url === '/api/zones') return response({ zones: [{ path: 'C:/zones/pi-work' }] })
    if (url === '/api/scheduler/maps/mixed.json/preflight') return response({ workspaceName: 'pi-work' })
    if (url === '/api/scheduler/maps/mixed.json/runs') return response({ id: 'pi-run' })
    throw new Error(`Unexpected request ${url}`)
  }))
  render(<RunLaunchForm file="mixed.json" active onClose={vi.fn()} onNode={vi.fn()} onBot={vi.fn()} onTask={vi.fn()} onZones={vi.fn()} />)
  const manager = screen.getByRole('combobox', { name: '管理者 Task' })
  await waitFor(() => expect(manager).not.toBeDisabled())
  expect(within(manager).getAllByRole('option').map(option => option.textContent)).toEqual(['请选择管理者 Task', 'omp manager', 'pi manager'])
  await user.selectOptions(manager, 'pi-manager')
  await user.selectOptions(screen.getByRole('combobox', { name: '管理者 zone' }), 'C:/zones/pi-work')
  await user.type(screen.getByRole('textbox', { name: '本次要求' }), 'Pi 原始要求')
  await user.click(screen.getByRole('button', { name: '校验启动配置' }))
  expect(await screen.findByRole('status')).toHaveTextContent('配置校验通过')
  await user.click(screen.getByRole('button', { name: '启动 Run' }))
  await waitFor(() => expect(window.location.hash).toBe('#workflow/pi-run'))
  expect(calls.filter(call => call.body !== null)).toEqual([
    { url: '/api/scheduler/maps/mixed.json/preflight', body: { managerTaskId: 'pi-manager', zone: 'C:/zones/pi-work', userInput: 'Pi 原始要求' } },
    { url: '/api/scheduler/maps/mixed.json/runs', body: { managerTaskId: 'pi-manager', zone: 'C:/zones/pi-work', userInput: 'Pi 原始要求' } },
  ])
})
