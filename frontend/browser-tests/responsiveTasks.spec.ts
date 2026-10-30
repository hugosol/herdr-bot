import fs from 'node:fs'
import path from 'node:path'
import { expect, test } from '@playwright/test'
import { startServer } from './support'
import { expectDialogInsideViewport, expectNoHorizontalOverflow, jsonPost, scrollToDialogAction, zoneRowFor } from './ui'

// P49 approved seam: production browser UI + public HTTP, real disk and TOML.
// Source: ticket 07. Plausible violation: navigation loses concrete selections, a
// narrow dialog clips actions, or failures pretend to save/erase the user's draft.
// Each viewport exercises the controls, observes public persisted values, and
// captures both normal and error states. No page routes, in-memory API or Herdr double.
for (const profile of [
  { name: 'desktop', width: 1280, height: 900 },
  { name: 'mobile-viewport', width: 390, height: 844 },
]) {
  test(`P49 ${profile.name}: edit, navigate, persist and correct real failures`, async ({ page, browser }, info) => {
    await page.setViewportSize({ width: profile.width, height: profile.height })
    const server = await startServer()
    const metadata = fs.readFileSync(new URL('../../meta-data/task-options.toml', import.meta.url), 'utf8')
    // Use the shipped metadata and actual installed toolkit entrypoints, not the
    // harness's portable file-check entries. Creation is rejected before Herdr.
    fs.writeFileSync(server.metadataFile, metadata)
    const actions: string[] = []
    const responses: unknown[] = []
    page.on('response', async response => {
      if (response.url().startsWith(`${server.baseURL}/api/`) && !response.url().includes('/sessions')) {
        try { responses.push({ url: response.url(), status: response.status(), body: await response.json() }) } catch { /* SSE is not a JSON receipt. */ }
      }
    })
    const dialog = page.getByTestId('task-dialog')
    const action = async (id: string) => {
      const control = page.getByTestId(id)
      await control.scrollIntoViewIfNeeded()
      await control.click()
      actions.push(`click ${id}`)
    }
    const capture = async (name: string, dialogId = 'task-dialog') => {
      await expectNoHorizontalOverflow(page)
      await expectDialogInsideViewport(page.getByTestId(dialogId))
      const file = info.outputPath(`${profile.name}-${name}.png`)
      await page.screenshot({ path: file })
      await info.attach(name, { path: file, contentType: 'image/png' })
      actions.push(`screenshot ${name}; no page overflow; dialog within viewport`)
    }
    const draft = {
      name: 'P49 发布巡检', chatName: '巡检 {zone.name}{task.name}{agent}{chat.id}',
      agent: 'omp', taskType: 'normal', model: 'gpt-6.1-sol', thinking: 'high', skill: 'code-review',
      prompt: '第一行：检查发布\n第二行：保留中文、"引号" 与 C:\\alpha',
      extensions: ['omp-codebase-tools', 'omp-connect-bot'],
    }
    try {
      await page.goto(`${server.baseURL}?layout=expanded`)
      await action('tab-tasks')
      await action('task-create-open')
      await page.getByTestId('task-name').fill(draft.name)
      const rule = page.getByTestId('task-chat-name')
      await rule.fill('巡检 ')
      await rule.press('End')
      for (const field of ['insert-zone-name', 'insert-task-name', 'insert-agent', 'insert-chat-id']) await action(field)
      await expect(dialog.getByTestId('chat-name-preview')).toHaveText('巡检 alphaP49 发布巡检ompa7f3c2')
      await page.getByTestId('task-model').selectOption('gpt-6.1-sol')
      await page.getByTestId('task-thinking').selectOption('high')
      await page.getByTestId('task-skill').selectOption('code-review')
      await page.getByTestId('task-prompt').fill(draft.prompt)
      actions.push('fill all preset fields; insert all four name fields using buttons')
      await rule.scrollIntoViewIfNeeded()
      await capture('preset')
      await action('task-next')
      await page.getByTestId('extension-omp-codebase-tools').check()
      await page.getByTestId('extension-omp-connect-bot').check()
      await expect(page.getByTestId('task-fixed-extensions')).toContainText('model-cost')
      await capture('extensions')
      await action('task-back')
      for (const [id, expected] of [['name', draft.name], ['agent', 'omp'], ['model', 'gpt-6.1-sol'], ['thinking', 'high'], ['skill', 'code-review'], ['prompt', draft.prompt]]) {
        await expect(page.getByTestId(`task-${id}`)).toHaveValue(expected)
      }
      await expect(dialog.getByTestId('chat-name-preview')).toHaveText('巡检 alphaP49 发布巡检ompa7f3c2')
      await action('task-next')
      await expect(page.getByTestId('extension-omp-codebase-tools')).toBeChecked()
      await expect(page.getByTestId('extension-omp-connect-bot')).toBeChecked()
      await action('task-next')
      for (const [id, expected] of Object.entries({ name: draft.name, chatName: draft.chatName, agent: 'oh-my-pi', model: 'GPT-6.1-Sol', thinking: 'high', skill: '/code-review', prompt: draft.prompt, fixed: 'herdr-agent-state、model-cost', optional: 'codebase-tools、connect-bot', discovery: '关闭' })) {
        await expect(page.getByTestId(`summary-${id}`)).toHaveText(expected)
      }
      expect((await server.getTasks()).tasks).toEqual([])
      await capture('summary')
      await scrollToDialogAction(dialog, page.getByTestId('save-task'))
      await capture('summary-actions')
      await action('save-task')
      await expect(page.getByTestId('task-feedback')).toHaveAttribute('data-kind', 'success')
      const saved = (await server.getTasks()).tasks[0]
      expect(saved).toEqual({ id: expect.any(String), ...draft })
      actions.push('public GET matches all summary literals; nothing saved before explicit save')

      await action('task-edit')
      await page.getByTestId('task-name').fill('P49 编辑后巡检')
      await rule.fill('编辑 {zone.name} / {task.name} / {agent} / {chat.id}')
      await page.getByTestId('task-agent').selectOption('pi')
      await expect(page.getByTestId('task-model')).toHaveValue('default')
      await expect(page.getByTestId('task-thinking')).toHaveValue('default')
      await page.getByTestId('task-model').selectOption('deepseek-flash')
      await page.getByTestId('task-thinking').selectOption('off')
      await page.getByTestId('task-skill').selectOption('teach')
      await page.getByTestId('task-prompt').fill('编辑第一行\n编辑第二行')
      await action('task-next')
      await expect(page.getByTestId('extension-pi-codebase-tools')).not.toBeChecked()
      await page.getByTestId('extension-pi-codebase-tools').check()
      await action('task-back')
      await expect(page.getByTestId('task-prompt')).toHaveValue('编辑第一行\n编辑第二行')
      await expect(page.getByTestId('task-thinking')).toHaveValue('off')
      await action('task-next')
      await expect(page.getByTestId('extension-pi-codebase-tools')).toBeChecked()
      await action('task-next')
      await expect(page.getByTestId('summary-optional')).toHaveText('codebase-tools')
      const edited = { ...saved, name: 'P49 编辑后巡检', chatName: '编辑 {zone.name} / {task.name} / {agent} / {chat.id}', agent: 'pi', model: 'deepseek-flash', thinking: 'off', skill: 'teach', prompt: '编辑第一行\n编辑第二行', extensions: ['pi-codebase-tools'] }
      fs.mkdirSync(`${server.storageFile}.tmp`)
      await action('save-task')
      await expect(page.getByTestId('task-form-error')).toHaveAttribute('data-kind', 'error')
      expect((await server.getTasks()).tasks).toEqual([saved])
      await page.getByTestId('task-form-error').scrollIntoViewIfNeeded()
      await capture('persistence-error')
      fs.rmdirSync(`${server.storageFile}.tmp`)
      await action('task-back')
      await expect(page.getByTestId('extension-pi-codebase-tools')).toBeChecked()
      await action('task-back')
      await expect(page.getByTestId('task-name')).toHaveValue(edited.name)
      await expect(page.getByTestId('task-prompt')).toHaveValue(edited.prompt)
      await action('task-next')
      await action('task-next')
      await action('save-task')
      await expect(page.getByTestId('task-feedback')).toHaveAttribute('data-kind', 'success')
      expect((await server.getTasks()).tasks).toEqual([edited])
      actions.push('real temporary-write obstruction rejected; prior task unchanged; draft retained; retry persisted edited values')

      // Remove an actually selected metadata definition on disk, then reopen.
      const withoutTools = metadata.replace(/\[\[extensions\]\]\r?\nid = 'pi-codebase-tools'[\s\S]*?(?=\[\[extensions\]\]|$)/, '')
      expect(withoutTools).not.toBe(metadata)
      fs.writeFileSync(server.metadataFile, withoutTools)
      await action('task-edit')
      await expect(page.getByTestId('task-selection-error')).toContainText('pi-codebase-tools')
      await action('task-next')
      await expect(page.getByTestId('extension-pi-codebase-tools')).toBeChecked()
      await expect(page.getByText('pi-codebase-tools（不可用）', { exact: true })).toBeVisible()
      await capture('unavailable-selection')
      await action('task-next')
      await action('save-task')
      await expect(page.getByTestId('task-form-error')).toContainText('pi-codebase-tools')
      expect((await server.getTasks()).tasks).toEqual([edited])
      await action('task-back')
      await page.getByTestId('extension-pi-codebase-tools').click()
      await action('task-next')
      await expect(page.getByTestId('summary-optional')).toHaveText('无')
      await action('save-task')
      await expect(page.getByTestId('task-feedback')).toHaveAttribute('data-kind', 'success')
      expect((await server.getTasks()).tasks).toEqual([{ ...edited, extensions: [] }])
      actions.push('unavailable selected ID visible and rejected; explicitly unchecked; public read confirms correction')
      fs.writeFileSync(server.metadataFile, metadata)

      // Real missing-file dependency, not a mocked error response. No real agent
      // or fixture is launched: the service must reject during dependency checks.
      const missing = path.join(path.dirname(server.metadataFile), 'missing-model-cost.ts')
      const broken = metadata.replace(/(id = 'pi-model-cost'[\s\S]*?entrypoint = )'[^']+'/, `$1'${missing}'`)
      expect(broken).not.toBe(metadata)
      fs.writeFileSync(server.metadataFile, broken)
      const zonePath = path.join(path.dirname(server.metadataFile), 'p49-zone')
      fs.mkdirSync(zonePath)
      const zone = (await jsonPost(server.baseURL, '/api/zones', { path: zonePath })).zones[0].path
      await action('tab-files')
      await zoneRowFor(page, zone).getByTestId('zone-create-chat').click()
      await expect(page.getByTestId('chat-name-preview')).toHaveAttribute('data-state', 'ready')
      await page.getByTestId('chat-input').fill('P49 依赖失败后修正\n保留本次输入')
      await action('chat-create-submit')
      await expect(page.getByTestId('chat-request-error')).toContainText(missing)
      await expect(page.getByTestId('chat-request-error')).toContainText('不存在')
      await expect(page.getByTestId('chat-request-error')).toContainText('未创建 Herdr Tab、未启动 agent、未提交输入')
      await expect(page.getByTestId('chat-create-success')).toHaveCount(0)
      await page.getByTestId('chat-request-error').scrollIntoViewIfNeeded()
      await capture('dependency-error', 'chat-creation-dialog')
      await scrollToDialogAction(page.getByTestId('chat-creation-dialog'), page.getByTestId('chat-create-close'))
      await capture('dependency-actions', 'chat-creation-dialog')
      await expect(page.getByTestId('chat-input')).toHaveValue('P49 依赖失败后修正\n保留本次输入')
      fs.writeFileSync(server.metadataFile, metadata)
      await action('chat-create-close')
      await zoneRowFor(page, zone).getByTestId('zone-create-chat').click()
      await expect(page.getByTestId('chat-name-preview')).toHaveAttribute('data-state', 'ready')
      await action('chat-create-close')
      actions.push('missing real dependency rejected before side effects; input retained; restored metadata; reopened creation; no successful real agent launch claimed')
      await server.restart()
      expect((await server.getTasks()).tasks).toEqual([{ ...edited, extensions: [] }])
      actions.push('real process restart retains corrected edited task')
    } finally {
      const record = { profile, browser: browser.version(), service: 'built Axum executable + Vite production assets', metadata: 'shipped meta-data/task-options.toml, installed toolkit paths; isolated copy modified for faults', baseURL: server.baseURL, actions, responses, limitation: 'P49 viewport only; not real phone touch, keyboard, input or P50/H2; no successful Herdr/agent launch tested' }
      fs.writeFileSync(info.outputPath(`${profile.name}-operations.json`), JSON.stringify(record, null, 2))
      await server.cleanup()
    }
  })
}
