import { spawn } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { chromium, type Browser } from '@playwright/test'

// Playwright's regular page installs an always-focused/visible emulation in its
// own CDP session. Connect without those defaults to observe native lifecycle
// events, while continuing to drive the delivered page through public APIs.
export async function nativeVisibilityBrowser(installedExecutable?: string) {
  const executable = installedExecutable ?? [process.env.PROGRAMFILES, process.env['PROGRAMFILES(X86)'], process.env.LOCALAPPDATA]
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
  const rootExited = Promise.withResolvers<void>()
  let confirmedRootExit = false
  child.once('exit', code => {
    clearTimeout(timeout)
    confirmedRootExit = true
    rootExited.resolve()
    endpoint.reject(new Error(`Chrome exited (${code}): ${diagnostics}`))
  })
  // Keep an independent Windows process handle: Chrome can terminate while
  // Node has not delivered its ChildProcess exit event yet.
  const handleReady = Promise.withResolvers<void>()
  const handleExited = Promise.withResolvers<void>()
  let confirmedProfileExit = false
  let handleDiagnostics = ''
  const watcher = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', `
    $ErrorActionPreference = 'Stop'
    $owned = [System.Diagnostics.Process]::GetProcessById(${child.pid})
    try {
      $null = $owned.Handle
      [Console]::Out.WriteLine('OWNED_READY')
      $owned.WaitForExit()
      [Console]::Out.WriteLine('OWNED_ROOT_EXITED')
      # Chrome's root can exit before its profile-owning utility processes.
      # Keep real handles to this exclusive profile's remaining processes.
      $remaining = @(Get-CimInstance Win32_Process -Filter "Name = 'chrome.exe'" | Where-Object { $_.CommandLine -and $_.CommandLine.Contains($env:OWNED_CHROME_PROFILE) } | ForEach-Object {
        $process = Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue
        if ($process) { $null = $process.Handle; $process }
      })
      try {
        $deadline = [DateTime]::UtcNow.AddMilliseconds(5000)
        foreach ($process in $remaining) {
          $budget = [Math]::Max(0, [int]($deadline - [DateTime]::UtcNow).TotalMilliseconds)
          if (-not $process.WaitForExit($budget)) { throw "Owned profile process $($process.Id) did not exit" }
        }
      } finally {
        foreach ($process in $remaining) { $process.Dispose() }
      }
      [Console]::Out.WriteLine('OWNED_EXITED')
    } finally { $owned.Dispose() }
  `], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: { ...process.env, OWNED_CHROME_PROFILE: profile } })
  watcher.stdout.on('data', chunk => {
    handleDiagnostics += chunk.toString()
    if (handleDiagnostics.includes('OWNED_READY')) handleReady.resolve()
    if (handleDiagnostics.includes('OWNED_ROOT_EXITED')) {
      confirmedRootExit = true
      rootExited.resolve()
    }
    if (handleDiagnostics.includes('OWNED_EXITED')) confirmedProfileExit = true
  })
  watcher.stderr.on('data', chunk => { handleDiagnostics += chunk.toString() })
  watcher.once('error', error => handleReady.reject(error))
  watcher.once('close', code => {
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
    try { await Promise.race([rootExited.promise, grace.promise]) } finally { clearTimeout(graceTimer) }
    if (child.pid && !confirmedRootExit) {
      const killed = Promise.withResolvers<number | null>()
      let terminationDiagnostics = ''
      const killer = spawn('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] })
      killer.stdout.on('data', chunk => { terminationDiagnostics += chunk.toString() })
      killer.stderr.on('data', chunk => { terminationDiagnostics += chunk.toString() })
      killer.once('error', killed.reject)
      killer.once('exit', killed.resolve)
      const code = await killed.promise
      // Confirm and, if needed, terminate the root through the original
      // ChildProcess handle rather than opening its potentially reused PID.
      if (!confirmedRootExit) child.kill('SIGKILL')
      const confirmed = Promise.withResolvers<void>()
      const deadline = setTimeout(confirmed.resolve, 5000)
      try { await Promise.race([rootExited.promise, confirmed.promise]) } finally { clearTimeout(deadline) }
      if (!confirmedRootExit) {
        watcher.kill()
        await handleExited.promise
        throw new Error(`Owned Chrome process ${child.pid} did not exit (taskkill ${code}): ${terminationDiagnostics}; handle: ${handleDiagnostics}; Chrome: ${diagnostics}`)
      }
    }
    // The root exit event is not proof that Chrome released this profile.
    // Let the independent handle observer finish checking utility processes.
    const profileDeadline = Promise.withResolvers<void>()
    const profileTimer = setTimeout(profileDeadline.resolve, 15000)
    try { await Promise.race([handleExited.promise, profileDeadline.promise]) } finally { clearTimeout(profileTimer) }
    if (!confirmedProfileExit) {
      watcher.kill()
      await handleExited.promise
      throw new Error(`Owned Chrome profile processes did not exit: ${handleDiagnostics}`)
    }
    fs.rmSync(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 })
    if (failure) throw failure
  }
  try {
    const handleTimer = setTimeout(() => handleReady.reject(new Error(`Chrome handle acquisition timed out: ${handleDiagnostics}`)), 15000)
    try { await handleReady.promise } finally { clearTimeout(handleTimer) }
    browser = await chromium.connectOverCDP(await endpoint.promise, { noDefaults: true })
    const context = browser.contexts()[0]
    const page = context.pages()[0] ?? await context.newPage()
    return { page, context, cleanup, processId: child.pid, profile,
      diagnostics: () => ({ confirmedExit: confirmedProfileExit, chrome: diagnostics, handles: handleDiagnostics }) }
  } catch (error) {
    try { await cleanup() } catch (cleanupError) { throw new AggregateError([error, cleanupError], 'Native visibility browser startup/cleanup failed.') }
    throw error
  }
}
