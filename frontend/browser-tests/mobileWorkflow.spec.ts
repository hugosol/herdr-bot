import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { expect, test } from '@playwright/test'
import type { Locator, Page } from '@playwright/test'

import { startServer, type E2EServer, type TaskView } from './support'
import {
  activate,
  expectDialogInsideViewport,
  expectNoHorizontalOverflow,
  jsonPost,
  saveTask,
  scrollToDialogAction,
  serializedChatRule,
  zoneRowFor,
} from './ui'

/**
 * 电脑与手机尺寸下的真实浏览器工作流（P45 / H2 的自动部分）。
 *
 * 桌面与移动各用一份独立服务、独立目录和真实浏览器完成同一组操作：Task 新增、读取、编辑、删除，
 * 以及从 zone 打开创建弹窗、选择 Task、键盘输入、提交、观察两个阶段和成功/失败结果。移动尺寸另外
 * 检查没有横向溢出、弹窗在视口内可滚动、关键控件保持触控目标，并用触摸事件点按。
 *
 * 手机实机体验（H2）仍需用户确认；本文件只证明窄屏真实浏览器的可用性，不冒充真机结论。
 */

type Profile = {
  label: string
  viewport: { width: number; height: number }
  isMobile: boolean
  hasTouch: boolean
}

const PROFILES: Profile[] = [
  {
    label: '桌面',
    viewport: { width: 1280, height: 900 },
    isMobile: false,
    hasTouch: false,
  },
  {
    // 常见手机纵向尺寸；触摸与移动视口都打开，但不以尺寸仿真声称真机通过。
    label: '移动',
    viewport: { width: 390, height: 844 },
    isMobile: true,
    hasTouch: true,
  },
]

const TASK = { name: '发布检查',
chatName: '{zone.name} · {task.name} · {agent} · {chat.id}',
agent: 'pi',
taskType: 'normal',
model: 'gpt-6.1-sol',
thinking: 'high',
skill: 'code-review', prompt: '第一行：检查登录\n第二行：保留 "引号" 与反斜杠 \\', extensions: [] }

const EDITED = { name: '发布检查（改）',
chatName: '{task.name} · {agent} · {chat.id}',
agent: 'omp',
taskType: 'normal',
model: 'deepseek-flash',
thinking: 'high',
skill: 'teach', prompt: '编辑后的第一行\n编辑后的第二行', extensions: [] }

const CREATION_TASK = { name: '创建用',
chatName: '{zone.name} · {task.name} · {chat.id}',
agent: 'omp',
taskType: 'normal',
model: 'default',
thinking: 'default',
skill: null, prompt: '', extensions: [] }

const MULTILINE_INPUT = '第一行任务：检查登录\n第二行任务：保留缩进\n第三行任务：完成'

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

/** 预建一个普通目录并登记到真实服务。 */
async function prepareZone(baseURL: string) {
  zoneRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'herdr-bot mobile workflow '))
  const directory = path.join(zoneRoot, 'alpha')
  fs.mkdirSync(directory)
  const listed = await jsonPost(baseURL, '/api/zones', { path: directory })
  return listed.zones[0].path as string
}

function taskFields(view: TaskView) {
  const { id: _id, ...fields } = view
  return fields
}

/**
 * 弹窗内的可操作控件保持手机触控目标，文本输入使用不触发 iOS 聚焦缩放的 16px 以上字号。
 */
async function expectTouchTargets(dialog: Locator) {
  const controls = await dialog.locator('button, select, input, textarea').evaluateAll((nodes) =>
    nodes.map((node) => {
      const rect = node.getBoundingClientRect()
      const style = getComputedStyle(node)
      return {
        tag: node.tagName,
        label: (node.textContent ?? node.getAttribute('aria-label') ?? '').trim().slice(0, 24),
        width: rect.width,
        height: rect.height,
        fontSize: Number.parseFloat(style.fontSize),
        visible: rect.width > 0 && rect.height > 0,
      }
    }),
  )
  for (const control of controls) {
    if (!control.visible) continue
    expect(
      control.height,
      `${control.tag}「${control.label}」的触控高度应至少 44px`,
    ).toBeGreaterThanOrEqual(44)
    expect(
      control.width,
      `${control.tag}「${control.label}」的触控宽度应至少 44px`,
    ).toBeGreaterThanOrEqual(44)
    if (control.tag === 'INPUT' || control.tag === 'TEXTAREA' || control.tag === 'SELECT') {
      expect(
        control.fontSize,
        `${control.tag}「${control.label}」的输入字号应至少 16px，避免手机聚焦时缩放`,
      ).toBeGreaterThanOrEqual(16)
    }
  }
}

