import { render, screen, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import App from '../src/App'
import { BotCreationDialog } from '../src/BotCreationDialog'
import { renderBotNameExample } from '../src/botTemplateNaming'

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

const TEMPLATE = { id: 'tpl-1',
name: '审查',
botName: '{zone.name} · {template.name} · {bot.id}',
agent: 'omp',
model: 'default',
thinking: 'default',
skill: null, prompt: '', extensions: [] }

const PREVIEW = {
  botId: 'a7f3c2',
  botNameRule: TEMPLATE.botName,
  templateName: TEMPLATE.name,
  zoneName: 'alpha',
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
  it('模板管理的三参数示例不变，真实上下文保留原值与未知字段', () => {
    const rule = '{zone.name}/{template.name}/{agent}/{bot.id}/{unknown}'
    expect(renderBotNameExample(rule, '  审查  ', 'omp')).toBe('alpha/审查/omp/a7f3c2/{unknown}')
    expect(renderBotNameExample(rule, '  审查  ', 'pi', { zoneName: '实际目录', botId: 'd4e5f6' })).toBe('实际目录/  审查  /pi/d4e5f6/{unknown}')
  })

  it.each([
    { label: '空白规则', botNameRule: '   ', templateName: '审查' },
    { label: '多行规则', botNameRule: '第一行\n第二行', templateName: '审查' },
    { label: '空白渲染结果', botNameRule: '{template.name}', templateName: '   ' },
    { label: '多行渲染结果', botNameRule: '{template.name}', templateName: '第一行\n第二行' },
  ])('$label 禁止提交且不发送创建请求', async ({ botNameRule, templateName }) => {
    const user = userEvent.setup()
    const { calls } = mockFetch((url) => {
      if (url === '/api/bot-templates') return jsonResponse(200, { templates: [TEMPLATE], options: { agents: ['omp'], models: [], thinking: {}, skills: [] } })
      if (url === '/api/bots/preview') return jsonResponse(200, { ...PREVIEW, botNameRule, templateName })
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })
    render(<BotCreationDialog zone={PREVIEW.zone} onClose={() => {}} />)
    expect(await screen.findByRole('alert')).toHaveTextContent('名称必须是非空单行文字。')
    expect(screen.getByTestId('bot-create-submit')).toBeDisabled()
    await user.click(screen.getByTestId('bot-create-submit'))
    expect(calls.some((call) => call.url === '/api/bots')).toBe(false)
  })

  it('每个 zone 行都有“+”，弹窗绑定所选 zone，名称规则仅本地编辑并实时预览', async () => {
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
    const rule = within(dialog).getByRole('textbox', { name: '本次 Bot 名称（可编辑规则）' })
    expect(rule.tagName).toBe('INPUT')
    expect(rule).toHaveValue(TEMPLATE.botName)
    const requestCount = calls.length
    await user.clear(rule)
    expect(within(dialog).getByTestId('bot-create-submit')).toBeDisabled()
    expect(within(dialog).getByRole('alert')).toHaveTextContent('名称必须是非空单行文字。')
    await user.paste('  {zone.name} / 发布 / {agent} / {bot.id} / {unknown}  ')
    expect(preview.textContent).toBe('  alpha / 发布 / omp / a7f3c2 / {unknown}  ')
    expect(within(dialog).getByTestId('bot-create-submit')).toBeEnabled()
    expect(calls).toHaveLength(requestCount)
    expect(TEMPLATE.botName).toBe(PREVIEW.botNameRule)

    const previewCall = calls.find((call) => call.url === '/api/bots/preview')
    expect(previewCall?.method).toBe('POST')
    expect(JSON.parse(previewCall?.body ?? '{}')).toEqual({
      zone: 'D:\\work\\beta',
      templateId: 'tpl-1',
    })
  })

  it('切换模板重置规则，使用真实上下文和稳定标识提交本次编辑，拒绝后可重试', async () => {
    const user = userEvent.setup()
    const second = { ...TEMPLATE, id: 'tpl-2', name: '发布', agent: 'pi', botName: '{template.name}/{zone.name}/{bot.id}' }
    const secondPreview = { ...PREVIEW, templateId: second.id, templateName: second.name, agent: second.agent, botNameRule: second.botName, zoneName: '实际目录', botId: 'b8c9d0', name: '发布/实际目录/b8c9d0' }
    let attempts = 0
    const { calls } = mockFetch((url, method, body) => {
      if (url === '/api/zones') return jsonResponse(200, { zones: [{ path: PREVIEW.zone }] })
      if (url === '/api/zones/overview') return jsonResponse(200, { zones: [] })
      if (url === '/api/bot-templates') return jsonResponse(200, { templates: [TEMPLATE, second], options: { agents: ['omp', 'pi'], models: [], thinking: {}, skills: [] } })
      if (url === '/api/bots/preview') return jsonResponse(200, JSON.parse(body ?? '{}').templateId === second.id ? secondPreview : PREVIEW)
      if (url === '/api/bots' && method === 'POST') {
        attempts += 1
        return attempts === 1
          ? jsonResponse(409, { code: 'name-conflict', message: '名称已过期，请重试。' })
          : jsonResponse(202, creation('succeeded'))
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })
    render(<App />)
    const dialog = await openDialog(user, 0)
    const rule = within(dialog).getByRole('textbox', { name: '本次 Bot 名称（可编辑规则）' })
    await vi.waitFor(() => expect(rule).toHaveValue(TEMPLATE.botName))
    await user.clear(rule)
    await user.paste('临时名称')
    await user.selectOptions(within(dialog).getByTestId('bot-template'), second.id)
    await vi.waitFor(() => expect(rule).toHaveValue(second.botName))
    expect(within(dialog).getByTestId('bot-name-preview').textContent).toBe(secondPreview.name)
    const requestCount = calls.length
    await user.clear(rule)
    await user.paste('  {zone.name} / {agent} / {bot.id} / {unknown}  ')
    expect(calls).toHaveLength(requestCount)
    const submit = within(dialog).getByTestId('bot-create-submit')
    await user.click(submit)
    expect(JSON.parse(calls.find((call) => call.url === '/api/bots')?.body ?? '{}')).toEqual({
      zone: PREVIEW.zone, templateId: second.id, botId: secondPreview.botId,
      botNameRule: '  {zone.name} / {agent} / {bot.id} / {unknown}  ',
      expectedName: '  实际目录 / pi / b8c9d0 / {unknown}  ', input: '', workspaceTarget: 'current',
    })
    expect(await within(dialog).findByTestId('bot-request-error')).toHaveTextContent('名称已过期')
    await vi.waitFor(() => expect(rule).toHaveValue(second.botName))
    expect(rule).toBeEnabled()
    await user.click(submit)
    expect(await within(dialog).findByTestId('bot-create-success')).toBeVisible()
    expect(rule).toBeDisabled()
    expect(calls.filter((call) => call.url === '/api/bots/preview')).toHaveLength(3)
    expect(calls.filter((call) => call.url === '/api/bot-templates')).toHaveLength(1)
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
      botNameRule: PREVIEW.botNameRule,
      input: '检查登录逻辑',
      workspaceTarget: 'current',
    })
    expect(body).not.toHaveProperty('workspace')
  })

  it('Workspace 每次打开初始为当前，选择随创建发送，并在提交和结果期间锁定', async () => {
    const user = userEvent.setup()
    let resolveCreate!: (response: Response) => void
    const createResponse = new Promise<Response>((resolve) => {
      resolveCreate = resolve
    })
    const { calls } = mockFetch((url, method) => {
      if (url === '/api/zones' && method === 'GET') {
        return jsonResponse(200, { zones: [{ path: PREVIEW.zone }] })
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
      if (url === '/api/bots' && method === 'POST') return createResponse
      if (url === '/api/bots/bc-1' && method === 'GET') {
        return jsonResponse(200, creation('succeeded'))
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    let dialog = await openDialog(user, 0)
    let workspace = within(dialog).getByRole('combobox', { name: 'Herdr Workspace' })
    expect(workspace).toHaveValue('current')
    expect(workspace).toHaveAccessibleDescription(/当前按 zone 目录名匹配/)
    await user.selectOptions(workspace, 'default')
    await user.click(within(dialog).getByTestId('bot-create-close'))

    dialog = await openDialog(user, 0)
    workspace = within(dialog).getByRole('combobox', { name: 'Herdr Workspace' })
    expect(workspace).toHaveValue('current')
    await user.selectOptions(workspace, 'current')
    await vi.waitFor(() =>
      expect(within(dialog).getByTestId('bot-name-preview')).toHaveTextContent(PREVIEW.name),
    )
    await user.click(within(dialog).getByTestId('bot-create-submit'))
    expect(workspace).toBeDisabled()
    expect(within(dialog).getByRole('textbox', { name: /本次 Bot 名称/ })).toBeDisabled()
    expect(within(dialog).getByTestId('bot-template')).toBeDisabled()
    expect(within(dialog).getByTestId('bot-input')).toBeDisabled()
    expect(workspace).toHaveValue('current')
    expect(JSON.parse(calls.find((call) => call.url === '/api/bots')?.body ?? '{}')).toEqual({
      zone: PREVIEW.zone,
      templateId: PREVIEW.templateId,
      botId: PREVIEW.botId,
      expectedName: PREVIEW.name,
      botNameRule: PREVIEW.botNameRule,
      input: '',
      workspaceTarget: 'current',
    })

    resolveCreate(jsonResponse(202, creation('starting')))
    expect(await within(dialog).findByText('正在启动')).toBeVisible()
    expect(workspace).toBeDisabled()
    expect(within(dialog).getByRole('textbox', { name: /本次 Bot 名称/ })).toBeDisabled()
    expect(await within(dialog).findByText('创建成功，初始指示已提交')).toBeVisible()
    expect(workspace).toBeDisabled()
    expect(within(dialog).getByRole('textbox', { name: /本次 Bot 名称/ })).toBeDisabled()
    await user.click(within(dialog).getByTestId('bot-create-close'))
    dialog = await openDialog(user, 0)
    expect(within(dialog).getByRole('combobox', { name: 'Herdr Workspace' })).toHaveValue('current')
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
    expect(within(dialog).getByRole('combobox', { name: 'Herdr Workspace' })).toBeDisabled()
    expect(within(dialog).queryByTestId('bot-create-success')).toBeNull()
    expect(within(dialog).getByTestId('bot-result-tab')).toHaveTextContent('w-target:t9')
    expect(within(dialog).getByTestId('bot-result-pane')).toHaveTextContent('w-target:p9')
    expect(screen.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')
  })

  it('初始 input 为空时不阻止提交，空正文仍交给后端与 Herdr 判定', async () => {
    const user = userEvent.setup()
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
        return jsonResponse(
          200,
          creation('failed', {
            tabId: 'w-target:t9',
            paneId: 'w-target:p9',
            failure: {
              stage: 'submit',
              code: 'herdr-command-failed',
              message: 'Herdr 的 agent prompt 失败：agent prompt must not be empty。',
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
    expect(within(dialog).getByTestId('bot-input')).toHaveValue('')
    await user.click(within(dialog).getByTestId('bot-create-submit'))

    // 页面不前置组合校验：空 input 仍发出创建请求，并呈现后端/Herdr 的普通失败。
    const failure = await within(dialog).findByTestId('bot-create-failure')
    expect(failure).toHaveTextContent('提交初始指示')
    expect(failure).toHaveTextContent('agent prompt must not be empty')
    expect(within(dialog).queryByTestId('bot-create-success')).toBeNull()

    const createCall = calls.find(
      (call) => call.url === '/api/bots' && call.method === 'POST',
    )
    const body = JSON.parse(createCall?.body ?? '{}')
    expect(body.input).toBe('')
    expect(body).not.toHaveProperty('workspace')
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
