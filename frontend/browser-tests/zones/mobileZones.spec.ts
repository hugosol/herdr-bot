import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { expect, test } from '@playwright/test'

import { startServer, type E2EServer } from '../support/support'
import {
  expectNoHorizontalOverflow,
  jsonPost,
  zoneRowFor,
} from '../support/ui'

let server: E2EServer | null = null
let zoneRoot: string | null = null

test.afterEach(async () => {
  try { await server?.cleanup() }
  finally {
    server = null
    if (zoneRoot !== null) {
      fs.rmSync(zoneRoot, { recursive: true, force: true })
      zoneRoot = null
    }
  }
})

test('手机宽度下同名 zone 可区分且 Git 状态与操作不溢出', async ({ page }) => {
  server = await startServer()
  zoneRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'herdr-bot ui zones '))
  const first = path.join(zoneRoot, 'one', 'alpha')
  const second = path.join(zoneRoot, 'two', 'alpha')
  fs.mkdirSync(first, { recursive: true })
  fs.mkdirSync(second, { recursive: true })
  execFileSync('git', ['init', first], { stdio: 'ignore' })
  fs.writeFileSync(path.join(first, 'untracked.txt'), 'pending\n', 'utf8')

  await jsonPost(server.baseURL, '/api/zones', { id: 'test/first', path: first })
  await jsonPost(server.baseURL, '/api/zones', { id: 'test/second', path: second })

  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-files').click()
  await expect(page.getByTestId('zone-list')).toBeVisible()

  const firstRow = zoneRowFor(page, 'test/first')
  const secondRow = zoneRowFor(page, 'test/second')
  await expect(firstRow).toHaveCount(1)
  await expect(secondRow).toHaveCount(1)
  await expect(firstRow.locator('.zone-item__name')).toHaveText('alpha')
  await expect(secondRow.locator('.zone-item__name')).toHaveText('alpha')
  await expect(firstRow.getByTestId('open-zone')).toHaveAttribute('title', first)
  await expect(secondRow.getByTestId('open-zone')).toHaveAttribute('title', second)
  await expect(firstRow.getByTestId('zone-detail')).toHaveCount(0)
  await expect(secondRow.getByTestId('zone-detail')).toHaveCount(0)

  const badge = page.getByTestId('zone-badge')
  await expect(badge).toHaveCount(1)
  await expect(badge).toHaveAttribute('aria-label', '1 项待提交')
  await expectNoHorizontalOverflow(page)

  // 320px 是常见的小屏宽度：数字角标和操作按钮不能让列表横向溢出。
  await page.setViewportSize({ width: 320, height: 720 })
  await expect(firstRow.getByTestId('open-zone')).toBeVisible()
  await expectNoHorizontalOverflow(page)
})
