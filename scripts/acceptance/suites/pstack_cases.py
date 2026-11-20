"""S2 + S3 scenario oracles. Decisions below return model tool calls, never tool results.

P2/P3/P14 reject wrong-source skills, removed fixed hooks or native instructions,
and implicit publication at either launch boundary. P17 compares the first
post-Resume request with its retained native contributions. P20 uses distinct
native cwds and known source pointers (not inherited chat material). P24/P28/P31
would fail for early binding requirements, stale/missing persona resolution, or
any change to an unrelated Suite's native model default.
"""
from contextlib import contextmanager
import json
import os
from pathlib import Path
import re
import shutil
import time

from acceptance.host.model import message_text
from acceptance.host.processes import require, until
from acceptance.host.provenance import digest
from acceptance.host.smoke_support import dump
from acceptance.host.wire import WireProxy
from .pstack import ACK, ALPHA, BETA, PROJECT, PROVIDER, TIMEOUT, write

SETUP_RULE = 'Do not save or generate anything without explicit confirmation.'
CORE_RULE = "First message → restate the user's goal and the smallest change that meets it, then proceed."
REMINDER = 'Poteto mode remains active.'
WORKER_RULE = 'You work for the operator, not the agent that wrote the kickoff.'
ROLE_RULE = 'Read the shipped poteto-mode SKILL.md in full before any work'
WRONG_SOURCE = 'WRONG-SAME-NAME-SOURCE-321'
WRONG_CWD = 'NOT-THE-SKILL-SOURCE-559'
OPERATOR_MARKER = re.compile(r'\bPSTACK-[A-Za-z0-9-]+\b')


def system(request):
    return '\n'.join(message_text(m) for m in request['messages'] if m['role'] in ('system', 'developer'))


def normalize(value):
    return str(value).replace('\\', '/').replace('//?/', '').casefold()


def turn_boundary(request):
    messages = request['messages']
    users = [(i, message_text(m)) for i, m in enumerate(messages) if m['role'] == 'user']
    # OMP can inject a user-role mode contribution after the submitted input.
    # Its separate side channels even carry the tool catalog; don't route them
    # as the operator's turn, despite retained test markers in their history.
    if not users or users[-1][1].startswith(('<title-request>', '<composer-prediction>')):
        return '', []
    index, text = next(((i, text) for i, text in reversed(users) if OPERATOR_MARKER.search(text)), (-1, ''))
    return text, [message_text(m) for m in messages[index + 1:] if m['role'] == 'tool']


def turn_requests(run, marker):
    return [record['request'] for record in run.transport.records if record.get('request', {}).get('tools')
            and marker in OPERATOR_MARKER.findall(turn_boundary(record['request'])[0])]


def require_steps(run, marker, count):
    requests = turn_requests(run, marker)
    require(len(requests) == count, ('Unexpected native model/tool sequence', marker, len(requests)))
    return requests


def call(tool, **args):
    return {'content': '', 'tool_calls': [{'name': tool, 'arguments': args}]}


def read(path):
    return call('read', path=str(path), i='Reading explicit owned source')


def task(name, assignment, **options):
    return call('task', context='Mechanically observe this explicit owned request.', tasks=[
        dict(name=name, task=assignment, solutionSpace='Use only the specified source and tool.', **options)])


def decision(run, request):
    # Some installed OMP versions also ask a separate tool-free title model.
    # It must not consume a planned native tool step.
    if not request.get('tools'):
        return {'content': '<title>Owned native probe</title>', 'tool_calls': []}
    user, results = turn_boundary(request)
    markers = OPERATOR_MARKER.findall(user)
    for marker, action in run.turns.items():
        if marker in markers:
            response = action(user, results)
            if response is not None:
                allowed = {tool['function']['name'] for tool in request['tools']}
                for tool in response['tool_calls']:
                    require(tool['name'] in allowed, ('Expected native tool not available', marker, tool['name']))
                return response
            break
    return {'content': ACK, 'tool_calls': []}


def one_read(run, marker, path):
    run.turns[marker] = lambda unused, results: None if results else read(path)


def result_of(request):
    results = turn_boundary(request)[1]
    require(results, 'Missing actual native tool result')
    return results[-1]


def source_result(result, path, phrase, skill=False):
    prefix = f'[Skill file: {path}]' if skill else f'[{path}#'
    require(normalize(prefix) in normalize(result) and phrase in result, ('Wrong native resource content or source', str(path), result))
    require(WRONG_SOURCE not in result and WRONG_CWD not in result, ('Read selected decoy', result))


