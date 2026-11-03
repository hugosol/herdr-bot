import fs from 'node:fs';
import crypto from 'node:crypto';
import readline from 'node:readline';
const [fixture,log]=process.argv.slice(2);
function record(event,data={}){fs.appendFileSync(log,JSON.stringify({event,pid:process.pid,fixture,...data})+'\n');}
function digest(){const data=fs.readFileSync(fixture);return {sha256:crypto.createHash('sha256').update(data).digest('hex'),bytes:data.length,fixture};}
record('start',{argv:process.argv});
process.on('exit',code=>record('exit',{code}));
for await(const line of readline.createInterface({input:process.stdin,crlfDelay:Infinity})) {
 let req;try{req=JSON.parse(line);}catch{continue;}
 record('request',{request:req});if(req.id===undefined)continue;
 let result;
 switch(req.method){
 case 'initialize':record('initialize');result={protocolVersion:'2024-11-05',capabilities:{tools:{},resources:{},prompts:{}},serverInfo:{name:'controlled-file-reader',version:'1.0.0'}};break;
 case 'tools/list':result={tools:[{name:'sha256_file',description:'Read owned fixture bytes and calculate SHA256',inputSchema:{type:'object',properties:{nonce:{type:'string'}},required:['nonce']}}]};break;
 case 'tools/call': {const actual=digest();record('call',{actual,arguments:req.params});result={content:[{type:'text',text:JSON.stringify(actual)}]};break;}
 case 'resources/list':result={resources:[{uri:'fixture://sha256',name:'owned fixture SHA256',mimeType:'application/json'}]};break;
 case 'resources/templates/list':result={resourceTemplates:[]};break;
 case 'resources/read': {const actual=digest();record('read-resource',{actual});result={contents:[{uri:'fixture://sha256',mimeType:'application/json',text:JSON.stringify(actual)}]};break;}
 case 'prompts/list':result={prompts:[{name:'fixture-prompt',description:'Controlled prompt'}]};break;
 case 'prompts/get':result={messages:[{role:'user',content:{type:'text',text:'controlled fixture prompt; no model request'}}]};break;
 case 'ping':result={};break;
 default:process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:req.id,error:{code:-32601,message:'unsupported'}})+'\n');continue;
 }
 const response={jsonrpc:'2.0',id:req.id,result};record('response',{response});process.stdout.write(JSON.stringify(response)+'\n');
}
record('stdin-eof');
