"""Revision-owned real-host cases at public Chat/Run/HTTP/file seams.

Independent outcomes: existing Bot identity/binding and complete target unchanged;
three separately approved literal versions saved once; every direct consumer
actually reads its named files. Completed Run status is only a stopping signal.
"""
import json
from pathlib import Path
import shutil
import time
import traceback
import urllib.parse

from workflow_create_acceptance import (TASK_IDS, FIELDS, bot, save_bot, fixture,
    task_chat, check, write_json, record_bytes, read_evidence, native_text,
    verify_target, verify_handoff, observed_saves, evaluate_guides,
    protected_configuration)

REPO = Path(__file__).resolve().parents[1]
DEDICATED = 'task-revise-bot-baseline-v1'
MAP = 'enhance-existing-bot--bc-301b25d9f8d6.json'
NODE_TASKS = dict(zip(['w1', 'w2', 'w3', 'w4', 'w5'],
    [DEDICATED, TASK_IDS[1], TASK_IDS[2], TASK_IDS[3], TASK_IDS[5]]))
OLD = {'when': '已有账单只读能力；扫描边界须明确，不付款。',
    'plan_guide': '先核实来源、只读授权和人民币分，再安排已有目标；不外网。',
    'node_guide': '提供账单 CSV、只读授权和差异报告位置；不修改账单。'}
CLARIFICATION = '扫描仅指读取本次用户明确提供且授权的账单 CSV，不搜索硬盘、不连接外网、不付款，金额保持人民币分整数。'
CASE_NAMES = ('dedicated-chat', 'task-no-materials', 'task-materials', 'map-no-materials', 'map-materials',
    'revise-unauthorized', 'revise-baseline-conflict', 'revise-target-conflict', 'revise-running',
    'revise-revision', 'revise-applied', 'revise-not-applied', 'revise-unknown', 'revision-baseline-files')


def select_cases(value):
    if value == 'all':
        return list(CASE_NAMES)
    names = value.split(',')
    if any(name not in CASE_NAMES for name in names) or len(names) != len(set(names)):
        raise ValueError('Invalid revision case selection: use all or unique comma-separated case names')
    return [name for name in CASE_NAMES if name in names]


def deploy(case):
    tasks = []
    for identity in [case.manager['id'], DEDICATED, TASK_IDS[1], TASK_IDS[2], TASK_IDS[3], TASK_IDS[5]]:
        original = case.original_tasks[identity]
        task = dict(original, model='probe', thinking='default')
        if case.host != 'declared':
            task['agent'] = case.host
        tasks.append(task)
    write_json(case.root / 'data/tasks.json', {'tasks': tasks})
    case.deployed = {task['id']: task for task in tasks}
    case.manager_id = case.manager['id']
    case.hosts = sorted({task['agent'] for task in tasks})
    for task in tasks:
        actual = case.http('/api/tasks/' + urllib.parse.quote(task['id'], safe=''))['task']
        expected = dict(task)
        if expected.get('suite') is None:
            expected.pop('suite', None)
        check(actual == expected, ('Revision changed shipped Task beyond isolated transport', actual, expected))
    definition = json.loads((REPO / 'meta-data/workflow-maps' / MAP).read_text(encoding='utf-8'))
    check(list(definition['nodes']) == list(NODE_TASKS), 'Revision Map must have exactly five works, no optimizer stage')
    published = {}
    for path in sorted((REPO / 'meta-data/bots').glob('*.json')):
        content = json.loads(path.read_text(encoding='utf-8'))
        check(content['id'] not in published, ('Duplicate published Bot identity', content['id']))
        published[content['id']] = (path, content)
    for node_id, node in definition['nodes'].items():
        node['zone'] = 'acceptance/create'
        identity = node['bot_id']
        check(identity in published, ('Missing published revision capability', identity))
        source, content = published[identity]
        check(content.get('task_id') == NODE_TASKS[node_id], ('Revision capability changed stable Task binding', identity))
        shutil.copy2(source, case.root / 'bots' / source.name)
    write_json(case.root / 'maps' / MAP, definition)
    case.revision_map = definition
    case.evidence['declaredTaskHosts'] = {key: case.original_tasks[key]['agent'] for key in case.deployed}
    case.evidence['deployedRevisionTaskHosts'] = {key: value['agent'] for key, value in case.deployed.items()}