def fixed_and_instructions(run, session, request):
    native, chat = session['native'], session['chat']
    require(PROJECT in system(request) and Path(native['cwd']).samefile(run.zone), 'Project instructions or actual cwd changed')
    require({'budget', 'connect-bot'} <= set(native['commands']), ('Actual fixed registrations missing', native))
    herdr = run.herdr('agent', 'get', chat['paneId'])
    # The official extension emits native session identity into the actual pane;
    # the independent observer does not report identity to Herdr or the backend.
    require(native['id'] in json.dumps(herdr), ('Official Herdr integration did not report actual Session', herdr, native))
    run.send(session, '/budget help')
    terminal = until('real model-cost fixed command output', lambda: text if '归档并重置统计' in (text := run.terminal(session)) else None, TIMEOUT)
    session['officialHerdr'] = herdr
    session['fixedBudgetHelp'] = terminal


@contextmanager
def read_only(run, roots):
    who = run.command(['whoami.exe', '/user', '/fo', 'csv', '/nh'])
    sid = re.search(r'S-1-(?:\d+-)*\d+', who)
    require(sid, ('Cannot resolve current Windows SID', who))
    identity = '*' + sid[0]
    protected = []
    try:
        for root in roots:
            # Deny additions, overwrites and deletion; preserve reads and ACL recovery.
            run.command(['icacls.exe', root, '/deny', identity + ':(OI)(CI)(WD,AD,DE,DC)', '/T', '/Q'])
            protected.append(root)
        sample = run.skill
        require(bool(sample.read_bytes()), 'Read-only control is not readable')
        for path, mode in [(sample, 'r+b'), (run.generated / 'must-not-create', 'xb')]:
            try:
                with path.open(mode):
                    pass
            except PermissionError:
                continue
            raise AssertionError(('Read-only control did not deny an actual write handle', str(path)))
        yield
    finally:
        for root in reversed(protected):
            run.command(['icacls.exe', root, '/remove:d', identity, '/T', '/Q'])


