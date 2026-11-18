// Ticket 15: consume published V2 configuration and actual native Run facts.
// No request interception, component replacement or generic editing acceptance.
import fs from 'node:fs'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { isDeepStrictEqual } from 'node:util'
import { chromium, expect } from '../frontend/node_modules/@playwright/test/index.mjs'

const input = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'))
const evidence = { passed: false, action: input.action, http: [], observations: [], cleanupErrors: [] }
let browser
let watcher
let watcherDone
const pending = []
const compare = (actual, expected, label) => {
  if (!isDeepStrictEqual(actual, expected)) throw new Error(label + ': ' + JSON.stringify({ actual, expected }))
}
const publicRead = async (page, route) => {
  const response = await page.request.get(input.base + route)
  if (response.status() !== 200) throw new Error('Public read failed: ' + route + ' ' + response.status())
  return response.json()
}
try {
  if (fs.existsSync(input.profile)) throw new Error('Browser profile must be new and invocation-owned')
  browser = await chromium.launchPersistentContext(input.profile, {
    executablePath: input.chrome, headless: true, viewport: { width: 1600, height: 1200 }, timeout: 30000,
  })
  // Hold real Windows process handles before closing Chrome. Never delete the
  // owned profile, and require both these handles and profile-bearing utilities
  // to exit. A taskkill acknowledgement is not exit evidence.
  const profile = input.profile.replaceAll("'", "''")
  const script = `$ErrorActionPreference='Stop';$profile='${profile}';`
    + `$rows=@(Get-CimInstance Win32_Process | Where-Object {$_.Name -eq 'chrome.exe' -and $_.CommandLine -like ('*'+$profile+'*')});`
    + `$held=@();foreach($r in $rows){$p=[System.Diagnostics.Process]::GetProcessById($r.ProcessId);$null=$p.Handle;$held+=,$p};`
    + `if($held.Count -eq 0){throw 'No owned Chrome process'};Write-Output ('READY '+(($held|ForEach-Object {$_.Id}) -join ','));`
    + `$null=[Console]::ReadLine();foreach($p in $held){if(-not $p.WaitForExit(20000)){throw ('Chrome still alive '+$p.Id)}};`
    + `$until=[DateTime]::UtcNow.AddSeconds(20);do {$left=@(Get-CimInstance Win32_Process|Where-Object {$_.Name -eq 'chrome.exe' -and $_.CommandLine -like ('*'+$profile+'*')});if($left.Count -eq 0){Write-Output 'EXIT-CONFIRMED';exit 0};Start-Sleep -Milliseconds 100}while([DateTime]::UtcNow -lt $until);throw 'Owned Chrome utility survived'`
  watcher = spawn('powershell.exe', ['-NoProfile', '-Command', script], { stdio: ['pipe', 'pipe', 'pipe'] })
  let watcherText = ''
  let watcherError = ''
  let readyResolve
  let readyReject
  const ready = new Promise((resolve, reject) => { readyResolve = resolve; readyReject = reject })
  watcher.stdout.on('data', chunk => { watcherText += chunk; if (watcherText.includes('READY ')) readyResolve() })
  watcher.stderr.on('data', chunk => { watcherError += chunk })
  watcherDone = once(watcher, 'exit').then(([code]) => {
    evidence.chromeExit = { code, stdout: watcherText, stderr: watcherError, profile: input.profile }
    if (!watcherText.includes('READY ')) readyReject(new Error('Chrome watcher failed before acquiring handles: ' + watcherError))
    return code
  })
  await ready
  const page = await browser.newPage()
  page.on('response', response => {
    if (!response.url().includes('/api/')) return
    const item = { url: response.url(), method: response.request().method(), status: response.status(), request: response.request().postData() }
    evidence.http.push(item)
    pending.push(response.text().then(text => { item.response = text }, error => { item.error = String(error) }))
  })
  await page.goto(input.base + '?layout=expanded')
  if (input.action === 'configuration') {
    for (const [file, definition] of Object.entries(input.definitions)) {
      await page.getByRole('tab', { name: 'scheduler', exact: true }).click()
      await page.getByRole('button', { name: definition.title, exact: true }).click()
      const independent = await publicRead(page, '/api/scheduler/maps/' + file)
      compare(independent, definition, 'Public definition drift')
      for (const [id, automatic] of Object.entries(input.modes[file])) {
        await page.getByRole('region', { name: '节点列表', exact: true }).getByRole('button', { name: `${definition.nodes[id].title} ${id}`, exact: true }).click()
        const field = page.locator(`.scheduler-status-setting[data-policy="${automatic ? 'auto' : 'manual'}"] .scheduler-state-mark`)
        await expect(field).toContainText(automatic ? '自动批准' : '人工批准')
        await expect(field.locator('small')).toHaveText(String(automatic))
        evidence.observations.push({ file, node: id, automatic, public: independent, rendered: await page.locator('body').innerText() })
        await page.screenshot({ path: input.evidence + '.' + id + '.png', fullPage: true })
      }
      await page.goto(input.base + '?layout=expanded')
    }
  } else if (input.action === 'launch') {
    await page.getByRole('tab', { name: 'scheduler', exact: true }).click()
    await page.getByRole('button', { name: input.mapTitle, exact: true }).click()
    await page.getByRole('button', { name: '+ 发起 Run', exact: true }).click()
    await page.getByRole('combobox', { name: '管理者 Task', exact: true }).selectOption(input.manager)
    await page.getByRole('combobox', { name: '管理者 zone', exact: true }).selectOption(input.zone)
    await page.getByRole('textbox', { name: '本次要求', exact: true }).fill(input.business)
    compare(await page.getByRole('textbox', { name: '本次要求', exact: true }).inputValue(), input.business, 'Actual business input')
    await page.getByRole('button', { name: '校验启动配置', exact: true }).click()
    await page.getByRole('form', { name: '发起 Run', exact: true }).getByRole('status').filter({ hasText: '配置校验通过' }).waitFor()
    await page.screenshot({ path: input.evidence + '.before.png', fullPage: true })
    const responsePromise = page.waitForResponse(response => response.request().method() === 'POST' && response.url().endsWith('/runs'), { timeout: 90000 })
    await page.getByRole('button', { name: '启动 Run', exact: true }).click()
    const response = await responsePromise
    evidence.launched = { status: response.status(), body: await response.json() }
    if (response.status() !== 201 || evidence.launched.body.startupError || !evidence.launched.body.run?.manager) throw new Error('Real native launch failed: ' + JSON.stringify(evidence.launched))
    await page.getByRole('heading', { name: 'Run 详情', exact: true }).waitFor({ timeout: 90000 })
    evidence.runId = page.url().split('#workflow/')[1]
    if (!evidence.runId) throw new Error('No actual Run identity from delivered page')
    await page.screenshot({ path: input.evidence + '.after.png', fullPage: true })
  } else if (input.action === 'facts') {
    for (const detail of input.runs) {
      const independent = await publicRead(page, '/api/workflow/runs/' + detail.run.id)
      compare(independent, detail, 'Independent Run/report readback differs')
      const definition = await publicRead(page, '/api/scheduler/maps/' + detail.run.map)
      await page.goto(input.base + '/#workflow/' + detail.run.id)
      await page.getByRole('heading', { name: 'Run 详情', exact: true }).waitFor()
      for (const [id, node] of Object.entries(independent.run.nodes)) {
        await page.getByRole('group', { name: 'Map 依赖图', exact: true }).getByRole('button', { name: `${definition.nodes[id].title} ${id}`, exact: true }).click()
        const facts = page.getByRole('region', { name: '当前执行事实', exact: true })
const field = name => facts.locator('dt').filter({ hasText: new RegExp('^' + name + '$') }).locator('+ dd')
        await expect(field('id')).toHaveText(node.execution.id)
        await expect(field('autoApproved')).toHaveText(String(node.execution.autoApproved))
        await expect(field('approved')).toHaveText('true · 已批准')
        await expect(field('approvalSource')).toHaveText(node.execution.approvalSource === 'auto' ? 'auto · 自动批准（非用户 Yes）' : 'manual · 人工批准')
        const reports = independent.reports.filter(report => report.nodeId === id && report.executionId === node.execution.id)
        const reportRegion = page.getByRole('region', { name: '当前执行适用报告', exact: true })
        const articles = reportRegion.getByRole('article')
        await expect(articles).toHaveCount(reports.length)
        const readOutputs = region => region.locator(':scope > dl > div').evaluateAll(rows => rows.map(row => ({
          id: row.querySelector('dt code').textContent, path: row.querySelector('dd code').textContent,
        })))
        for (const [index, report] of reports.entries()) {
          const article = articles.nth(index)
          await expect(article.locator('.workflow-report-meta > code')).toHaveText(report.id)
          compare(await article.locator('pre').textContent(), report.body, 'Report body for ' + report.id)
          const handoff = article.getByRole('region', { name: '报告交接声明', exact: true })
          await expect(handoff.locator('p').first()).toHaveText('canComplete: ' + (report.canComplete ?? '缺失（历史报告未声明）'))
          compare(await readOutputs(handoff), report.outputs ?? [], 'Report outputs for ' + report.id)
        }
        const handoff = page.getByRole('region', { name: '当前执行交接事实', exact: true })
        await expect(handoff.locator('p').first()).toHaveText('最新 canComplete: ' + reports.at(-1).canComplete)
        const accumulated = new Map(reports.flatMap(report => (report.outputs ?? []).map(output => [output.id, output])))
        const outputs = await readOutputs(handoff)
        compare(outputs, [...accumulated.values()], 'Cumulative outputs for ' + node.execution.id)
        evidence.observations.push({ run: independent.run.id, node: id, executionId: node.execution.id,
          reports: reports.map(report => report.id), outputs, independent,
          rendered: await page.getByRole('region', { name: '当前节点详情', exact: true }).innerText() })
        await page.screenshot({ path: input.evidence + '.' + detail.run.id + '.' + id + '.png', fullPage: true })
      }
    }
  } else throw new Error('Unknown V2 browser action')
  evidence.passed = true
} catch (error) {
  evidence.error = error.stack || String(error)
} finally {
  if (watcher) watcher.stdin.end('\n')
  if (browser) {
    try { await browser.close() } catch (error) { evidence.cleanupErrors.push(String(error)) }
  }
  if (watcherDone) {
    try {
      const code = await watcherDone
      if (code !== 0 || !evidence.chromeExit.stdout.includes('EXIT-CONFIRMED')) throw new Error('Chrome exit unconfirmed: ' + JSON.stringify(evidence.chromeExit))
    } catch (error) { evidence.cleanupErrors.push(String(error)) }
  }
  await Promise.all(pending)
  evidence.passed = evidence.passed && evidence.cleanupErrors.length === 0
  fs.writeFileSync(input.evidence, JSON.stringify(evidence, null, 2))
}
console.log(JSON.stringify({ passed: evidence.passed, evidence: input.evidence }))
if (!evidence.passed) process.exitCode = 1