def revision_fixture(case, label, kind, optional=False):
    context = fixture(case, 'revise-' + label, kind)
    case.evidence.setdefault('revisionTargets', []).append({'taskId': context['targetTask']['id'], 'binding': context['target']})
    context['id'] = 'p22-' + label
    save_bot(case, context['id'], {'name': '已有修订账单能力 ' + label, **context['target'], **OLD})
    original, revision = bot(case, context['id'])
    context.update(original=original, revision=revision)
    request = (f'修订真实既有 Bot ID={context["id"]}，只修订 when、plan_guide、node_guide 三字段；本次独立业务要求是澄清“扫描”的边界并保留账单只读汇总和人民币分差异报告。'
        '扫描是否包括搜索本机硬盘或外网尚未决定，必须在当前 Chat 提问真实澄清，不能猜测，也不能沿用旧文本替用户决定。'
        '先自行读取目录、完整 Bot/revision 和唯一绑定的完整真实 Task/Map；本请求故意不抄配置。'
        '不创建、修改、运行、改绑目标，不优化 Task.prompt，不修改名称或其他字段，不增设独立表达优化阶段。'
        f'仅授权在本次独占目录 {context["directory"]} 安全新建 baseline、text、handoff、saved；写前核实真实路径、父路径、链接、授权和已有文件，完整 native read 读回。'
        '三路各自展示 BEGIN_APPROVAL_when/END_APPROVAL_when、BEGIN_APPROVAL_plan_guide/END_APPROVAL_plan_guide、BEGIN_APPROVAL_node_guide/END_APPROVAL_node_guide 对应标记间所属完整正文；标记不进文件。'
        '整理表达在各自 Chat 完成，最终文件必须与实际全文批准版本逐字相同；任何变化必须重新展示确认。'
        '每路全文确认独立于 manual 报告发送确认；最终三路确认后授权保存者仅一次合法三字段保存。'
        '共同 baseline 用无围栏 JSON 对象，键 bot、revision、target、requirements、authorization；bot 是包含真实 id 与全部配置的完整 Bot 对象，revision 原样取自 edit；Task 型 target 原样取自 GET 响应中的完整 task 对象（不含外层 task 包装），Map 型 target 原样取自完整 GET 响应对象，并保留本委托及实际澄清依据。saved 同样是包含真实 id 与全部配置的完整公开读回 Bot JSON，不是只有字段片段；保存请求 content 仍按公开接口规则不包含顶层 id。'
        'handoff 只记录准确来源、确认标识、授权、保留语义和未验证项，不复制配置或正文，不隐式传地址。')
    context['requirements'] = request
    context['request'].write_text(request, encoding='utf-8')
    context['materials'] = None
    if optional:
        path = context['directory'] / 'materials.md'
        path.write_text('可选业务参考 P22-MATERIAL-802：报告按类别给出人民币分整数差异，保留只读与不付款约束。此材料不决定扫描范围或授权执行。', encoding='utf-8')
        context['materials'] = path
    return context


def verify_baseline(case, context, path):
    raw = record_bytes(case, path, 'revision-common-baseline')
    value = json.loads(raw)
    check(value['bot'] == dict(id=context['id'], **context['original']) and value['revision'] == context['revision'],
        'Baseline omitted or changed full existing Bot identity/configuration/revision')
    check(value['target'] == context['before'], 'Baseline omitted complete real Task/Map target')
    check('P22-CLARIFY-' + context['label'] in raw, 'Baseline lost actual business clarification provenance')
    return raw


def clarify(case, chat, context):
    history = case.await_chat(chat)
    visible = native_text(case, history)
    check(any(word in visible for word in ['扫描', '范围']) and any(word in visible for word in
        ['？', '?', '请确认', '请明确', '请澄清', '需要你', '需要您', '请选择']),
        'Executor did not actually ask the unresolved business question')
    marker = 'P22-CLARIFY-' + context['label']
    case.send(chat, f'用户业务澄清 {marker}：{CLARIFICATION} 仅核实与修订指南，不授权运行目标。', marker)


