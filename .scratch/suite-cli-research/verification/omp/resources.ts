import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import {loadAllMCPConfigs} from 'D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent/src/mcp/config.ts';
import {loadSkills} from 'D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent/src/extensibility/skills.ts';
import {setDisabledProviders,invalidate} from 'D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent/src/discovery/index.ts';
import {loadExtensions} from 'D:/nvm4w/nodejs/node_modules/@oh-my-pi/pi-coding-agent/src/extensibility/extensions/loader.ts';
const root=process.env.SMOKE_ROOT!; const cwd=path.join(root,'project'); const agent=process.env.PI_CODING_AGENT_DIR!;
const suite=path.join(root,'suite package');const skills=path.join(root,'selected skills');
async function put(file:string,body:string){await fs.mkdir(path.dirname(file),{recursive:true});await fs.writeFile(file,body);}
async function skill(base:string,name:string){await put(path.join(base,name,'SKILL.md'),`---\nname: ${name}\ndescription: isolated smoke skill\n---\n${name}\n`);}
await skill(path.join(agent,'skills'),'user-default');await skill(path.join(cwd,'.omp','skills'),'project-default');await skill(skills,'suite-selected');
await put(path.join(suite,'package.json'),JSON.stringify({name:'fixture-suite',version:'1.0.0',omp:{extensions:['./index.ts']}}));
await put(path.join(suite,'index.ts'),'export default function() {}');
await put(path.join(suite,'mcp.json'),JSON.stringify({mcpServers:{'suite-selected':{command:'not-executed-fixture',args:[]},'suite-force':{command:'not-executed-force',args:[],enabled:false}}}));
await put(path.join(cwd,'.omp','mcp.json'),JSON.stringify({mcpServers:{'project-default':{command:'not-executed-project'}}}));
const fixed=path.join(root,'fixed.ts');await put(fixed,`export default function(pi) { pi.registerCommand('fixed-retained', {description:'fixed fixture', handler: async()=>{}}); }`);
const roots={explicit:[suite,fixed],mode:'explicit-only' as const,configured:[],configuredLevel:'user' as const};
const disabled=['native','omp-managed','agents','claude','claude-plugins','codex','github','opencode','skillshare','agent-plugins','mcp-json','cursor','gemini','vscode','windsurf'];
setDisabledProviders(disabled);
const observedSkills=await loadSkills({cwd,customDirectories:[skills],extensionRoots:roots});
assert.deepEqual(observedSkills.skills.map(s=>s.name),['suite-selected']);
const ext=await loadExtensions([fixed,path.join(suite,'index.ts')],cwd);
assert.deepEqual(ext.errors,[]); assert(ext.extensions.some(e=>e.commands.has('fixed-retained')));
const evidence:any={skills:observedSkills.skills.map(s=>({name:s.name,source:s.source})),extensions:ext.extensions.map(e=>({path:e.path,commands:[...e.commands.keys()]})),disabledProviders:disabled,mcp:[]};
for(const [label,lists] of [['empty',{}],['deny',{disabledServers:['suite-selected']}],['force',{enabledServers:['suite-force']}]] as const){
 await put(path.join(agent,'mcp.json'),JSON.stringify({mcpServers:{'user-default':{command:'not-executed-user'}},...lists}));
 invalidate(path.join(agent,'mcp.json'));
 const r=await loadAllMCPConfigs(cwd,{enableProjectConfig:false,filterExa:false,extensionRoots:roots});
 evidence.mcp.push({label,names:Object.keys(r.configs),sources:r.sources});
 assert.deepEqual(Object.keys(r.configs).sort(),label==='empty'?['suite-selected']:label==='deny'?[]:['suite-force','suite-selected']);
}
console.log(JSON.stringify(evidence,null,2));
