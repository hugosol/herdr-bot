import os, json, pathlib, subprocess, tempfile, shutil
OUT=pathlib.Path(__file__).resolve().parent
root=pathlib.Path(tempfile.mkdtemp(prefix='omp-suite-smoke-'))
env={k:v for k,v in os.environ.items() if not any(t in k.upper() for t in ['API_KEY','TOKEN','SECRET','PASSWORD']) and not k.startswith(('PI_','OMP_','HERDR_','XDG_'))}
for k in ['HOME','USERPROFILE','APPDATA','LOCALAPPDATA','XDG_CONFIG_HOME','XDG_DATA_HOME','XDG_STATE_HOME','XDG_CACHE_HOME']:
 env[k]=str(root/k.lower());pathlib.Path(env[k]).mkdir(parents=True)
env['PI_CODING_AGENT_DIR']=str(root/'agent');pathlib.Path(env['PI_CODING_AGENT_DIR']).mkdir()
cwd=root/'project';cwd.mkdir()
cli='D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js'
results=[]
def run(name,args):
 try:
  p=subprocess.run(args,cwd=cwd,env=env,text=True,capture_output=True,timeout=35)
  r={'name':name,'args':args,'returncode':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
 except subprocess.TimeoutExpired as e:r={'name':name,'args':args,'timeout':True,'stdout':str(e.stdout),'stderr':str(e.stderr)}
 results.append(r);print(json.dumps(r,ensure_ascii=False))
try:
 run('installed-version',['bun',cli,'--version'])
 run('installed-help',['bun',cli,'--help'])
 env['SMOKE_ROOT']=str(root)
 run('resources',['bun',str(OUT/'resources.ts')])
 for name,base in [('checkout','E:/Developer/myhub/oh-my-pi/packages/coding-agent/src'),('installed','D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent/src')]:
  script=root/(name+'.ts');script.write_text('import {loadAllMCPConfigs} from '+json.dumps(base+'/mcp/config.ts')+'; console.log(JSON.stringify(await loadAllMCPConfigs(process.cwd(), {enableProjectConfig:false})));',encoding='utf8')
  run(name+'-import',['bun',str(script)])
finally:
 (OUT/'probe-output.json').write_text(json.dumps(results,ensure_ascii=False,indent=2),encoding='utf8')
 shutil.rmtree(root)
