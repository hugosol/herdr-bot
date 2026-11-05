import { afterEach, describe, expect, it, vi } from 'vitest'
import { suiteRequest, type Suite } from '../src/suites/suiteApi'

const suite: Suite = {
  id: 'source-tools', name: '源入口工具', agents: ['omp', 'pi'], skills: [],
  extensions: { omp: ['../omp native package', '../omp/entry.ts'], pi: ['../pi/entry.ts', '${roots.toolkit}/pi/extra.ts'] },
  prompts: [], lsp: {}, mcp: [], pi: { codemode: 'on' },
}
const catalog = {
  suites: [suite], mcp: ['search'],
  fixedExtensions: [{ id: 'pi-connect-bot', label: '连接 Bot', agent: 'pi', kind: 'fixed', entrypoint: '/fixed/connect.ts', roles: ['standalone', 'manager', 'node'] }],
}

afterEach(() => vi.unstubAllGlobals())

describe('Suite direct-source API contract', () => {
  it.each([
    { suites: [], mcp: [] },
    { ...catalog, extensions: [] },
    { ...catalog, fixedExtensions: [{ ...catalog.fixedExtensions[0], kind: 'optional' }] },
    { ...catalog, fixedExtensions: [{ ...catalog.fixedExtensions[0], roles: ['unknown'] }] },
  ])('rejects incomplete or obsolete catalog responses: %j', async response => {
    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: true, json: async () => response })))
    await expect(suiteRequest()).rejects.toThrow('Suite 响应无效，未确认保存。')
  })
})
