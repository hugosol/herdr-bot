import { render, screen, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'

function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response
}

type RecordedCall = { url: string; method: string; body: string | null }

function mockFetch(
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

const TEMPLATE = {
  id: 'tpl-1',
  name: '审查',
  botName: '{zone.name} · {template.name} · {bot.id}',
  agent: 'omp',
  model: 'default',
  thinking: 'default',
  skill: null,
  prompt: '',
}

const PREVIEW = {
  botId: 'a7f3c2',
  name: 'alpha · 审查 · a7f3c2',
  zone: 'C:\\work\\alpha',
  templateId: 'tpl-1',
  agent: 'omp',
}

function creation(stage: string, extra: Record<string, unknown> = {}) {
  return {
    id: 'bc-1',
    name: PREVIEW.name,
    zone: PREVIEW.zone,
    templateId: 'tpl-1',
    stage,
    inputSubmitted: stage === 'succeeded',
    ...extra,
  }
}

afterEach(() => {
  vi.unstubAllGlobals()
})

async function openDialog(user: UserEvent, rowIndex = 1) {
  await user.click(screen.getByTestId('tab-files'))
  const list = await screen.findByTestId('zone-list')
  const rows = within(list).getAllByTestId('zone-item')
  await user.click(within(rows[rowIndex]).getByTestId('zone-create-bot'))
  return screen.getByTestId('bot-creation-dialog')
}

/** 打开弹窗后创建立即进入指定的失败结果；用于检查页面侧的失败表述。 */
function mockCreationFailure(failed: Record<string, unknown>) {
  return mockFetch((url, method) => {
    if (url === '/api/zones' && method === 'GET') {
      return jsonResponse(200, { zones: [{ path: 'C:\\work\\alpha' }] })
    }
    if (url === '/api/zones/overview' && method === 'GET') {
      return jsonResponse(200, { zones: [] })
    }
    if (url === '/api/bot-templates' && method === 'GET') {
      return jsonResponse(200, {
        templates: [TEMPLATE],
        options: { agents: ['omp'], models: [], thinking: {}, skills: [] },
      })
    }
    if (url === '/api/bots/preview' && method === 'POST') {
      return jsonResponse(200, PREVIEW)
    }
    if (url === '/api/bots' && method === 'POST') {
      return jsonResponse(202, creation('starting'))
    }
    if (url === '/api/bots/bc-1' && method === 'GET') {
      return jsonResponse(200, failed)
    }
    return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
  })
}

async function submitCreation(user: UserEvent) {
  const dialog = await openDialog(user, 0)
  await vi.waitFor(() =>
    expect(within(dialog).getByTestId('bot-name-preview')).toHaveTextContent(PREVIEW.name),
  )
  await user.click(within(dialog).getByTestId('bot-create-submit'))
  return dialog
}

describe('从 zone 创建 Bot：名称预览与阶段', () => {
  it('每个 zone 行都有“+”，弹窗绑定所选 zone，并只显示后端预览的只读名称', async () => {
    const user = userEvent.setup()
    const { calls } = mockFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, {
          zones: [{ path: 'C:\\work\\alpha' }, { path: 'D:\\work\\beta' }],
        })
      }
      if (url === '/api/zones/overview' && method === 'GET') {
        return jsonResponse(200, { zones: [] })
      }
      if (url === '/api/bot-templates' && method === 'GET') {
        return jsonResponse(200, {
          templates: [TEMPLATE],
          options: { agents: ['omp', 'pi'], models: [], thinking: {}, skills: [] },
        })
      }
      if (url === '/api/bots/preview' && method === 'POST') {
        return jsonResponse(200, PREVIEW)
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await user.click(screen.getByTestId('tab-files'))
    const list = await screen.findByTestId('zone-list')
    const rows = within(list).getAllByTestId('zone-item')
    expect(within(rows[0]).getByTestId('zone-create-bot')).toHaveTextContent('+')
    expect(within(rows[1]).getByTestId('zone-create-bot')).toHaveTextContent('+')

    await user.click(within(rows[1]).getByTestId('zone-create-bot'))
    const dialog = screen.getByTestId('bot-creation-dialog')
    expect(within(dialog).getByTestId('bot-zone')).toHaveTextContent('D:\\work\\beta')

    const preview = await within(dialog).findByTestId('bot-name-preview')
    await vi.waitFor(() => expect(preview).toHaveTextContent(PREVIEW.name))
    expect(preview.tagName).toBe('OUTPUT')
    // 名称没有手工覆盖入口：弹窗只有模板选择与初始 input。
    expect(dialog.querySelector('input')).toBeNull()
    expect(within(dialog).queryByRole('textbox', { name: /Bot 名称/ })).toBeNull()

    const previewCall = calls.find((call) => call.url === '/api/bots/preview')
    expect(previewCall?.method).toBe('POST')
    expect(JSON.parse(previewCall?.body ?? '{}')).toEqual({
      zone: 'D:\\work\\beta',
      templateId: 'tpl-1',
    })
  })

  it('接受后分别呈现正在启动与正在提交，两者确认后才显示成功并保留在弹窗', async () => {
    const user = userEvent.setup()
    let polls = 0
    const { calls } = mockFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, { zones: [{ path: 'C:\\work\\alpha' }] })
      }
      if (url === '/api/zones/overview' && method === 'GET') {
        return jsonResponse(200, { zones: [] })
      }
      if (url === '/api/bot-templates' && method === 'GET') {
        return jsonResponse(200, {
          templates: [TEMPLATE],
          options: { agents: ['omp'], models: [], thinking: {}, skills: [] },
        })
      }
      if (url === '/api/bots/preview' && method === 'POST') {
        return jsonResponse(200, PREVIEW)
      }
      if (url === '/api/bots' && method === 'POST') {
        return jsonResponse(202, creation('starting'))
      }
      if (url === '/api/bots/bc-1' && method === 'GET') {
        polls += 1
        return jsonResponse(
          200,
          polls === 1
            ? creation('submitting')
            : creation('succeeded', {
                tabId: 'w-target:t9',
                paneId: 'w-target:p9',
                agentName: 'bot-000000000009',
              }),
        )
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    const dialog = await openDialog(user, 0)
    await user.type(within(dialog).getByTestId('bot-input'), '检查登录逻辑')
    await vi.waitFor(() =>
      expect(within(dialog).getByTestId('bot-name-preview')).toHaveTextContent(PREVIEW.name),
    )
    await user.click(within(dialog).getByTestId('bot-create-submit'))

    expect(await within(dialog).findByText('正在启动')).toBeVisible()
    expect(await within(dialog).findByText('正在提交初始指示')).toBeVisible()
    expect(
      await within(dialog).findByText('创建成功，初始指示已提交'),
    ).toBeVisible()
    expect(within(dialog).getByTestId('bot-result-tab')).toHaveTextContent('w-target:t9')
    expect(within(dialog).getByTestId('bot-result-pane')).toHaveTextContent('w-target:p9')

    // 结果留在创建弹窗，文件列表仍是当前 tab；只有用户关闭才收起。
    expect(screen.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('bot-creation-dialog')).toBeVisible()
    await user.click(within(dialog).getByTestId('bot-create-close'))
    expect(screen.queryByTestId('bot-creation-dialog')).toBeNull()

    const createCall = calls.find((call) => call.url === '/api/bots')
    const body = JSON.parse(createCall?.body ?? '{}')
    expect(body).toEqual({
      zone: 'C:\\work\\alpha',
      templateId: 'tpl-1',
      botId: PREVIEW.botId,
      expectedName: PREVIEW.name,
      input: '检查登录逻辑',
    })
    expect(body).not.toHaveProperty('workspace')
  })

  it('就绪或提交任一失败都不显示成功，并保留已知定位信息', async () => {
    const user = userEvent.setup()
    mockFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, { zones: [{ path: 'C:\\work\\alpha' }] })
      }
      if (url === '/api/zones/overview' && method === 'GET') {
        return jsonResponse(200, { zones: [] })
      }
      if (url === '/api/bot-templates' && method === 'GET') {
        return jsonResponse(200, {
          templates: [TEMPLATE],
          options: { agents: ['omp'], models: [], thinking: {}, skills: [] },
        })
      }
      if (url === '/api/bots/preview' && method === 'POST') {
        return jsonResponse(200, PREVIEW)
      }
      if (url === '/api/bots' && method === 'POST') {
        return jsonResponse(202, creation('starting'))
      }
      if (url === '/api/bots/bc-1' && method === 'GET') {
        return jsonResponse(
          200,
          creation('failed', {
            tabId: 'w-target:t9',
            paneId: 'w-target:p9',
            failure: {
              stage: 'start',
              code: 'herdr-command-failed',
              message: 'agent 没有在时限内就绪。',
            },
          }),
        )
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    const dialog = await openDialog(user, 0)
    await vi.waitFor(() =>
      expect(within(dialog).getByTestId('bot-name-preview')).toHaveTextContent(PREVIEW.name),
    )
    await user.click(within(dialog).getByTestId('bot-create-submit'))

    const failure = await within(dialog).findByTestId('bot-create-failure')
    expect(failure).toHaveTextContent('启动 agent')
    expect(failure).toHaveTextContent('agent 没有在时限内就绪。')
    expect(within(dialog).queryByTestId('bot-create-success')).toBeNull()
    expect(within(dialog).getByTestId('bot-result-tab')).toHaveTextContent('w-target:t9')
    expect(within(dialog).getByTestId('bot-result-pane')).toHaveTextContent('w-target:p9')
    expect(screen.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')
  })

  it('提交被明确拒绝时说明现场已存在与提交失败，而不是结果未确认', async () => {
    const user = userEvent.setup()
    mockCreationFailure(
      creation('failed', {
        tabId: 'w-target:t9',
        paneId: 'w-target:p9',
        agentName: 'bot-000000000009',
        failure: {
          stage: 'submit',
          code: 'herdr-command-failed',
          message:
            'agent 已就绪，但初始指示的提交被明确拒绝。Tab、Pane 与 agent 已保留。',
        },
      }),
    )

    render(<App />)
    const dialog = await submitCreation(user)

    const failure = await within(dialog).findByTestId('bot-create-failure')
    expect(failure).toHaveTextContent('提交初始指示')
    expect(failure).toHaveTextContent(
      '现场已存在：agent 已就绪，但初始指示的提交被明确拒绝；不会自动重发。',
    )
    expect(failure).not.toHaveTextContent('投递结果未确认')
    expect(within(dialog).queryByTestId('bot-create-success')).toBeNull()
    expect(within(dialog).getByTestId('bot-result-tab')).toHaveTextContent('w-target:t9')
    expect(within(dialog).getByTestId('bot-result-pane')).toHaveTextContent('w-target:p9')
    expect(within(dialog).getByTestId('bot-result-agent')).toHaveTextContent(
      'bot-000000000009',
    )
    expect(screen.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')
  })

  it('投递超时只表述为结果未确认，不宣称未发送也不显示成功', async () => {
    const user = userEvent.setup()
    mockCreationFailure(
      creation('failed', {
        tabId: 'w-target:t9',
        paneId: 'w-target:p9',
        agentName: 'bot-000000000009',
        failure: {
          stage: 'submit',
          code: 'herdr-timeout',
          message:
            'agent 已就绪；初始指示的投递结果未确认，不能据此断定未发送。Tab、Pane 与 agent 已保留。',
        },
      }),
    )

    render(<App />)
    const dialog = await submitCreation(user)

    const failure = await within(dialog).findByTestId('bot-create-failure')
    expect(failure).toHaveTextContent('提交初始指示')
    expect(failure).toHaveTextContent(
      '现场已存在：agent 已就绪；初始指示的投递结果未确认，不能断定它一定没有发送；不会自动重发。',
    )
    expect(failure).not.toHaveTextContent('确定未发送')
    expect(within(dialog).queryByTestId('bot-create-success')).toBeNull()
    expect(within(dialog).getByTestId('bot-result-tab')).toHaveTextContent('w-target:t9')
    expect(within(dialog).getByTestId('bot-result-pane')).toHaveTextContent('w-target:p9')
    expect(screen.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')
  })
})
