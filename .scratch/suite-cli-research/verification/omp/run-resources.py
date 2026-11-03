import os,json,pathlib,subprocess,tempfile,shutil,sys,hashlib
out=pathlib.Path(__file__).resolve().parent
root=pathlib.Path(tempfile.mkdtemp(prefix='omp-suite-public-'))
env={k:v for k,v in os.environ.items() if not any(t in k.upper() for t in ['API_KEY','TOKEN','SECRET','PASSWORD']) and not k.startswith(('PI_','OMP_','HERDR_','XDG_'))}
for k in ['HOME','USERPROFILE','APPDATA','LOCALAPPDATA','XDG_CONFIG_HOME','XDG_DATA_HOME','XDG_STATE_HOME','XDG_CACHE_HOME']:
 env[k]=str(root/k.lower());pathlib.Path(env[k]).mkdir(parents=True)
env['PI_CODING_AGENT_DIR']=str(root/'agent');(root/'agent').mkdir();(root/'project').mkdir();env['SMOKE_ROOT']=str(root)
fixture=root/'owned binary fixture.dat'
fixture.write_bytes(b'OMP suite file-read verification\r\n'+bytes(range(256))+os.urandom(73))
env['SMOKE_FIXTURE_FILE']=str(fixture)
env['SMOKE_EXPECTED_SHA256']=hashlib.sha256(fixture.read_bytes()).hexdigest()
env['SMOKE_EXPECTED_BYTES']=str(fixture.stat().st_size)
results=[]
try:
 for script in (sys.argv[1:] or ['resources.ts','bridge.ts']):
  p=subprocess.run(['bun',str(out/script)],cwd=root/'project',env=env,capture_output=True,text=True,timeout=35)
  r={'script':script,'returncode':p.returncode,'stdout':p.stdout,'stderr':p.stderr};results.append(r);print(json.dumps(r,ensure_ascii=False))
finally:
 (out/(('-'.join(sys.argv[1:])+'-file-output.json') if sys.argv[1:] else 'resources-output.json')).write_text(json.dumps(results,ensure_ascii=False,indent=2),encoding='utf8')
 shutil.rmtree(root)
