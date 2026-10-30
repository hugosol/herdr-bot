import { fireEvent, render, screen, within } from '@testing-library/react'
import userEvent, { type UserEvent } from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from '../src/app/App'

beforeEach(() => {
  window.history.replaceState(null, '', '/?layout=expanded')
})

/** 一个只提供客户端真正用到的成员的 JSON 答复。 */
function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response
}

type RecordedCall = { url: string; method: string; body: string | null }

/** 记录 Task 请求并按用例给定的答复返回。 */
function mockTaskFetch(
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
      'gpt-6.1-sol': ['minimal', 'low', 'medium', 'high', 'xhigh', 'max'],
      'deepseek-flash': ['off', 'low', 'high', 'max'],
    },
  },
  skills: ['teach', 'code-review'],
}

async function openTasks(user: UserEvent) {
  await user.click(screen.getByTestId('tab-tasks'))
}

async function saveTask(user: UserEvent, dialog: HTMLElement) {
  await user.click(within(dialog).getByTestId('task-next'))
  await user.click(within(dialog).getByTestId('task-next'))
  await user.click(within(dialog).getByTestId('save-task'))
}

/** 规整化编辑器 DOM：变量块按占位符读取，固定文字按文本节点读取。 */
function serializedRule(editor: HTMLElement): string {
  return Array.from(editor.childNodes)
    .map((node) =>
      node instanceof HTMLElement && node.dataset.chatNameToken !== undefined
        ? node.dataset.chatNameToken
        : (node.textContent ?? ''),
    )
    .join('')
}

function setRule(editor: HTMLElement, rule: string): void {
  editor.replaceChildren()
  if (rule !== '') editor.append(document.createTextNode(rule))
  fireEvent.input(editor)
}

