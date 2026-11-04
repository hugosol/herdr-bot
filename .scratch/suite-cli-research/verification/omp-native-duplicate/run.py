import os,json,pathlib,subprocess,tempfile,shutil,hashlib,time,sys
out=pathlib.Path(__file__).resolve().parent
pkg=pathlib.Path('D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent')
root=pathlib.Path(tempfile.mkdtemp(prefix='omp-native-duplicate-'))
base={k:v for k,v in os.environ.items() if not any(t in k.upper() for t in ['API_KEY','TOKEN','SECRET','PASSWORD','PROXY']) and not k.startswith(('PI_','OMP_','HERDR_','XDG_'))}
results={'root':str(root),'installedPackage':str(pkg),'packageVersion':json.loads((pkg/'package.json').read_text())['version'],'cases':[],'cleanup':{}}
def put(p,data):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(data) if not isinstance(data,str) else data,encoding='utf8')
def events(p):return [json.loads(s) for s in p.read_text().splitlines()] if p.exists() else []
try:
 for label,mode in ([('root-same-id-different-args','different'),('root-project-enabled-false','disabled')] if '--root' in sys.argv else [('same-id-same-config','same'),('same-id-different-args','different'),('different-id-same-config','rename'),('project-enabled-false','disabled'),('suite-distinct-id-different-args','both')]):
  case=root/label;project=case/'project';suite=case/'suite package';agent=case/'agent';project.mkdir(parents=True);agent.mkdir()
  env=dict(base)
  for k in ['HOME','USERPROFILE','APPDATA','LOCALAPPDATA','XDG_CONFIG_HOME','XDG_DATA_HOME','XDG_STATE_HOME','XDG_CACHE_HOME']:
   env[k]=str(case/k.lower());pathlib.Path(env[k]).mkdir()
  env.update(PI_CODING_AGENT_DIR=str(agent),CASE_LABEL=label,RESULT_FILE=str(case/'result.json'))
  fixtureA=case/'fixture-A.bin';fixtureB=case/'fixture-B.bin'
  fixtureA.write_bytes(b'PROJECT-A\x00'+bytes(range(256))+os.urandom(43));fixtureB.write_bytes(b'SUITE-B\x00'+bytes(range(255,-1,-1))+os.urandom(79))
  expected={str(p):{'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size} for p in [fixtureA,fixtureB]}
  logA=case/'A-events.jsonl';logB=case/'B-events.jsonl'
  cfgA={'command':'bun','args':[str(out/'controlled-mcp.mjs'),str(fixtureA),str(logA)]}
  cfgB={'command':'bun','args':[str(out/'controlled-mcp.mjs'),str(fixtureB),str(logB)]}
  projectConfig={'mcpServers':{'duplicate':dict(cfgA,**({'enabled':False} if mode=='disabled' else {}))}}
  suiteConfig={'mcpServers':{('suite-other' if mode in ['rename','both'] else 'duplicate'):(cfgB if mode in ['different','both'] else cfgA)}}
  put(project/('mcp.json' if '--root' in sys.argv else '.omp/mcp.json'),projectConfig);put(suite/'mcp.json',suiteConfig)
  put(suite/'package.json',{'name':'owned-native-suite','version':'1.0.0','omp':{'extensions':['./index.ts']}})
  shutil.copyfile(out/'probe-extension.ts',suite/'index.ts')
  put(agent/'models.json',{'providers':{'owned-local':{'baseUrl':'http://127.0.0.1:1/v1','api':'openai-completions','apiKey':'owned-unused-no-request','models':[{'id':'smoke','name':'smoke','reasoning':False,'input':['text'],'contextWindow':4096,'maxTokens':256,'cost':{'input':0,'output':0,'cacheRead':0,'cacheWrite':0}}]}}})
  put(agent/'settings.json',{'mcp':{'enableProjectConfig':True},'update':{'enabled':False},'extensions':[]})
  cmd=['bun',str(pkg/'src/cli.ts'),'--mode','rpc','--no-session','--no-tools','--no-skills','--model','owned-local/smoke','-e',str(suite)]
  row={'label':label,'cwd':str(project),'command':cmd,'projectConfig':projectConfig,'suiteConfig':suiteConfig,'expected':expected}
  proc=subprocess.Popen(cmd,cwd=project,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   stdout,stderr=proc.communicate(timeout=65)
  except subprocess.TimeoutExpired:
   subprocess.run(['taskkill','/PID',str(proc.pid),'/T','/F'],capture_output=True);stdout,stderr=proc.communicate();row['timeout']=True
  row.update(returncode=proc.returncode,stdout=stdout,stderr=stderr,result=json.loads((case/'result.json').read_text()) if (case/'result.json').exists() else None,A=events(logA),B=events(logB))
  row['hashChecks']=[{'fixture':ev['actual']['fixture'],'ok':expected[ev['actual']['fixture']]=={k:ev['actual'][k] for k in ['sha256','bytes']}} for ev in row['A']+row['B'] if ev['event'] in ['call','read-resource']]
  results['cases'].append(row)
  print(json.dumps({'label':label,'returncode':row['returncode'],'result':row['result'],'eventsA':[x['event'] for x in row['A']],'eventsB':[x['event'] for x in row['B']],'stderr':stderr},ensure_ascii=False))
finally:
 # Only owned MCP PIDs may need cleanup if a failed CLI did not dispose them.
 pids={ev['pid'] for r in results['cases'] for ev in r['A']+r['B'] if ev['event']=='start'}
 results['cleanup']['ownedServerPids']=sorted(pids)
 for pid in pids:
  status=subprocess.run(['tasklist','/FI',f'PID eq {pid}','/FO','CSV','/NH'],capture_output=True,text=True,errors='replace')
  if f'"{pid}"' in status.stdout:
   kill=subprocess.run(['taskkill','/PID',str(pid),'/T','/F'],capture_output=True,text=True,errors='replace');results['cleanup'][str(pid)]={'killCode':kill.returncode,'stdout':kill.stdout,'stderr':kill.stderr}
  else:results['cleanup'][str(pid)]='already-exited'
 shutil.rmtree(root)
 results['cleanup']['tempRemoved']=not root.exists()
 (out/('evidence-root.json' if '--root' in sys.argv else 'evidence.json')).write_text(json.dumps(results,ensure_ascii=False,indent=2),encoding='utf8')
