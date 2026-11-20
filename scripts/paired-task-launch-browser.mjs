// Acceptance-only driver: the packaged page owns each POST and the status UI.
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import fs from 'node:fs'
import path from 'node:path'
import { setTimeout as delay } from 'node:timers/promises'
import { chromium, expect } from '../frontend/node_modules/@playwright/test/index.mjs'

const config = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'))
const file = name => path.join(config.dir, `browser-${name}.json`)
const save = (name, value) => {
  fs.writeFileSync(`${file(name)}.tmp`, JSON.stringify(value, null, 2))
  fs.renameSync(`${file(name)}.tmp`, file(name))
}
const result = { creates: [], observations: [], cleanupErrors: [] }
let browser
let watcher
let exited
try {
  browser = await chromium.launchPersistentContext(config.profile, {
    executablePath: config.chrome, headless: true, viewport: { width: 1440, height: 960 }, timeout: 15000,
  })
  const profile = config.profile.replaceAll("'", "''")
  const script = `$ErrorActionPreference='Stop';$profile='${profile}';`
    + `$owned={ @(Get-CimInstance Win32_Process | Where-Object {$_.Name -eq 'chrome.exe' -and $_.CommandLine -like ('*'+$profile+'*')}) };`
    + `$held=@();foreach($r in (& $owned)){$p=[System.Diagnostics.Process]::GetProcessById($r.ProcessId);$null=$p.Handle;$held+=,$p};`
    + `if($held.Count -eq 0){throw 'No owned Chrome process'};Write-Output ('READY '+(($held|ForEach-Object {$_.Id}) -join ','));`
    + `$null=[Console]::ReadLine();foreach($p in $held){if(-not $p.WaitForExit(15000)){throw ('Chrome still alive '+$p.Id)}};`
    + `$until=[DateTime]::UtcNow.AddSeconds(15);do {if((& $owned).Count -eq 0){Write-Output 'EXIT-CONFIRMED';exit 0};`
    + `Start-Sleep -Milliseconds 100}while([DateTime]::UtcNow -lt $until);throw 'Owned Chrome utility survived'`
  const powershell = path.join(process.env.SystemRoot, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe')
  watcher = spawn(powershell, ['-NoProfile', '-Command', script], { stdio: ['pipe', 'pipe', 'pipe'] })
  let stdout = ''
  let stderr = ''
  watcher.stderr.on('data', chunk => { stderr += chunk })
  const observing = new Promise((resolve, reject) => {
    watcher.stdout.on('data', chunk => { stdout += chunk; if (stdout.includes('READY ')) resolve() })
    watcher.on('error', reject)
    watcher.on('exit', code => { if (!stdout.includes('READY ')) reject(new Error(`Chrome ownership observer ${code}: ${stderr}`)) })
  })
  exited = once(watcher, 'close').then(([code]) => ({ code, stdout, stderr }))
  await observing
  const page = browser.pages()[0]
  page.on('request', request => {
    if (request.method() === 'POST' && new URL(request.url()).pathname === '/api/chats') result.creates.push(request.postDataJSON())
  })
  await page.goto(config.base)
  await page.getByTestId('tab-files').click()
  await page.getByTestId('zone-create-chat').click()
  const dialog = page.getByTestId('chat-creation-dialog')
  await dialog.getByTestId('task').selectOption(config.task)
  await dialog.locator('#chat-workspace-target').selectOption(config.workspace)
  await dialog.getByTestId('chat-input').fill(config.input)
  const response = page.waitForResponse(response => response.status() === 202 && new URL(response.url()).pathname === '/api/chats')
  await dialog.getByTestId('chat-create-submit').click()
  save('accepted', await (await response).json())
  // The driver requests observations only after independently seeing the matching public stage.
  let number = 0
  for (;;) {
    const next = file(`step-${number}`)
    const deadline = Date.now() + 105000
    while (!fs.existsSync(next)) {
      if (Date.now() >= deadline) throw new Error('No next acceptance observation')
      await delay(40)
    }
    const { action, expected } = JSON.parse(fs.readFileSync(next, 'utf8'))
    if (action === 'done') break
    const selector = {
      initializing: 'chat-create-stage', retrying: 'chat-create-stage',
      succeeded: 'chat-create-success', failed: 'chat-create-failure', unknown: 'chat-create-failure',
    }[action]
    if (action === 'refresh') {
      await page.reload()
      await expect(dialog).toHaveCount(0)
    } else {
      const outcome = dialog.getByTestId(selector)
      await expect(outcome).toContainText(expected, { timeout: 10000 })
      if (action === 'unknown') await expect(outcome).not.toContainText('初始指示未提交')
      result.observations.push({ action, text: await outcome.innerText() })
    }
    await page.screenshot({ path: path.join(config.dir, `browser-${number}-${action}.png`) })
    save(`ack-${number++}`, { action, creates: result.creates.length })
  }
  if (result.creates.length !== 1) throw new Error(`Page issued ${result.creates.length} creation requests`)
} catch (error) {
  result.error = error.stack ?? String(error)
} finally {
  try { await browser?.close() } catch (error) { result.cleanupErrors.push(String(error)) }
  if (watcher) {
    watcher.stdin.on('error', error => result.cleanupErrors.push(String(error)))
    watcher.stdin.end('\n')
    try {
      result.chromeExit = await exited
      if (result.chromeExit.code !== 0 || !result.chromeExit.stdout.includes('EXIT-CONFIRMED')) {
        result.cleanupErrors.push('Owned Chrome exit not confirmed')
      }
    } catch (error) { result.cleanupErrors.push(String(error)) }
  }
  result.passed = !result.error && result.cleanupErrors.length === 0
  save('result', result)
  process.exitCode = result.passed ? 0 : 1
}
