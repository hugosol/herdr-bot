"""Offline public-wire, publication and evidence consumers for ticket 14."""
import copy
import json
import unittest

from node_approval_v1_acceptance import (
    V1, approval_page_candidates, manager_complete_observations, require_form_sources, require_v1_request,
)

# Independent HTTP fixture follows backend/tests/workflow/reports/approval.rs:
# 409 snapshot, ready current report, approved execution, required delivery absent.
# The status and body are inputs to the same evidence reader as the live proxy.
COMPLETE = {
    'index': 23, 'method': 'POST', 'path': '/api/workflow/manager/complete',
    'status': 409, 'started': 100, 'ended': 101,
    'request': '{"sessionId":"approval-manager","nodeId":"A","reason":"Accept only current handoff"}',
    'response': json.dumps({
        'changed': False, 'code': 'output-missing',
        'message': '当前 execution 尚未登记约定交付 delivery。',
        'snapshot': {'id': 'owned-run', 'manager': {'sessionId': 'approval-manager'}, 'nodes': {
            'A': {'status': 'running', 'execution': {'id': 'current-execution', 'approved': True,
                  'outputRule': [{'id': 'delivery'}, {'id': 'notes'}]}}}},
        'reports': [
            {'id': 'initial', 'nodeId': 'A', 'executionId': 'current-execution', 'canComplete': False, 'outputs': [{'id': 'notes'}]},
            {'id': 'old', 'nodeId': 'A', 'executionId': 'old-execution', 'canComplete': True, 'outputs': [{'id': 'delivery'}]},
            {'id': 'ready', 'nodeId': 'A', 'executionId': 'current-execution', 'canComplete': True, 'outputs': []},
        ],
    }),
}


class CaseEvidence(unittest.TestCase):
    def test_real_manager_output_refusal_keeps_current_execution_and_exact_wire(self):
        event, = manager_complete_observations([COMPLETE])
        self.assertEqual(event['wire'], COMPLETE)
        self.assertEqual(event['runId'], 'owned-run')
        self.assertEqual(event['executionId'], 'current-execution')
        self.assertEqual(event['missingOutputIds'], ['delivery'])
        self.assertEqual(event['registeredOutputIds'], ['notes'])
        self.assertTrue(event['confirmedOutputMissing'])

    def test_claim_in_model_reason_is_not_a_wire_refusal(self):
        claim = dict(COMPLETE, status=200, response='{"reason":"409 output-missing"}')
        self.assertFalse(manager_complete_observations([claim])[0]['confirmedOutputMissing'])

    def test_later_public_commit_is_distinct_from_earlier_refusal(self):
        committed = json.loads(COMPLETE['response'])
        committed.update(changed=True, historyError=None)
        del committed['code'], committed['message']
        committed['snapshot']['nodes']['A'].update(status='completed', completion={
            'id': 'server-assigned-acceptance', 'actor': {'sessionId': 'approval-manager'},
            'reason': 'Accept only current handoff', 'at': 200,
        })
        committed['reports'].append({'id': 'delivery-ready', 'nodeId': 'A', 'executionId': 'current-execution',
                                     'canComplete': True, 'outputs': [{'id': 'delivery'}]})
        accepted = dict(COMPLETE, index=25, status=200, started=199, ended=201, response=json.dumps(committed))
        refused, accepted = manager_complete_observations([COMPLETE, accepted])
        self.assertTrue(refused['confirmedOutputMissing'])
        self.assertFalse(refused['confirmedComplete'])
        self.assertFalse(accepted['confirmedOutputMissing'])
        self.assertTrue(accepted['confirmedComplete'])
        self.assertEqual(accepted['registeredOutputIds'], ['delivery', 'notes'])

    def test_wrong_manager_cannot_correlate_public_refusal(self):
        event, = manager_complete_observations([dict(COMPLETE, request=COMPLETE['request'].replace('approval-manager', 'different-session'))])
        self.assertFalse(event['confirmedOutputMissing'])

    def test_candidate_eligible_before_completion_without_admitting_child_or_wrong_source(self):
        definition = {'nodes': {'write_when': {'auto_approved': False}, 'save_bot': {'auto_approved': True}}}
        detail = {'id': 'owned-v1', 'run': {'map': V1, 'status': 'running', 'nodes': {
            'write_when': {'execution': {'id': 'current-write', 'approved': True,
                                         'autoApproved': False, 'approvalSource': 'manual'}},
            'save_bot': {'execution': {'id': 'wrong-source', 'approved': True,
                                       'autoApproved': True, 'approvalSource': 'manual'}},
        }}}
        child = copy.deepcopy(detail)
        child['id'] = 'other-v1'
        modes = {'write_when': False, 'save_bot': True}
        self.assertEqual(approval_page_candidates(child, 'owned-v1', V1, definition, modes), {})
        self.assertEqual(approval_page_candidates(detail, 'owned-v1', V1, definition, modes), {
            V1 + '/write_when': {'runId': 'owned-v1', 'executionId': 'current-write',
                                'definition': definition, 'autoApproved': False},
        })

    def test_ask_option_does_not_supply_its_own_authority(self):
        facts = {'actualCallerInput': '保留原配置；标题使用"本地报告"。', 'actualFullCandidate': None,
                 'fields': [{'options': [{'name': 'Allow remote upload', 'value': 'allow'}]}]}
        with self.assertRaises(AssertionError):
            require_form_sources([{'name': 'upload', 'value': 'allow', 'sourceQuote': 'Allow remote upload'}], facts)
        require_form_sources([{'name': 'title', 'value': '本地报告', 'sourceQuote': '"本地报告"'}], facts)

    def test_valid_schema_alone_cannot_replace_literal_business_requirements(self):
        # Independent V1 case requirements; the public script schema alone
        # permits different business text and array contents.
        request = {'schema_version': 1, 'kind': 'request',
                   'target': {'type': 'task', 'task_id': 'owned-normal-task'},
                   'bot': {'id': 'native-v1-pi', 'name': 'Native V1 pi'},
                   'requirements': '本地授权账单只读差异核对能力的配置交付',
                   'constraints': ['只读', '不联网', '不上传', '不付款', '不执行目标'],
                   'non_goals': [], 'resource_limits': []}
        require_v1_request(request, 'owned-normal-task', 'pi')
        for key, value in [('requirements', '方便地分析账单'), ('constraints', ['只读']),
                           ('non_goals', ['更改目标']), ('resource_limits', ['新授权权限']),
                           ('confirmation', 'yes'), ('schema_version', True)]:
            with self.subTest(field=key), self.assertRaises(AssertionError):
                require_v1_request(dict(request, **{key: value}), 'owned-normal-task', 'pi')


if __name__ == '__main__':
    unittest.main()
