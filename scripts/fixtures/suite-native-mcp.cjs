// Controlled external MCP server. Results come from actual owned files, not echoes.
const fs = require('node:fs');
const readline = require('node:readline');
const [logfile, data, identity] = process.argv.slice(2);
const log = value => fs.appendFileSync(logfile, JSON.stringify(value) + '\n');
const send = value => process.stdout.write(JSON.stringify({jsonrpc:'2.0', ...value}) + '\n');
let refreshed = false;
log({event:'start', pid:process.pid, identity});
// The runner holds a Windows process handle before initialization proceeds.
// This also accounts for a native late reconnect during teardown.
const acknowledged = logfile + '.' + process.pid + '.ack';
const gate = setInterval(() => {
  if (!fs.existsSync(acknowledged)) return;
  clearInterval(gate);
  readline.createInterface({input:process.stdin}).on('line', line => {
  const m = JSON.parse(line); log(m);
  const reply = result => send({id:m.id, result});
  if (m.method === 'initialize') reply({protocolVersion:'2024-11-05', capabilities:{tools:{listChanged:true}, resources:{listChanged:true}, prompts:{listChanged:true}}, serverInfo:{name:identity,version:'1'}, instructions:'SERVER-INSTRUCTIONS-916: use the owned file probe.'});
  else if (m.method === 'tools/list') reply({tools:[{name:refreshed?'refreshed':'probe',description:'Read owned acceptance file',inputSchema:{type:'object',properties:{}}}]});
  else if (m.method === 'tools/call') {
    reply({content:[{type:'text',text:identity + ':' + fs.readFileSync(data,'utf8')}]});
    if (!refreshed) {
      refreshed = true;
      for (const surface of ['tools','resources','prompts']) send({method:`notifications/${surface}/list_changed`});
    }
  }
  else if (m.method === 'resources/list') reply({resources:[{uri:refreshed?'owned://refreshed':'owned://payload',name:refreshed?'Refreshed owned file':'Initial owned file',mimeType:'text/plain'}]});
  else if (m.method === 'resources/templates/list') reply({resourceTemplates:[]});
  else if (m.method === 'resources/read') reply({contents:[{uri:m.params.uri,mimeType:'text/plain',text:fs.readFileSync(data,'utf8')}]});
  else if (m.method === 'prompts/list') reply({prompts:[{name:'owned-prompt',description:refreshed?'Refreshed prompt':'Initial prompt'}]});
  else if (m.method === 'prompts/get') reply({description:'Owned file prompt',messages:[{role:'user',content:{type:'text',text:'PROMPT:' + fs.readFileSync(data,'utf8')}}]});
  else if (m.id !== undefined) reply({});
  });
}, 10);
