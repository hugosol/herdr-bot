import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'
import { startServer } from '../../support/support'

// P40/P41, S1/S2/S4. Selecting a Map from another release, silently restoring
// its legacy file interface, or starting a Run to inspect the definition would
// disagree with the real page, independent expected graph, or host recorder.
test('selected creation Map is visible with only a textual business request', async ({ page, request }, info) => {
  const server = await startServer({ separateCwd: true, herdr: {} })
  try {
    const release = fileURLToPath(new URL('../../../../meta-data/', import.meta.url))
    const maps = path.join(server.cwd, 'meta-data/workflow-maps')
    const bots = path.join(server.cwd, 'meta-data/bots')
    fs.mkdirSync(maps, { recursive: true })
    fs.mkdirSync(bots, { recursive: true })
    const savedPath = path.join(maps, 'plan_workflow.json')
    fs.copyFileSync(path.join(release, 'workflow-maps/plan_workflow.json'), savedPath)
    for (const file of [
      'bot-1794388558375394800-31524-0.json', 'bot-1793304246857557100-58200-0.json',
      'bot-1793305512701853300-58200-1.json', 'bot-1794222010819446100-10104-0.json',
    ]) fs.copyFileSync(path.join(release, 'bots', file), path.join(bots, file))
    fs.copyFileSync(path.join(release, 'tasks.json'), path.join(path.dirname(server.configPath), 'tasks.json'))
    const original = fs.readFileSync(savedPath)
    const readMap = await request.get(`${server.baseURL}/api/scheduler/maps/plan_workflow.json`)
    expect(readMap.ok()).toBe(true)
    const definition = await readMap.json()
    expect(definition.title).toBe('创建map')
    expect(definition.map_input_rule).toBeUndefined()
    expect(definition.map_output_rule).toBeUndefined()

    await page.goto(`${server.baseURL}?layout=expanded`)
    await page.getByRole('tab', { name: 'scheduler', exact: true }).click()
    await page.getByRole('button', { name: '创建map', exact: true }).click()
    for (const title of ['概要规划与用户确认', '概要方案转换为业务 Map', 'prompt优化者——Map维度']) {
      await expect(page.getByRole('button', { name: new RegExp(title) }).first()).toBeVisible()
    }
    await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
    const launch = page.getByRole('form', { name: '发起 Run', exact: true })
    await expect(launch.getByRole('textbox', { name: '本次要求', exact: true })).toBeVisible()
    const bindings = launch.getByRole('group', { name: 'Run 文件输入绑定' })
    await expect(bindings).toContainText('此 Map 无文件输入声明，无需绑定文件。')
    await expect(bindings.getByRole('textbox')).toHaveCount(0)
    await page.screenshot({ path: info.outputPath('selected-creation-map.png'), fullPage: true })
    expect(fs.readFileSync(savedPath)).toEqual(original)
    expect((await (await request.get(`${server.baseURL}/api/workflow/runs`)).json()).runs).toEqual([])
    expect(server.herdrCalls()).toEqual([])
    await info.attach('selected-map-http.json', { body: JSON.stringify(definition, null, 2), contentType: 'application/json' })
  } finally {
    await server.cleanup()
  }
})