def approve_field(case, chat, field, context):
    history = case.await_chat(chat)
    visible = native_text(case, history)
    start, end = 'BEGIN_APPROVAL_' + field, 'END_APPROVAL_' + field
    check(start in visible and end in visible, ('Missing displayed full approval version', field))
    text = visible.rsplit(start, 1)[1].split(end, 1)[0].strip('\r\n')
    check(len(text) > 40, ('Incomplete guide offered for approval', field))
    marker = 'P22-APPROVE-' + context['label'] + '-' + field
    approval = (f'用户独立全文确认 {marker}：明确批准 {field} 以下完整正文，整理已完成，保存时逐字使用；若再变动须重新确认。\n{text}\n'
        '三字段全部确认后授权最终保存者一次合法保存，保持目标、名称、绑定及其他字段不变。当前尚未同意发送 manual 报告，不运行目标。')
    case.send(chat, approval, marker)
    case.evidence.setdefault('fieldApprovals', []).append({'sessionId': chat['sessionId'], 'field': field,
        'draft': text, 'marker': marker, 'userApproval': approval})
    return text


def send_report(case, chat, context, node):
    marker = 'P22-SEND-' + context['label'] + '-' + node
    terminal = case.command([case.args.herdr, 'pane', 'read', chat['paneId']])
    if '发送报告？（仅批准本次报告发送）' not in terminal:
        case.send(chat, f'用户请求准备报告 {marker}：请调用 report 展示本节点已核实的真实交付位置和结论，等待我在 UI 上确认；此文字不批准发送，不新增业务批准、保存授权或执行权限。', marker)
        terminal = case.command([case.args.herdr, 'pane', 'read', chat['paneId']])
    check('发送报告？（仅批准本次报告发送）' in terminal and 'Yes' in terminal and 'No' in terminal,
        'Missing actual native report confirmation UI')
    check('→ Yes' in terminal or '❯ Yes' in terminal, 'Native UI does not visibly select Yes')
    before = [r for r in case.backend_wire.records if r['path'] == '/api/workflow/node/report'
        and json.loads(r['request']).get('sessionId') == chat['sessionId']]
    check(not before, 'Manual report HTTP preceded explicit UI Yes')
    approval = {'purpose': marker, 'chat': chat, 'preview': terminal, 'choice': 'Yes',
        'beforeReportRequests': before, 'at': time.monotonic()}
    case.evidence.setdefault('reportUIApprovals', []).append(approval)
    approval['result'] = case.herdr('agent', 'send-keys', chat['paneId'], 'enter')
    # Await the actual tool result, not the stale confirmation screen immediately after Enter.
    deadline = time.monotonic() + case.args.timeout
    while True:
        history = case.native(chat)
        messages = case.messages(history) if history else []
        if messages and messages[-1].get('role') == 'assistant' and not any(
                part.get('type') == 'toolCall' for part in messages[-1].get('content', []) if isinstance(part, dict)):
            break
        check(time.monotonic() < deadline, 'UI-approved report did not reach native result boundary')
        time.sleep(.25)
    case.await_chat(chat)
    approval['reportRequests'] = [r for r in case.backend_wire.records if r['path'] == '/api/workflow/node/report'
        and json.loads(r['request']).get('sessionId') == chat['sessionId']]
    check(len(approval['reportRequests']) == 1 and approval['reportRequests'][0].get('status') == 200,
        'Explicit UI Yes did not produce exactly one successful report HTTP')


def verify_public_reads(case, context, records):
    check(any(record['method'] == 'GET' and record['path'] == '/api/scheduler/bots' and record.get('status') == 200
        for record in records), 'Revision executor never read the actual Bot catalog')
    check(any(record['path'] == '/api/scheduler/bots/edit' and record.get('status') == 200
        and json.loads(record['request']).get('id') == context['id'] for record in records),
        'Revision executor never read the real full Bot/revision')
    check(any(record['method'] == 'GET' and record['path'] == context['route'] and record.get('status') == 200
        for record in records), 'Revision executor never read its real complete target')
    check(not any(record['path'] == '/api/chats' and record['method'] == 'POST'
        and json.loads(record['request']).get('taskId') == context['targetTask']['id'] for record in records),
        'Revision executor automatically launched the existing target Task')
    case.evidence.setdefault('publicBaselineReads', []).append({'case': context['label'], 'records': records})