def assembly(run):
    # First show that the undeclared set and its same-named file are real and
    # discoverable in an actual native session when selected by its own Suite.
    other = run.task('Alternate source positive control', 'owned-other')
    marker = 'PSTACK-AMBIENT-POSITIVE-094'
    one_read(run, marker, 'skill://setup-pstack')
    baseline = run.launch(other, marker)
    chosen, result = require_steps(run, marker, 2)
    require('ambient-pstack-decoy' in system(chosen) and WRONG_SOURCE in result_of(result)
            and normalize(run.agent_dir / 'skills/setup-pstack/SKILL.md') in normalize(result_of(result)),
            'The alternate collection was not an effective collision control')
    run.close(baseline)

    deliverable_roots = (run.generated, run.suite / 'upstream')
    snapshots = lambda: {str(root): run.tree(root) for root in deliverable_roots}
    skill_files = sorted((run.generated / 'skills').glob('*/SKILL.md'))
    expected_all = sorted(file.parent.name for file in skill_files)
    expected = sorted(file.parent.name for file in skill_files
                      if 'disable-model-invocation: true' not in file.read_text(encoding='utf-8').split('---', 2)[1])
    task_id = run.task('Read-only delivered pstack Suite')
    upstream = json.loads((run.suite / 'manifest.json').read_text(encoding='utf-8'))['upstream']['repository']
    network = WireProxy(run.transport.base, run.public_log / 'upstream-net')
    try:
        # Git attempts against the pstack source are caught locally, even if an
        # invocation sought the network rather than modifying the snapshot.
        for url in [upstream, 'git@github.com:cursor/plugins', 'ssh://git@github.com/cursor/plugins']:
            run.command([run.args.git, 'config', '--global', '--add', f'url.{network.base}/upstream.insteadOf', url])
        with read_only(run, deliverable_roots):
            run.evidence['readOnlyBefore'] = before = snapshots()
            calls_offset = len(run.calls())
            trace = run.root / 'git-trace.jsonl'
            trace_offset = len(trace.read_bytes()) if trace.is_file() else 0
            bun_trace = run.public_log / 'bun-processes.jsonl'
            bun_offset = len(bun_trace.read_bytes())
            native_pids = set()
            for marker in ('PSTACK-READONLY-LAUNCH-587', 'PSTACK-READONLY-RESUME-679'):
                one_read(run, marker, 'skill://setup-pstack')
                if marker.endswith('587'):
                    session = run.launch(task_id, marker)
                else:
                    session = run.start_native_resume(session)
                    run.turn(session, marker)
                first, continuation = require_steps(run, marker, 2)
                tables = re.findall(r'<skills>\s*(.*?)</skills>', system(first), re.S)
                catalog = re.findall(r'^- ([^:]+):', '\n'.join(tables), re.M)
                commands = sorted(name.removeprefix('skill:') for name in session['native']['commands'] if name.startswith('skill:'))
                session['nativeSkillCatalog'] = {'modelVisible': catalog, 'explicitCommands': commands}
                require(commands == expected_all and expected and sorted(catalog) == expected and 'setup-pstack' in catalog,
                        ('Wrong final native catalog or manually callable Skill set', catalog, commands, expected, expected_all))
                source_result(result_of(continuation), run.skill, SETUP_RULE, skill=True)
                fixed_and_instructions(run, session, first)
                native_pids.add(session['native']['pid'])
                run.close(session)
            processes = [json.loads(line) for line in bun_trace.read_bytes()[bun_offset:].decode('utf-8').splitlines()]
            run.evidence['launchBunProcessEvents'] = processes
            launches = {p['pid'] for p in processes if '__omp_worker_daemon' not in p['argv']
                        and any(normalize(arg) == normalize(run.args.omp_entry) for arg in p['argv'])}
            require(launches == native_pids, ('The bound installed OMP Chats did not both run under observation', native_pids, processes))
            maintenance = ('pstack:build', 'pstack:upstream', 'pstack-suite/maintenance/')
            require(not any(term in normalize([p['argv'], p['execArgv']]) for p in processes for term in maintenance),
                    ('Startup invoked suite maintenance', processes))
            run.evidence['readOnlyAfter'] = after = snapshots()
            require(before == after, 'Launch or Resume changed the delivered output or upstream snapshot')
            calls = run.calls()[calls_offset:]
            require(sum(c['argv'][:2] == ['agent', 'start'] for c in calls) == 2, ('Wrong actual launch/resume attempts', calls))
            require(not any(any(part in ' '.join(c['argv']).lower() for part in ('pstack:build', 'pstack:upstream', 'maintenance/cli.ts'))
                            for c in calls), ('Herdr started maintenance', calls))
            # Trace2 is inherited by the installed agent, Herdr and production
            # backend. Short-lived Git invocations are visible independently.
            require(trace.is_file(), 'No Git Trace2 positive control from the owned fixture commands')
            observed = [json.loads(line) for line in trace.read_bytes()[trace_offset:].decode('utf-8').splitlines()]
            run.evidence['launchGitProcessEvents'] = observed
            forbidden = {'fetch', 'pull', 'clone', 'push', 'ls-remote', 'submodule'}
            require(not any(forbidden.intersection(event.get('argv', [])) for event in observed if event['event'] == 'start'),
                    'A real startup Git subprocess attempted maintenance/network')
            require(not network.records, ('Unexpected upstream requests', network.records))
    finally:
        network.close()
        if (run.root / 'git-trace.jsonl').is_file():
            shutil.copy2(run.root / 'git-trace.jsonl', run.public_log / 'git-trace.jsonl')


def saved_mode(session):
    file = Path(session['file'])
    rows = [json.loads(line) for line in file.read_text(encoding='utf-8').splitlines()]
    values = [entry['data']['enabled'] for entry in rows if entry.get('customType') == 'poteto-mode:state']
    guidance = [entry for entry in rows if entry.get('customType') in ('poteto-mode:engineering', 'poteto-mode:reminder')]
    return values, guidance


def disable(run, session):
    run.send(session, '/poteto-mode')
    terminal = until('real disabled status', lambda: text if 'Poteto mode disabled' in (text := run.terminal(session)) else None, TIMEOUT)
    session['modeCommandTerminal'] = terminal
    until('native state journal committed disabled value', lambda: saved_mode(session)[0][-1:] == [False], TIMEOUT)


def mode_resume_on(run):
    mode_resume(run, enabled=True)


def mode_resume_off(run):
    mode_resume(run, enabled=False)


def mode_resume(run, *, enabled):
    task_id = run.task(f'Public mode {enabled}')
    original = run.launch(task_id, f'PSTACK-STORE-{enabled}')
    if not enabled:
        disable(run, original)
    run.close(original)
    before, original_guidance = saved_mode(original)
    require(before == ([True] if enabled else [True, False]) and len(original_guidance) == 1, ('Wrong pre-Resume native state', before))
    marker = f'PSTACK-FIRST-REQUEST-AFTER-{enabled}'
    session = run.start_native_resume(original)
    run.turn(session, marker)
    first, = require_steps(run, marker, 1)
    run.close(session)
    after, guidance = saved_mode(session)
    retained = '\n'.join(message_text(message) for message in first['messages'])
    require(after == before and guidance[:1] == original_guidance and len(guidance) == 1 + enabled,
            ('Public Resume ignored saved value or rewrote guidance', enabled, before, after, guidance))
    require(retained.count(CORE_RULE) == 1 and retained.count(REMINDER) == int(enabled),
            ('First post-Resume request has wrong new contribution', enabled, first))
    session['stateAfterFirstTurn'] = {'values': after, 'nativeContributions': guidance}


