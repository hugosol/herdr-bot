import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from './support'

const examples = fileURLToPath(new URL('../../docs/example/', import.meta.url))
let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

// S1/S2/S4: copy the delivered examples, compare against independent literal
// inputs and A/B -> C with isolated D. Do not derive expectations from files.
for (const viewport of [{ width: 1280, height: 900 }, { width: 390, height: 844 }]) {
  test(`P31 P32 delivered examples follow the guide at ${viewport.width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport)
    server = await startServer({ separateCwd: true, herdr: {} })
    await page.goto(`${server.baseURL}?layout=expanded`)
    await page.getByTestId('tab-scheduler').click()
    await expect(page.getByTestId('scheduler-empty')).toBeVisible()
    await expect(page.getByTestId('scheduler-bot-empty')).toBeVisible()
    expect(fs.readdirSync(path.join(server.cwd, '.data/workflow-maps'))).toEqual([])
    expect(fs.readdirSync(path.join(server.cwd, '.data/bots'))).toEqual([])
    await server.stop()
    fs.appendFileSync(server.configPath, "\n[scheduler]\nmap_directory = 'plans'\nbot_directory = 'capabilities'\n")
    fs.mkdirSync(path.join(server.cwd, 'plans'))
    fs.mkdirSync(path.join(server.cwd, 'capabilities'))
    const mapTime = new Date('2026-01-01T00:00:00Z')
    for (const name of ['trip-readiness.json', 'another-trip.json']) {
      const target = path.join(server.cwd, 'plans', name)
      fs.copyFileSync(path.join(examples, 'maps', name), target)
      fs.utimesSync(target, mapTime, mapTime)
    }
    for (const name of ['weather-capability.json', 'quota-capability.json', 'summary-capability.json']) fs.copyFileSync(path.join(examples, 'bots', name), path.join(server.cwd, 'capabilities', name))
    await server.restart()
    await page.goto(`${server.baseURL}?layout=expanded`)
    await page.getByTestId('tab-scheduler').click()
    await expect(page.getByTestId('scheduler-map-list').getByRole('button')).toHaveText(['另一张出行计划（人工示例）', '出行准备核对（人工示例）'])
    await page.getByRole('button', { name: '出行准备核对（人工示例）', exact: true }).click()
    const nodes = page.getByRole('region', { name: '节点列表', exact: true })
    const graph = page.getByRole('group', { name: 'Map 依赖图', exact: true })
    const details = page.getByRole('region', { name: 'node 详情', exact: true })
    await expect(graph.getByRole('button')).toHaveCount(4)
    await expect(graph.getByRole('img')).toHaveCount(2)
    await expect(graph.getByRole('img', { name: 'A → C', exact: true })).toBeAttached()
    await expect(graph.getByRole('img', { name: 'B → C', exact: true })).toBeAttached()
    for (const [name, input, botName] of [
      ['上海出行天气 A', '城市：上海\n日期：2026-11-01\n偏好：步行；带伞。', '目的地天气摘要'],
      ['北京出行天气 B', '城市：北京\n日期：2026-11-02', '目的地天气摘要'],
      ['汇总出行准备 C', '范围：上海与北京的出行准备\n核对主题：天气、随身物品。材料位置由管理者明确提供。', '出行准备清单汇总'],
      ['独立额度估算 D', '账户：人工示例账户\n人工观察的剩余额度：25%\n观察时间：未知。', '额度恢复时间估算'],
    ]) {
      await nodes.getByRole('button', { name, exact: true }).click()
      await expect(details.getByTestId('node-input')).toHaveText(input)
      await expect(details).toContainText('C:/example/travel-not-registered')
      const prerequisites = details.getByRole('region', { name: '前置依赖', exact: true })
      if (name === '汇总出行准备 C') await expect(prerequisites.getByRole('listitem')).toHaveText(['上海出行天气 · A', '北京出行天气 · B'])
      else await expect(prerequisites).toContainText('无前置依赖')
      await details.getByRole('button', { name: 'Bot 明细', exact: true }).click()
      const bot = page.getByRole('region', { name: 'Bot 预览', exact: true })
      await expect(bot.getByRole('heading', { name: botName, exact: true })).toBeVisible()
      await expect(bot).toContainText('尚未绑定 Task')
      await page.getByTestId('tab-scheduler').click()
    }
    await page.screenshot({ path: testInfo.outputPath('graph.png'), fullPage: true })
    await nodes.getByRole('button', { name: '上海出行天气 A', exact: true }).click()
    await details.getByRole('button', { name: 'Bot 明细', exact: true }).click()
    const bot = page.getByRole('region', { name: 'Bot 预览', exact: true })
    await expect(bot.getByRole('region', { name: 'when', exact: true })).toHaveText('出行前需要了解目的地天气与随身物品建议时。')
    await bot.getByRole('button', { name: 'plan_guide', exact: true }).click()
    await expect(bot.getByRole('region', { name: 'plan_guide', exact: true })).toHaveText('整理指定城市和日期的天气摘要及出行建议；不预订行程，不提供实时查询保证。')
    await bot.getByRole('button', { name: 'node_guide', exact: true }).click()
    await expect(bot.getByRole('region', { name: 'node_guide', exact: true })).toHaveText('请在 input 中说明城市、计划出行日期与偏好。输出温度、降水与随身物品建议的文本摘要，标明信息来源和不确定性。这是工作指引，不是字段校验规则。')
    await page.screenshot({ path: testInfo.outputPath('details.png'), fullPage: true })
    await page.getByTestId('tab-scheduler').click()
    await page.getByRole('button', { name: '另一张出行计划（人工示例）', exact: true }).click()
    await nodes.getByRole('button', { name: '广州出行天气 E', exact: true }).click()
    await expect(details.getByTestId('node-input')).toHaveText('城市：广州\n日期：2026-11-03')
    await details.getByRole('button', { name: 'Bot 明细', exact: true }).click()
    await expect(bot.getByRole('heading', { name: '目的地天气摘要', exact: true })).toBeVisible()
    const file = path.join(server.cwd, 'capabilities/weather-capability.json')
    const updatedWeatherGuide = '# 最新天气指引\n<不执行查询>\n  保留原始缩进与 **文本**。'
    const weather = JSON.parse(fs.readFileSync(file, 'utf8'))
    fs.writeFileSync(file, JSON.stringify({ ...weather, name: '更新后的目的地天气摘要', node_guide: updatedWeatherGuide }))
    await page.getByTestId('tab-scheduler').click()
    await page.getByRole('button', { name: '刷新全部', exact: true }).click()
    await nodes.getByRole('button', { name: '广州出行天气 E', exact: true }).click()
    await details.getByRole('button', { name: 'Bot 明细', exact: true }).click()
    await expect(bot.getByRole('heading', { name: '更新后的目的地天气摘要', exact: true })).toBeVisible()
    await page.getByTestId('tab-scheduler').click()
    fs.copyFileSync(path.join(examples, 'maps/another-trip.json'), path.join(server.cwd, 'plans/new-copy.json'))
    await page.getByRole('button', { name: '刷新全部', exact: true }).click()
    await expect(page.getByTestId('scheduler-map-list').locator('[data-map-file="new-copy.json"]')).toHaveText('另一张出行计划（人工示例）')
    await nodes.getByRole('button', { name: '广州出行天气 E', exact: true }).click()
    fs.writeFileSync(file, '{invalid Bot JSON')
    await page.getByRole('button', { name: '刷新全部', exact: true }).click()
    await nodes.getByRole('button', { name: '广州出行天气 E', exact: true }).click()
    await expect(details.getByRole('alert')).toContainText('destination-weather')
    fs.writeFileSync(file, JSON.stringify(weather))
    await page.getByRole('button', { name: '刷新全部', exact: true }).click()
    await nodes.getByRole('button', { name: '广州出行天气 E', exact: true }).click()
    fs.unlinkSync(file)
    await page.getByRole('button', { name: '刷新全部', exact: true }).click()
    await nodes.getByRole('button', { name: '广州出行天气 E', exact: true }).click()
    await expect(details.getByRole('alert')).toContainText('destination-weather')
    await page.getByTestId('server-button').click()
    await page.getByTestId('server-back').click()
    await expect(page.getByTestId('tab-scheduler')).toHaveAttribute('aria-selected', 'true')
    expect(server.herdrCalls()).toEqual([])
  })
}
