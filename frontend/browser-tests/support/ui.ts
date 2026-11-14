/**
 * 真实浏览器用例共用的小工具：点按/点击、公开 HTTP 请求与窄屏布局断言。
 *
 * 这些断言只读浏览器可见结果与公开响应，不读取页面内部状态。
 */

import { expect } from '@playwright/test'
import type { Locator, Page } from '@playwright/test'

/** Narrow workspaces show one region at a time; use the same back/directory controls as users. */
export async function showMapNodes(page: Page) {
  const nodes = page.getByRole('region', { name: '节点列表', exact: true, includeHidden: true })
  await expect(nodes).toBeAttached()
  if (!(await nodes.isVisible())) {
    await page.getByRole('button', { name: '关闭节点详情', exact: true }).click()
  }
}
export async function showMapDirectory(page: Page) {
  if (!(await page.getByLabel('筛选 Map', { exact: true }).isVisible())) {
    await page.getByRole('button', { name: 'Map 目录', exact: true }).click()
  }
}

/** Enter the advanced editor through its visible control; never bypass the form UI. */
export async function openNodeJson(scope: Page | Locator): Promise<Locator> {
  const json = scope.getByLabel('node 内容 JSON', { exact: true })
  if (!(await json.isVisible())) await scope.getByRole('button', { name: 'JSON 编辑', exact: true }).click()
  return json
}

/** 从变量编辑器 DOM 读取保存格式；变量块按占位符计算，不把删除按钮的 × 计入。 */
export async function serializedChatRule(locator: Locator): Promise<string> {
  return locator.evaluate((element) =>
    Array.from(element.childNodes)
      .map((node) =>
        node instanceof HTMLElement && node.dataset.chatNameToken !== undefined
          ? node.dataset.chatNameToken
          : (node.textContent ?? ''),
      )
      .join(''),
  )
}

/** 按稳定 ID 定位 zone 行，目录路径可重绑。 */
export function zoneRowFor(page: Page, zoneId: string): Locator {
  return page.locator(`[data-testid="zone-item"][data-zone-id=${JSON.stringify(zoneId)}]`)
}

/** 触摸档用 tap，桌面档用 click；两种尺寸的操作路径保持一致。 */
export async function activate(locator: Locator, touch: boolean) {
  if (touch) {
    await locator.tap()
  } else {
    await locator.click()
  }
}

/** 从 Task 预设逐步确认，再明确保存；每一步均操作真实页面。 */
export async function saveTask(page: Page, touch = false) {
  await activate(page.getByTestId('task-next'), touch)
  await activate(page.getByTestId('task-next'), touch)
  await activate(page.getByTestId('save-task'), touch)
}

/** 页面不得因弹窗或长路径出现横向滚动。 */
export async function expectNoHorizontalOverflow(page: Page) {
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - window.innerWidth,
  )
  expect(overflow, '页面不应出现横向溢出').toBeLessThanOrEqual(0)
}

/**
 * 弹窗必须完整落在视口内；更高的内容由弹窗自己滚动。
 *
 * 手机键盘与长失败结果都会让弹窗变高；固定定位的遮罩如果不可滚动，底部操作会永远不可达。
 */
export async function expectDialogInsideViewport(dialog: Locator) {
  const box = await dialog.boundingBox()
  const viewport = dialog.page().viewportSize()
  expect(box, '弹窗必须有可测量的边界').not.toBeNull()
  expect(viewport).not.toBeNull()
  if (box === null || viewport === null) return
  expect(box.y, '弹窗顶部不应被裁掉').toBeGreaterThanOrEqual(0)
  expect(
    box.y + box.height,
    '弹窗底部不应超出视口（内容应在弹窗内部滚动）',
  ).toBeLessThanOrEqual(viewport.height + 0.5)
}

/** 把弹窗内部滚到操作按钮，并确认它真的落在视口内（不是只存在于长页面里）。 */
export async function scrollToDialogAction(dialog: Locator, action: Locator) {
  await action.scrollIntoViewIfNeeded()
  const box = await action.boundingBox()
  const viewport = action.page().viewportSize()
  expect(box).not.toBeNull()
  expect(viewport).not.toBeNull()
  if (box === null || viewport === null) return
  expect(box.y, '操作按钮顶部应进入视口').toBeGreaterThanOrEqual(0)
  expect(box.y + box.height, '操作按钮底部应进入视口').toBeLessThanOrEqual(
    viewport.height + 0.5,
  )
  expect(box.x, '操作按钮左侧应进入视口').toBeGreaterThanOrEqual(0)
  expect(box.x + box.width, '操作按钮右侧应进入视口').toBeLessThanOrEqual(
    viewport.width + 0.5,
  )
}

/** 通过公开 HTTP 接口提交 JSON；不经页面缓存或页面状态。 */
export async function jsonPost(baseURL: string, endpoint: string, body: unknown) {
  const response = await fetch(`${baseURL}${endpoint}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!response.ok) {
    throw new Error(`${endpoint} 失败：HTTP ${response.status} ${await response.text()}`)
  }
  return response.json()
}
