import { afterEach, describe, expect, it, vi } from 'vitest'
import { suiteRequest, type Suite } from '../src/suites/suiteApi'

const suite: Suite = {
  id: 'source-tools', name: '源入口工具', agents: ['omp', 'pi'], skills: { omp: ['../skill collections'], pi: ['../skills/review'] },
  extensions: { omp: ['../omp native package', '../omp/entry.ts'], pi: ['../pi/entry.ts', '${roots.toolkit}/pi/extra.ts'] },
  prompts: [], lsp: {}, mcp: [], pi: { codemode: 'on' },
}
const catalog = {
  suites: [suite], mcp: ['search'],
  fixedExtensions: [{ id: 'pi-connect-bot', label: '连接 Bot', agent: 'pi', kind: 'fixed', entrypoint: '/fixed/connect.ts', roles: ['standalone', 'manager', 'node', 'chief'] }],
}

afterEach(() => vi.unstubAllGlobals())

describe('Suite direct-source API contract', () => {
  it.each([{}, { omp: ['../skill collections'] }, { pi: ['../skills/review'] }, suite.skills])('preserves host-specific skill paths and accepts omitted hosts: %j', async skills => {
    const saved = { ...suite, skills }
    const fetch = vi.fn(async () => ({ ok: true, json: async () => ({ ...catalog, suites: [saved] }) }))
    vi.stubGlobal('fetch', fetch)
    expect((await suiteRequest('PUT', saved.id, saved)).suites).toEqual([saved])
    expect(fetch).toHaveBeenCalledWith(`/api/suites/${saved.id}`, {
      method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(saved),
    })
  })
  it.each([
    { suites: [], mcp: [] },
    { ...catalog, extensions: [] },
    ...[[], ['../legacy'], null, { other: [] }, { omp: 'directory' }, { pi: [false] }].map(skills => ({ ...catalog, suites: [{ ...suite, skills }] })),
    { ...catalog, fixedExtensions: [{ ...catalog.fixedExtensions[0], kind: 'optional' }] },
    { ...catalog, fixedExtensions: [{ ...catalog.fixedExtensions[0], roles: ['unknown'] }] },
  ])('rejects incomplete or obsolete catalog responses: %j', async response => {
    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: true, json: async () => response })))
    await expect(suiteRequest()).rejects.toThrow('Suite 响应无效，未确认保存。')
  })
})
