// H1-only external fixture. Copy into an isolated zone's native extension directory.
// A positive control must create this marker before its absence is meaningful.
import { appendFileSync } from 'node:fs'
import { join } from 'node:path'

export default function ambientProbe() {
  appendFileSync(join(process.cwd(), 'h1-ambient-loaded.jsonl'), JSON.stringify({
    probe: 'h1-ambient-extra', cwd: process.cwd(), pid: process.pid,
    loadedAt: new Date().toISOString(),
  }) + '\n')
}
