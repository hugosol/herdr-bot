import { afterEach, describe, expect, it, vi } from 'vitest'
import { suiteRequest, type Suite } from '../../src/controllers/suiteController'

const omp = {
  id: 'source-tools-omp', name: '源入口工具 · OMP', harness: 'omp', skills: ['../skill collections'],
  extensions: ['../omp native package', '../omp/entry.ts'], lsp: {}, mcp: ['search'],
} satisfies Suite
const pi = {
  id: 'source-tools-pi', name: '源入口工具 · Pi', harness: 'pi', skills: ['../skills/review'],
  extensions: ['../pi/entry.ts', '${roots.toolkit}/pi/extra.ts'], prompts: ['../prompts/review.md'], codemode: 'only', mcp: [],
} satisfies Suite
const catalog = {
  suites: [omp, pi], mcp: ['search'],
  fixedExtensions: [{ id: 'pi-connect-bot', label: '连接 Bot', agent: 'pi', kind: 'fixed', entrypoint: '/fixed/connect.ts', roles: ['standalone', 'manager', 'node', 'chief', 'chief-delegated'] }],
}

afterEach(() => vi.unstubAllGlobals())

describe('Single-harness Suite API contract', () => {
  it.each([omp, pi])('preserves the valid fields and direct source paths of $harness', async saved => {
    const fetch = vi.fn(async () => ({ ok: true, json: async () => catalog }))
    vi.stubGlobal('fetch', fetch)
    expect((await suiteRequest('PUT', saved.id, saved)).suites).toEqual([omp, pi])
    expect(fetch).toHaveBeenCalledWith(`/api/suites/${saved.id}`, {
      method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(saved),
    })
  })
  it.each([
    { suites: [], mcp: [] },
    { ...catalog, extensions: [] },
    ...[
      { ...omp, skills: { omp: ['../legacy'] } }, { ...pi, skills: [false] },
      { ...omp, extensions: { omp: [] } }, { ...omp, agents: ['omp'] },
      { ...omp, prompts: [] }, { ...omp, codemode: 'on' }, { ...pi, lsp: {} },
      { ...pi, codemode: undefined }, { ...pi, codemode: 'off' }, { ...pi, pi: { codemode: 'on' } },
      { ...omp, id: 'source-tools-pi' }, { ...pi, name: '源入口工具 · OMP' },
    ].map(suite => ({ ...catalog, suites: [suite] })),
    { ...catalog, fixedExtensions: [{ ...catalog.fixedExtensions[0], kind: 'optional' }] },
    { ...catalog, fixedExtensions: [{ ...catalog.fixedExtensions[0], roles: ['unknown'] }] },
  ])('rejects incomplete or obsolete catalog responses: %j', async response => {
    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: true, json: async () => response })))
    await expect(suiteRequest()).rejects.toThrow('Suite 响应无效，未确认保存。')
  })
})
