"""Read-only native process ownership and installed session-history observation."""
import json

from acceptance.host import processes


def remember_process(self, pid):
    with self.handles_lock:
        old = self.handles.get(pid)
        if old and not old.alive():
            self.evidence.setdefault('retiredProcessExits', []).append({'pid': pid, 'exited': True})
            old.close()
            del self.handles[pid]
        if pid not in self.handles:
            self.handles[pid] = processes.LiveProcess(pid)

def capture(self):
    # Only OS descendants of this invocation's named Herdr server; no user cmdlines.
    pid = self.processes[0].child.pid
    script = (f'$owned=[System.Collections.Generic.HashSet[int]]::new();[void]$owned.Add({pid});'
        '$tree=@(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,CreationDate);'
        '$by=@{};foreach($p in $tree){$by[[int]$p.ProcessId]=$p};'
        'do {$added=$false;foreach($p in $tree){if($owned.Contains([int]$p.ParentProcessId) '
        '-and $by.ContainsKey([int]$p.ParentProcessId) '
        '-and $p.CreationDate -ge ($by[[int]$p.ParentProcessId]).CreationDate '
        '-and $owned.Add([int]$p.ProcessId)){$added=$true}}}while($added);'
        # Only runtimes this deterministic fixture starts are ours. OS input
        # method/desktop services are not owned even when a console triggers them.
        '$names=@("herdr.exe","cmd.exe","powershell.exe","node.exe","bun.exe","OpenConsole.exe","conhost.exe");'
        '$r=@(Get-CimInstance Win32_Process | Where-Object {$owned.Contains([int]$_.ProcessId) '
        '-and $_.Name -in $names -and $_.CreationDate -eq ($by[[int]$_.ProcessId]).CreationDate} '
        '| Select-Object ProcessId,ParentProcessId,CreationDate,CommandLine);ConvertTo-Json -InputObject $r -Compress')
    snapshot = json.loads(self.command(['powershell.exe', '-NoProfile', '-Command', script]))
    self.evidence.setdefault('ownedSnapshots', []).append(snapshot)
    for item in snapshot:
        self.remember_process(item['ProcessId'])

def history(self, file):
    package = self.args.pi_package if self.agent == 'pi' else self.args.omp_package
    module = package / ('dist/core/session-manager.js' if self.agent == 'pi' else 'src/session/session-manager.ts')
    code = f'import {{SessionManager}} from {json.dumps(module.as_uri())};const s=await SessionManager.open({json.dumps(str(file))});console.log(JSON.stringify({{id:s.getSessionId(),name:s.getSessionName(),branch:s.getBranch()}}));'
    return json.loads(self.command(['bun', '-e', code]))

