import os,sys,json,tempfile,pathlib,subprocess,socket,time,threading,urllib.request,shutil
binary=r'E:\Developer\tools\codebase-memory-mcp\codebase-memory-mcp.exe'
root=pathlib.Path(tempfile.mkdtemp(prefix='suite-mcp-'))
report={'binary':binary,'root':str(root),'scenarios':[],'cleanup':[]}
children=[]; envs=[]
def env_for(name):
 p=root/name;p.mkdir();e={k:v for k,v in os.environ.items() if not any(x in k.upper() for x in ['API_KEY','TOKEN','SECRET','CBM_'])}
 for k in ['HOME','USERPROFILE','APPDATA','LOCALAPPDATA','XDG_CONFIG_HOME','XDG_CACHE_HOME','CBM_CACHE_DIR','CBM_RUNTIME_DIR']:
  q=p/k;q.mkdir();e[k]=str(q)
 envs.append(e);return e
def run(e,args):
 r=subprocess.run([binary]+args,env=e,cwd=root,capture_output=True,text=True,timeout=25);return {'command':[binary]+args,'code':r.returncode,'stdout':r.stdout,'stderr':r.stderr}
def spawn(e,port):
 p=subprocess.Popen([binary,'--ui=true','--port='+str(port)],cwd=root,env=e,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,bufsize=1); out=[];err=[]
 def consume(stream,target):
  for line in stream: target.append(line.rstrip())
 for stream,target in [(p.stdout,out),(p.stderr,err)]:threading.Thread(target=consume,args=(stream,target),daemon=True).start()
 children.append((p,out,err));p.stdin.write(json.dumps({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'isolated-resource-smoke','version':'1'}}})+'\n');p.stdin.flush();return p,out,err
def initialized(child):
 return any('"id":1' in x.replace(' ','') and '"result"' in x for x in child[1])
try:
 s=socket.socket();s.bind(('127.0.0.1',0));port=s.getsockname()[1];s.close();report['port']=port
 a=env_for('profile-a');first=spawn(a,port)
 deadline=time.monotonic()+20
 while time.monotonic()<deadline and not initialized(first) and first[0].poll() is None:time.sleep(.1)
 second=spawn(a,port)
 deadline=time.monotonic()+15
 while time.monotonic()<deadline and not initialized(second) and second[0].poll() is None:time.sleep(.1)
 report['scenarios'].append({'name':'same profile two simultaneous MCP clients','initialized':[initialized(first),initialized(second)],'status':run(a,['daemon','status'])})
 try:
  with urllib.request.urlopen('http://127.0.0.1:'+str(port)+'/',timeout=3) as response:report['http']={'status':response.status,'bytes':len(response.read())}
 except Exception as e:report['httpError']=str(e)
 b=env_for('profile-b');third=spawn(b,port);time.sleep(3)
 report['scenarios'].append({'name':'separate daemon namespace same dynamic port','initialized':initialized(third),'status':run(b,['daemon','status'])})
except Exception as e:report['error']=repr(e)
finally:
 for p,out,err in children:
  try:p.stdin.close();p.wait(timeout=10)
  except Exception as e:
   report['cleanup'].append(str(e));p.kill();p.wait(timeout=10)
  report.setdefault('clients',[]).append({'pid':p.pid,'exit':p.returncode,'stdout':out,'stderr':err})
 for e in envs:
  try:report['cleanup'].append(run(e,['daemon','stop']));report['cleanup'].append(run(e,['daemon','status']))
  except Exception as ex:report['cleanup'].append(repr(ex))
 shutil.rmtree(root);report['tempRemoved']=not root.exists()
 target=pathlib.Path(__file__).with_name('mcp-evidence.json');target.write_text(json.dumps(report,indent=2),encoding='utf8');print(json.dumps(report,indent=2))
