import fs from 'node:fs'
import path from 'node:path'
import { expect, test, type Locator, type Page } from '@playwright/test'
import { startServer, type E2EServer } from './support'

let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved seam S1, P36. Expected outcomes are the shipped example's literal
// Shanghai node and the independently authored edits below. A mouse-only action,
// cropped input/error, lost draft, or stale return selection must fail this flow.
// Real HTTP, files, process and viewport remain in the path; no response mocks.
async function tabTo(page: Page, target: Locator) {
  await expect(target).toBeVisible()
  const controls = await page.locator('button, input, textarea, summary, [tabindex="0"]').count()
  for (let n = 0; n <= controls && !(await target.evaluate(el => el === document.activeElement)); n++) {
    await page.keyboard.press('Tab')
  }
  await expect(target).toBeFocused()
  const box = await target.boundingBox()
  expect(box).not.toBeNull()
  const viewport = page.viewportSize()!
  expect(box!.x).toBeGreaterThanOrEqual(0)
  expect(box!.x + box!.width).toBeLessThanOrEqual(viewport.width)
  // Native textarea focus scrolls its caret, not necessarily the entire field.
  // Require an unobscured visible editing surface; actual typing and subsequent
  // Tab/Enter save below prove usability without requiring all JSON on screen.
  const top = Math.max(0, box!.y), bottom = Math.min(viewport.height, box!.y + box!.height)
  expect(bottom - top).toBeGreaterThan(1)
  expect(await target.evaluate((el, point) => {
    const hit = document.elementFromPoint(point.x, point.y)
    return hit !== null && (hit === el || el.contains(hit))
  }, { x: box!.x + box!.width / 2, y: (top + bottom) / 2 })).toBe(true)
}
async function activate(page: Page, target: Locator) {
  await tabTo(page, target)
  await page.keyboard.press('Enter')
}
async function type(page: Page, target: Locator, value: string) {
  await tabTo(page, target)
  await page.keyboard.press('ControlOrMeta+A')
  await page.keyboard.insertText(value)
}
async function fitted(page: Page) {
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(page.viewportSize()!.width)
}
async function discard(page: Page, target: Locator, accept: boolean) {
  const observed = page.waitForEvent('dialog')
  const activation = activate(page, target)
  const dialog = await observed
  expect(dialog.type()).toBe('confirm')
  if (accept) await dialog.accept(); else await dialog.dismiss()
  await activation
}

