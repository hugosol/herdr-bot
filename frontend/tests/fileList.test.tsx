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

/** 记录 zone 请求并按用例给定的答复返回；未覆盖的地址一律视为错误。 */
function mockZoneFetch(
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

describe('文件列表：zone 登记', () => {
  it('进入文件列表才读取后端共享列表，不显示客户端文件选择器', async () => {
    const user = userEvent.setup()
    const { calls } = mockZoneFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, {
          zones: [{ path: 'C:\\work\\alpha' }, { path: 'D:\\projects\\beta' }],
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    expect(calls).toHaveLength(0)
    expect(document.querySelector('input[type="file"]')).toBeNull()

    await openFileList(user)

    const list = await screen.findByTestId('zone-list')
    expect(within(list).getAllByTestId('zone-item')).toHaveLength(2)
    expect(within(list).getByText('C:\\work\\alpha')).toBeVisible()
    expect(within(list).getByText('D:\\projects\\beta')).toBeVisible()
    // 整行就是浏览入口：路径与角标位于同一个按钮内，不再有独立的「浏览」按钮。
    const row = within(list).getByRole('button', { name: '浏览 C:\\work\\alpha' })
    expect(row).toHaveTextContent('C:\\work\\alpha')
    expect(within(list).queryByRole('button', { name: '浏览' })).toBeNull()
    expect(calls[0]).toMatchObject({ url: '/api/zones', method: 'GET' })
    expect(document.querySelector('input[type="file"]')).toBeNull()
  })

  it('直接显示 zone 列表，登记与新增都只在弹窗中输入', async () => {
    const user = userEvent.setup()
    mockZoneFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, {
          zones: [{ path: 'C:\\work\\alpha' }],
          creation: {
            available: true,
            parentDirectory: 'E:\\Developer\\bot-playground',
          },
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    await screen.findByText('C:\\work\\alpha')

    expect(
      screen.queryByText(/这里的路径指 omp-bot 服务所在电脑上的目录/),
    ).toBeNull()
    expect(screen.queryByTestId('zone-path')).toBeNull()

    await user.click(screen.getByTestId('register-open'))
    const registerDialog = screen.getByTestId('register-dialog')
    expect(registerDialog).toBeVisible()
    expect(within(registerDialog).getByLabelText('服务器目录路径')).toBeVisible()
    await user.click(screen.getByTestId('register-cancel'))
    expect(screen.queryByTestId('register-dialog')).toBeNull()
    expect(screen.queryByTestId('zone-path')).toBeNull()

    await user.click(screen.getByTestId('create-open'))
    const createDialog = screen.getByTestId('create-dialog')
    expect(createDialog).toBeVisible()
    expect(within(createDialog).getByLabelText('新 zone 名称')).toBeVisible()
    expect(createDialog).toHaveTextContent('E:\\Developer\\bot-playground')
    await user.click(screen.getByTestId('create-cancel'))
    expect(screen.queryByTestId('create-dialog')).toBeNull()
  })

  it('未配置创建父目录时禁用新增并给出明确提示', async () => {
    const user = userEvent.setup()
    mockZoneFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, {
          zones: [],
          creation: { available: false },
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    await screen.findByText('尚未登记 zone。')

    expect(screen.getByTestId('create-open')).toBeDisabled()
    expect(screen.getByTestId('create-unavailable')).toHaveTextContent(
      'create_parent_directory',
    )
    expect(screen.getByTestId('register-open')).toBeEnabled()
  })

  it('新增 zone 在弹窗确认后才发送名称并显示创建路径', async () => {
    const user = userEvent.setup()
    const reply = deferredReply()
    const { calls } = mockZoneFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, {
          zones: [],
          creation: {
            available: true,
            parentDirectory: 'E:\\Developer\\bot-playground',
          },
        })
      }
      if (url === '/api/zones/create' && method === 'POST') {
        return reply.promise
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    await screen.findByText('尚未登记 zone。')

    await user.click(screen.getByTestId('create-open'))
    await user.type(screen.getByLabelText('新 zone 名称'), 'new-project')
    await user.click(screen.getByTestId('create-zone'))

    expect(screen.queryByTestId('zone-feedback')).toBeNull()
    expect(screen.getByTestId('create-zone')).toBeDisabled()

    reply.settle(
      jsonResponse(200, {
        zones: [{ path: 'E:\\Developer\\bot-playground\\new-project' }],
        creation: {
          available: true,
          parentDirectory: 'E:\\Developer\\bot-playground',
        },
      }),
    )

    const feedback = await screen.findByTestId('zone-feedback')
    expect(feedback).toHaveAttribute('data-kind', 'success')
    expect(feedback).toHaveTextContent(
      '已创建：E:\\Developer\\bot-playground\\new-project',
    )
    expect(screen.queryByTestId('create-dialog')).toBeNull()
    expect(
      screen.getByText('E:\\Developer\\bot-playground\\new-project'),
    ).toBeVisible()

    const post = calls.find(
      (call) => call.url === '/api/zones/create' && call.method === 'POST',
    )
    expect(post).toBeDefined()
    expect(JSON.parse(post!.body!)).toEqual({ name: 'new-project' })
  })

  it('新增失败时保留弹窗并显示后端给出的失败阶段与完整目录', async () => {
    const user = userEvent.setup()
    const keptPath = 'E:\\Developer\\bot-playground\\kept-project'
    mockZoneFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, {
          zones: [{ path: 'C:\\work\\alpha' }],
          creation: {
            available: true,
            parentDirectory: 'E:\\Developer\\bot-playground',
          },
        })
      }
      if (url === '/api/zones/create' && method === 'POST') {
        return jsonResponse(500, {
          code: 'zone-registration-failed',
          message: `目录与 Git 初始化已完成，但 zone 登记失败，目录已保留：${keptPath}；原登记文件未被修改。`,
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    await screen.findByText('C:\\work\\alpha')

    await user.click(screen.getByTestId('create-open'))
    await user.type(screen.getByLabelText('新 zone 名称'), 'kept-project')
    await user.click(screen.getByTestId('create-zone'))

    const feedback = await screen.findByTestId('zone-feedback')
    expect(feedback).toHaveAttribute('data-kind', 'error')
    expect(feedback).toHaveTextContent('目录与 Git 初始化已完成')
    expect(feedback).toHaveTextContent(keptPath)
    expect(screen.getByTestId('create-dialog')).toBeVisible()
    expect(screen.queryByText('已创建')).toBeNull()
    expect(screen.getByText('C:\\work\\alpha')).toBeVisible()
  })

  it('登记成功后显示成功并更新列表；答复到达之前不显示成功', async () => {
    const user = userEvent.setup()
    const reply = deferredReply()
    const { calls } = mockZoneFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, { zones: [] })
      }
      if (url === '/api/zones' && method === 'POST') {
        return reply.promise
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    expect(await screen.findByText('尚未登记 zone。')).toBeVisible()

    await user.click(screen.getByTestId('register-open'))
    await user.type(screen.getByLabelText('服务器目录路径'), 'E:\\work\\new-project')
    await user.click(screen.getByTestId('register-zone'))

    // 后端还没有确认持久化：页面不能抢先显示成功。
    expect(screen.queryByTestId('zone-feedback')).toBeNull()
    expect(screen.getByTestId('register-zone')).toBeDisabled()

    reply.settle(
      jsonResponse(200, { zones: [{ path: 'E:\\work\\new-project' }] }),
    )

    const feedback = await screen.findByTestId('zone-feedback')
    expect(feedback).toHaveAttribute('data-kind', 'success')
    expect(feedback).toHaveTextContent('已登记')
    expect(screen.queryByText('尚未登记 zone。')).toBeNull()
    expect(screen.getByText('E:\\work\\new-project')).toBeVisible()

    const post = calls.find((call) => call.url === '/api/zones' && call.method === 'POST')
    expect(post).toBeDefined()
    expect(JSON.parse(post!.body!)).toEqual({ path: 'E:\\work\\new-project' })
  })

  it('非法路径被后端拒绝时显示明确失败，不显示成功也不改动列表', async () => {
    const user = userEvent.setup()
    mockZoneFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, { zones: [{ path: 'C:\\work\\alpha' }] })
      }
      if (url === '/api/zones' && method === 'POST') {
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

    await user.click(screen.getByTestId('register-open'))
    await user.type(screen.getByLabelText('服务器目录路径'), 'C:\\work\\missing')
    await user.click(screen.getByTestId('register-zone'))

    const feedback = await screen.findByTestId('zone-feedback')
    expect(feedback).toHaveAttribute('data-kind', 'error')
    expect(feedback).toHaveTextContent('该路径在服务器上不存在。')
    expect(feedback).not.toHaveTextContent('已登记')
    expect(screen.getByText('C:\\work\\alpha')).toBeVisible()
  })

  it('存储写入失败显示失败并保留原列表，不把失败当成成功', async () => {
    const user = userEvent.setup()
    mockZoneFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, { zones: [{ path: 'C:\\work\\alpha' }] })
      }
      if (url === '/api/zones' && method === 'POST') {
        return jsonResponse(500, {
          code: 'registry-write-failed',
          message: '保存 zone 登记列表失败：拒绝访问。原登记文件未被修改。',
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    await screen.findByText('C:\\work\\alpha')

    await user.click(screen.getByTestId('register-open'))
    await user.type(screen.getByLabelText('服务器目录路径'), 'C:\\work\\second')
    await user.click(screen.getByTestId('register-zone'))

    const feedback = await screen.findByTestId('zone-feedback')
    expect(feedback).toHaveAttribute('data-kind', 'error')
    expect(feedback).toHaveTextContent('原登记文件未被修改')
    expect(screen.getByText('C:\\work\\alpha')).toBeVisible()
    expect(screen.queryByText('C:\\work\\second')).toBeNull()
  })

  it('列表读取失败显示错误，不伪装成空列表', async () => {
    const user = userEvent.setup()
    mockZoneFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(500, {
          code: 'registry-corrupt',
          message: 'zone 登记文件内容已损坏：expected value。未覆盖原文件。',
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)

    const error = await screen.findByTestId('zone-list-error')
    expect(error).toHaveTextContent('zone 登记文件内容已损坏')
    expect(screen.queryByText('尚未登记 zone。')).toBeNull()
    expect(screen.queryByTestId('zone-list')).toBeNull()
  })

  it('取消登记只移除登记项，并明确目录与文件未删除', async () => {
    const user = userEvent.setup()
    const { calls } = mockZoneFetch((url, method, body) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, {
          zones: [{ path: 'C:\\work\\alpha' }, { path: 'D:\\projects\\beta' }],
        })
      }
      if (url === '/api/zones/remove' && method === 'POST') {
        const path = JSON.parse(body!).path as string
        return jsonResponse(200, {
          zones: [{ path: 'C:\\work\\alpha' }, { path: 'D:\\projects\\beta' }].filter(
            (entry) => entry.path !== path,
          ),
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    await screen.findByText('C:\\work\\alpha')

    const removeButton = screen.getByRole('button', { name: '取消登记 C:\\work\\alpha' })
    expect(removeButton).toHaveTextContent('×')
    await user.click(removeButton)

    const feedback = await screen.findByTestId('zone-feedback')
    expect(feedback).toHaveAttribute('data-kind', 'success')
    expect(feedback).toHaveTextContent('已取消登记：C:\\work\\alpha')
    expect(feedback).toHaveTextContent('目录与文件未删除')
    expect(screen.queryByText('C:\\work\\alpha')).toBeNull()
    expect(screen.getByText('D:\\projects\\beta')).toBeVisible()

    const remove = calls.find((call) => call.url === '/api/zones/remove')
    expect(remove).toBeDefined()
    expect(JSON.parse(remove!.body!)).toEqual({ path: 'C:\\work\\alpha' })
  })

  it('请求没有响应时给出未确认反馈，不冒充成功或失败', async () => {
    const user = userEvent.setup()
    mockZoneFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, { zones: [] })
      }
      if (url === '/api/zones' && method === 'POST') {
        return Promise.reject(new Error('connection reset'))
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openFileList(user)
    await screen.findByText('尚未登记 zone。')

    await user.click(screen.getByTestId('register-open'))
    await user.type(screen.getByLabelText('服务器目录路径'), 'E:\\work\\new-project')
    await user.click(screen.getByTestId('register-zone'))

    const feedback = await screen.findByTestId('zone-feedback')
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
    mockZoneFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        const zones = reads[Math.min(read, reads.length - 1)]
        read += 1
        return jsonResponse(200, { zones })
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