def standalone_baseline(case, context):
    before_wire = len(case.backend_wire.records)
    root = context['directory']
    baseline, handoff = root / 'baseline.json', root / 'baseline-handoff.md'
    chat = task_chat(case, DEDICATED, context['requirements'] + f'\n独立 Chat，本次只交付 {baseline} 与 {handoff}，不是 workflow，无需报告。')
    clarify(case, chat, context)
    history = case.await_chat(chat)
    raw = verify_baseline(case, context, baseline)
    for path in [baseline, handoff]:
        read_evidence(case, history, path)
    verify_handoff(case, handoff, [], raw)
    check(not observed_saves(case, context['id']), 'Read-only baseline Task wrote Bot')
    verify_target(case, context)
    check(bot(case, context['id']) == (context['original'], context['revision']), 'Standalone baseline changed existing Bot')
    verify_public_reads(case, context, case.backend_wire.records[before_wire:])
    case.evidence['taskPairs'][DEDICATED]['standalone'] = True
    case.evidence['cases'].append({'mode': 'standalone-revision-baseline', 'kind': context['kind'], 'chat': chat})


def workflow(case, context):
    files = [{'id': 'request', 'path': str(context['request'])}]
    if context['materials']:
        files.append({'id': 'materials', 'path': str(context['materials'])})
    protected = protected_configuration(case)
    before_wire = len(case.backend_wire.records)
    previous_pids = {pid for pid, _ in case.handles}
    source_bytes = {str(Path(item['path'])): Path(item['path']).read_bytes() for item in files}
    started = case.http('/api/scheduler/maps/' + MAP + '/runs', {'managerTaskId': case.manager_id,
        'zone': 'acceptance/create', 'userInput': context['requirements'] + '\n管理者自主 run/collect，核实真实交接后显式 complete，全部完成后 finish；不能代用户确认或运行目标。',
        'fileInputs': files}, status=201)
    run_id = started['id']
    case.evidence.setdefault('workflowRuns', []).append({'inputFiles': files, 'started': started})
    handled, approved = set(), {}
    deadline = time.monotonic() + case.args.timeout * 8
    while True:
        detail = case.http('/api/workflow/runs/' + run_id)
        manager = detail['run'].get('manager')
        if manager:
            case.chat_hosts[manager['sessionId']] = case.deployed[case.manager_id]['agent']
        for node_id, node in detail['run']['nodes'].items():
            execution = node.get('execution')
            if not execution or not execution.get('chat'):
                continue
            expected_id = NODE_TASKS[node_id]
            check(execution['taskId'] == expected_id and execution['agent'] == case.deployed[expected_id]['agent'],
                ('Changed actual stable Task/declared host', node_id, execution))
            chat = execution['chat']
            case.chat_hosts[chat['sessionId']] = execution['agent']
            if node_id == 'w1' and node_id not in handled:
                clarify(case, chat, context)
                current = case.http('/api/workflow/runs/' + run_id)
                check(not any(r['nodeId'] == node_id and r.get('canComplete') for r in current['reports']), 'Baseline manual report preceded send authorization')
                send_report(case, chat, context, node_id)
                handled.add(node_id)
            elif node_id in ['w2', 'w3', 'w4'] and node_id not in handled:
                field = FIELDS[int(node_id[1]) - 2]
                approved[field] = approve_field(case, chat, field, context)
                current = case.http('/api/workflow/runs/' + run_id)
                check(not any(r['nodeId'] == node_id and r.get('canComplete') for r in current['reports']), 'Guide manual report preceded send authorization')
                send_report(case, chat, context, node_id)
                handled.add(node_id)
        if not detail['run']['nodes']['w5'].get('execution'):
            check(not observed_saves(case, context['id']), 'Baseline/writer directly saved Bot before final saver')
        if detail['run']['status'] == 'completed':
            break
        check(detail['run']['status'] not in ['failed', 'cancelled'], detail)
        check(time.monotonic() < deadline, 'Real revision workflow did not finish')
        time.sleep(.25)
    check(set(approved) == set(FIELDS), 'Missing independent full-field confirmations')
    actual = {}
    for node_id in NODE_TASKS:
        reports = [r for r in detail['reports'] if r['nodeId'] == node_id and r.get('canComplete') and r.get('outputs')]
        check(len(reports) == 1, ('Expected one actual material report', node_id, reports))
        execution = detail['run']['nodes'][node_id]['execution']
        actual[node_id] = {item['id']: Path(item['path']) for item in reports[0]['outputs']}
        check(set(actual[node_id]) == {item['id'] for item in case.revision_map['nodes'][node_id]['output_rule']}, 'Wrong material outputs')
        if node_id == 'w1':
            expected = {item['id']: Path(item['path']) for item in files}
        elif node_id != 'w5':
            expected = {'baseline': actual['w1']['baseline']}
        else:
            expected = {'baseline': actual['w1']['baseline']}
            for branch, field in zip(['w2', 'w3', 'w4'], FIELDS):
                expected[field + '_text'] = actual[branch]['text']
                expected[field + '_handoff'] = actual[branch]['handoff']
        check({item['id']: Path(item['path']) for item in execution['inputs']} == expected,
            ('Consumer did not receive direct unique baseline/text/handoff', node_id, execution))
        history = case.native(execution['chat'])
        for path in actual[node_id].values():
            check(path.is_absolute() and path.is_relative_to(context['directory']), ('Revision output escaped explicit business write authorization', node_id, str(path)))
        for path in [*expected.values(), *actual[node_id].values()]:
            read_evidence(case, history, path)
        if node_id in ['w2', 'w3', 'w4']:
            siblings = [field for branch, field in zip(['w2', 'w3', 'w4'], FIELDS) if branch != node_id]
            calls = [json.dumps(message, ensure_ascii=False) for message in case.messages(history) if message.get('role') == 'assistant']
            case.evidence.setdefault('independentBranchHistories', []).append({'node': node_id, 'sessionId': history['id'], 'nonOwnedFields': siblings, 'actualCalls': calls})
    addresses = [path.resolve() for outputs in actual.values() for path in outputs.values()]
    check(len(addresses) == len(set(addresses)), 'Duplicate/overwritten material output path')
    baseline = verify_baseline(case, context, actual['w1']['baseline'])
    verify_public_reads(case, context, case.backend_wire.records[before_wire:])
    saved, _ = bot(case, context['id'])
    saves = observed_saves(case, context['id'])
    check(len(saves) == 1 and saves[0].get('status') == 200, ('Expected exactly one legal full three-field save', saves))
    payload = json.loads(saves[0]['request'])
    check(payload['revision'] == context['revision'], 'Saver did not use real common baseline revision')
    check(json.loads(payload['content']) == {key: value for key, value in saved.items() if key != 'id'}, 'Save payload/full readback mismatch')
    check({key: value for key, value in saved.items() if key not in FIELDS} ==
        {key: value for key, value in context['original'].items() if key not in FIELDS}, 'Changed non-target Bot fields/binding')
    check(json.loads(record_bytes(case, actual['w5']['saved'], 'revision-full-saved')) == dict(id=context['id'], **saved),
        'Incomplete saved Bot identity/configuration readback artifact')
    for branch, field in zip(['w2', 'w3', 'w4'], FIELDS):
        text = record_bytes(case, actual[branch]['text'], 'revision-' + field)
        check(saved[field] == text == approved[field], ('Saved field differs from approved exact source', field))
        handoff = verify_handoff(case, actual[branch]['handoff'], [text], baseline)
        check('P22-APPROVE-' + context['label'] + '-' + field in handoff, 'Lost actual approval provenance')
        # A writer cannot consume a sibling final body, even incidentally.
        history = case.native(detail['run']['nodes'][branch]['execution']['chat'])
        for other in ['w2', 'w3', 'w4']:
            if other != branch:
                forbidden = str(actual[other]['text']).replace('\\', '/')
                for message in case.messages(history):
                    if message.get('role') != 'assistant':
                        continue
                    for call in message.get('content', []) if isinstance(message.get('content'), list) else []:
                        if isinstance(call, dict) and call.get('type') == 'toolCall':
                            arguments = json.dumps(call.get('arguments', {}), ensure_ascii=False).replace('\\\\', '/').replace('\\', '/')
                            check(forbidden not in arguments, 'Writer routed a native tool to a sibling final text')
    verify_handoff(case, actual['w1']['handoff'], [], baseline)
    saved_handoff = verify_handoff(case, actual['w5']['handoff'], list(approved.values()), baseline,
        additional_objects=[dict(id=context['id'], **saved), saved])
    check(all('P22-APPROVE-' + context['label'] + '-' + field in saved_handoff for field in FIELDS), 'Saver lost branch approval source')
    verify_target(case, context)
    after = protected_configuration(case)
    expected_changed = str(case.root / 'bots' / context['revision']['file'])
    check(all(after.get(path) == value for path, value in protected.items() if path != expected_changed), 'Revision changed another definition/resource')
    check(set(after) == set(protected), 'Revision created unexpected definitions')
    check(all(Path(path).read_bytes() == value for path, value in source_bytes.items()), 'Revision changed actual request/optional source bytes')
    unique_sources = [actual['w1']['baseline'], *[actual[branch]['text'] for branch in ['w2', 'w3', 'w4']]]
    for source in unique_sources:
        copies = [path for path in context['directory'].rglob('*') if path.is_file() and path.read_bytes() == source.read_bytes()]
        check(copies == [source], ('Additional common baseline/final body copy', str(source), list(map(str, copies))))
    evaluate_guides(case, context, approved, approved)
    processes = case.observe_processes()
    expected_hosts = [case.deployed[case.manager_id]['agent']] + [node['execution']['agent'] for node in detail['run']['nodes'].values()]
    counts = {}
    for host in set(expected_hosts):
        entry = Path(case.evidence['installedHosts'][host]['entry']).resolve()
        observed = [process for process in processes if process['ProcessId'] not in previous_pids and any(
            Path(argument).resolve() == entry for argument in process['arguments'] if argument.replace('\\', '/').endswith('/cli.js'))]
        counts[host] = len(observed)
        check(counts[host] == expected_hosts.count(host), ('Actual installed host OS inventory differs from revision role declarations', host, counts, expected_hosts))
    case.evidence.setdefault('workflowHostInventory', []).append({'runId': run_id, 'declared': expected_hosts, 'actualProcessCounts': counts})
    case.evidence['taskPairs'][DEDICATED]['workflow'] = True
    case.evidence['cases'].append({'mode': 'workflow-revision', 'kind': context['kind'], 'materialsPresent': bool(context['materials']), 'run': detail, 'saved': saved})


