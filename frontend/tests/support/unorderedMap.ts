import type { MapNode, WorkflowMap } from '../../src/controllers/schedulerController'

const node = (title: string, blocked_by: string[] = []): MapNode => ({ title, blocked_by, bot_id: 'shared', input: '', zone: 'workspace' })
export const map: WorkflowMap = {
  title: 'Unordered dependencies', description: '',
  nodes: {
    finish: node('Finish', ['join', 'branch']),
    join: node('Join', ['alpha', 'beta']),
    branch: node('Branch', ['beta']),
    beta: node('Beta'),
    alpha: node('Alpha'),
    independent: node('Independent'),
  },
}