async function openTasks(page: Page, touch: boolean) {
  await activate(page.getByTestId('tab-tasks'), touch)
  await expect(page.getByTestId('task-empty')).toBeVisible()
}

async function openFiles(page: Page, touch: boolean) {
  await activate(page.getByTestId('tab-files'), touch)
  await expect(page.getByTestId('panel-files')).toBeVisible()
}

/** 通过页面新增一个 Task，字号/触控只在移动档检查。 */
async function addTaskThroughUi(
  page: Page,
  options: {
    touch: boolean
    checkTouch: boolean
    literal: typeof TASK | typeof EDITED | typeof CREATION_TASK
  },
) {
  const { touch, checkTouch, literal } = options
  await activate(page.getByTestId('task-create-open'), touch)
  const dialog = page.getByTestId('task-dialog')
  await expect(dialog).toBeVisible()
  await expectDialogInsideViewport(dialog)
  if (checkTouch) await expectTouchTargets(dialog)
  // 取消与保存都要在视口内；窄屏下不能被主操作挤出屏幕左侧。
  await scrollToDialogAction(dialog, page.getByTestId('task-cancel'))

  // 名称用真实键盘事件输入。
  const name = page.getByTestId('task-name')
  await name.click()
  await name.pressSequentially(literal.name)

  const chatName = page.getByTestId('task-chat-name')
  await chatName.fill('')
  await activate(page.getByTestId('insert-zone-name'), touch)
  await activate(page.getByTestId('insert-task-name'), touch)
  await expect
    .poll(() => serializedChatRule(chatName))
    .toBe('{zone.name}{task.name}')
  await chatName.fill(literal.chatName)

  await page.getByTestId('task-agent').selectOption(literal.agent)
  await page.getByTestId('task-model').selectOption(literal.model)
  await page.getByTestId('task-thinking').selectOption(literal.thinking)
  await page.getByTestId('task-skill').selectOption(literal.skill ?? '')

  // 多行 prompt 用回车逐行输入，确认换行没有被表单或移动键盘吞掉。
  const prompt = page.getByTestId('task-prompt')
  await prompt.click()
  const lines = literal.prompt.split('\n')
  for (const [index, line] of lines.entries()) {
    if (index > 0) await prompt.press('Enter')
    await prompt.pressSequentially(line)
  }
  await expect(prompt).toHaveValue(literal.prompt)

  const example = literal.chatName
    .replace('{zone.name}', 'alpha')
    .replace('{task.name}', literal.name)
    .replace('{agent}', literal.agent)
    .replace('{chat.id}', 'a7f3c2')
  await expect(page.getByTestId('chat-name-preview')).toHaveText(example)

  await saveTask(page, touch)
  await expect(page.getByTestId('task-dialog')).toHaveCount(0)
  await expect(page.getByTestId('task-item')).toHaveCount(1)
}

