import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

const examples = fileURLToPath(new URL('../../docs/example/', import.meta.url))
let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// Approved seam S1. P31/P32 source: the guide's independently prescribed
// A/B -> C, isolated D, and cross-Map E. Copy the delivered bytes, never build
// equivalent fixtures here. A missing independent node, reversed/extra edge,
// filename-based template identity, or wrong node parameters must disagree.
for (const viewport of [{ width: 1280, height: 900 }, { width: 390, height: 844 }]) {
  test(`P31 P32 delivered examples follow the guide at ${viewport.width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport)
    server = await startServer({ separateCwd: true, herdr: {} })
    await page.goto(server.baseURL)
    await page.getByTestId('tab-scheduler').click()
    await expect(page.getByTestId('scheduler-empty')).toHaveText('此目录没有 Map JSON 文件。')
    await expect(page.getByTestId('scheduler-template-empty')).toBeVisible()
    expect(fs.readdirSync(path.join(server.cwd, '.data', 'workflow-maps'))).toEqual([])
    expect(fs.readdirSync(path.join(server.cwd, '.data', 'node-templates'))).toEqual([])

    // Same relative directories and copies as the PowerShell guide. The config
    // is intentionally elsewhere: resolving relative to its folder must fail.
    await server.stop()
    fs.appendFileSync(server.configPath, "\n[scheduler]\nmap_directory = 'plans'\nnode_template_directory = 'capabilities'\n")
    fs.mkdirSync(path.join(server.cwd, 'plans'))
    fs.mkdirSync(path.join(server.cwd, 'capabilities'))
    for (const filename of ['trip-readiness.json', 'another-trip.json']) {
      fs.copyFileSync(path.join(examples, 'maps', filename), path.join(server.cwd, 'plans', filename))
    }
    for (const filename of ['weather-capability.json', 'quota-capability.json', 'summary-capability.json']) {
      fs.copyFileSync(path.join(examples, 'node-templates', filename), path.join(server.cwd, 'capabilities', filename))
    }
    await server.restart()
    await page.goto(server.baseURL)
    await page.getByTestId('tab-scheduler').click()
    const list = page.getByTestId('scheduler-map-list')
    await expect(list.getByRole('button')).toHaveText(['another-trip.json', 'trip-readiness.json'])
    await page.getByRole('button', { name: 'trip-readiness.json', exact: true }).click()
    const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
    const details = page.getByRole('region', { name: 'node 详情', exact: true })
    await expect(graph.getByRole('button')).toHaveCount(4)
    for (const name of ['上海出行天气 A', '北京出行天气 B', '汇总出行准备 C', '独立额度估算 D']) {
      await expect(graph.getByRole('button', { name, exact: true })).toBeVisible()
    }
    await expect(graph.getByRole('img')).toHaveCount(2)
    await expect(graph.getByRole('img', { name: 'A → C', exact: true })).toBeAttached()
    await expect(graph.getByRole('img', { name: 'B → C', exact: true })).toBeAttached()
    for (const [name, expected] of [
      ['上海出行天气 A', { city: '上海', date: '2026-11-01', preferences: { walking: true, notes: ['带伞', null] } }],
      ['北京出行天气 B', { city: '北京', date: '2026-11-02', zone: '尚未登记的出行资料目录' }],
      ['汇总出行准备 C', { scope: '上海与北京的出行准备', checklist: ['天气', '随身物品'] }],
      ['独立额度估算 D', { account: '人工示例账户', remaining_percent: 25, observed_at: null }],
    ] as const) {
      await graph.getByRole('button', { name, exact: true }).click()
      const params = details.getByRole('region', { name: '本次参数', exact: true })
      await expect(params.locator('dd')).toHaveCount(Object.keys(expected).length)
      for (const [key, value] of Object.entries(expected)) {
        const rendered = params.getByTestId(`parameter-${key}`)
        if (typeof value === 'string') await expect(rendered).toHaveText(value)
        else expect(JSON.parse((await rendered.textContent())!)).toEqual(value)
      }
      const prerequisites = details.getByRole('region', { name: '前置依赖', exact: true })
      if (name === '汇总出行准备 C') {
        await expect(prerequisites.getByRole('listitem')).toHaveText(['上海出行天气 · A', '北京出行天气 · B'])
      } else await expect(prerequisites).toContainText('无前置依赖')
      await expect(details.getByRole('button', { name: '模板明细', exact: true })).toHaveAttribute('aria-expanded', 'false')
      const expectedTemplate = name === '汇总出行准备 C' ? [
        '出行准备清单汇总', '需要将多个目的地的准备事项整理为核对清单时。',
        '按指定范围整理准备事项与待确认问题；前置依赖不自动传入天气结果，也不预订或修改任何资源。',
        'scope', '本次清单涵盖的出行范围。', 'checklist', '人工给定的核对主题说明。',
        'summary', '准备事项及待确认问题的清单；不是已经完成的工作。',
      ] : name === '独立额度估算 D' ? [
        '额度恢复时间估算', '需要根据人工观察的额度情况安排后续工作时。',
        '根据给定观察说明估算恢复时间与不确定性；不登录账户，不读取实际用量，也不承诺服务商重置规则。',
        'account', '人工区分账户的说明，不是登录凭据。', 'remaining_percent', '人工观察的剩余额度说明。',
        'observed_at', '人工观察时间；未知时可在参数中写 null。', 'estimate', '恢复时间估算及依据；不是实际额度记录。',
      ] : ['目的地天气摘要']
      await details.getByRole('button', { name: '模板明细', exact: true }).click()
      for (const text of [...expectedTemplate, 'chat']) {
        await expect(page.getByRole('region', { name: '模板明细', exact: true }).getByText(text, { exact: true })).toBeVisible()
      }
      await details.getByRole('button', { name: '模板明细', exact: true }).click()
    }
    await page.screenshot({ path: testInfo.outputPath('graph.png'), fullPage: true })
    await graph.getByRole('button', { name: '上海出行天气 A', exact: true }).click()
    await details.getByRole('button', { name: '模板明细', exact: true }).click()
    const capability = page.getByRole('region', { name: '模板明细', exact: true })
    for (const text of ['目的地天气摘要', '出行前需要了解目的地天气与随身物品建议时。',
      '整理指定城市和日期的天气摘要及出行建议；不预订行程，不提供实时查询保证。',
      'city', '需要了解天气的城市名称。', 'date', '计划出行日期的文字说明。',
      'forecast', '温度、降水与随身物品建议的文本摘要；不是已查询的天气结果。', 'chat']) {
      await expect(capability.getByText(text, { exact: true })).toBeVisible()
    }
    await page.screenshot({ path: testInfo.outputPath('details.png'), fullPage: true })
    await graph.getByRole('button', { name: '北京出行天气 B', exact: true }).click()
    await details.getByRole('button', { name: '模板明细', exact: true }).click()
    await expect(capability.getByText('目的地天气摘要', { exact: true })).toBeVisible()
    await page.getByRole('button', { name: 'another-trip.json', exact: true }).click()
    await graph.getByRole('button', { name: '广州出行天气 E', exact: true }).click()
    await expect(details.getByTestId('parameter-city')).toHaveText('广州')
    await expect(details.getByTestId('parameter-date')).toHaveText('2026-11-03')
    await details.getByRole('button', { name: '模板明细', exact: true }).click()
    await expect(capability.getByText('目的地天气摘要', { exact: true })).toBeVisible()

    // Follow the reload instructions against these same delivered files.
    const template = path.join(server.cwd, 'capabilities', 'weather-capability.json')
    fs.writeFileSync(template, fs.readFileSync(template, 'utf8').replace('目的地天气摘要', '更新后的目的地天气摘要'))
    await page.getByRole('button', { name: '重新加载当前 Map', exact: true }).click()
    await graph.getByRole('button', { name: '广州出行天气 E', exact: true }).click()
    await details.getByRole('button', { name: '模板明细', exact: true }).click()
    await expect(capability.getByText('更新后的目的地天气摘要', { exact: true })).toBeVisible()
    fs.copyFileSync(path.join(examples, 'maps', 'another-trip.json'), path.join(server.cwd, 'plans', 'new-copy.json'))
    await page.getByRole('button', { name: '刷新 Map 列表', exact: true }).click()
    await expect(page.getByRole('button', { name: 'new-copy.json', exact: true })).toBeVisible()
    await page.getByTestId('server-button').click()
    await expect(page.getByTestId('server-page')).toBeVisible()
    await page.getByRole('button', { name: '返回 scheduler', exact: true }).click()
    await expect(page.getByTestId('tab-scheduler')).toHaveAttribute('aria-selected', 'true')
    expect(server.herdrCalls()).toEqual([])
  })
}
