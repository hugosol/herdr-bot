import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

async function post(endpoint: string, body: unknown) {
  const response = await fetch(`${server!.baseURL}${endpoint}`, {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body),
  })
  expect(response.ok, await response.clone().text()).toBe(true)
  return response.json()
}

for (const [agent, width] of [['pi', 1280], ['omp', 390]] as const) {
  test(`Retro prefills ${agent} history and submits edited input at ${width}px`, async ({ page }, info) => {
    await page.setViewportSize({ width, height: 900 })
    server = await startServer({ herdr: {} })
    const root = path.dirname(server.configPath)
    const piRoot = path.join(root, 'pi-agent')
    const ompRoot = path.join(root, 'omp-agent')
    fs.appendFileSync(path.join(root, 'paths.toml'), `pi_agent = '${piRoot}'\nomp_agent = '${ompRoot}'\n`)
    fs.writeFileSync(server.storageFile, JSON.stringify({ tasks: [{
      id: 'retro', name: 'Retro review', chatName: 'Retro {chat.id}', taskType: 'normal',
      agent: 'omp', model: 'default', thinking: 'default', skill: null, prompt: '', suite: null,
    }] }))
    await server.restart()
    const zones = await post('/api/zones', { id: 'test/history', path: root })
    const cwd = zones.zones[0].path as string
    const id = `historical-${agent}`
    await post('/api/sessions/events', { sessionId: id, kind: 'register', data: { agentType: agent, cwd, sessionName: `History ${agent}` } })
    const definitions = (await server.getTasks()).tasks
    const expected = `agent 类型：${agent}\nsession ID：${id}\n历史 cwd：${cwd}\nsession 根目录：${path.join(agent === 'pi' ? piRoot : ompRoot, 'sessions')}`
    await page.goto(server.baseURL)
    await page.getByRole('button', { name: '查看历史会话' }).click()
    await expect(page.getByRole('button', { name: 'Retro', exact: true })).toBeVisible()
    await page.screenshot({ path: info.outputPath(`retro-history-${agent}.png`), fullPage: true })
    await page.getByRole('row').filter({ hasText: `History ${agent}` }).getByRole('button', { name: 'Retro', exact: true }).click()
    const dialog = page.getByTestId('chat-creation-dialog')
    await expect(dialog).toBeVisible()
    await expect(dialog.getByTestId('task')).toHaveValue('retro')
    await expect(dialog.getByTestId('chat-zone')).toHaveValue('test/history')
    await expect(dialog.getByTestId('chat-input')).toHaveValue(expected)
    expect(server.herdrTabs()).toEqual([])
    await dialog.getByTestId('chat-input').fill(`${expected}\n仅分析失败原因。`)
    await dialog.getByTestId('chat-create-close').click()
    expect(server.herdrTabs()).toEqual([])
    await page.getByRole('row').filter({ hasText: `History ${agent}` }).getByRole('button', { name: 'Retro', exact: true }).click()
    await expect(dialog.getByTestId('chat-input')).toHaveValue(expected)
    await dialog.getByTestId('chat-input').fill(`${expected}\n用户确认的要求`)
    await dialog.locator('#chat-workspace-target').selectOption('default')
    await expect(dialog.getByTestId('chat-create-submit')).toBeEnabled()
    await page.screenshot({ path: info.outputPath(`retro-${agent}.png`), fullPage: true })
    await dialog.getByTestId('chat-create-submit').click()
    await expect(dialog.getByTestId('chat-create-success')).toHaveText('创建成功，初始指示已提交', { timeout: 15_000 })
    expect(server.herdrCalls().filter(call => call.kind === 'agent_prompt').map(call => call.text)).toEqual([`${expected}\n用户确认的要求`])
    expect(server.herdrTabs()).toHaveLength(1)
    expect((await server.getTasks()).tasks).toEqual(definitions)
  })
}

test('Retro leaves missing Task and missing or ambiguous zone unselected', async ({ page }) => {
  server = await startServer({ herdr: {} })
  const root = path.dirname(server.configPath)
  const saved = await post('/api/tasks', { name: 'Other task', agent: 'omp', chatName: 'Other {chat.id}', model: 'default', thinking: 'default', skill: null, prompt: '' })
  await post('/api/sessions/events', { sessionId: 'missing-defaults', kind: 'register', data: { agentType: 'pi', cwd: root, sessionName: 'Missing defaults' } })
  await page.goto(server.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  const retro = page.getByRole('row').filter({ hasText: 'Missing defaults' }).getByRole('button', { name: 'Retro', exact: true })
  await retro.click()
  const dialog = page.getByTestId('chat-creation-dialog')
  await expect(dialog.getByText('默认 Task「retro」不存在，请选择。')).toBeVisible()
  await expect(dialog.getByTestId('task')).toHaveValue('')
  await expect(dialog.getByTestId('chat-zone')).toHaveValue('')
  await expect(dialog.getByTestId('chat-input')).toHaveValue(/session 根目录：未配置，请补充/)
  await expect(dialog.getByTestId('chat-create-submit')).toBeDisabled()
  await dialog.getByTestId('chat-create-close').click()
  await post('/api/zones', { id: 'test/one', path: root })
  await post('/api/zones', { id: 'test/two', path: root })
  await retro.click()
  await expect(dialog.getByText('历史目录未唯一匹配已登记 zone，请选择。')).toBeVisible()
  await expect(dialog.getByTestId('chat-zone')).toHaveValue('')
  await dialog.getByTestId('task').selectOption(saved.tasks[0].id)
  await dialog.getByTestId('chat-zone').selectOption('test/two')
  await expect(dialog.getByTestId('chat-create-submit')).toBeEnabled()
  expect(server.herdrTabs()).toEqual([])
})
