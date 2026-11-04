import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const output=fileURLToPath(new URL('.',import.meta.url));
const root=await mkdtemp(join(tmpdir(),'pi-suite-cli-'));
const source=process.argv.includes('--source');
const prefix=source?'source-':'';
const executable=source?'E:/Developer/myhub/pi/packages/coding-agent/src/cli.ts':'D:/nvm4w/nodejs/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js';
const nodeArgs=source?['--import',pathToFileURL(join(output,'dependency-resolver.mjs')).href,'--import','file:///E:/Developer/myhub/pi/packages/coding-agent/src/experimental/source-resolver.ts']:[];
const children=new Set();
const results=[];
const fixture=join(root,'fixture.txt');
const payload='suite isolation '+Date.now()+'\n';
await writeFile(fixture,payload);
const expected=createHash('sha256').update(payload).digest('hex');
try {
 for(const scenario of ['no-suite','on','only','empty']) {
  const dir=join(root,scenario), agent=join(dir,'agent'), cwd=join(dir,'project');
  await mkdir(join(cwd,'.pi'),{recursive:true}); await mkdir(agent,{recursive:true});
  const record=join(dir,'mcp.jsonl'), observations=join(dir,'observations.jsonl');
  await writeFile(record,''); await writeFile(observations,'');
  const config=name=>({command:process.execPath,args:[join(output,'mcp-fixture.mjs'),record,name,fixture],exposure:'direct'});
  await writeFile(join(agent,'mcp.json'),JSON.stringify({mcpServers:{global:config('global')}}));
  await writeFile(join(cwd,'.pi','mcp.json'),JSON.stringify({mcpServers:{project:config('project')}}));
  await writeFile(join(agent,'settings.json'),JSON.stringify({codemode:{mode:scenario==='on'?'only':'on'}}));
  const suitePath=join(dir,'suite.json');
  await writeFile(suitePath,JSON.stringify({codemode_mode:scenario==='only'?'only':'on',servers:scenario==='empty'?[]:[{name:'selected',config:config('selected')}]}));
  const requests=[];
  let serverFailure;
  const server=createServer(async(req,res)=>{
   try {
    const chunks=[]; for await(const c of req) chunks.push(c);
    const body=JSON.parse(Buffer.concat(chunks).toString()); requests.push(body);
    let delta, finish;
    if(requests.length===1 && scenario!=='empty') {
     const tool=scenario==='no-suite'?'mcp__global__inspect':scenario==='only'?'codemode':'mcp__selected__inspect';
     const args=scenario==='only'?{code:'const digest = await tools.mcp__selected__inspect({}); const local = await tools.read({path:"'+fixture.replaceAll('\\','/')+'"}); text(digest); text(local);'}:{};
     delta={role:'assistant',tool_calls:[{index:0,id:'call_digest',type:'function',function:{name:tool,arguments:JSON.stringify(args)}}]};finish='tool_calls';
    } else {delta={role:'assistant',content:'verified owned fixture'};finish='stop';}
    res.writeHead(200,{'content-type':'text/event-stream'});
    res.write('data: '+JSON.stringify({id:'owned-smoke',object:'chat.completion.chunk',created:1,model:'smoke',choices:[{index:0,delta,finish_reason:null}]})+'\n\n');
    res.end('data: '+JSON.stringify({id:'owned-smoke',object:'chat.completion.chunk',created:1,model:'smoke',choices:[{index:0,delta:{},finish_reason:finish}]})+'\n\ndata: [DONE]\n\n');
   }catch(e){serverFailure=String(e);res.writeHead(500);res.end(String(e));}
  });
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  const port=server.address().port;
  await writeFile(join(agent,'models.json'),JSON.stringify({providers:{'suite-smoke':{baseUrl:`http://127.0.0.1:${port}/v1`,api:'openai-completions',apiKey:'owned-dummy-not-a-credential',models:[{id:'smoke',name:'smoke',contextWindow:32768,maxTokens:2048,reasoning:false,input:['text'],cost:{input:0,output:0,cacheRead:0,cacheWrite:0}}]}}}));
  const args=[...nodeArgs,executable,'--offline','--approve','--no-session','--mode','json','--model','suite-smoke/smoke','--no-skills','--no-prompt-templates','-e',join(output,'pi-suite.ts')];
  if(scenario!=='no-suite') args.push('--no-extensions','--no-mcp');
  args.push('Inspect the fixture using the provided tools.');
  // Deliberately do not inherit credentials, model/provider env, proxy, or user home.
  const env={PATH:process.env.PATH,SystemRoot:process.env.SystemRoot,TEMP:root,TMP:root,HOME:dir,USERPROFILE:dir,APPDATA:join(dir,'appdata'),LOCALAPPDATA:join(dir,'localappdata'),PI_CODING_AGENT_DIR:agent,PI_OFFLINE:'1',PI_SUITE_OBSERVATIONS:observations};
  if(scenario!=='no-suite') env.PI_SUITE_CONFIG=suitePath;
  let stdout='',stderr='';
  let child;
  try {
   child=spawn(process.execPath,args,{cwd,env,stdio:['ignore','pipe','pipe']});children.add(child);
   child.stdout.on('data',c=>stdout+=c);child.stderr.on('data',c=>stderr+=c);
   const timer=setTimeout(()=>child.kill(),60000);
   const code=await new Promise((r,j)=>{child.once('error',j);child.once('exit',r)});clearTimeout(timer);children.delete(child);
   const logs=await readFile(record,'utf8'),obs=await readFile(observations,'utf8');
   await writeFile(join(output,prefix+scenario+'.json'),JSON.stringify({command:[process.execPath,...args],code,stdout,stderr,requests,mcp:logs,observations:obs,serverFailure},null,2));
   assert.equal(code,0,stderr+'\n'+stdout);
   assert.equal(serverFailure,undefined);
   assert.ok(requests.length>0,'model received request');
   const names=requests[0].tools?.map(t=>t.function.name)??[];
   const started=logs.trim().split('\n').filter(Boolean).map(l=>JSON.parse(l)).filter(e=>e.event==='start');
   const starts=started.map(e=>e.identity);
   const cleanup=[];
   for(const proc of started) {
    let alive=true;
    for(let attempt=0;attempt<100;attempt++) {
     try {process.kill(proc.pid,0);} catch {alive=false;break;}
     await new Promise(r=>setTimeout(r,20));
    }
    if(alive) {process.kill(proc.pid);throw new Error('MCP child leaked after CLI exit: '+proc.pid);}
    cleanup.push({identity:proc.identity,pid:proc.pid,exited:true});
   }
   if(scenario==='no-suite') {assert.deepEqual(starts.sort(),['global','project']);assert.ok(names.includes('mcp__global__inspect'));assert.ok(names.includes('mcp__project__inspect'));assert.equal(JSON.parse(obs).suite,null);}
   else {assert.deepEqual(starts,scenario==='empty'?[]:['selected']);assert.ok(!names.some(n=>n.includes('global')||n.includes('project')));}
   if(scenario==='only') {
    assert.deepEqual(names,['codemode']);
    const returned=requests[1].messages.find(m=>m.role==='tool')?.content??'';
    assert.ok(returned.startsWith('Script completed'));
    assert.ok(returned.includes(payload.trim()),'native read returned actual fixture content');
   }
   if(scenario==='on') {assert.ok(names.includes('codemode'));assert.ok(names.includes('read'));assert.ok(names.includes('mcp__selected__inspect'));}
   if(scenario!=='empty') {assert.ok(JSON.stringify(requests.slice(1)).includes(expected),'real digest reached second model request');assert.ok(logs.includes('tools/call'));}
   results.push({scenario,code,modelRequests:requests.length,declaredTools:names,serverStarts:starts,cleanup,digestVerified:scenario!=='empty'});
  } finally {await new Promise(r=>server.close(r));}
 }
} finally {
 for(const c of children)c.kill();
 await rm(root,{recursive:true,force:true});
 await writeFile(join(output,prefix+'summary.json'),JSON.stringify({node:process.version,executable,expectedDigest:expected,results},null,2));
}
console.log(JSON.stringify(results,null,2));
