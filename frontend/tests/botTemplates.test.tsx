import { fireEvent, render, screen, within } from '@testing-library/react'
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

type RecordedCall = { url: string; method: string; body: string | null }

/** 记录模板请求并按用例给定的答复返回。 */
function mockTemplateFetch(
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

/** 页面从后端读取的固定选项：与目标 agent 源码中的能力事实一致。 */
const OPTIONS = {
  agents: ['omp', 'pi'],
  models: ['default', 'gpt-6.1-sol', 'deepseek-flash'],
  thinking: {
    omp: {
      default: [],
      'gpt-6.1-sol': ['low', 'medium', 'high', 'xhigh', 'max'],
      'deepseek-flash': ['low', 'high', 'max'],
    },
    pi: {
      default: [],
      'gpt-6.1-sol': ['off', 'minimal', 'low', 'medium', 'high'],
      'deepseek-flash': ['off', 'low', 'high', 'max'],
    },
  },
  skills: ['teach', 'code-review'],
}

async function openTemplates(user: UserEvent) {
  await user.click(screen.getByTestId('tab-templates'))
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('Bot 模板 tab：读取与展示', () => {
  it('进入独立 tab 才读取后端共享模板，并显示全部已保存字段', async () => {
    const user = userEvent.setup()
    const literal = {
      id: 'tpl-literal-1',
      name: '发布检查',
      botName: '{zone.name} · {template.name} · {bot.id}',
      agent: 'pi',
      model: 'deepseek-flash',
      thinking: 'max',
      skill: 'code-review',
      prompt: '第一行：检查登录\n第二行：保留 "引号" 与反斜杠 \\',
    }
    const { calls } = mockTemplateFetch((url, method) => {
      if (url === '/api/bot-templates' && method === 'GET') {
        return jsonResponse(200, { templates: [literal], options: OPTIONS })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    expect(screen.getByTestId('tab-templates')).toHaveTextContent('Bot 模板')
    expect(calls).toHaveLength(0)

    await openTemplates(user)

    const list = await screen.findByTestId('bot-template-list')
    expect(calls[0]).toMatchObject({ url: '/api/bot-templates', method: 'GET' })
    const row = within(list).getByTestId('bot-template-item')
    expect(row).toHaveTextContent('发布检查')
    expect(row).toHaveTextContent('{zone.name} · {template.name} · {bot.id}')
    expect(row).toHaveTextContent('pi')
    expect(row).toHaveTextContent('deepseek-flash')
    expect(row).toHaveTextContent('max')
    expect(row).toHaveTextContent('/code-review')
    // prompt 按字面呈现（多行内容不丢失）。
    expect(row.textContent).toContain('第一行：检查登录')
    expect(row.textContent).toContain('第二行：保留 "引号" 与反斜杠 \\')

    // 既有 tab 仍可操作。
    await user.click(screen.getByTestId('tab-control'))
    expect(screen.getByTestId('panel-control')).toBeVisible()
    expect(screen.getByTestId('start-button')).toBeEnabled()
    await user.click(screen.getByTestId('tab-files'))
    expect(screen.getByTestId('panel-files')).toBeVisible()
  })

  it('按 agent/model 能力限制 thinking，并在切换时保留或重置', async () => {
    const user = userEvent.setup()
    mockTemplateFetch((url, method) => {
      if (url === '/api/bot-templates' && method === 'GET') {
        return jsonResponse(200, { templates: [], options: OPTIONS })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openTemplates(user)
    await screen.findByTestId('template-empty')
    await user.click(screen.getByTestId('template-create-open'))
    const dialog = screen.getByTestId('template-dialog')
    const agent = within(dialog).getByTestId('template-agent') as HTMLSelectElement
    const model = within(dialog).getByTestId('template-model') as HTMLSelectElement
    const thinking = within(dialog).getByTestId('template-thinking') as HTMLSelectElement
    const skill = within(dialog).getByTestId('template-skill') as HTMLSelectElement
    const optionValues = (select: HTMLSelectElement) =>
      Array.from(select.options).map((option) => option.value)

    // 固定选项：agent、model、Skill 都只有已确认值，Skill 仍保留“无”。
    expect(optionValues(agent)).toEqual(['omp', 'pi'])
    expect(optionValues(model)).toEqual(['default', 'gpt-6.1-sol', 'deepseek-flash'])
    expect(optionValues(skill)).toEqual(['', 'teach', 'code-review'])
    expect(Array.from(skill.options).map((option) => option.textContent)).toEqual([
      '无',
      '/teach',
      '/code-review',
    ])

    // model 默认时 thinking 只能沿用默认值。
    expect(optionValues(thinking)).toEqual(['default'])

    await user.selectOptions(model, 'gpt-6.1-sol')
    expect(optionValues(thinking)).toEqual([
      'default',
      'low',
      'medium',
      'high',
      'xhigh',
      'max',
    ])
    await user.selectOptions(thinking, 'high')

    // 更换 model：仍适用的 thinking 保留。
    await user.selectOptions(model, 'deepseek-flash')
    expect(optionValues(thinking)).toEqual(['default', 'low', 'high', 'max'])
    expect(thinking.value).toBe('high')
    expect(screen.queryByTestId('thinking-notice')).toBeNull()

    await user.selectOptions(thinking, 'max')
    await user.selectOptions(model, 'gpt-6.1-sol')
    expect(thinking.value).toBe('max')

    // 更换为不适用的 model：重置为默认并提示。
    await user.selectOptions(thinking, 'xhigh')
    await user.selectOptions(model, 'deepseek-flash')
    expect(thinking.value).toBe('default')
    expect(screen.getByTestId('thinking-notice')).toHaveTextContent(
      '当前思考级别「xhigh」不适用于 omp / deepseek-flash，已重置为「沿用默认值」。',
    )

    // 更换 agent：model 与 thinking 都重置，提示清除。
    await user.selectOptions(model, 'gpt-6.1-sol')
    await user.selectOptions(thinking, 'medium')
    await user.selectOptions(agent, 'pi')
    expect(model.value).toBe('default')
    expect(thinking.value).toBe('default')
    expect(optionValues(thinking)).toEqual(['default'])
    expect(screen.queryByTestId('thinking-notice')).toBeNull()

    // Pi 的能力表与 OMP 不同：不包含 xhigh / max。
    await user.selectOptions(model, 'gpt-6.1-sol')
    expect(optionValues(thinking)).toEqual([
      'default',
      'off',
      'minimal',
      'low',
      'medium',
      'high',
    ])
    await user.selectOptions(model, 'deepseek-flash')
    expect(optionValues(thinking)).toEqual(['default', 'off', 'low', 'high', 'max'])

    // 从显式模型改回默认模型：thinking 必须重置并提示。
    await user.selectOptions(thinking, 'max')
    await user.selectOptions(model, 'default')
    expect(thinking.value).toBe('default')
    expect(optionValues(thinking)).toEqual(['default'])
    expect(screen.getByTestId('thinking-notice')).toHaveTextContent(
      '当前思考级别「max」不适用于 pi / default，已重置为「沿用默认值」。',
    )
  })

  it('新增模板时插入四个命名占位符、查看示例并保存完整字面值', async () => {
    const user = userEvent.setup()
    const prompt = '第一行\n第二行：保留 "引号" 与反斜杠 \\'
    const saved = {
      id: 'tpl-created',
      name: '审查',
      botName: '{zone.name} · {template.name} · {agent} · {bot.id}',
      agent: 'pi',
      model: 'deepseek-flash',
      thinking: 'max',
      skill: 'code-review',
      prompt,
    }
    const { calls } = mockTemplateFetch((url, method) => {
      if (url === '/api/bot-templates' && method === 'GET') {
        return jsonResponse(200, { templates: [], options: OPTIONS })
      }
      if (url === '/api/bot-templates' && method === 'POST') {
        return jsonResponse(200, { templates: [saved], options: OPTIONS })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openTemplates(user)
    await screen.findByTestId('template-empty')
    await user.click(screen.getByTestId('template-create-open'))

    const dialog = screen.getByTestId('template-dialog')
    const botName = within(dialog).getByTestId('template-bot-name') as HTMLInputElement
    expect(botName.value).toBe('{zone.name} · {template.name} · {bot.id}')

    // 编辑命名规则：只提供四个已承诺字段，每次插入都能在规则中观察到对应占位符。
    await user.clear(botName)
    await user.click(within(dialog).getByTestId('insert-zone-name'))
    expect(botName.value).toBe('{zone.name}')
    await user.click(within(dialog).getByTestId('insert-template-name'))
    expect(botName.value).toBe('{zone.name}{template.name}')
    await user.click(within(dialog).getByTestId('insert-agent'))
    await user.click(within(dialog).getByTestId('insert-bot-id'))
    expect(botName.value).toBe('{zone.name}{template.name}{agent}{bot.id}')
    fireEvent.change(botName, {
      target: { value: '{zone.name} · {template.name} · {agent} · {bot.id}' },
    })
    expect(botName.value).toBe('{zone.name} · {template.name} · {agent} · {bot.id}')

    await user.type(within(dialog).getByTestId('template-name'), '审查')
    expect(within(dialog).getByTestId('bot-name-preview')).toHaveTextContent(
      'alpha · 审查 · omp · a7f3c2',
    )

    await user.selectOptions(within(dialog).getByTestId('template-agent'), 'pi')
    await user.selectOptions(within(dialog).getByTestId('template-model'), 'deepseek-flash')
    await user.selectOptions(within(dialog).getByTestId('template-thinking'), 'max')
    await user.selectOptions(within(dialog).getByTestId('template-skill'), 'code-review')
    await user.type(within(dialog).getByTestId('template-prompt'), prompt)
    await user.click(within(dialog).getByTestId('save-template'))

    await screen.findByTestId('bot-template-list')
    expect(screen.queryByTestId('template-dialog')).toBeNull()
    const post = calls.find((call) => call.method === 'POST')
    expect(post?.url).toBe('/api/bot-templates')
    expect(post?.body).not.toBeNull()
    expect(JSON.parse(post?.body ?? 'null')).toEqual({
      name: '审查',
      botName: '{zone.name} · {template.name} · {agent} · {bot.id}',
      agent: 'pi',
      model: 'deepseek-flash',
      thinking: 'max',
      skill: 'code-review',
      prompt,
    })
    const row = screen.getByTestId('bot-template-item')
    expect(row).toHaveTextContent('审查')
    expect(row).toHaveTextContent('deepseek-flash')
    expect(row).toHaveTextContent('/code-review')
    expect(screen.getByTestId('template-feedback')).toHaveAttribute('data-kind', 'success')
  })

  it('编辑与删除模板，只有后端确认后才更新列表', async () => {
    const user = userEvent.setup()
    const first = {
      id: 'tpl-a',
      name: '旧名字',
      botName: '{zone.name}',
      agent: 'omp',
      model: 'default',
      thinking: 'default',
      skill: null,
      prompt: '旧 prompt',
    }
    const second = {
      id: 'tpl-b',
      name: '待删除',
      botName: '{template.name}',
      agent: 'pi',
      model: 'gpt-6.1-sol',
      thinking: 'off',
      skill: 'teach',
      prompt: '',
    }
    const edited = {
      ...first,
      name: '新名字',
      botName: '{template.name} · {agent} · {bot.id}',
      agent: 'pi',
      model: 'gpt-6.1-sol',
      thinking: 'medium',
      skill: 'code-review',
      prompt: '第一行\n第二行',
    }
    const { calls } = mockTemplateFetch((url, method) => {
      if (url === '/api/bot-templates' && method === 'GET') {
        return jsonResponse(200, { templates: [first, second], options: OPTIONS })
      }
      if (url === '/api/bot-templates/tpl-a' && method === 'PUT') {
        return jsonResponse(200, { templates: [edited, second], options: OPTIONS })
      }
      if (url === '/api/bot-templates/tpl-b' && method === 'DELETE') {
        return jsonResponse(200, { templates: [edited], options: OPTIONS })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openTemplates(user)
    const list = await screen.findByTestId('bot-template-list')
    const rows = within(list).getAllByTestId('bot-template-item')
    expect(rows.map((row) => row.getAttribute('data-template-id'))).toEqual(['tpl-a', 'tpl-b'])

    // 编辑：表单带回全部已保存值。
    await user.click(within(rows[0]).getByTestId('template-edit'))
    const dialog = screen.getByTestId('template-dialog')
    expect((within(dialog).getByTestId('template-name') as HTMLInputElement).value).toBe('旧名字')
    expect((within(dialog).getByTestId('template-bot-name') as HTMLInputElement).value).toBe(
      '{zone.name}',
    )
    expect((within(dialog).getByTestId('template-prompt') as HTMLTextAreaElement).value).toBe(
      '旧 prompt',
    )
    await user.clear(within(dialog).getByTestId('template-name'))
    await user.type(within(dialog).getByTestId('template-name'), '新名字')
    fireEvent.change(within(dialog).getByTestId('template-bot-name'), {
      target: { value: '{template.name} · {agent} · {bot.id}' },
    })
    await user.selectOptions(within(dialog).getByTestId('template-agent'), 'pi')
    await user.selectOptions(within(dialog).getByTestId('template-model'), 'gpt-6.1-sol')
    await user.selectOptions(within(dialog).getByTestId('template-thinking'), 'medium')
    await user.selectOptions(within(dialog).getByTestId('template-skill'), 'code-review')
    await user.clear(within(dialog).getByTestId('template-prompt'))
    await user.type(within(dialog).getByTestId('template-prompt'), '第一行\n第二行')
    await user.click(within(dialog).getByTestId('save-template'))

    await screen.findByTestId('bot-template-list')
    const put = calls.find((call) => call.method === 'PUT')
    expect(put?.url).toBe('/api/bot-templates/tpl-a')
    expect(JSON.parse(put?.body ?? 'null')).toEqual({
      name: '新名字',
      botName: '{template.name} · {agent} · {bot.id}',
      agent: 'pi',
      model: 'gpt-6.1-sol',
      thinking: 'medium',
      skill: 'code-review',
      prompt: '第一行\n第二行',
    })
    expect(screen.getAllByTestId('bot-template-item')[0]).toHaveTextContent('新名字')

    // 删除：只有后端成功返回后才从列表消失。
    const targetRow = screen
      .getAllByTestId('bot-template-item')
      .find((row) => row.getAttribute('data-template-id') === 'tpl-b')
    expect(targetRow).toBeDefined()
    await user.click(within(targetRow as HTMLElement).getByTestId('template-delete'))
    await screen.findByTestId('bot-template-list')
    const remove = calls.find((call) => call.method === 'DELETE')
    expect(remove?.url).toBe('/api/bot-templates/tpl-b')
    expect(within(screen.getByTestId('bot-template-list')).queryByText('待删除')).toBeNull()
  })

  it('保存失败不表示已保存，列表保持此前读取到的值', async () => {
    const user = userEvent.setup()
    const existing = {
      id: 'tpl-existing',
      name: '已保存',
      botName: '{zone.name}',
      agent: 'omp',
      model: 'default',
      thinking: 'default',
      skill: null,
      prompt: '',
    }
    mockTemplateFetch((url, method) => {
      if (url === '/api/bot-templates' && method === 'GET') {
        return jsonResponse(200, { templates: [existing], options: OPTIONS })
      }
      if (url === '/api/bot-templates' && method === 'POST') {
        return jsonResponse(500, {
          code: 'templates-write-failed',
          message: '保存 Bot 模板失败：磁盘不可写；此前已保存的模板保持不变。',
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openTemplates(user)
    await screen.findByTestId('bot-template-list')
    await user.click(screen.getByTestId('template-create-open'))
    const dialog = screen.getByTestId('template-dialog')
    await user.type(within(dialog).getByTestId('template-name'), '不应保存')
    await user.click(within(dialog).getByTestId('save-template'))

    expect(await within(dialog).findByTestId('template-form-error')).toHaveTextContent(
      '保存 Bot 模板失败：磁盘不可写；此前已保存的模板保持不变。',
    )
    expect(screen.getByTestId('template-dialog')).toBeVisible()
    expect(screen.queryByTestId('template-feedback')).toBeNull()
    const list = screen.getByTestId('bot-template-list')
    expect(list).toHaveTextContent('已保存')
    expect(list).not.toHaveTextContent('不应保存')
  })

  it('空值与多行 prompt 都能保存，Skill 可保持不选', async () => {
    const user = userEvent.setup()
    const { calls } = mockTemplateFetch((url, method, body) => {
      if (url === '/api/bot-templates' && method === 'GET') {
        return jsonResponse(200, { templates: [], options: OPTIONS })
      }
      if (url === '/api/bot-templates' && method === 'POST') {
        const payload = JSON.parse(body ?? '{}')
        return jsonResponse(200, {
          templates: [{ id: 'tpl-empty', ...payload }],
          options: OPTIONS,
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openTemplates(user)
    await screen.findByTestId('template-empty')
    await user.click(screen.getByTestId('template-create-open'))
    const dialog = screen.getByTestId('template-dialog')
    await user.type(within(dialog).getByTestId('template-name'), '空 prompt 模板')
    expect((within(dialog).getByTestId('template-prompt') as HTMLTextAreaElement).value).toBe('')
    expect((within(dialog).getByTestId('template-skill') as HTMLSelectElement).value).toBe('')
    await user.click(within(dialog).getByTestId('save-template'))

    await screen.findByTestId('bot-template-list')
    const post = calls.find((call) => call.method === 'POST')
    expect(JSON.parse(post?.body ?? 'null')).toMatchObject({
      name: '空 prompt 模板',
      skill: null,
      prompt: '',
      model: 'default',
      thinking: 'default',
    })
    expect(screen.getByTestId('bot-template-item')).toHaveTextContent('空 prompt 模板')
  })
})