def attempt(case, label, action):
    # Independent cases run once even when a previous business case failed. A
    # known failed model transport is a missing prerequisite, not a retry target.
    if case.evidence['cleanupErrors'] or any(record.get('status') != 200 or record.get('error')
            for record in case.model_wire.records if 'ended' in record):
        case.evidence.setdefault('scenarioErrors', []).append({'case': label, 'attempted': False,
            'error': 'Actual model transport or owned cleanup already failed; no further business/model decision is retried.'})
        return False
    try:
        action()
        return True
    except Exception:
        case.evidence.setdefault('scenarioErrors', []).append({'case': label, 'attempted': True,
            'error': traceback.format_exc()})
        return False


def exercise(case):
    deploy(case)
    selected = case.args.cases
    if 'dedicated-chat' in selected:
        attempt(case, 'dedicated-chat', lambda: standalone_baseline(case, revision_fixture(case, 'dedicated-chat', 'task')))
    for kind in ['task', 'map']:
        for optional in [False, True]:
            label = kind + ('-materials' if optional else '-no-materials')
            if label in selected:
                attempt(case, label, lambda: workflow(case, revision_fixture(case, label, kind, optional)))
    from workflow_revise_safety import exercise_safety
    exercise_safety(case)
    targets = {item['taskId'] for item in case.evidence.get('revisionTargets', [])}
    check(not any(record['path'] == '/api/chats' and record['method'] == 'POST'
        and json.loads(record['request']).get('taskId') in targets for record in case.backend_wire.records),
        'Revision matrix automatically executed an existing target Task')
    maps = {item['binding']['map_file'] for item in case.evidence.get('revisionTargets', []) if 'map_file' in item['binding']}
    facts = case.http('/api/workflow/runs')
    check(all(not item.get('readError') and item.get('run') and item['run']['map'] not in maps for item in facts['runs']),
        'Revision executed an existing Map target or Run facts could not establish nonexecution')
    check(not any(node.get('execution', {}).get('taskId') in targets for item in facts['runs']
        for node in item['run']['nodes'].values() if node.get('execution')),
        'Revision dispatched an existing target Task')
    case.evidence['finalReadOnlyRunFacts'] = facts
    check(not case.evidence.get('scenarioErrors'),
        ('Real revision scenarios failed or remained unverified', case.evidence.get('scenarioErrors')))
    if case.evidence['filtered']:
        case.evidence['subsetPassed'] = True
        return
    check(case.evidence['safetyVerified'], 'Revision safety matrix remained unverified')
    check(all(all(pair.values()) for pair in case.evidence['taskPairs'].values()), 'Dedicated revision Task lacks public Chat/workflow pair')
    for promise in case.evidence['coverage']:
        case.evidence['coverage'][promise] = {'verified': True, 'evidence': ['cases', 'taskPairs', 'materialReads', 'fieldApprovals', 'safety']}
    case.evidence['verifiedHosts'] = list(case.hosts)
    case.evidence['passed'] = True