for (const [size, width, height] of [['desktop', 1280, 900], ['narrow', 390, 844]] as const) {
  test(`P36 ${size}: keyboard manages both definitions, checks diagnostics and recovers retained drafts`, async ({ page }, testInfo) => {
    test.setTimeout(180_000)
    await page.setViewportSize({ width, height })
    server = await startServer({ herdr: {}, safeShutdown: true, separateCwd: true })
    const maps = path.join(server.cwd, '.data/workflow-maps')
    const templates = path.join(server.cwd, '.data/node-templates')
    fs.mkdirSync(maps, { recursive: true }); fs.mkdirSync(templates, { recursive: true })
    const examples = path.resolve(import.meta.dirname, '../../docs/example')
    fs.copyFileSync(path.join(examples, 'maps/trip-readiness.json'), path.join(maps, 'trip-readiness.json'))
    fs.copyFileSync(path.join(examples, 'node-templates/weather-capability.json'), path.join(templates, 'weather-capability.json'))
    fs.writeFileSync(path.join(templates, 'broken.json'), '{ invalid JSON')
    await page.goto(server.baseURL)
    const button = (name: string) => page.getByRole('button', { name, exact: true })
    await activate(page, page.getByTestId('tab-scheduler'))
    await fitted(page)
    const problems = page.getByRole('region', { name: '模板问题', exact: true })
    await expect(problems).toContainText('broken.json')
    await expect(problems).toContainText('结构无效')
    await activate(page, button('trip-readiness.json'))
    const selected = button('上海出行天气 A')
    await activate(page, selected)
    await activate(page, button('模板明细'))
    const preview = page.getByRole('region', { name: '模板预览', exact: true })
    await expect(preview).toContainText('destination-weather')
    await expect(preview).toContainText('目的地天气摘要')
    await page.screenshot({ path: testInfo.outputPath(`${size}-template-preview.png`), fullPage: true })
    await activate(page, button('修改模板'))
    const templateID = page.getByLabel('模板 ID', { exact: true })
    const templateJSON = page.getByLabel('模板内容 JSON', { exact: true })
    await expect(templateID).toHaveValue('destination-weather')
    await expect(templateID).toHaveAttribute('readonly', '')
    const weather = JSON.parse(await templateJSON.inputValue())
    await type(page, templateJSON, JSON.stringify({ ...weather, description: '人工核对新版天气说明，不执行查询。' }, null, 2))
    await activate(page, button('保存模板'))
    await expect(preview).toContainText('人工核对新版天气说明，不执行查询。')
    await activate(page, button('返回 Map'))
    await expect(selected).toHaveAttribute('aria-pressed', 'true')
    await activate(page, button('修改 node'))
    const nodeJSON = page.getByLabel('node 内容 JSON', { exact: true })
    const node = JSON.parse(await nodeJSON.inputValue())
    await type(page, nodeJSON, JSON.stringify({ ...node, title: '上海天气复核' }, null, 2))
    await activate(page, button('保存 node'))
    await expect(button('上海天气复核 A')).toHaveAttribute('aria-pressed', 'true')

    // New independent ID, raw invalid JSON and server-side conflict must remain
    // usable on the actual narrow page, not merely present in a screenshot.
    await activate(page, button('新增 node'))
    const nodeID = page.getByLabel('node ID', { exact: true })
    await type(page, nodeID, 'keyboard-node')
    await type(page, nodeJSON, '{ "title": ')
    await activate(page, button('保存 node'))
    const error = page.getByRole('alert').filter({ hasText: 'JSON' })
    await expect(error).toBeVisible()
    await expect(nodeID).toHaveValue('keyboard-node')
    await expect(nodeJSON).toHaveValue('{ "title": ')
    await fitted(page)
    await error.scrollIntoViewIfNeeded()
    await page.screenshot({ path: testInfo.outputPath(`${size}-validation-draft.png`), fullPage: true })
    const newNode = { title: '键盘新增工作', node_template_id: 'destination-weather', parameters: { city: '杭州', id: '保留业务参数' }, blocked_by: [] }
    const raw = JSON.stringify(newNode, null, 2)
    await type(page, nodeJSON, raw)
    await discard(page, button('放弃编辑'), false)
    await expect(nodeJSON).toHaveValue(raw)
    await activate(page, button('保存 node'))
    await expect(button('键盘新增工作 keyboard-node')).toHaveAttribute('aria-pressed', 'true')
    await activate(page, button('修改 node'))
    const conflictDraft = JSON.stringify({ ...newNode, title: '未提交的窄屏草稿' }, null, 2)
    await type(page, nodeJSON, conflictDraft)
    const file = path.join(maps, 'trip-readiness.json')
    const external = JSON.parse(fs.readFileSync(file, 'utf8'))
    external.description = '外部编辑器已经更新 Map。'
    fs.writeFileSync(file, JSON.stringify(external))
    await activate(page, button('保存 node'))
    const conflict = page.getByRole('alert').filter({ hasText: '重新加载' })
    await expect(conflict).toBeVisible()
    await expect(nodeID).toHaveValue('keyboard-node')
    await expect(nodeJSON).toHaveValue(conflictDraft)
    await conflict.scrollIntoViewIfNeeded()
    await fitted(page)
    await page.screenshot({ path: testInfo.outputPath(`${size}-conflict-draft.png`), fullPage: true })
    await discard(page, button('返回 Map'), false)
    await expect(nodeJSON).toHaveValue(conflictDraft)
    await discard(page, button('返回 Map'), true)
    await activate(page, button('修改 node'))
    expect(JSON.parse(await nodeJSON.inputValue())).toEqual(newNode)
    await type(page, nodeJSON, JSON.stringify({ ...newNode, title: '冲突后重新保存' }, null, 2))
    await activate(page, button('保存 node'))
    await expect(button('冲突后重新保存 keyboard-node')).toHaveAttribute('aria-pressed', 'true')
    await activate(page, button('模板管理'))
    await activate(page, button('新增模板'))
    await type(page, templateID, 'keyboard-template')
    const capability = { schema_version: 1, title: '键盘核对能力', use_when: '人工核对时。', description: '只作静态说明。', inputs: { city: '城市名称。' }, outputs: { summary: '说明文本。' }, implementation: { type: 'chat' } }
    await type(page, templateJSON, JSON.stringify(capability, null, 2))
    await page.screenshot({ path: testInfo.outputPath(`${size}-template-editor.png`), fullPage: true })
    await fitted(page)
    await activate(page, button('保存模板'))
    await expect(preview).toContainText('键盘核对能力')
    await expect(page.getByRole('region', { name: '共享模板列表', exact: true })).toBeVisible()
    await activate(page, button('目的地天气摘要 · destination-weather'))
    await expect(preview).toContainText('人工核对新版天气说明，不执行查询。')
    await activate(page, button('键盘核对能力 · keyboard-template'))
    await expect(preview).toContainText('keyboard-template')
    fs.unlinkSync(path.join(templates, 'broken.json'))
    await activate(page, button('刷新全部'))
    await expect(problems).toContainText('trip-summary')
    await expect(problems).not.toContainText('broken.json')
    await fitted(page)
    expect(server.herdrCalls()).toEqual([])
    expect(server.herdrTabs()).toEqual([])
    expect(fs.existsSync(path.join(path.dirname(server.configPath), 'operations'))).toBe(false)
    expect(server.diagnostics()).not.toContain('记录到关机动作调用')
    const persisted = JSON.parse(fs.readFileSync(file, 'utf8'))
    expect(persisted.nodes['keyboard-node']).toEqual({ ...newNode, title: '冲突后重新保存' })
    expect(persisted.nodes.A.title).toBe('上海天气复核')
  })
}
