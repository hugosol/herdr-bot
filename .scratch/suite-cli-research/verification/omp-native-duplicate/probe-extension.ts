import fs from 'node:fs';
import {MCPManager} from 'D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent/src/mcp/manager.ts';
export default function(pi:any) {
 pi.on('session_start', async (_event:any,ctx:any)=> {
  const result:any={event:'native-session-start',case:process.env.CASE_LABEL,tools:[],servers:[]};
  try {
   const manager=MCPManager.instance();
   await manager.waitForStartup(15000);
   result.allNames=manager.getAllServerNames();result.connected=manager.getConnectedServers();
   result.registeredTools=pi.getAllTools().map((t:any)=>t.name).filter((n:string)=>n.startsWith('mcp'));
   for(const name of manager.getConnectedServers()) {
    await manager.ensureServerResources(name);await manager.refreshServerPrompts(name);
    const resource=await manager.readServerResource(name,'fixture://sha256');
    result.servers.push({name,config:manager.getServerConfig(name),source:manager.getSource(name),resources:manager.getServerResources(name),prompts:manager.getServerPrompts(name),resource,prompt:await manager.executePrompt(name,'fixture-prompt',{})});
   }
   for(const loaded of manager.getTools()) {
    const tool=loaded;
    const output=await tool.execute('owned-smoke-call',{nonce:'actual-file-read'},undefined,ctx);
    result.tools.push({name:tool.name,output});
   }
   result.success=true;
  } catch(e:any) {result.error=e.stack??String(e);result.success=false;}
  finally {fs.writeFileSync(process.env.RESULT_FILE!,JSON.stringify(result,null,2));ctx.shutdown();}
 });
}