for (const profile of PROFILES) {
  test.describe(`${profile.label}尺寸`, () => {
    test.use({
      viewport: profile.viewport,
      isMobile: profile.isMobile,
      hasTouch: profile.hasTouch,
    })

    test(`${profile.label}浏览器完成 Task 管理与从 zone 发起 Chat`, { tag: '@time-contract' }, async ({ page }) => {
      server = await startServer({
        herdr: {
          behavior: { agent_start_delay_ms: 2_000, prompt_delay_ms: 1_500 },
        },
      })
      const zone = await prepareZone(server.baseURL)
      await page.goto(server.baseURL)
      await expectNoHorizontalOverflow(page)

      // Task 新增：字段插入、配置联动、多行 prompt 与示例预览都在该尺寸可用。
      await openTasks(page, profile.hasTouch)
      await expectNoHorizontalOverflow(page)
      await addTaskThroughUi(page, {
        touch: profile.hasTouch,
        checkTouch: profile.isMobile,
        literal: TASK,
      })
      await expectNoHorizontalOverflow(page)

      // 独立客户端读取同一份已保存值（不是同页缓存）。
      const shared = await server.getTasks()
      expect(shared.tasks).toHaveLength(1)
      expect(taskFields(shared.tasks[0])).toEqual(TASK)

      // 编辑：带回已保存值，保存后独立客户端读到新字面值。
      const row = page.getByTestId('task-item')
      await activate(row.getByTestId('task-edit'), profile.hasTouch)
      const editDialog = page.getByTestId('task-dialog')
      await expect(editDialog).toBeVisible()
      await expectDialogInsideViewport(editDialog)
      await expect(page.getByTestId('task-name')).toHaveValue(TASK.name)
      await expect(page.getByTestId('task-prompt')).toHaveValue(TASK.prompt)

      await page.getByTestId('task-name').fill(EDITED.name)
      await page.getByTestId('task-chat-name').fill(EDITED.chatName)
      await page.getByTestId('task-agent').selectOption(EDITED.agent)
      await page.getByTestId('task-model').selectOption(EDITED.model)
      await page.getByTestId('task-thinking').selectOption(EDITED.thinking)
      await page.getByTestId('task-skill').selectOption(EDITED.skill ?? '')
      await page.getByTestId('task-prompt').fill(EDITED.prompt)
      await saveTask(page, profile.hasTouch)

      await expect(row).toContainText(EDITED.name)
      const afterEdit = await server.getTasks()
      expect(afterEdit.tasks).toHaveLength(1)
      expect(taskFields(afterEdit.tasks[0])).toEqual(EDITED)

      // 删除：页面与独立客户端都不再提供该 Task。
      await activate(row.getByTestId('task-delete'), profile.hasTouch)
      await expectDialogInsideViewport(page.getByTestId('task-delete-dialog'))
      await activate(page.getByTestId('task-delete-confirm'), profile.hasTouch)
      await expect(page.getByTestId('task-empty')).toBeVisible()
      expect((await server.getTasks()).tasks).toEqual([])
      const removed = await fetch(
        `${server.baseURL}/api/tasks/${encodeURIComponent(afterEdit.tasks[0].id)}`,
      )
      expect(removed.status).toBe(404)

      // 再通过页面保存一个创建用 Task；创建弹窗从同一份后端列表读取它。
      await addTaskThroughUi(page, {
        touch: profile.hasTouch,
        checkTouch: false,
        literal: CREATION_TASK,
      })

      // 从 zone 行打开创建弹窗：绑定正确 zone，Task 可选，名称由后端预览。
      await openFiles(page, profile.hasTouch)
      const zoneRow = zoneRowFor(page, zone)
      await expect(zoneRow).toHaveCount(1)
      await activate(zoneRow.getByTestId('zone-create-chat'), profile.hasTouch)
      const dialog = page.getByTestId('chat-creation-dialog')
      await expect(dialog).toBeVisible()
      await expect(dialog.getByTestId('chat-zone')).toContainText(zone)
      await expectDialogInsideViewport(dialog)
      if (profile.isMobile) await expectTouchTargets(dialog)

      await dialog.getByTestId('task').selectOption({ label: '创建用（omp）' })
      const preview = dialog.getByTestId('chat-name-preview')
      await expect(preview).toHaveText(/^alpha · 创建用 · [0-9a-f]{6}$/)
      const previewName = (await preview.textContent()) ?? ''
      expect(previewName.length).toBeGreaterThan(0)

      await dialog.locator('#chat-workspace-target').selectOption('default')
      // 多行 input 用键盘输入。
      const input = dialog.getByTestId('chat-input')
      await input.click()
      for (const [index, line] of MULTILINE_INPUT.split('\n').entries()) {
        if (index > 0) await input.press('Enter')
        await input.pressSequentially(line)
      }
      await expect(input).toHaveValue(MULTILINE_INPUT)
      await expectNoHorizontalOverflow(page)

      await activate(dialog.getByTestId('chat-create-submit'), profile.hasTouch)

      // “正在启动”是稳定阶段：受控 agent 未就绪前绝不提交初始输入。
      await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在启动')
      await expectDialogInsideViewport(dialog)
      await expect
        .poll(
          () => server?.herdrCalls().filter((call) => call.kind === 'agent_start').length ?? 0,
          { message: '受控 Herdr 应收到 agent_start', timeout: 10_000 },
        )
        .toBe(1)
      expect(server.herdrCalls().filter((call) => call.kind === 'agent_prompt')).toHaveLength(0)
      await page.waitForTimeout(300)
      expect(server.herdrCalls().filter((call) => call.kind === 'agent_prompt')).toHaveLength(0)

      // 就绪后才进入“正在提交初始指示”；提交确认前没有成功文案。
      await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在提交初始指示', {
        timeout: 10_000,
      })
      await expect(dialog.getByTestId('chat-create-success')).toHaveCount(0)

      await expect(dialog.getByTestId('chat-create-success')).toHaveText(
        '创建成功，初始指示已提交',
        { timeout: 10_000 },
      )
      await expect(dialog.getByTestId('chat-result-name')).toHaveText(previewName)
      await expect(dialog.getByTestId('chat-result-zone')).toHaveText(zone)

      // 外部记录独立证明名称、目录与一次多行投递。
      const tabs = server.herdrTabs()
      expect(tabs).toHaveLength(1)
      expect(tabs[0].label).toBe(previewName)
      expect(tabs[0].cwd).toBe(zone)
      await expect(dialog.getByTestId('chat-result-tab')).toHaveText(String(tabs[0].tab_id))
      await expect(dialog.getByTestId('chat-result-pane')).toHaveText(String(tabs[0].pane_id))
      const prompts = server.herdrCalls().filter((call) => call.kind === 'agent_prompt')
      expect(prompts).toHaveLength(1)
      expect(prompts[0].text).toBe(MULTILINE_INPUT)

      // 结果变长后弹窗仍在视口内；用户自己滚到操作区并关闭，页面停留在文件列表。
      await expectDialogInsideViewport(dialog)
      const close = dialog.getByTestId('chat-create-close')
      await scrollToDialogAction(dialog, close)
      await activate(close, profile.hasTouch)
      await expect(page.getByTestId('chat-creation-dialog')).toHaveCount(0)
      await expect(page.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')

      // 关闭弹窗不等于取消后端已接受的创建：外部没有额外创建或重发。
      await page.waitForTimeout(300)
      expect(server.herdrTabs()).toHaveLength(1)
      expect(server.herdrCalls().filter((call) => call.kind === 'agent_prompt')).toHaveLength(1)
    })
  })
}