function selectCollapsedRange(editor: HTMLElement, range: Range): void {
  editor.focus()
  const selection = window.getSelection()
  if (selection === null) throw new Error('测试环境没有 Selection')
  selection.removeAllRanges()
  selection.addRange(range)
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('Task tab：读取与展示', () => {
  it('进入 task 入口才读取共享 Task，并显示字段与 chat-name 示例', async () => {
    const user = userEvent.setup()
    const literal = { id: 'tpl-literal-1',
    name: '发布检查',
    chatName: '{zone.name} · {task.name} · {chat.id}',
    agent: 'pi',
    taskType: 'normal',
    model: 'deepseek-flash',
    thinking: 'max',
    skill: 'code-review', prompt: '第一行：检查登录\n第二行：保留 "引号" 与反斜杠 \\', extensions: [] }
    const { calls } = mockTaskFetch((url, method) => {
      if (url === '/api/tasks' && method === 'GET') {
        return jsonResponse(200, { tasks: [literal], options: OPTIONS })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    expect(calls).toHaveLength(0)

    await openTasks(user)

    const list = await screen.findByTestId('task-list')
    expect(calls[0]).toMatchObject({ url: '/api/tasks', method: 'GET' })
    const row = within(list).getByTestId('task-item')
    expect(row).toHaveTextContent('发布检查')
    expect(row).toHaveTextContent('alpha · 发布检查 · a7f3c2')
    expect(row).not.toHaveTextContent('{zone.name}')
    expect(row).toHaveTextContent('pi')
    expect(row).toHaveTextContent('deepseek-flash')
    expect(row).toHaveTextContent('max')
    expect(row).toHaveTextContent('/code-review')
    // prompt 按字面呈现（多行内容不丢失）。
    expect(row.textContent).toContain('第一行：检查登录')
    expect(row.textContent).toContain('第二行：保留 "引号" 与反斜杠 \\')

    // 既有 tab 仍可操作。
    await user.click(screen.getByTestId('server-button'))
    expect(screen.getByTestId('server-page')).toBeVisible()
    expect(screen.getByTestId('start-button')).toBeEnabled()
    await user.click(screen.getByTestId('server-back'))
    await user.click(screen.getByTestId('tab-files'))
    expect(screen.getByTestId('panel-files')).toBeVisible()
  })

  it('按 agent/model 能力限制 thinking，并在切换时保留或重置', async () => {
    const user = userEvent.setup()
    mockTaskFetch((url, method) => {
      if (url === '/api/tasks' && method === 'GET') {
        return jsonResponse(200, { tasks: [], options: OPTIONS })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openTasks(user)
    await screen.findByTestId('task-empty')
    await user.click(screen.getByTestId('task-create-open'))
    const dialog = screen.getByTestId('task-dialog')
    const agent = within(dialog).getByTestId('task-agent') as HTMLSelectElement
    const model = within(dialog).getByTestId('task-model') as HTMLSelectElement
    const thinking = within(dialog).getByTestId('task-thinking') as HTMLSelectElement
    const skill = within(dialog).getByTestId('task-skill') as HTMLSelectElement
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

    // model 默认时 thinking 只能沿用默认值；选中项文本必须与控件当前值一致。
    expect(optionValues(thinking)).toEqual(['default'])
    expect(agent.selectedOptions[0].textContent).toBe('omp')
    expect(model.selectedOptions[0].textContent).toBe('沿用 Agent 默认值')
    expect(thinking.selectedOptions[0].textContent).toBe('沿用 Agent 默认值')
    expect(skill.selectedOptions[0].textContent).toBe('无')

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
      '当前思考级别「xhigh」不适用于 omp / deepseek-flash，已重置为「沿用 Agent 默认值」。',
    )

    // 更换 agent：model 与 thinking 都重置，提示清除。
    await user.selectOptions(model, 'gpt-6.1-sol')
    await user.selectOptions(thinking, 'medium')
    await user.selectOptions(agent, 'pi')
    expect(model.value).toBe('default')
    expect(thinking.value).toBe('default')
    expect(optionValues(thinking)).toEqual(['default'])
    expect(screen.queryByTestId('thinking-notice')).toBeNull()

    // Pi 的能力表与 OMP 不同：gpt-6.1-sol 不支持 off 但支持 xhigh / max。
    await user.selectOptions(model, 'gpt-6.1-sol')
    expect(optionValues(thinking)).toEqual([
      'default',
      'minimal',
      'low',
      'medium',
      'high',
      'xhigh',
      'max',
    ])
    await user.selectOptions(model, 'deepseek-flash')
    expect(optionValues(thinking)).toEqual(['default', 'off', 'low', 'high', 'max'])

    // 从显式模型改回默认模型：thinking 必须重置并提示。
    await user.selectOptions(thinking, 'max')
    await user.selectOptions(model, 'default')
    expect(thinking.value).toBe('default')
    expect(optionValues(thinking)).toEqual(['default'])
    expect(screen.getByTestId('thinking-notice')).toHaveTextContent(
      '当前思考级别「max」不适用于 pi / default，已重置为「沿用 Agent 默认值」。',
    )
  })

  it.each(['omp', 'pi'] as const)('%s 切换 Task 类型保留合法宿主、模型和可选扩展，角色固定扩展按类型筛选并保存', async (agent) => {
    const user = userEvent.setup()
    const original = { id: 'role-task', name: '角色编辑', chatName: '{chat.id}', agent,
      taskType: 'normal', model: 'deepseek-flash', thinking: 'high', skill: null, prompt: '', extensions: [`${agent}-tools`] }
    const options = { ...OPTIONS, extensions: (['omp', 'pi'] as const).flatMap(host => [
      { id: `${host}-state`, label: `${host} 状态`, agent: host, kind: 'fixed', entrypoint: '/state.ts' },
      { id: `${host}-manager-chat`, label: `${host} manager-chat`, agent: host, kind: 'fixed', entrypoint: '/manager.ts' },
      { id: `${host}-node-chat`, label: `${host} node-chat`, agent: host, kind: 'fixed', entrypoint: '/node.ts' },
      { id: `${host}-tools`, label: `${host} 工具`, agent: host, kind: 'optional', entrypoint: '/tools.ts' },
    ]) }
    const { calls } = mockTaskFetch((url, method, body) => {
      if (url === '/api/tasks' && method === 'GET') return jsonResponse(200, { tasks: [original], options })
      if (url === '/api/tasks/role-task' && method === 'PUT') return jsonResponse(200, { tasks: [{ id: original.id, ...JSON.parse(body!) }], options })
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })
    render(<App />)
    await openTasks(user)
    await user.click(await screen.findByTestId('task-edit'))
    const dialog = screen.getByTestId('task-dialog')
    expect(within(within(dialog).getByTestId('task-type')).getAllByRole('option').map(option => (option as HTMLOptionElement).value)).toEqual(['normal', 'manager'])
    for (const role of ['manager', 'normal', 'manager']) {
      await user.selectOptions(within(dialog).getByTestId('task-type'), role)
      expect(within(dialog).getByTestId('task-agent')).toHaveValue(agent)
      expect(within(dialog).getByTestId('task-model')).toHaveValue('deepseek-flash')
      expect(within(dialog).getByTestId('task-thinking')).toHaveValue('high')
      await user.click(within(dialog).getByTestId('task-next'))
      const fixed = within(dialog).getByTestId('task-fixed-extensions')
      expect(fixed).toHaveTextContent(`${agent} 状态`)
      expect(fixed).not.toHaveTextContent(`${agent === 'pi' ? 'omp' : 'pi'} `)
      if (role === 'manager') expect(fixed).toHaveTextContent(`${agent} manager-chat`)
      else expect(fixed).not.toHaveTextContent('manager-chat')
      expect(fixed).not.toHaveTextContent('node-chat')
      expect(within(dialog).getByTestId(`extension-${agent}-tools`)).toBeChecked()
      expect(within(fixed).queryByRole('checkbox')).toBeNull()
      await user.click(within(dialog).getByTestId('task-back'))
    }
    await saveTask(user, dialog)
    expect(JSON.parse(calls.find(call => call.method === 'PUT')!.body!)).toEqual({ ...original, id: undefined, taskType: 'manager' })
    expect(await screen.findByTestId('task-feedback')).toHaveAttribute('data-kind', 'success')
  })

  it('新增 Task 时插入四个命名占位符、查看示例并保存完整字面值', async () => {
    const user = userEvent.setup()
    const prompt = '第一行\n第二行：保留 "引号" 与反斜杠 \\'
    const saved = { id: 'tpl-created',
    name: '审查',
    chatName: '{zone.name} · {task.name} · {agent} · {chat.id}',
    agent: 'pi',
    taskType: 'normal',
    model: 'deepseek-flash',
    thinking: 'max',
    skill: 'code-review', prompt, extensions: [] }
    const { calls } = mockTaskFetch((url, method) => {
      if (url === '/api/tasks' && method === 'GET') {
        return jsonResponse(200, { tasks: [], options: OPTIONS })
      }
      if (url === '/api/tasks' && method === 'POST') {
        return jsonResponse(200, { tasks: [saved], options: OPTIONS })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openTasks(user)
    await screen.findByTestId('task-empty')
    await user.click(screen.getByTestId('task-create-open'))

    const dialog = screen.getByTestId('task-dialog')
    const chatName = within(dialog).getByTestId('task-chat-name') as HTMLElement
    expect(within(dialog).getAllByTestId('chat-name-token')).toHaveLength(3)
    expect(serializedRule(chatName)).toBe('{zone.name} · {task.name} · {chat.id}')

    // 编辑命名规则：只提供四个已承诺字段，每次插入都能在规则中观察到对应占位符。
    setRule(chatName, '')
    await user.click(within(dialog).getByTestId('insert-zone-name'))
    expect(serializedRule(chatName)).toBe('{zone.name}')
    expect(within(dialog).getAllByTestId('chat-name-token')).toHaveLength(1)
    await user.click(within(dialog).getByTestId('insert-task-name'))
    expect(serializedRule(chatName)).toBe('{zone.name}{task.name}')
    await user.click(within(dialog).getByTestId('insert-agent'))
    await user.click(within(dialog).getByTestId('insert-chat-id'))
    expect(serializedRule(chatName)).toBe('{zone.name}{task.name}{agent}{chat.id}')
    expect(within(dialog).getAllByTestId('chat-name-token')).toHaveLength(4)
    expect(within(dialog).getByTestId('chat-name-preview')).toHaveTextContent(
      'alpha审查ompa7f3c2',
    )

    setRule(chatName, '{zone.name} · {task.name} · {agent} · {chat.id}')
    expect(serializedRule(chatName)).toBe(
      '{zone.name} · {task.name} · {agent} · {chat.id}',
    )

    await user.type(within(dialog).getByTestId('task-name'), '审查')
    expect(within(dialog).getByTestId('chat-name-preview')).toHaveTextContent(
      'alpha · 审查 · omp · a7f3c2',
    )

    await user.selectOptions(within(dialog).getByTestId('task-agent'), 'pi')
    await user.selectOptions(within(dialog).getByTestId('task-model'), 'deepseek-flash')
    await user.selectOptions(within(dialog).getByTestId('task-thinking'), 'max')
    await user.selectOptions(within(dialog).getByTestId('task-skill'), 'code-review')
    await user.type(within(dialog).getByTestId('task-prompt'), prompt)
    await saveTask(user, dialog)

    await screen.findByTestId('task-list')
    expect(screen.queryByTestId('task-dialog')).toBeNull()
    const post = calls.find((call) => call.method === 'POST')
    expect(post?.url).toBe('/api/tasks')
    expect(post?.body).not.toBeNull()
    expect(JSON.parse(post?.body ?? 'null')).toEqual({ name: '审查',
    chatName: '{zone.name} · {task.name} · {agent} · {chat.id}',
    agent: 'pi',
    taskType: 'normal',
    model: 'deepseek-flash',
    thinking: 'max',
    skill: 'code-review', prompt, extensions: [] })
    const row = screen.getByTestId('task-item')
    expect(row).toHaveTextContent('审查')
    expect(row).toHaveTextContent('alpha · 审查 · pi · a7f3c2')
    expect(row).not.toHaveTextContent('{task.name}')
    expect(row).toHaveTextContent('deepseek-flash')
    expect(row).toHaveTextContent('/code-review')
    expect(screen.getByTestId('task-feedback')).toHaveAttribute('data-kind', 'success')
  })

  it('变量块可按退格或 × 整块删除，插入位置跟随光标而不是固定追加', async () => {
    const user = userEvent.setup()
    mockTaskFetch((url, method) => {
      if (url === '/api/tasks' && method === 'GET') {
        return jsonResponse(200, { tasks: [], options: OPTIONS })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openTasks(user)
    await screen.findByTestId('task-empty')
    await user.click(screen.getByTestId('task-create-open'))
    const dialog = screen.getByTestId('task-dialog')
    const chatName = within(dialog).getByTestId('task-chat-name') as HTMLElement

    // 退格紧跟在变量块之后：整个 {task.name} 消失，固定分隔符仍留给用户编辑。
    const tokens = within(dialog).getAllByTestId('chat-name-token')
    expect(tokens).toHaveLength(3)
    const afterTaskToken = document.createRange()
    afterTaskToken.setStartAfter(tokens[1])
    afterTaskToken.collapse(true)
    selectCollapsedRange(chatName, afterTaskToken)
    fireEvent.keyDown(chatName, { key: 'Backspace' })

    expect(within(dialog).getAllByTestId('chat-name-token')).toHaveLength(2)
    expect(serializedRule(chatName)).toBe('{zone.name} ·  · {chat.id}')

    // 普通文字中的光标位置决定插入点，而不是总追加到末尾。
    setRule(chatName, '前缀后缀')
    const text = chatName.firstChild
    expect(text?.nodeType).toBe(Node.TEXT_NODE)
    const middle = document.createRange()
    middle.setStart(text as Text, 2)
    middle.collapse(true)
    selectCollapsedRange(chatName, middle)
    fireEvent.click(within(dialog).getByTestId('insert-zone-name'))
    expect(serializedRule(chatName)).toBe('前缀{zone.name}后缀')
    expect(within(dialog).getAllByTestId('chat-name-token')).toHaveLength(1)

    // × 只删除变量块，不影响前后固定文字。
    const token = within(dialog).getByTestId('chat-name-token')
    fireEvent.click(within(token).getByRole('button', { name: '删除变量 zone.name' }))
    expect(serializedRule(chatName)).toBe('前缀后缀')
    expect(within(dialog).queryByTestId('chat-name-token')).toBeNull()
  })

  it('编辑 Task，只有后端确认后才更新列表', async () => {
    const user = userEvent.setup()
    const first = { id: 'tpl-a',
    name: '旧名字',
    chatName: '{zone.name}',
    agent: 'omp',
    taskType: 'normal',
    model: 'default',
    thinking: 'default',
    skill: null, prompt: '旧 prompt', extensions: [] }
    const second = { id: 'tpl-b',
    name: '待删除',
    chatName: '{task.name}',
    agent: 'pi',
    taskType: 'normal',
    model: 'gpt-6.1-sol',
    thinking: 'off',
    skill: 'teach', prompt: '', extensions: [] }
    const edited = { ...first,
    name: '新名字',
    chatName: '{task.name} · {agent} · {chat.id}',
    agent: 'pi',
    taskType: 'normal',
    model: 'gpt-6.1-sol',
    thinking: 'medium',
    skill: 'code-review', prompt: '第一行\n第二行', extensions: [] }
    const { calls } = mockTaskFetch((url, method) => {
      if (url === '/api/tasks' && method === 'GET') {
        return jsonResponse(200, { tasks: [first, second], options: OPTIONS })
      }
      if (url === '/api/tasks/tpl-a' && method === 'PUT') {
        return jsonResponse(200, { tasks: [edited, second], options: OPTIONS })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openTasks(user)
    const list = await screen.findByTestId('task-list')
    const rows = within(list).getAllByTestId('task-item')
    expect(rows.map((row) => row.getAttribute('data-task-id'))).toEqual(['tpl-a', 'tpl-b'])

    // 编辑：表单带回全部已保存值。
    await user.click(within(rows[0]).getByTestId('task-edit'))
    const dialog = screen.getByTestId('task-dialog')
    expect((within(dialog).getByTestId('task-name') as HTMLInputElement).value).toBe('旧名字')
    const chatName = within(dialog).getByTestId('task-chat-name') as HTMLElement
    expect(serializedRule(chatName)).toBe('{zone.name}')
    expect(within(dialog).getAllByTestId('chat-name-token')).toHaveLength(1)
    expect((within(dialog).getByTestId('task-prompt') as HTMLTextAreaElement).value).toBe(
      '旧 prompt',
    )
    await user.clear(within(dialog).getByTestId('task-name'))
    await user.type(within(dialog).getByTestId('task-name'), '新名字')
    setRule(chatName, '{task.name} · {agent} · {chat.id}')
    await user.selectOptions(within(dialog).getByTestId('task-agent'), 'pi')
    await user.selectOptions(within(dialog).getByTestId('task-model'), 'gpt-6.1-sol')
    await user.selectOptions(within(dialog).getByTestId('task-thinking'), 'medium')
    await user.selectOptions(within(dialog).getByTestId('task-skill'), 'code-review')
    await user.clear(within(dialog).getByTestId('task-prompt'))
    await user.type(within(dialog).getByTestId('task-prompt'), '第一行\n第二行')
    await saveTask(user, dialog)

    await screen.findByTestId('task-list')
    const put = calls.find((call) => call.method === 'PUT')
    expect(put?.url).toBe('/api/tasks/tpl-a')
    expect(JSON.parse(put?.body ?? 'null')).toEqual({ name: '新名字',
    chatName: '{task.name} · {agent} · {chat.id}',
    agent: 'pi',
    taskType: 'normal',
    model: 'gpt-6.1-sol',
    thinking: 'medium',
    skill: 'code-review', prompt: '第一行\n第二行', extensions: [] })
    expect(screen.getAllByTestId('task-item')[0]).toHaveTextContent('新名字')
    expect(screen.getAllByTestId('task-item')[0]).toHaveTextContent(
      '新名字 · pi · a7f3c2',
    )

  })

  it('保存失败不表示已保存，列表保持此前读取到的值', async () => {
    const user = userEvent.setup()
    const existing = { id: 'tpl-existing',
    name: '已保存',
    chatName: '{zone.name}',
    agent: 'omp',
    taskType: 'normal',
    model: 'default',
    thinking: 'default',
    skill: null, prompt: '', extensions: [] }
    mockTaskFetch((url, method) => {
      if (url === '/api/tasks' && method === 'GET') {
        return jsonResponse(200, { tasks: [existing], options: OPTIONS })
      }
      if (url === '/api/tasks' && method === 'POST') {
        return jsonResponse(500, {
          code: 'tasks-write-failed',
          message: '保存 Task 失败：磁盘不可写；此前已保存的 Task 保持不变。',
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openTasks(user)
    await screen.findByTestId('task-list')
    await user.click(screen.getByTestId('task-create-open'))
    const dialog = screen.getByTestId('task-dialog')
    await user.type(within(dialog).getByTestId('task-name'), '不应保存')
    await saveTask(user, dialog)

    expect(await within(dialog).findByTestId('task-form-error')).toHaveTextContent(
      '保存 Task 失败：磁盘不可写；此前已保存的 Task 保持不变。',
    )
    expect(screen.getByTestId('task-dialog')).toBeVisible()
    expect(screen.queryByTestId('task-feedback')).toBeNull()
    const list = screen.getByTestId('task-list')
    expect(list).toHaveTextContent('已保存')
    expect(list).not.toHaveTextContent('不应保存')
  })

  it('空值与多行 prompt 都能保存，Skill 可保持不选', async () => {
    const user = userEvent.setup()
    const { calls } = mockTaskFetch((url, method, body) => {
      if (url === '/api/tasks' && method === 'GET') {
        return jsonResponse(200, { tasks: [], options: OPTIONS })
      }
      if (url === '/api/tasks' && method === 'POST') {
        const payload = JSON.parse(body ?? '{}')
        return jsonResponse(200, {
          tasks: [{ id: 'tpl-empty', ...payload }],
          options: OPTIONS,
        })
      }
      return jsonResponse(404, { code: 'unknown-endpoint', message: '未知请求' })
    })

    render(<App />)
    await openTasks(user)
    await screen.findByTestId('task-empty')
    await user.click(screen.getByTestId('task-create-open'))
    const dialog = screen.getByTestId('task-dialog')
    await user.type(within(dialog).getByTestId('task-name'), '空 prompt Bot')
    expect((within(dialog).getByTestId('task-prompt') as HTMLTextAreaElement).value).toBe('')
    expect((within(dialog).getByTestId('task-skill') as HTMLSelectElement).value).toBe('')
    await saveTask(user, dialog)

    await screen.findByTestId('task-list')
    const post = calls.find((call) => call.method === 'POST')
    expect(JSON.parse(post?.body ?? 'null')).toMatchObject({
      name: '空 prompt Bot',
      skill: null,
      prompt: '',
      model: 'default',
      thinking: 'default',
    })
    expect(screen.getByTestId('task-item')).toHaveTextContent('空 prompt Bot')
  })
})
