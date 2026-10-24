import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved S1/S2/S3, P5/P6/P15: the normal launch establishes the trusted source;
// real isolated files then disappear. A successful partial recovery must identify
// the skipped item, while an empty explicit set without identity stays unconfirmed.
// Herdr is controlled here: these argv observations do NOT prove real loading.
for (const agent of ['pi', 'omp']) {
  test(`${agent} page shows each filtered extension and preserves empty-set uncertainty`, { tag: '@time-contract' }, async ({ page }, info) => {
    server = await startServer({ herdr: {} })
    const root = path.dirname(server.configPath)
    const cwd = path.join(root, `filter-${agent}`)
    fs.mkdirSync(cwd)
    const fixed = path.join(cwd, 'identity-fixed.ts')
    const selected = path.join(cwd, 'selected-optional.ts')
    fs.writeFileSync(fixed, "throw new Error('validation must never import this fixture')\n")
    fs.writeFileSync(selected, "throw new Error('validation must never import this fixture')\n")
    fs.writeFileSync(server.metadataFile, `skills = []\n[[agents]]\nid = '${agent}'\nlabel = '${agent}'\n[[models]]\nid = 'default'\nlabel = 'Default'\n[[extensions]]\nid = 'identity-fixed'\nlabel = 'Identity fixed'\nagent = '${agent}'\nkind = 'fixed'\nentrypoint = '${fixed}'\n[[extensions]]\nid = 'selected-optional'\nlabel = 'Selected optional'\nagent = '${agent}'\nkind = 'optional'\nentrypoint = '${selected}'\n`)
    const id = `page-${agent}-filter-native`
    const native = path.join(cwd, 'original.jsonl')
    fs.writeFileSync(native, `${JSON.stringify({ type: 'session', version: 3, id, cwd })}\n`)
    const identity = { native_session: { source: `herdr:${agent}`, agent, kind: 'path', value: native } }
    server.setHerdrBehavior(identity)
    const post = (endpoint: string, body: unknown) => fetch(`${server!.baseURL}${endpoint}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) })
    expect((await post('/api/zones', { path: cwd })).status).toBe(200)
    const saved = await post('/api/bots', { name: `Filter ${agent}`, chatName: `Filter ${agent} {chat.id}`, agent, model: 'default', thinking: 'default', skill: null, prompt: 'ONLY INITIAL', extensions: ['selected-optional'] })
    expect(saved.status).toBe(200)
    const botId = (await saved.json()).bots[0].id
    const preview = await (await post('/api/chats/preview', { zone: cwd, botId })).json()
    const launch = await post('/api/chats', { zone: cwd, botId, chatId: preview.chatId, input: 'seed' })
    expect(launch.status).toBe(202)
    const job = await launch.json()
    await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/chats/${job.id}`)).json()).stage).toBe('succeeded')
    await expect.poll(async () => (await (await fetch(`${server!.baseURL}/api/sessions/history`)).json()).total).toBe(1)
    fs.unlinkSync(selected)
    const before = server.herdrCalls().length
    await page.goto(server.baseURL)
    await page.getByRole('button', { name: '查看历史会话' }).click()
    const row = page.getByRole('row').filter({ hasText: `Filter ${agent}` })
    const response = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    const partial = await (await response).json()
    expect(partial.outcome).toBe('restored')
    expect(partial.degradations).toHaveLength(1)
    expect(partial.degradations[0].element).toBe('extension')
    const status = row.getByRole('status')
    for (const text of ['恢复存在降级', 'Selected optional', 'selected-optional', '入口不存在', '跳过', '仍关闭扩展自动发现', '已确认恢复同一']) await expect(status).toContainText(text)
    const resumed = server.herdrCalls().slice(before)
    expect(resumed.find(c => c.kind === 'agent_start')!.args).toEqual(['--no-extensions', '--extension', fixed, agent === 'omp' ? '--resume' : '--session', native])
    expect(resumed.some(c => ['agent_prompt', 'agent_input'].includes(String(c.kind)))).toBe(false)
    await page.screenshot({ path: info.outputPath(`${agent}-partial-filter.png`), fullPage: true })

    fs.unlinkSync(fixed)
    server.setHerdrBehavior({ native_session: null })
    const emptyBefore = server.herdrCalls().length
    const emptyResponse = page.waitForResponse(r => r.url().endsWith('/api/chats/resume') && r.request().method() === 'POST')
    await row.getByRole('button', { name: 'Resume', exact: true }).click()
    const response2 = await emptyResponse
    expect(response2.status()).toBe(502)
    const empty = await response2.json()
    expect(empty.outcome).toBe('unconfirmed')
    expect(empty.stage).toBe('confirm')
    expect(empty.degradations).toHaveLength(2)
    const alert = row.getByRole('alert')
    for (const text of ['Identity fixed', 'identity-fixed', 'Selected optional', 'selected-optional', '入口不存在', '跳过', '仍关闭扩展自动发现', '启动结果未确认']) await expect(alert).toContainText(text)
    await expect(alert).not.toContainText('已确认恢复同一')
    const emptyCalls = server.herdrCalls().slice(emptyBefore)
    expect(emptyCalls.filter(c => c.kind === 'agent_start')).toHaveLength(1)
    expect(emptyCalls.find(c => c.kind === 'agent_start')!.args).toEqual(['--no-extensions', agent === 'omp' ? '--resume' : '--session', native])
    expect(emptyCalls.some(c => ['agent_prompt', 'agent_input', 'agent_focus'].includes(String(c.kind)))).toBe(false)
    await page.screenshot({ path: info.outputPath(`${agent}-empty-unconfirmed.png`), fullPage: true })
    const evidence = info.outputPath(`${agent}-extension-filter-NOT-real-loading.json`)
    fs.writeFileSync(evidence, JSON.stringify({ scope: 'Controlled Herdr; not real extension loading', agent, cwd, native, botId, partial, empty, partialCalls: resumed, emptyCalls }, null, 2))
    await info.attach('extension-filter-NOT-real-loading', { path: evidence, contentType: 'application/json' })
  })
}