def child_history(session, name, marker):
    file = Path(session['file']).with_suffix('') / (name + '.jsonl')
    def read_committed():
        if not file.is_file():
            return None
        raw = file.read_text(encoding='utf-8')
        rows = [json.loads(line) for line in raw[:raw.rfind('\n') + 1].splitlines()]
        header = next((row for row in rows if row['type'] == 'session'), None)
        return (header, rows) if header and marker in raw and ACK in raw else None
    header, rows = until('owned native child history committed', read_committed, TIMEOUT)
    return file, header, rows


def source_base(run):
    skill = run.generated / 'skills/poteto-mode/SKILL.md'
    worker = skill.with_name('worker.md')
    write(run.zone / 'worker.md', WRONG_CWD + '\n')
    source = 'PSTACK-MAIN-SOURCE-390'
    spawn = 'PSTACK-EXPLICIT-READ-CHILD-253'
    child = 'PSTACK-CHILD-KNOWN-SOURCE-625'
    cwds = {}

    def route(actor, user, results):
        cwd = re.search(r"current working directory: '([^']+)'", user)
        if cwd:
            previous = cwds.setdefault(actor, Path(cwd[1]))
            require(previous == Path(cwd[1]), ('Native cwd changed during read', actor, cwd[1]))
        if not results:
            # The child gets its explicit source in the assignment, not a
            # parent's loaded skill, user history or inherited resource table.
            return read('skill://poteto-mode' if actor == source else skill)
        if len(results) == 1:
            require(actor in cwds, ('Missing cwd from native user context', actor, user))
            return read(Path(os.path.relpath(worker, cwds[actor])).as_posix())
        return None

    run.turns[source] = lambda user, results: route(source, user, results)
    run.turns[child] = lambda user, results: route(child, user, results)
    run.turns[spawn] = lambda user, results: None if results else task('OwnedSourceReader',
        f'{child}: Read {skill}, then read worker.md relative to that exact skill source. Read only.', isolated=True)
    session = run.launch(run.task('Explicit source provenance'), 'PSTACK-OPEN-RESOURCE-CHAIN-351')
    disable(run, session)
    cwds[source] = Path(session['native']['cwd'])
    run.turn(session, f'{source}: Read skill://poteto-mode then worker.md relative to {skill.as_posix()}.')
    run.turn(session, spawn)
    require(cwds[source].samefile(run.zone) and cwds[source] != cwds[child], ('Child did not have a distinct native cwd', cwds))
    for actor in (source, child):
        first, skill_result, worker_result = require_steps(run, actor, 3)
        result = result_of(skill_result)
        if actor == source:
            source_result(result, skill, CORE_RULE, skill=True)
        else:
            source_result(result, skill, CORE_RULE)
            require(WORKER_RULE not in json.dumps(first, ensure_ascii=False) and source not in json.dumps(first, ensure_ascii=False),
                    'Child started with the parent turn or already-read worker material')
        source_result(result_of(worker_result), worker, WORKER_RULE)
    child_file, child_header, child_rows = child_history(session, 'OwnedSourceReader', child)
    require(normalize(child_header['cwd']) == normalize(cwds[child])
            and child in json.dumps(child_rows) and WORKER_RULE in json.dumps(child_rows, ensure_ascii=False),
            ('Child request did not correspond to an actual independent child history', child_header))
    session['sourceBases'] = {'parentCwd': str(cwds[source]), 'childCwd': str(cwds[child]), 'skill': str(skill), 'asset': str(worker)}
    shutil.copy2(child_file, run.public_log / 'child-source-history.jsonl')
    run.close(session)


