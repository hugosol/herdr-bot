
import { expect, test } from '@playwright/test'
import { startServer, type E2EServer } from '../support/support'

// S1/P2 + owned P4: keyboard events exercise native focus/activation. Keeping
// server inline alongside tabs, returning to chat unconditionally, or losing
// focus on return must fail, even though a click handler might otherwise work.
test('P2/P4: keyboard opens a same-page server view and returns to each originating tab', async ({ page, context }) => {
  server = await startServer({ safeShutdown: true })
  await page.goto(`${server.baseURL}?layout=expanded`)
  await expect(page.getByRole('button', { name: '收起导航' })).toBeVisible()
  await page.keyboard.press('Tab')
  await expect(page.getByRole('button', { name: '收起导航' })).toBeFocused()
  await page.keyboard.press('Tab')
  await expect(page.getByRole('button', { name: 'chief bot', exact: true })).toBeFocused()
  await page.keyboard.press('Tab')
  await expect(page.getByRole('button', { name: 'server', exact: true })).toBeFocused()
  // Traverse the visible native controls rather than assuming an old tab count.
  const scheduler = page.getByRole('tab', { name: 'scheduler', exact: true })
  const controls = await page.locator('button').count()
  for (let i = 0; i < controls && !await scheduler.evaluate(node => node === document.activeElement); i++) await page.keyboard.press('Tab')
  await expect(page.getByRole('tab', { name: 'scheduler', exact: true })).toBeFocused()
  await page.keyboard.press('Space')
  await expect(page.getByRole('tab', { selected: true })).toHaveText('scheduler')
  await page.keyboard.press('Home')
  const originalURL = page.url()
  for (const [index, label] of ['chat', 'zone', 'workflow', 'task', 'suite', 'bot', 'scheduler'].entries()) {
    if (index > 0) await page.keyboard.press('ArrowRight')
    await expect(page.getByRole('tab', { selected: true })).toHaveText(label)
    for (let i = 0; i <= index; i++) await page.keyboard.press('Shift+Tab')
    await expect(page.getByRole('button', { name: 'server', exact: true })).toBeFocused()
    await page.keyboard.press(index % 2 === 0 ? 'Enter' : 'Space')
    await expect(page.getByRole('region', { name: 'Server', exact: true })).toBeVisible()
    await expect(page.getByRole('tablist')).toHaveCount(0)
    await expect(page.getByRole('tabpanel')).toHaveCount(0)
    await expect(page.getByTestId('server-back')).toHaveAccessibleName('返回')
    await expect(page.getByTestId('server-button')).toHaveCount(0)
    await expect(page.getByTestId('server-back')).toBeFocused()
    expect(page.url()).toBe(originalURL)
    expect(context.pages()).toHaveLength(1)
    await page.keyboard.press('Tab')
    await expect(page.getByTestId('start-button')).toBeFocused()
    await page.keyboard.press('Shift+Tab')
    await page.keyboard.press('Enter')
    await expect(page.getByRole('tab', { name: label, exact: true })).toBeFocused()
    await expect(page.getByRole('tab', { selected: true })).toHaveText(label)
    await expect(page.getByRole('region', { name: 'Server', exact: true })).toHaveCount(0)
  }
  await page.getByRole('button', { name: 'server', exact: true }).click()
  await page.reload()
  await expect(page.getByRole('tab', { selected: true })).toHaveText('chat')
})


let server: E2EServer | null = null
test.afterEach(async () => { await server?.cleanup(); server = null })

test('P1: compact work navigation expands on desktop and reload preserves layout but returns to Chat', async ({ page }) => {
  server = await startServer()
  await page.goto(server.baseURL)
  await expect(page.getByRole('tab', { selected: true })).toHaveText('chat')
  await expect(page.getByRole('tab', { name: 'task', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '展开导航' }).click()
  await page.getByRole('tab', { name: 'scheduler', exact: true }).click()
  await expect(page.getByTestId('panel-scheduler')).toBeVisible()
  await page.reload()
  await expect(page.getByRole('tab', { selected: true })).toHaveText('chat')
  await expect(page.getByRole('button', { name: '收起导航' })).toBeVisible()
  await expect(page.getByTestId('panel-sessions')).toBeVisible()
})
