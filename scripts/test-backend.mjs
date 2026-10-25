import { spawn } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { createInterface, emitKeypressEvents } from 'node:readline'
import { fileURLToPath } from 'node:url'

const manifest = fileURLToPath(new URL('../backend/Cargo.toml', import.meta.url))
const packageDirectory = path.dirname(manifest)

// This small seam lets disposable probes exercise dispatch/drain without Cargo.
// Results retain input order; queued entries were never handed to run().
export async function runQueue(targets, { jobs, run, isCancelled = () => false }) {
  if (!Number.isSafeInteger(jobs) || jobs < 1) throw new Error('jobs must be a positive integer')
  const results = targets.map(target => ({ ...target, status: 'queued' }))
  let next = 0
  let failed = false
  async function worker() {
    while (!failed && !isCancelled() && next < targets.length) {
      const index = next++
      results[index].status = 'running'
      try {
        const result = await run(targets[index], index)
        results[index] = { ...targets[index], ...result }
        if (result.status !== 'passed') failed = true
      } catch (error) {
        results[index] = { ...targets[index], status: 'failed', exitCode: null, signal: null, error: String(error) }
        failed = true
      }
    }
  }
  await Promise.all(Array.from({ length: Math.min(jobs, targets.length) }, () => worker()))
  return { results, queued: results.filter(result => result.status === 'queued'), failed, cancelled: isCancelled() }
}

function optionsFrom(argv) {
  const options = { binaryJobs: 4, testThreads: null, outputDir: null, help: false }
  for (let index = 0; index < argv.length; index++) {
    const argument = argv[index]
    if (argument === '--help') {
      options.help = true
      continue
    }
    if (!['--binary-jobs', '--test-threads', '--output-dir'].includes(argument)) {
      throw new Error(`Unknown option: ${argument}`)
    }
    const value = argv[++index]
    if (!value || value.startsWith('--')) throw new Error(`Missing value for ${argument}`)
    if (argument === '--output-dir') options.outputDir = path.resolve(value)
    else {
      if (!/^[1-9]\d*$/.test(value) || !Number.isSafeInteger(Number(value))) {
        throw new Error(`${argument} must be a positive integer`)
      }
      options[argument === '--binary-jobs' ? 'binaryJobs' : 'testThreads'] = Number(value)
    }
  }
  return options
}

function cancellationHandler(state) {
  function cancel(reason) {
    if (state.cancelled) return
    state.cancelled = true
    state.reason = reason
    console.error(`Cancellation requested (${reason}); stopping dispatch and waiting for started processes.`)
  }
  const interrupt = () => cancel('SIGINT')
  const terminate = () => cancel('SIGTERM')
  process.on('SIGINT', interrupt)
  process.on('SIGTERM', terminate)
  // On Windows, raw input also prevents a console Ctrl+C broadcast to children.
  // Detached process groups isolate children on POSIX, too; none are unref'ed.
  const input = process.stdin
  const useRaw = Boolean(input.isTTY && input.setRawMode)
  const wasRaw = input.isRaw
  const wasFlowing = input.readableFlowing === true
  const keypress = (_text, key) => {
    if (key?.ctrl && key.name === 'c') interrupt()
  }
  if (useRaw) {
    emitKeypressEvents(input)
    input.setRawMode(true)
    input.on('keypress', keypress)
    input.resume()
  }
  return () => {
    process.off('SIGINT', interrupt)
    process.off('SIGTERM', terminate)
    if (useRaw) {
      input.off('keypress', keypress)
      input.setRawMode(Boolean(wasRaw))
      if (!wasFlowing) input.pause()
    }
  }
}

function seconds(milliseconds) {
  return `${(milliseconds / 1000).toFixed(1)}s`
}

function showFailure(result) {
  console.error(`FAIL ${result.name}: ${result.error ?? result.signal ?? result.exitCode}`)
  for (const stream of ['stdout', 'stderr']) {
    const filename = result.log[stream]
    console.error(`--- ${stream}: ${filename} ---`)
    // Stream logs rather than retaining a potentially very large failed build.
    const fd = fs.openSync(filename, 'r')
    try {
      const buffer = Buffer.allocUnsafe(64 * 1024)
      let count
      while ((count = fs.readSync(fd, buffer)) > 0) fs.writeSync(2, buffer, 0, count)
    } finally {
      fs.closeSync(fd)
    }
    console.error('')
  }
}

