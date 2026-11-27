import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, expect, it, vi } from 'vitest'
import { RunLaunchForm } from '../../src/workflow/launch/RunLaunchForm'
import type { Task } from '../../src/controllers/taskController'

const options = { agents: ['omp', 'pi'], models: ['default'], thinking: {}, skills: [] }
const tasks: Task[] = (['omp', 'pi'] as const).flatMap(agent =>
  (['normal', 'manager', 'chief'] as const).map(taskType => ({
    id: `${agent}-${taskType}`, name: `${agent} ${taskType}`, agent, taskType,
    chatName: '{chat.id}', model: 'default', thinking: 'default', skill: null, prompt: '',
  })))

function response(body: unknown): Response {
  return { ok: true, status: 200, json: async () => body } as Response
}

afterEach(() => { vi.unstubAllGlobals(); window.location.hash = '' })

it('sends GitHub clone fields separately from 本次要求 to both preflight and launch', async () => {
  const user = userEvent.setup()
  const file = 'github-repository-exploration.json'
  const calls: { url: string; body: unknown }[] = []
  vi.stubGlobal('fetch', vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    if (url === '/api/tasks') return response({ tasks, options })
    if (url === '/api/zones') return response({ zones: [{ id: 'test/pi-work', path: 'C:/zones/pi-work' }] })
    calls.push({ url, body: JSON.parse(String(init?.body)) })
    if (url === `/api/scheduler/maps/${file}/preflight`) return response({ workspaceName: 'pi-work' })
    if (url === `/api/scheduler/maps/${file}/runs`) return response({ id: 'github-run' })
    throw new Error(`Unexpected request ${url}`)
  }))
  const { container } = render(<RunLaunchForm file={file} declarations={[{ id: 'clone_request', required: true }]} active onClose={vi.fn()} onNode={vi.fn()} onBot={vi.fn()} onTask={vi.fn()} onZones={vi.fn()} />)
  const manager = screen.getByRole('combobox', { name: '管理者 Task' })
  await waitFor(() => expect(manager).not.toBeDisabled())
  await user.selectOptions(manager, 'pi-manager')
  await user.selectOptions(screen.getByRole('combobox', { name: '管理者 zone' }), 'test/pi-work')
  const link = screen.getByRole('textbox', { name: 'GitHub 仓库链接' })
  const parent = screen.getByRole('textbox', { name: '克隆父目录' })
  const preflight = screen.getByRole('button', { name: '校验启动配置' })
  const launch = screen.getByRole('button', { name: '启动 Run' })
  expect(link).toBeRequired()
  expect(parent).toBeRequired()
  await user.click(preflight)
  expect(screen.getByRole('alert')).toHaveTextContent('请填写 GitHub 仓库链接。')
  expect(link).toHaveFocus()
  await user.type(parent, 'D:/repos')
  await user.click(launch)
  expect(link).toHaveFocus()
  await user.type(link, 'https://github.com/example/repo')
  await user.clear(parent)
  await user.type(parent, '   ')
  await user.click(launch)
  expect(screen.getByRole('alert')).toHaveTextContent('请填写克隆父目录。')
  expect(parent).toHaveFocus()
  expect(calls).toEqual([])
  await user.clear(parent)
  await user.type(parent, 'D:/repos')
  const requirements = screen.getByRole('textbox', { name: '本次要求' })
  expect(container.querySelectorAll('textarea')).toHaveLength(1)
  expect(requirements.tagName).toBe('TEXTAREA')
  await user.type(requirements, '  **梳理 CLI 入口**\n\n只读检查。  ')
  await user.click(screen.getByRole('button', { name: '校验启动配置' }))
  expect(await screen.findByRole('status')).toHaveTextContent('配置校验通过')
  await user.click(screen.getByRole('button', { name: '启动 Run' }))
  await waitFor(() => expect(window.location.hash).toBe('#workflow/github-run'))
  const body = {
    managerTaskId: 'pi-manager', zone: 'test/pi-work', userInput: '  **梳理 CLI 入口**\n\n只读检查。  ',
    githubExploration: { githubUrl: 'https://github.com/example/repo', parentDirectory: 'D:/repos' },
  }
  expect(calls).toEqual([
    { url: '/api/scheduler/maps/github-repository-exploration.json/preflight', body },
    { url: '/api/scheduler/maps/github-repository-exploration.json/runs', body },
  ])
})

it('lists both hosts’ manager Tasks only and sends the selected Pi manager unchanged to preflight and launch', async () => {
  const user = userEvent.setup()
  const calls: { url: string; body: unknown }[] = []
  vi.stubGlobal('fetch', vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    calls.push({ url, body: init?.body ? JSON.parse(String(init.body)) : null })
    if (url === '/api/tasks') return response({ tasks, options })
    if (url === '/api/zones') return response({ zones: [{ id: 'test/pi-work', path: 'C:/zones/pi-work' }] })
    if (url === '/api/scheduler/maps/mixed.json/preflight') return response({ workspaceName: 'pi-work' })
    if (url === '/api/scheduler/maps/mixed.json/runs') return response({ id: 'pi-run' })
    throw new Error(`Unexpected request ${url}`)
  }))
  render(<RunLaunchForm file="mixed.json" declarations={[{ id: 'brief', required: true, description: '业务委托及约束。' }]} requirements={'# 输入材料\n\n请提供 **背景**。\n\n<script>window.unsafe = true</script>'} active onClose={vi.fn()} onNode={vi.fn()} onBot={vi.fn()} onTask={vi.fn()} onZones={vi.fn()} />)
  expect(screen.getByRole('heading', { name: '输入材料' })).toBeInTheDocument()
  expect(screen.getByText('背景').tagName).toBe('STRONG')
  expect(document.querySelector('script')).toBeNull()
  expect(screen.getByRole('textbox', { name: /业务委托及约束/ })).not.toBeRequired()
  const manager = screen.getByRole('combobox', { name: '管理者 Task' })
  await waitFor(() => expect(manager).not.toBeDisabled())
  expect(within(manager).getAllByRole('option').map(option => option.textContent)).toEqual(['请选择管理者 Task', 'omp manager', 'pi manager'])
  await user.selectOptions(manager, 'pi-manager')
  await user.selectOptions(screen.getByRole('combobox', { name: '管理者 zone' }), 'test/pi-work')
  await user.type(screen.getByRole('textbox', { name: '本次要求' }), 'Pi 原始要求')
  await user.click(screen.getByRole('button', { name: '校验启动配置' }))
  expect(await screen.findByRole('status')).toHaveTextContent('配置校验通过')
  await user.click(screen.getByRole('button', { name: '启动 Run' }))
  await waitFor(() => expect(window.location.hash).toBe('#workflow/pi-run'))
  expect(calls.filter(call => call.body !== null)).toEqual([
    { url: '/api/scheduler/maps/mixed.json/preflight', body: { managerTaskId: 'pi-manager', zone: 'test/pi-work', userInput: 'Pi 原始要求' } },
    { url: '/api/scheduler/maps/mixed.json/runs', body: { managerTaskId: 'pi-manager', zone: 'test/pi-work', userInput: 'Pi 原始要求' } },
  ])
})
