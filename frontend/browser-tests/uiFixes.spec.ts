import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { expect, test } from '@playwright/test'

import { startServer, type E2EServer } from './support'
import {
  expectNoHorizontalOverflow,
  jsonPost,
  serializedChatRule,
  zoneRowFor,
} from './ui'

let server: E2EServer | null = null
let zoneRoot: string | null = null

function contrastRatio(foreground: string, background: string): number {
  const rgb = (css: string): [number, number, number] => {
    const match = css.match(/rgba?\(([^)]+)\)/)
    if (match === null) throw new Error(`无法解析颜色：${css}`)
    const [red, green, blue] = match[1].split(',').slice(0, 3).map(Number)
    return [red, green, blue]
  }
  const luminance = (css: string) => {
    const channels = rgb(css).map((channel) => {
      const value = channel / 255
      return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
    })
    return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]
  }
  const first = luminance(foreground)
  const second = luminance(background)
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05)
}

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

test('顶层标签、变量块增删、光标插入和下拉开字对比度', async ({ page }) => {
  server = await startServer()
  await page.goto(`${server.baseURL}?layout=expanded`)

  await expect(page.getByTestId('tab-sessions')).toHaveAttribute('aria-selected', 'true')

  await page.getByTestId('tab-tasks').click()
  await expect(page.getByTestId('task-empty')).toBeVisible()
  await page.getByTestId('task-create-open').click()
  const dialog = page.getByTestId('task-dialog')
  await expect(dialog).toBeVisible()

  // 选中项文本与控件值一致；实际渲染颜色对深色背景保持可读对比度。
  const model = page.getByTestId('task-model')
  const initialModel = await model.evaluate((select) => {
    const control = getComputedStyle(select)
    const option = select.selectedOptions[0]
    const optionStyle = getComputedStyle(option)
    return {
      color: control.color,
      backgroundColor: control.backgroundColor,
      optionColor: optionStyle.color,
      selectedText: option.textContent,
    }
  })
  expect(initialModel.selectedText).toBe('Default Model')
  expect(contrastRatio(initialModel.color, initialModel.backgroundColor)).toBeGreaterThan(4.5)
  expect(contrastRatio(initialModel.optionColor, initialModel.backgroundColor)).toBeGreaterThan(4.5)

  await model.selectOption('deepseek-flash')
  const selectedModel = await model.evaluate((select) => ({
    value: select.value,
    selectedText: select.selectedOptions[0]?.textContent ?? '',
    color: getComputedStyle(select).color,
    backgroundColor: getComputedStyle(select).backgroundColor,
  }))
  expect(selectedModel.value).toBe('deepseek-flash')
  expect(selectedModel.selectedText).toBe('DeepSeek-Flash')
  expect(contrastRatio(selectedModel.color, selectedModel.backgroundColor)).toBeGreaterThan(4.5)

  const thinking = page.getByTestId('task-thinking')
  const skill = page.getByTestId('task-skill')
  await expect(thinking).toHaveValue('default')
  await expect(skill).toHaveValue('')
  expect(
    await thinking.evaluate((select) => select.selectedOptions[0]?.textContent),
  ).toBe('沿用 Agent 默认值')
  expect(await skill.evaluate((select) => select.selectedOptions[0]?.textContent)).toBe('无')

  // 变量编辑：光标位于 A 与 B 之间时插入，而不是追加到末尾。
  const chatName = page.getByTestId('task-chat-name')
  await chatName.fill('AB')
  await page.keyboard.press('ArrowLeft')
  await page.getByTestId('insert-zone-name').click()
  expect(await serializedChatRule(chatName)).toBe('A{zone.name}B')
  await expect(page.getByTestId('chat-name-preview')).toHaveText('AalphaB')

  // × 删除变量块，前后固定文字保留。
  await chatName.getByRole('button', { name: '删除变量 zone.name' }).click()
  expect(await serializedChatRule(chatName)).toBe('AB')
  await expect(chatName.getByTestId('chat-name-token')).toHaveCount(0)
  await expect(page.getByTestId('chat-name-preview')).toHaveText('AB')

  // × 按钮本身也可用键盘激活，不被编辑器的 Enter 单行限制吞掉。
  await chatName.fill('K')
  await page.getByTestId('insert-chat-id').click()
  const keyboardRemove = chatName.getByRole('button', { name: '删除变量 chat.id' })
  await keyboardRemove.focus()
  await page.keyboard.press('Enter')
  expect(await serializedChatRule(chatName)).toBe('K')
  await expect(chatName.getByTestId('chat-name-token')).toHaveCount(0)

  // 变量块后按退格会整块删除，而不是只删除 × 或一个字符。
  await chatName.fill('X')
  await page.getByTestId('insert-agent').click()
  expect(await serializedChatRule(chatName)).toBe('X{agent}')
  await expect(chatName).toBeFocused()
  await page.keyboard.press('Backspace')
  expect(await serializedChatRule(chatName)).toBe('X')
  await expect(chatName.getByTestId('chat-name-token')).toHaveCount(0)
  await expect(page.getByTestId('chat-name-preview')).toHaveText('X')
})

test('手机宽度下 zone 只显示末级名称与数字角标', async ({ page }) => {
  server = await startServer()
  zoneRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'herdr-bot ui zones '))
  const first = path.join(zoneRoot, 'one', 'alpha')
  const second = path.join(zoneRoot, 'two', 'alpha')
  fs.mkdirSync(first, { recursive: true })
  fs.mkdirSync(second, { recursive: true })
  execFileSync('git', ['init', first], { stdio: 'ignore' })
  fs.writeFileSync(path.join(first, 'untracked.txt'), 'pending\n', 'utf8')

  await jsonPost(server.baseURL, '/api/zones', { path: first })
  await jsonPost(server.baseURL, '/api/zones', { path: second })

  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto(`${server.baseURL}?layout=expanded`)
  await page.getByTestId('tab-files').click()
  await expect(page.getByTestId('zone-list')).toBeVisible()

  const firstRow = zoneRowFor(page, first)
  const secondRow = zoneRowFor(page, second)
  await expect(firstRow).toHaveCount(1)
  await expect(secondRow).toHaveCount(1)
  await expect(firstRow.getByTestId('open-zone')).toHaveText('alpha')
  await expect(secondRow.getByTestId('open-zone')).toHaveText('alpha')
  expect(await firstRow.getByTestId('open-zone').innerText()).not.toContain(first)
  await expect(firstRow.getByTestId('open-zone')).toHaveAttribute('title', first)
  await expect(secondRow.getByTestId('open-zone')).toHaveAttribute('title', second)
  await expect(firstRow.getByTestId('zone-detail')).toHaveCount(0)
  await expect(secondRow.getByTestId('zone-detail')).toHaveCount(0)

  const badge = page.getByTestId('zone-badge')
  await expect(badge).toHaveCount(1)
  await expect(badge).toHaveText('1')
  await expect(badge).toHaveAttribute('aria-label', '1 项待提交')
  await expect(page.getByText('项待提交')).toHaveCount(0)
  await expectNoHorizontalOverflow(page)

  // 320px 是常见的小屏宽度：数字角标和操作按钮不能让列表横向溢出。
  await page.setViewportSize({ width: 320, height: 720 })
  await expect(firstRow.getByTestId('open-zone')).toBeVisible()
  await expectNoHorizontalOverflow(page)
})
