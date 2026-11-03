import os,json,pathlib,subprocess,tempfile,shutil,hashlib,sys
out=pathlib.Path(__file__).resolve().parent
results=[]
for location in ['dot','root']:
 for kind,deny in ([(k,True) for k in ['url','project-off']] if '--extra' in sys.argv else [(k,False) for k in ['same','args','command','url','alias','project-disabled','suite-disabled','project-off']]+[(k,True) for k in ['same','args','command','project-disabled','suite-disabled']]):
  spec=dict(location=location,kind=kind,deny=deny)
  root=pathlib.Path(tempfile.mkdtemp(prefix='omp-native-merge-'))
  env={k:v for k,v in os.environ.items() if not any(t in k.upper() for t in ['API_KEY','TOKEN','SECRET','PASSWORD']) and not k.startswith(('PI_','OMP_','HERDR_','XDG_'))}
  for k in ['HOME','USERPROFILE','APPDATA','LOCALAPPDATA','XDG_CONFIG_HOME','XDG_DATA_HOME','XDG_STATE_HOME','XDG_CACHE_HOME']:
   env[k]=str(root/k.lower());pathlib.Path(env[k]).mkdir()
  (root/'project').mkdir();(root/'agent').mkdir();env['PI_CODING_AGENT_DIR']=str(root/'agent');env['SMOKE_ROOT']=str(root);env['SMOKE_CASE']=json.dumps(spec)
  fixture=root/'fixture.bin';fixture.write_bytes(b'owned native MCP merge fixture\r\n'+bytes(range(256))+os.urandom(57))
  command=['bun',str(out/'probe.ts')]
  r={'spec':spec,'command':command,'cwd':str(root/'project'),'root':str(root),'expected':{'sha256':hashlib.sha256(fixture.read_bytes()).hexdigest(),'bytes':fixture.stat().st_size}}
  try:
   p=subprocess.run(command,cwd=root/'project',env=env,capture_output=True,text=True,timeout=40)
   r.update(returncode=p.returncode,stdout=p.stdout,stderr=p.stderr)
   if p.returncode==0:r['parsed']=json.loads(p.stdout)
  except Exception as e:r['exception']=repr(e)
  finally:
   shutil.rmtree(root);r['cleanup']='Temp removed after native manager disconnectAll / child command exit'
   results.append(r);(out/('evidence-extra.json' if '--extra' in sys.argv else 'evidence.json')).write_text(json.dumps(results,ensure_ascii=False,indent=2),encoding='utf8')
  print(json.dumps({'spec':spec,'returncode':r.get('returncode'),'configs':list(r.get('parsed',{}).get('loaded',{}).get('configs',{})),'stderr':r.get('stderr','') if r.get('returncode') else ''},ensure_ascii=False))
  if r.get('returncode')!=0:raise RuntimeError('Matrix probe failed; raw evidence preserved')
