import { expect, test } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved S1/S2/S3: operate the delivered page, not a rendered component or
// mocked fetch. P6/P10/P11/P13/P14/P15/P22 require identity + focus before
// success, native resume without restore/input/Bot settings, and visible failure.
// A UI treating command submission as success would fail the wrong-ID scenario.
test('OMP history Resume confirms identity and focus without input, and exposes failed native loading', async ({ page }) => {
  server = await startServer({ herdr: {} })
  const cwd = path.join(path.dirname(server.configPath), 'OMP historical cwd with spaces')
  fs.mkdirSync(cwd)
  const native = path.join(cwd, 'old omp conversation.jsonl')
  fs.writeFileSync(native, [
    { type: 'session', version: 3, id: 'omp-browser-9ed4-exact', cwd },
    { type: 'model_change', provider: 'native-provider', modelId: 'native-model' },
    { type: 'thinking_level_change', thinkingLevel: 'low' },
  ].map((line) => JSON.stringify(line)).join('\n') + '\n')
  const bot = await fetch(`${server.baseURL}/api/bots`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name: 'Different Bot', chatName: 'Bot Chat', agent: 'omp', model: 'gpt-6.1-sol', thinking: 'high', prompt: 'BOT INITIAL MUST NEVER BE SUBMITTED', extensions: [] }) })
  expect(bot.status).toBe(200)
  const register = await fetch(`${server.baseURL}/api/sessions/events`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ sessionId: 'omp-browser-9ed4-exact', kind: 'register', data: { cwd, agentType: 'omp', sessionName: 'Historical OMP', sessionFile: native } }) })
  expect(register.status).toBe(200)
  server.setHerdrBehavior({ native_session: { agent: 'omp', source: 'herdr:omp', kind: 'path', value: native } })
  await page.goto(server.baseURL)
  await page.getByRole('button', { name: '查看历史会话' }).click()
  const row = page.getByRole('row').filter({ hasText: 'Historical OMP' })
  const response = page.waitForResponse((r) => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  const result = await (await response).json()
  expect(result.outcome).toBe('restored')
  await expect(row.getByRole('status')).toContainText('已确认恢复同一 OMP 原生对话')
  await expect(row.getByRole('status')).toContainText('不表示 Windows 窗口已置前')
  const calls = server.herdrCalls()
  expect(calls.some(call => call.kind === 'agent_prompt')).toBe(false)
  for (const value of Object.values(result.target)) await expect(row.locator('dl')).toContainText(String(value))
  expect(server.herdrTabs()[0].cwd).toBe(cwd)
  const args = calls.find((c) => c.kind === 'agent_start')!.args as string[]
  expect(args.slice(-2)).toEqual(['--resume', native])
  for (const forbidden of ['--restore', '--model', '--thinking', 'BOT INITIAL MUST NEVER BE SUBMITTED']) expect(args).not.toContain(forbidden)
  await page.screenshot({ path: test.info().outputPath('omp-resume-confirmed.png'), fullPage: true })

  server.setHerdrBehavior({ native_session: { agent: 'omp', source: 'herdr:omp', kind: 'id', value: 'different-omp-chat' } })
  const failed = page.waitForResponse((r) => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  expect((await (await failed).json()).code).toBe('native-session-mismatch')
  await expect(row.getByRole('alert')).toContainText('OMP 加载的原生对话或实际工作目录与历史不符')
  await expect(row.getByRole('alert')).toContainText('尚未确认恢复成功')
  expect(server.herdrTabs()).toHaveLength(2)
  expect(server.herdrCalls().filter((c) => c.kind === 'agent_focus')).toHaveLength(1)

  server.setHerdrBehavior({ fail_agent_list: true })
  await row.getByRole('button', { name: 'Resume', exact: true }).click()
  await expect(row.getByRole('alert')).toContainText('Herdr 查找')
  await expect(row.getByRole('alert')).toContainText('没有创建 Tab')
  expect(server.herdrTabs()).toHaveLength(2)
  expect(server.herdrCalls().some((c) => c.kind === 'agent_prompt')).toBe(false)
  await page.screenshot({ path: test.info().outputPath('omp-resume-failed.png'), fullPage: true })
})
