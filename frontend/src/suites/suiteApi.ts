export type Suite = {
  id: string
  name: string
  agents: ('omp' | 'pi')[]
  skills: string[]
  extensions: Partial<Record<'omp' | 'pi', string[]>>
  prompts: string[]
  lsp: Record<string, unknown>
  mcp: string[]
  pi?: { codemode: 'on' | 'only' }
}
export type SuiteList = { suites: Suite[]; mcp: string[] }

export async function suiteRequest(method = 'GET', id?: string, suite?: Suite): Promise<SuiteList> {
  const response = await fetch(`/api/suites${id === undefined ? '' : `/${encodeURIComponent(id)}`}`, {
    method,
    ...(suite === undefined ? {} : { headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(suite) }),
  })
  const result = await response.json()
  if (!response.ok) throw new Error(typeof result.message === 'string' ? result.message : 'Suite 请求失败。')
  if (!Array.isArray(result.suites) || !Array.isArray(result.mcp)) throw new Error('Suite 响应无效，未确认保存。')
  return result as SuiteList
}
