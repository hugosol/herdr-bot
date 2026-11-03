import { appendFileSync, readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { createInterface } from 'node:readline';
const [record, identity, fixture] = process.argv.slice(2);
appendFileSync(record, JSON.stringify({event:'start',identity,pid:process.pid})+'\n');
process.on('exit',()=>appendFileSync(record,JSON.stringify({event:'exit',identity,pid:process.pid})+'\n'));
createInterface({input:process.stdin}).on('line', line=>{
  const req=JSON.parse(line);
  appendFileSync(record, JSON.stringify({event:'request',identity,request:req})+'\n');
  if(req.id === undefined) return;
  let result;
  if(req.method==='initialize') result={protocolVersion:'2024-11-05',capabilities:{tools:{}},serverInfo:{name:'suite-digest-fixture',version:'1'}};
  else if(req.method==='tools/list') result={tools:[{name:'inspect',description:'Read the owned fixture and calculate its SHA256 digest.',inputSchema:{type:'object',properties:{},additionalProperties:false}}]};
  else if(req.method==='tools/call') {
    const content=readFileSync(fixture);
    result={content:[{type:'text',text:JSON.stringify({identity,bytes:content.length,sha256:createHash('sha256').update(content).digest('hex')})}]};
  } else if(req.method==='ping') result={};
  else { process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:req.id,error:{code:-32601,message:'unknown method'}})+'\n'); return; }
  process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:req.id,result})+'\n');
});