async function runProcess(target, command, args, outputDir, onLine) {
  const started = performance.now()
  const log = {
    stdout: path.join(outputDir, `${target.logName}.stdout.log`),
    stderr: path.join(outputDir, `${target.logName}.stderr.log`),
  }
  const stdout = fs.openSync(log.stdout, 'w')
  let stderr
  try {
    stderr = fs.openSync(log.stderr, 'w')
  } catch (error) {
    fs.closeSync(stdout)
    throw error
  }
  console.log(`START ${target.name}`)
  let timer
  try {
    const result = await new Promise(resolve => {
      let error = null
      let child
      try {
        child = spawn(command, args, {
          cwd: target.cwd,
          env: process.env,
          // Windows DETACHED_PROCESS changes Git/MSYS child behavior. Raw TTY
          // input above handles Ctrl+C without broadcasting it to those children.
          // POSIX needs a separate process group to shield in-flight tests.
          detached: process.platform !== 'win32',
          windowsHide: true,
          stdio: ['ignore', 'pipe', 'pipe'],
        })
      } catch (spawnError) {
        resolve({ exitCode: null, signal: null, error: String(spawnError) })
        return
      }
      child.stdout.on('data', chunk => fs.writeSync(stdout, chunk))
      child.stderr.on('data', chunk => fs.writeSync(stderr, chunk))
      if (onLine) {
        const lines = createInterface({ input: child.stdout, crlfDelay: Infinity })
        lines.on('line', onLine)
      }
      child.on('error', spawnError => { error = String(spawnError) })
      child.on('close', (exitCode, signal) => resolve({ exitCode, signal, error }))
      timer = setInterval(() => {
        console.log(`RUNNING ${target.name} (${seconds(performance.now() - started)})`)
      }, 10000)
    })
    const durationMs = performance.now() - started
    const status = !result.error && result.exitCode === 0 && !result.signal ? 'passed' : 'failed'
    const complete = { ...target, ...result, status, durationMs, log, command, args }
    console.log(`${status === 'passed' ? 'DONE' : 'FAILED'} ${target.name} (${seconds(durationMs)})`)
    if (status === 'failed') showFailure(complete)
    return complete
  } finally {
    clearInterval(timer)
    fs.closeSync(stdout)
    fs.closeSync(stderr)
  }
}