def role_lifecycle(run):
    model_files = [run.agent_dir / 'config.yml', run.agent_dir / 'models.yml']
    immutable = {path.name: digest(path) for path in model_files}
    run.evidence['unchangedNativeConfig'] = immutable
    for path in model_files:
        shutil.copy2(path, run.public_log / ('initial-' + path.name))
    settings = run.suite / 'models.local.json'
    index = run.generated / 'roles.local.json'
    require(not settings.exists() and not index.exists(), 'Initial fixture must have no model bindings')
    other = run.task('Other Suite uses native default', 'owned-other')
    source = run.task('Published pstack role consumer')

    def control(stage):
        marker = f'PSTACK-UNRELATED-CONTROL-{stage}'
        session = run.launch(other, marker)
        first, = require_steps(run, marker, 1)
        require(first['model'] == ALPHA and session['native']['model'] == {'provider': PROVIDER, 'id': ALPHA},
                ('An unrelated Suite did not actually retain the native default model', first['model'], session['native']))
        require({path.name: digest(path) for path in model_files} == immutable, 'Global native model settings changed')
        run.close(session)

    control('BEFORE')
    marker = 'PSTACK-FIRST-SETUP-543'
    setup = run.launch(source, f'/skill:setup-pstack {marker}: explicitly open setup without saving yet.')
    setup_request, = require_steps(run, marker, 1)
    invoked, = [row['details'] for row in setup['history']['branch'] if row.get('customType') == 'skill-prompt']
    require(SETUP_RULE in turn_boundary(setup_request)[0] and invoked['name'] == 'setup-pstack'
            and Path(invoked['path']).samefile(run.skill) and marker in invoked['prompt'],
            'Actual native setup skill did not expand from the shipped source')
    require(not settings.exists() and not index.exists(), 'Opening setup alone generated or saved roles')
    run.close(setup)

    # Authorized fixture operator uses the installed model catalog and gives
    # the same explicit confirmation as the documented out-of-session command.
    catalog = run.root / 'native-model-catalog.json'
    write(catalog, run.command([run.args.bun, run.args.omp_entry, 'models', '--json', '--no-extensions']))
    observed = {entry['selector'] for entry in json.loads(catalog.read_text(encoding='utf-8'))['models']}
    require({PROVIDER + '/' + ALPHA, PROVIDER + '/' + BETA} <= observed, 'Configured own models absent from installed catalog')
    shutil.copy2(catalog, run.public_log / catalog.name)
    for label, model, level in [('A', BETA, 'high'), ('B', ALPHA, 'medium')]:
        require(all(s.get('exited') for s in run.sessions), 'Stop every same-package session before confirmation/publication')
        confirm = run.public_log / f'confirmed-{label}.json'
        roles = {'poteto-agent': {'model': f'{PROVIDER}/{model}', 'thinking': level},
                 'comment-sicko': {'model': 'inherit-parent', 'thinking': 'inherit'}}
        dump(confirm, roles)
        response = run.command([run.args.bun, 'run', '--cwd', run.deployment, 'pstack:setup', '--', '--confirm', confirm, '--catalog', catalog])
        require('Start a new pstack session' in response, 'Confirmation omitted restart instruction')
        require(json.loads(settings.read_text(encoding='utf-8')) == {'confirmed': True, 'roles': roles}
                and json.loads(index.read_text(encoding='utf-8')) == roles, 'Confirmed role settings were not published verbatim')
        shutil.copy2(settings, run.public_log / f'published-{label}.json')
        shutil.copy2(run.generated / 'agents/poteto-agent.md', run.public_log / f'persona-{label}.md')
        marker = f'PSTACK-PUBLIC-NATIVE-TASK-{label}'
        child = f'PSTACK-ROLE-ASSIGNMENT-{label}'
        output = run.zone / f'owned-role-{label}.txt'
        text = f'role-{label}-literal-output-979'
        run.turns[marker] = lambda user, results: None if results else task('OwnedRole',
            f'{child}: Write only the exact supplied literal to {output}.', agent='poteto-agent')
        run.turns[child] = lambda user, results: None if results else call('write', path=str(output), content=text, i='Writing owned role result')
        session = run.launch(source, marker)
        assigned, continuation = require_steps(run, child, 2)
        require(assigned['model'] == model and assigned.get('reasoning_effort') == level and ROLE_RULE in system(assigned),
                ('Native task did not actually consume the confirmed role/model/thinking', model, level, assigned))
        require(output.read_text(encoding='utf-8') == text, ('Generated role did not execute native write', result_of(continuation)))
        run.close(session)
        child_file, child_header, _ = child_history(session, 'OwnedRole', child)
        require(child_header['id'] != session['native']['id'], 'Role result used the parent journal, not a new native child')
        shutil.copy2(child_file, run.public_log / f'role-{label}-native-child.jsonl')
        shutil.copy2(output, run.public_log / output.name)
        control(label)
    for path in model_files:
        shutil.copy2(path, run.public_log / ('final-' + path.name))
