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
    const metadata = fs.readFileSync(new URL('../../meta-data/bot-options.toml', import.meta.url), 'utf8')
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
    const dialog = page.getByTestId('template-dialog')
    const action = async (id: string) => {
      const control = page.getByTestId(id)
      await control.scrollIntoViewIfNeeded()
      await control.click()
      actions.push(`click ${id}`)
    }
    const capture = async (name: string, dialogId = 'template-dialog') => {
      await expectNoHorizontalOverflow(page)
      await expectDialogInsideViewport(page.getByTestId(dialogId))
      const file = info.outputPath(`${profile.name}-${name}.png`)
      await page.screenshot({ path: file })
      await info.attach(name, { path: file, contentType: 'image/png' })
      actions.push(`screenshot ${name}; no page overflow; dialog within viewport`)
    }
    const draft = {
      name: 'P49 发布巡检', botName: '巡检 {zone.name}{template.name}{agent}{bot.id}',
      agent: 'omp', model: 'gpt-6.1-sol', thinking: 'high', skill: 'code-review',
      prompt: '第一行：检查发布\n第二行：保留中文、"引号" 与 C:\\alpha',
      extensions: ['omp-codebase-tools', 'omp-connect-bot'],
    }
    try {
      await page.goto(server.baseURL)
      await action('tab-templates')
      await action('template-create-open')
      await page.getByTestId('template-name').fill(draft.name)
      const rule = page.getByTestId('template-bot-name')
      await rule.fill('巡检 ')
      await rule.press('End')
      for (const field of ['insert-zone-name', 'insert-template-name', 'insert-agent', 'insert-bot-id']) await action(field)
      await expect(dialog.getByTestId('bot-name-preview')).toHaveText('巡检 alphaP49 发布巡检ompa7f3c2')
      await page.getByTestId('template-model').selectOption('gpt-6.1-sol')
      await page.getByTestId('template-thinking').selectOption('high')
      await page.getByTestId('template-skill').selectOption('code-review')
      await page.getByTestId('template-prompt').fill(draft.prompt)
      actions.push('fill all preset fields; insert all four name fields using buttons')
      await rule.scrollIntoViewIfNeeded()
      await capture('preset')
      await action('template-next')
      await page.getByTestId('extension-omp-codebase-tools').check()
      await page.getByTestId('extension-omp-connect-bot').check()
      await expect(page.getByTestId('template-fixed-extensions')).toContainText('model-cost')
      await capture('extensions')
      await action('template-back')
      for (const [id, expected] of [['name', draft.name], ['agent', 'omp'], ['model', 'gpt-6.1-sol'], ['thinking', 'high'], ['skill', 'code-review'], ['prompt', draft.prompt]]) {
        await expect(page.getByTestId(`template-${id}`)).toHaveValue(expected)
      }
      await expect(dialog.getByTestId('bot-name-preview')).toHaveText('巡检 alphaP49 发布巡检ompa7f3c2')
      await action('template-next')
      await expect(page.getByTestId('extension-omp-codebase-tools')).toBeChecked()
      await expect(page.getByTestId('extension-omp-connect-bot')).toBeChecked()
      await action('template-next')
      for (const [id, expected] of Object.entries({ name: draft.name, botName: draft.botName, agent: 'OMP', model: draft.model, thinking: 'high', skill: '/code-review', prompt: draft.prompt, fixed: 'model-cost', optional: 'codebase-tools、connect-bot', discovery: '关闭' })) {
        await expect(page.getByTestId(`summary-${id}`)).toHaveText(expected)
      }
      expect((await server.getTemplates()).templates).toEqual([])
      await capture('summary')
      await scrollToDialogAction(dialog, page.getByTestId('save-template'))
      await capture('summary-actions')
      await action('save-template')
      await expect(page.getByTestId('template-feedback')).toHaveAttribute('data-kind', 'success')
      const saved = (await server.getTemplates()).templates[0]
      expect(saved).toEqual({ id: expect.any(String), ...draft })
      actions.push('public GET matches all summary literals; nothing saved before explicit save')

      await action('template-edit')
      await page.getByTestId('template-name').fill('P49 编辑后巡检')
      await rule.fill('编辑 {zone.name} / {template.name} / {agent} / {bot.id}')
      await page.getByTestId('template-agent').selectOption('pi')
      await expect(page.getByTestId('template-model')).toHaveValue('default')
      await expect(page.getByTestId('template-thinking')).toHaveValue('default')
      await page.getByTestId('template-model').selectOption('deepseek-flash')
      await page.getByTestId('template-thinking').selectOption('off')
      await page.getByTestId('template-skill').selectOption('teach')
      await page.getByTestId('template-prompt').fill('编辑第一行\n编辑第二行')
      await action('template-next')
      await expect(page.getByTestId('extension-pi-codebase-tools')).not.toBeChecked()
      await page.getByTestId('extension-pi-codebase-tools').check()
      await action('template-back')
      await expect(page.getByTestId('template-prompt')).toHaveValue('编辑第一行\n编辑第二行')
      await expect(page.getByTestId('template-thinking')).toHaveValue('off')
      await action('template-next')
      await expect(page.getByTestId('extension-pi-codebase-tools')).toBeChecked()
      await action('template-next')
      await expect(page.getByTestId('summary-optional')).toHaveText('codebase-tools')
      const edited = { ...saved, name: 'P49 编辑后巡检', botName: '编辑 {zone.name} / {template.name} / {agent} / {bot.id}', agent: 'pi', model: 'deepseek-flash', thinking: 'off', skill: 'teach', prompt: '编辑第一行\n编辑第二行', extensions: ['pi-codebase-tools'] }
      fs.mkdirSync(`${server.storageFile}.tmp`)
      await action('save-template')
      await expect(page.getByTestId('template-form-error')).toContainText('保存 Bot 模板失败')
      expect((await server.getTemplates()).templates).toEqual([saved])
      await page.getByTestId('template-form-error').scrollIntoViewIfNeeded()
      await capture('persistence-error')
      fs.rmdirSync(`${server.storageFile}.tmp`)
      await action('template-back')
      await expect(page.getByTestId('extension-pi-codebase-tools')).toBeChecked()
      await action('template-back')
      await expect(page.getByTestId('template-name')).toHaveValue(edited.name)
      await expect(page.getByTestId('template-prompt')).toHaveValue(edited.prompt)
      await action('template-next')
      await action('template-next')
      await action('save-template')
      await expect(page.getByTestId('template-feedback')).toHaveAttribute('data-kind', 'success')
      expect((await server.getTemplates()).templates).toEqual([edited])
      actions.push('real temporary-write obstruction rejected; prior template unchanged; draft retained; retry persisted edited values')

      // Remove an actually selected metadata definition on disk, then reopen.
      const withoutTools = metadata.replace(/\[\[extensions\]\]\r?\nid = 'pi-codebase-tools'[\s\S]*?(?=\[\[extensions\]\]|$)/, '')
      expect(withoutTools).not.toBe(metadata)
      fs.writeFileSync(server.metadataFile, withoutTools)
      await action('template-edit')
      await expect(page.getByTestId('template-selection-error')).toContainText('pi-codebase-tools')
      await action('template-next')
      await expect(page.getByTestId('extension-pi-codebase-tools')).toBeChecked()
      await expect(page.getByText('pi-codebase-tools（不可用）', { exact: true })).toBeVisible()
      await capture('unavailable-selection')
      await action('template-next')
      await action('save-template')
      await expect(page.getByTestId('template-form-error')).toContainText('pi-codebase-tools')
      expect((await server.getTemplates()).templates).toEqual([edited])
      await action('template-back')
      await page.getByTestId('extension-pi-codebase-tools').click()
      await action('template-next')
      await expect(page.getByTestId('summary-optional')).toHaveText('无')
      await action('save-template')
      await expect(page.getByTestId('template-feedback')).toHaveAttribute('data-kind', 'success')
      expect((await server.getTemplates()).templates).toEqual([{ ...edited, extensions: [] }])
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
      await zoneRowFor(page, zone).getByTestId('zone-create-bot').click()
      await expect(page.getByTestId('bot-name-preview')).toHaveAttribute('data-state', 'ready')
      await page.getByTestId('bot-input').fill('P49 依赖失败后修正\n保留本次输入')
      await action('bot-create-submit')
      await expect(page.getByTestId('bot-request-error')).toContainText(missing)
      await expect(page.getByTestId('bot-request-error')).toContainText('不存在')
      await expect(page.getByTestId('bot-request-error')).toContainText('未创建 Herdr Tab、未启动 agent、未提交输入')
      await expect(page.getByTestId('bot-create-success')).toHaveCount(0)
      await page.getByTestId('bot-request-error').scrollIntoViewIfNeeded()
      await capture('dependency-error', 'bot-creation-dialog')
      await scrollToDialogAction(page.getByTestId('bot-creation-dialog'), page.getByTestId('bot-create-close'))
      await capture('dependency-actions', 'bot-creation-dialog')
      await expect(page.getByTestId('bot-input')).toHaveValue('P49 依赖失败后修正\n保留本次输入')
      fs.writeFileSync(server.metadataFile, metadata)
      await action('bot-create-close')
      await zoneRowFor(page, zone).getByTestId('zone-create-bot').click()
      await expect(page.getByTestId('bot-name-preview')).toHaveAttribute('data-state', 'ready')
      await action('bot-create-close')
      actions.push('missing real dependency rejected before side effects; input retained; restored metadata; reopened creation; no successful real agent launch claimed')
      await server.restart()
      expect((await server.getTemplates()).templates).toEqual([{ ...edited, extensions: [] }])
      actions.push('real process restart retains corrected edited template')
    } finally {
      const record = { profile, browser: browser.version(), service: 'built Axum executable + Vite production assets', metadata: 'shipped meta-data/bot-options.toml, installed toolkit paths; isolated copy modified for faults', baseURL: server.baseURL, actions, responses, limitation: 'P49 viewport only; not real phone touch, keyboard, input or P50/H2; no successful Herdr/agent launch tested' }
      fs.writeFileSync(info.outputPath(`${profile.name}-operations.json`), JSON.stringify(record, null, 2))
      await server.cleanup()
    }
  })
}
