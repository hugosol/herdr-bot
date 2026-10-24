import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// Run from any working directory. The npm lifecycle entry deliberately retains
// pretest:browser's network guard; --list never launches the browser or backend.
const frontend = fileURLToPath(new URL('../frontend/', import.meta.url))
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'herdr-browser-partition-'))
const tag = '@time-contract'
const selections = {
  full: [],
  fast: ['--grep-invert', tag],
  time: ['--grep', tag],
}

function cases(report, selection) {
  assert.deepEqual(report.errors, [], `${selection}: runner collection errors`)
  const found = new Map()
  function visit(suite, parents = []) {
    const titles = [...parents, suite.title]
    for (const spec of suite.specs ?? []) {
      assert.equal(typeof spec.id, 'string', `${selection}: missing case id`)
      assert.ok(spec.id.length > 0, `${selection}: empty case id`)
      assert.ok(Array.isArray(spec.tests) && spec.tests.length > 0, `${selection}: case without projects`)
      assert.ok(Array.isArray(spec.tags), `${selection}: missing tag metadata`)
      assert.ok(!spec.file.endsWith('.manual.spec.ts'), `${selection}: manual case entered default collection`)
      for (const test of spec.tests) {
        assert.equal(typeof test.projectId, 'string', `${selection}: missing project id`)
        const id = JSON.stringify([spec.id, test.projectId])
        assert.ok(!found.has(id), `${selection}: duplicate case id ${id}`)
        found.set(id, {
          id,
          file: spec.file,
          line: spec.line,
          column: spec.column,
          title: [...titles, spec.title],
          projectId: test.projectId,
          tags: spec.tags,
        })
      }
    }
    for (const child of suite.suites ?? []) visit(child, titles)
  }
  assert.ok(Array.isArray(report.suites), `${selection}: missing runner suites`)
  for (const suite of report.suites) visit(suite)
  assert.ok(found.size > 0, `${selection}: empty collection`)
  return found
}

function collect(selection, filters) {
  const outputFile = path.join(temporary, `${selection}.json`)
  const args = ['run', 'test:browser', '--', '--list', '--reporter=json', ...filters]
  // Windows npm is a command shim. Only these fixed, non-user-controlled args
  // enter cmd.exe; no shell is needed on other platforms.
  const windows = process.platform === 'win32'
  const result = spawnSync(windows ? (process.env.ComSpec ?? 'cmd.exe') : 'npm',
    windows ? ['/d', '/s', '/c', `npm ${args.join(' ')}`] : args, {
      cwd: frontend,
      env: { ...process.env, PLAYWRIGHT_JSON_OUTPUT_FILE: outputFile },
      encoding: 'utf8',
      maxBuffer: 16 * 1024 * 1024,
    })
  if (result.error) throw result.error
  if (result.status !== 0) {
    throw new Error(`${selection}: npm test:browser collection failed (${result.status ?? result.signal})\n${result.stdout}\n${result.stderr}`)
  }
  return cases(JSON.parse(fs.readFileSync(outputFile, 'utf8')), selection)
}

try {
  const manifests = Object.fromEntries(Object.entries(selections).map(([name, filters]) => [name, collect(name, filters)]))
  const { full, fast, time } = manifests
  for (const [id, original] of full) {
    assert.notEqual(fast.has(id), time.has(id), `case must belong to exactly one partition: ${id} ${original.title.join(' > ')}`)
    const partition = time.has(id) ? time : fast
    assert.deepEqual(partition.get(id), original, `case metadata differs between default and partition: ${id}`)
    assert.equal(time.has(id), original.tags.includes(tag.slice(1)), `selection disagrees with details.tag: ${id}`)
  }
  for (const [selection, manifest] of Object.entries({ fast, time })) {
    for (const id of manifest.keys()) assert.ok(full.has(id), `${selection}: case absent from full collection: ${id}`)
  }
  // Emit complete identities, not merely counts, so newly collected parameterized
  // cases are automatically included in the equality checks and audit output.
  console.log(JSON.stringify({
    counts: Object.fromEntries(Object.entries(manifests).map(([name, manifest]) => [name, manifest.size])),
    manifests: Object.fromEntries(Object.entries(manifests).map(([name, manifest]) => [name, [...manifest.values()]])),
  }, null, 2))
} finally {
  fs.rmSync(temporary, { recursive: true, force: true })
}
