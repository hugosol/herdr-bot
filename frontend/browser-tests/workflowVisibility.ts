import { spawn } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { chromium, type Browser } from '@playwright/test'

// Playwright's regular page installs an always-focused/visible emulation in its
// own CDP session. Connect without those defaults to observe native lifecycle
// events, while continuing to drive the delivered page through public APIs.
export async function nativeVisibilityBrowser() {
  const executable = [process.env.PROGRAMFILES, process.env['PROGRAMFILES(X86)'], process.env.LOCALAPPDATA]
    .filter((root): root is string => !!root)
    .map(root => path.join(root, 'Google/Chrome/Application/chrome.exe'))
    .find(file => fs.existsSync(file))
  if (!executable) throw new Error('Native visibility acceptance requires installed Chrome.')
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'herdr-workflow-visibility-'))
  const endpoint = Promise.withResolvers<string>()
  let diagnostics = ''
  // Ignore other desktop windows covering Chrome; real minimization must still hide the page.
  const child = spawn(executable, ['--remote-debugging-address=127.0.0.1', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', '--disable-background-networking', '--disable-backgrounding-occluded-windows', 'about:blank'], { stdio: ['ignore', 'pipe', 'pipe'], windowsHide: false })
  const timeout = setTimeout(() => endpoint.reject(new Error(`Chrome startup timed out: ${diagnostics}`)), 15000)
  const consume = (chunk: Buffer) => {
    diagnostics += chunk.toString()
    const match = /DevTools listening on (ws:\/\/127\.0\.0\.1:[0-9]+\/[^\s]+)/.exec(diagnostics)
    if (match) { clearTimeout(timeout); endpoint.resolve(match[1]) }
  }
  child.stdout.on('data', consume)
  child.stderr.on('data', consume)
  child.once('error', error => { clearTimeout(timeout); endpoint.reject(error) })
  const childExited = Promise.withResolvers<void>()
  child.once('exit', code => {
    clearTimeout(timeout)
    childExited.resolve()
    endpoint.reject(new Error(`Chrome exited (${code}): ${diagnostics}`))
  })
  // Keep an independent Windows process handle: Chrome can terminate while
  // Node has not delivered its ChildProcess exit event yet.
  const handleReady = Promise.withResolvers<void>()
  const handleExited = Promise.withResolvers<void>()
  let confirmedExit = false
  let handleDiagnostics = ''
  const watcher = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', `$ErrorActionPreference = 'Stop'; $owned = [System.Diagnostics.Process]::GetProcessById(${child.pid}); try { $null = $owned.Handle; [Console]::Out.WriteLine('OWNED_READY'); $owned.WaitForExit(); [Console]::Out.WriteLine('OWNED_EXITED') } finally { $owned.Dispose() }`], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] })
  watcher.stdout.on('data', chunk => {
    handleDiagnostics += chunk.toString()
    if (handleDiagnostics.includes('OWNED_READY')) handleReady.resolve()
    if (handleDiagnostics.includes('OWNED_EXITED')) confirmedExit = true
  })
  watcher.stderr.on('data', chunk => { handleDiagnostics += chunk.toString() })
  watcher.once('error', error => handleReady.reject(error))
  watcher.once('exit', code => {
    if (!handleDiagnostics.includes('OWNED_READY')) handleReady.reject(new Error(`Chrome handle acquisition failed (${code}): ${handleDiagnostics}`))
    handleExited.resolve()
  })
  let browser: Browser | undefined
  async function cleanup() {
    let failure: unknown
    if (browser) {
      try {
        const session = await browser.newBrowserCDPSession()
        await session.send('Browser.close')
      } catch (error) { if (child.exitCode === null) failure = error }
      await browser.close()
    }
    const grace = Promise.withResolvers<void>()
    const graceTimer = setTimeout(grace.resolve, 5000)
    const hasExited = () => confirmedExit || child.exitCode !== null || child.signalCode !== null
    try { await Promise.race([handleExited.promise, childExited.promise, grace.promise]) } finally { clearTimeout(graceTimer) }
    if (child.pid && !hasExited()) {
      const killed = Promise.withResolvers<number | null>()
      let terminationDiagnostics = ''
      const killer = spawn('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] })
      killer.stdout.on('data', chunk => { terminationDiagnostics += chunk.toString() })
      killer.stderr.on('data', chunk => { terminationDiagnostics += chunk.toString() })
      killer.once('error', killed.reject)
      killer.once('exit', killed.resolve)
      const code = await killed.promise
      const confirmed = Promise.withResolvers<void>()
      const deadline = setTimeout(confirmed.resolve, 5000)
      try { await Promise.race([handleExited.promise, childExited.promise, confirmed.promise]) } finally { clearTimeout(deadline) }
      if (!hasExited()) {
        watcher.kill()
        await handleExited.promise
        throw new Error(`Owned Chrome process ${child.pid} did not exit (taskkill ${code}): ${terminationDiagnostics}; handle: ${handleDiagnostics}; Chrome: ${diagnostics}`)
      }
    }
    // Either observer confirms the same owned process; do not leave the
    // independent handle watcher alive when Node delivers exit first.
    if (hasExited() && watcher.exitCode === null && watcher.signalCode === null) watcher.kill()
    await handleExited.promise
    fs.rmSync(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 })
    if (failure) throw failure
  }
  try {
    const handleTimer = setTimeout(() => handleReady.reject(new Error(`Chrome handle acquisition timed out: ${handleDiagnostics}`)), 15000)
    try { await handleReady.promise } finally { clearTimeout(handleTimer) }
    browser = await chromium.connectOverCDP(await endpoint.promise, { noDefaults: true })
    const context = browser.contexts()[0]
    const page = context.pages()[0] ?? await context.newPage()
    return { page, context, cleanup }
  } catch (error) {
    try { await cleanup() } catch (cleanupError) { throw new AggregateError([error, cleanupError], 'Native visibility browser startup/cleanup failed.') }
    throw error
  }
}
