import { render, screen, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'

/** 一个只提供客户端真正用到的成员的 JSON 答复。 */
function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response
}

/** 一个可以稍后再答复的请求：用来观察「答复到达之前」的界面。 */
function deferredReply() {
  let settle: (response: Response) => void = () => {}
  const promise = new Promise<Response>((resolve) => {
    settle = resolve
  })
  return { promise, settle }
}

type RecordedCall = { url: string; method: string; body: string | null }

/** 记录 workspace 请求并按用例给定的答复返回；未覆盖的地址一律视为错误。 */
function mockWorkspaceFetch(
  reply: (url: string, method: string, body: string | null) => Promise<Response> | Response,
) {
  const calls: RecordedCall[] = []
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    const method = init?.method ?? 'GET'
    const body = typeof init?.body === 'string' ? init.body : null
    calls.push({ url, method, body })
    return reply(url, method, body)
  })
  vi.stubGlobal('fetch', fetchMock)
  return { calls, fetchMock }
}

async function openFileList(user: UserEvent) {
  await user.click(screen.getByTestId('tab-files'))
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('文件列表：workspace 登记', () => {
  it('进入文件列表才读取后端共享列表，不显示客户端文件选择器', async () => {
    const user = userEvent.setup()
    const { calls } = mockWorkspaceFetch((url, method) => {
      if (url === '/api/workspaces' && method === 'GET') {
        return jsonResponse(200, {
          workspaces: [{ path: 'C:\\work\\alpha' }, { path: 'D:\\projects\\beta' }],
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    expect(calls).toHaveLength(0)
    expect(document.querySelector('input[type="file"]')).toBeNull()

    await openFileList(user)

    const list = await screen.findByTestId('workspace-list')
    expect(within(list).getAllByTestId('workspace-item')).toHaveLength(2)
    expect(within(list).getByText('C:\\work\\alpha')).toBeVisible()
    expect(within(list).getByText('D:\\projects\\beta')).toBeVisible()
    expect(calls[0]).toMatchObject({ url: '/api/workspaces', method: 'GET' })
    expect(document.querySelector('input[type="file"]')).toBeNull()
  })

  it('登记成功后显示成功并更新列表；答复到达之前不显示成功', async () => {
    const user = userEvent.setup()
    const reply = deferredReply()
    const { calls } = mockWorkspaceFetch((url, method) => {
      if (url === '/api/workspaces' && method === 'GET') {
        return jsonResponse(200, { workspaces: [] })
      }
      if (url === '/api/workspaces' && method === 'POST') {
        return reply.promise
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    expect(await screen.findByText('尚未登记 workspace。')).toBeVisible()

    await user.type(screen.getByLabelText('服务器目录路径'), 'E:\\work\\new-project')
    await user.click(screen.getByTestId('register-workspace'))

    // 后端还没有确认持久化：页面不能抢先显示成功。
    expect(screen.queryByTestId('workspace-feedback')).toBeNull()
    expect(screen.getByTestId('register-workspace')).toBeDisabled()

    reply.settle(
      jsonResponse(200, { workspaces: [{ path: 'E:\\work\\new-project' }] }),
    )

    const feedback = await screen.findByTestId('workspace-feedback')
    expect(feedback).toHaveAttribute('data-kind', 'success')
    expect(feedback).toHaveTextContent('已登记')
    expect(screen.queryByText('尚未登记 workspace。')).toBeNull()
    expect(screen.getByText('E:\\work\\new-project')).toBeVisible()

    const post = calls.find((call) => call.url === '/api/workspaces' && call.method === 'POST')
    expect(post).toBeDefined()
    expect(JSON.parse(post!.body!)).toEqual({ path: 'E:\\work\\new-project' })
  })

  it('非法路径被后端拒绝时显示明确失败，不显示成功也不改动列表', async () => {
    const user = userEvent.setup()
    mockWorkspaceFetch((url, method) => {
      if (url === '/api/workspaces' && method === 'GET') {
        return jsonResponse(200, { workspaces: [{ path: 'C:\\work\\alpha' }] })
      }
      if (url === '/api/workspaces' && method === 'POST') {
        return jsonResponse(400, {
          code: 'path-not-found',
          message: '该路径在服务器上不存在。',
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    await screen.findByText('C:\\work\\alpha')

    await user.type(screen.getByLabelText('服务器目录路径'), 'C:\\work\\missing')
    await user.click(screen.getByTestId('register-workspace'))

    const feedback = await screen.findByTestId('workspace-feedback')
    expect(feedback).toHaveAttribute('data-kind', 'error')
    expect(feedback).toHaveTextContent('该路径在服务器上不存在。')
    expect(feedback).not.toHaveTextContent('已登记')
    expect(screen.getByText('C:\\work\\alpha')).toBeVisible()
  })

  it('存储写入失败显示失败并保留原列表，不把失败当成成功', async () => {
    const user = userEvent.setup()
    mockWorkspaceFetch((url, method) => {
      if (url === '/api/workspaces' && method === 'GET') {
        return jsonResponse(200, { workspaces: [{ path: 'C:\\work\\alpha' }] })
      }
      if (url === '/api/workspaces' && method === 'POST') {
        return jsonResponse(500, {
          code: 'registry-write-failed',
          message: '保存 workspace 登记列表失败：拒绝访问。原登记文件未被修改。',
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    await screen.findByText('C:\\work\\alpha')

    await user.type(screen.getByLabelText('服务器目录路径'), 'C:\\work\\second')
    await user.click(screen.getByTestId('register-workspace'))

    const feedback = await screen.findByTestId('workspace-feedback')
    expect(feedback).toHaveAttribute('data-kind', 'error')
    expect(feedback).toHaveTextContent('原登记文件未被修改')
    expect(screen.getByText('C:\\work\\alpha')).toBeVisible()
    expect(screen.queryByText('C:\\work\\second')).toBeNull()
  })

  it('列表读取失败显示错误，不伪装成空列表', async () => {
    const user = userEvent.setup()
    mockWorkspaceFetch((url, method) => {
      if (url === '/api/workspaces' && method === 'GET') {
        return jsonResponse(500, {
          code: 'registry-corrupt',
          message: 'workspace 登记文件内容已损坏：expected value。未覆盖原文件。',
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)

    const error = await screen.findByTestId('workspace-list-error')
    expect(error).toHaveTextContent('workspace 登记文件内容已损坏')
    expect(screen.queryByText('尚未登记 workspace。')).toBeNull()
    expect(screen.queryByTestId('workspace-list')).toBeNull()
  })

  it('取消登记只移除登记项，并明确目录与文件未删除', async () => {
    const user = userEvent.setup()
    const { calls } = mockWorkspaceFetch((url, method, body) => {
      if (url === '/api/workspaces' && method === 'GET') {
        return jsonResponse(200, {
          workspaces: [{ path: 'C:\\work\\alpha' }, { path: 'D:\\projects\\beta' }],
        })
      }
      if (url === '/api/workspaces/remove' && method === 'POST') {
        const path = JSON.parse(body!).path as string
        return jsonResponse(200, {
          workspaces: [{ path: 'C:\\work\\alpha' }, { path: 'D:\\projects\\beta' }].filter(
            (entry) => entry.path !== path,
          ),
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    await screen.findByText('C:\\work\\alpha')

    await user.click(screen.getByRole('button', { name: '取消登记 C:\\work\\alpha' }))

    const feedback = await screen.findByTestId('workspace-feedback')
    expect(feedback).toHaveAttribute('data-kind', 'success')
    expect(feedback).toHaveTextContent('已取消登记：C:\\work\\alpha')
    expect(feedback).toHaveTextContent('目录与文件未删除')
    expect(screen.queryByText('C:\\work\\alpha')).toBeNull()
    expect(screen.getByText('D:\\projects\\beta')).toBeVisible()

    const remove = calls.find((call) => call.url === '/api/workspaces/remove')
    expect(remove).toBeDefined()
    expect(JSON.parse(remove!.body!)).toEqual({ path: 'C:\\work\\alpha' })
  })

  it('请求没有响应时给出未确认反馈，不冒充成功或失败', async () => {
    const user = userEvent.setup()
    mockWorkspaceFetch((url, method) => {
      if (url === '/api/workspaces' && method === 'GET') {
        return jsonResponse(200, { workspaces: [] })
      }
      if (url === '/api/workspaces' && method === 'POST') {
        return Promise.reject(new Error('connection reset'))
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    await screen.findByText('尚未登记 workspace。')

    await user.type(screen.getByLabelText('服务器目录路径'), 'E:\\work\\new-project')
    await user.click(screen.getByTestId('register-workspace'))

    const feedback = await screen.findByTestId('workspace-feedback')
    expect(feedback).toHaveAttribute('data-kind', 'error')
    expect(feedback).toHaveTextContent('无法确认')
    expect(feedback).not.toHaveTextContent('已登记')
    expect(screen.queryByText('E:\\work\\new-project')).toBeNull()
  })

  it('重新进入文件列表会重新读取后端列表，显示另一设备新增的登记', async () => {
    const user = userEvent.setup()
    const reads = [
      [{ path: 'C:\\work\\alpha' }],
      [{ path: 'C:\\work\\alpha' }, { path: 'D:\\projects\\from-phone' }],
    ]
    let read = 0
    mockWorkspaceFetch((url, method) => {
      if (url === '/api/workspaces' && method === 'GET') {
        const workspaces = reads[Math.min(read, reads.length - 1)]
        read += 1
        return jsonResponse(200, { workspaces })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    await screen.findByText('C:\\work\\alpha')
    expect(screen.queryByText('D:\\projects\\from-phone')).toBeNull()

    await user.click(screen.getByTestId('tab-sessions'))
    await openFileList(user)

    expect(await screen.findByText('D:\\projects\\from-phone')).toBeVisible()
    expect(read).toBeGreaterThanOrEqual(2)
  })
})