test.describe('移动尺寸失败结果', () => {
  test.use({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true })

  test('提交被拒绝时移动弹窗保留可读的现场说明并可自行关闭', { tag: '@time-contract' }, async ({ page }) => {
    server = await startServer({
      herdr: { behavior: { agent_start_delay_ms: 500, prompt_delay_ms: 1_200, fail_prompt: true } },
    })
    const zone = await prepareZone(server.baseURL)
    await jsonPost(server.baseURL, '/api/tasks', CREATION_TASK)

    await page.goto(server.baseURL)
    await openFiles(page, true)
    const zoneRow = zoneRowFor(page, zone)
    await zoneRow.getByTestId('zone-create-chat').tap()
    const dialog = page.getByTestId('chat-creation-dialog')
    await expect(dialog).toBeVisible()
    await expect(dialog.getByTestId('chat-name-preview')).toHaveText(/^alpha · 创建用 · [0-9a-f]{6}$/)

    await dialog.locator('#chat-workspace-target').selectOption('default')
    await dialog.getByTestId('chat-input').fill('会被拒绝的指示')
    await dialog.getByTestId('chat-create-submit').tap()

    // 两个阶段在失败前都能稳定观察，而不是碰巧截到瞬态。
    await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在启动')
    await expect(dialog.getByTestId('chat-create-stage')).toHaveText('正在提交初始指示', {
      timeout: 10_000,
    })
    await expect(dialog.getByTestId('chat-create-failure')).toBeVisible({ timeout: 10_000 })
    await expect(dialog.getByTestId('chat-create-success')).toHaveCount(0)

    const failure = dialog.getByTestId('chat-create-failure')
    await failure.scrollIntoViewIfNeeded()
    await expect(failure).toContainText('提交初始指示')
    await expect(failure).toContainText('现场已存在：agent 已就绪，但初始指示的提交被明确拒绝')
    await expect(dialog.getByTestId('chat-result-name')).toBeVisible()
    await expect(dialog.getByTestId('chat-result-zone')).toHaveText(zone)

    // 长失败结果与定位信息之后，弹窗仍可滚动、关闭按钮可达。
    await expectDialogInsideViewport(dialog)
    await expectNoHorizontalOverflow(page)
    const close = dialog.getByTestId('chat-create-close')
    await scrollToDialogAction(dialog, close)
    await close.tap()
    await expect(page.getByTestId('chat-creation-dialog')).toHaveCount(0)
    await expect(page.getByTestId('tab-files')).toHaveAttribute('aria-selected', 'true')

    // 失败不触发自动重建或重发，已创建的现场保留。
    await page.waitForTimeout(300)
    expect(server.herdrTabs()).toHaveLength(1)
    expect(server.herdrCalls().filter((call) => call.kind === 'agent_prompt')).toHaveLength(1)
  })
})