export async function main(argv = process.argv.slice(2)) {
  let options
  try {
    options = optionsFrom(argv)
  } catch (error) {
    console.error(error.message)
    return 1
  }
  if (options.help) {
    console.log(`Usage: node scripts/test-backend.mjs [--binary-jobs N] [--test-threads N] [--output-dir PATH]

Build Cargo's default test targets, run discovered test binaries with bounded
parallelism, then run doctests only if every binary passed.
  --binary-jobs N   Maximum concurrent test binaries (default: 4)
  --test-threads N  Explicit libtest threads (default: libtest/RUST_TEST_THREADS)
  --output-dir PATH Store separate logs and summary.json here (default: new temp dir)
  --help           Show this help

CARGO_TARGET_DIR and the rest of the environment are inherited unchanged.
Ctrl+C/SIGTERM stop dispatch and wait for started processes; force-killing or
closing the terminal cannot guarantee cleanup. Logs are retained.`)
    return 0
  }

  const started = performance.now()
  const state = { cancelled: false, reason: null }
  const outputDir = options.outputDir ?? fs.mkdtempSync(path.join(os.tmpdir(), 'herdr-backend-tests-'))
  fs.mkdirSync(outputDir, { recursive: true })
  const summaryFile = path.join(outputDir, 'summary.json')
  const skipped = name => ({ name, status: 'not-run', durationMs: 0, exitCode: null, signal: null })
  const summary = {
    manifest,
    outputDir,
    configuration: { ...options, outputDir, rustTestThreads: process.env.RUST_TEST_THREADS ?? null, cargoTargetDir: process.env.CARGO_TARGET_DIR ?? null },
    startedAt: new Date().toISOString(),
    status: 'running',
    cancelled: false,
    cancellationReason: null,
    stages: { build: skipped('build'), binaries: skipped('binaries'), doctest: skipped('doctest') },
    targets: [],
    queued: [],
  }
  const restore = cancellationHandler(state)
  console.log(`Backend test logs: ${outputDir}`)
  try {
    const targets = []
    const seen = new Set()
    const discoveryErrors = []
    summary.stages.build = await runProcess({ name: 'build', cwd: packageDirectory, logName: 'build' },
      'cargo', ['test', '--manifest-path', manifest, '--no-run', '--message-format=json'], outputDir, line => {
        let artifact
        try {
          artifact = JSON.parse(line)
        } catch (error) {
          discoveryErrors.push(`Invalid Cargo JSON: ${error.message}`)
          return
        }
        if (artifact.reason !== 'compiler-artifact' || artifact.profile?.test !== true || !artifact.executable) return
        if (!artifact.manifest_path || !artifact.target?.name || !artifact.package_id) {
          discoveryErrors.push('Test artifact is missing package/target identity or manifest_path')
          return
        }
        if (seen.has(artifact.executable)) return
        seen.add(artifact.executable)
        targets.push({
          id: JSON.stringify([artifact.package_id, artifact.target.name, artifact.executable]),
          name: artifact.target.name,
          package: artifact.package_id,
          kind: artifact.target.kind,
          executable: artifact.executable,
          cwd: path.dirname(artifact.manifest_path),
          manifest: artifact.manifest_path,
          status: 'queued',
          exitCode: null,
          signal: null,
          durationMs: 0,
          log: null,
        })
      })
    // Cargo emits artifacts as compilation completes; use identity order so
    // scheduling does not depend on build completion timing or locale.
    targets.sort((left, right) => {
      for (const key of ['package', 'kind', 'name', 'executable']) {
        const a = Array.isArray(left[key]) ? left[key].join(',') : left[key]
        const b = Array.isArray(right[key]) ? right[key].join(',') : right[key]
        if (a < b) return -1
        if (a > b) return 1
      }
      return 0
    })
    summary.targets = targets
    if (discoveryErrors.length) {
      summary.stages.build.status = 'failed'
      summary.stages.build.discoveryErrors = discoveryErrors
      console.error(discoveryErrors.join('\n'))
    }
    if (summary.stages.build.status === 'passed' && !state.cancelled) {
      const binariesStarted = performance.now()
      const innerArgs = options.testThreads === null ? [] : ['--test-threads', String(options.testThreads)]
      const queue = await runQueue(targets, {
        jobs: options.binaryJobs,
        isCancelled: () => state.cancelled,
        run: (target, index) => runProcess({ ...target, logName: `binary-${String(index + 1).padStart(3, '0')}` },
          target.executable, innerArgs, outputDir),
      })
      summary.targets = queue.results
      summary.stages.binaries = {
        name: 'binaries',
        status: queue.failed ? 'failed' : queue.cancelled ? 'cancelled' : 'passed',
        durationMs: performance.now() - binariesStarted,
      }
      if (!queue.failed && !state.cancelled) {
        const docArgs = ['test', '--manifest-path', manifest, '--doc']
        if (options.testThreads !== null) docArgs.push('--', '--test-threads', String(options.testThreads))
        summary.stages.doctest = await runProcess({ name: 'doctest', cwd: packageDirectory, logName: 'doctest' },
          'cargo', docArgs, outputDir)
      }
    }
    summary.status = state.cancelled ? 'cancelled'
      : Object.values(summary.stages).every(stage => stage.status === 'passed') ? 'passed' : 'failed'
  } catch (error) {
    summary.status = state.cancelled ? 'cancelled' : 'failed'
    summary.error = error.stack ?? String(error)
    console.error(summary.error)
  } finally {
    restore()
    summary.cancelled = state.cancelled
    summary.cancellationReason = state.reason
    summary.queued = summary.targets.filter(target => target.status === 'queued')
    summary.durationMs = performance.now() - started
    summary.finishedAt = new Date().toISOString()
    fs.writeFileSync(summaryFile, `${JSON.stringify(summary, null, 2)}\n`)
    console.log(`${summary.status.toUpperCase()} backend tests (${seconds(summary.durationMs)}); ${summary.queued.length} binaries not dispatched`)
    for (const [name, stage] of Object.entries(summary.stages)) {
      console.log(`  ${name}: ${stage.status} (${seconds(stage.durationMs)})`)
    }
    console.log(`Summary: ${summaryFile}`)
  }
  return summary.status === 'passed' ? 0 : 1
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().then(code => { process.exitCode = code }).catch(error => {
    console.error(error.stack ?? String(error))
    process.exitCode = 1
  })
}
