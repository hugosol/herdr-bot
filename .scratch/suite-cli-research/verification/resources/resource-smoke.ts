import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
const root=fs.mkdtempSync(path.join(os.tmpdir(),'suite resources '));
// Dynamic imports intentionally exercise loader boundaries and capture missing native modules.
const evidence:Record<string,unknown> & {results:Record<string,unknown>[]}={runtime:Bun.version,root,manualSymlinks:process.env.HERDR_BOT_MANUAL_SYMLINKS??null,results:[]};
const put=(p:string,s:string)=>{fs.mkdirSync(path.dirname(p),{recursive:true});fs.writeFileSync(p,s)};
try {
 const entity=path.join(root,'shared entity');
 put(path.join(entity,'skill','SKILL.md'),'---\nname: fixture-resource\ndescription: Harmless resource fixture\n---\nRead references/value.txt only.\n');
 put(path.join(entity,'skill','references','value.txt'),'fixture-skill-value');
 put(path.join(entity,'extension','helper.ts'),"export const value = 'relative-import-ok';\n");
 put(path.join(entity,'extension','index.ts'),"import { value } from './helper.ts';\nimport { readFileSync } from 'node:fs';\nconst asset=readFileSync(new URL('./asset.txt',import.meta.url),'utf8');\nexport default function(pi) { pi.registerTool({name:'fixture_resource',label:'Fixture',description:value+':'+asset,parameters:{type:'object',properties:{}},async execute(){return {content:[{type:'text',text:value+':'+asset}]}}}); }\n");
 put(path.join(entity,'extension','asset.txt'),'relative-asset-ok');
 const piSkills=await import('E:/Developer/myhub/pi/packages/coding-agent/src/core/skills.ts');
 const {DefaultPackageManager}=await import('E:/Developer/myhub/pi/packages/coding-agent/src/core/package-manager.ts');
 const {SettingsManager}=await import('E:/Developer/myhub/pi/packages/coding-agent/src/core/settings-manager.ts');
 const {loadExtensions}=await import('E:/Developer/myhub/pi/packages/coding-agent/src/core/extensions/loader.ts');
 for(const kind of ['junction',...(process.env.HERDR_BOT_MANUAL_SYMLINKS==='1'?['dir']:[])]) {
  for(const suite of ['suite a','suite b']) {
   const base=path.join(root,kind,suite); const skills=path.join(base,'skills'); const extensions=path.join(base,'extensions');
   fs.mkdirSync(skills,{recursive:true});fs.mkdirSync(extensions,{recursive:true});
   try{fs.symlinkSync(path.join(entity,'skill'),path.join(skills,'fixture-resource'),kind as 'junction' | 'dir');fs.symlinkSync(path.join(entity,'extension'),path.join(extensions,'fixture-extension'),kind as 'junction' | 'dir');}catch(e){evidence.results.push({kind,suite,linkError:String(e)});continue;}
   put(path.join(extensions,'package.json'),JSON.stringify({name:'isolated-suite',pi:{extensions:['./fixture-extension/index.ts']},omp:{extensions:['./fixture-extension/index.ts']}}));
   const skillResult=piSkills.loadSkills({cwd:base,agentDir:path.join(base,'agent'),skillPaths:[skills],includeDefaults:false});
   assert.equal(skillResult.skills.length,1); assert.equal(fs.readFileSync(path.join(skillResult.skills[0].baseDir,'references','value.txt'),'utf8'),'fixture-skill-value');
   const pm=new DefaultPackageManager({cwd:base,agentDir:path.join(base,'agent'),settingsManager:SettingsManager.inMemory({}),builtinExtensions:[]});
   const resolved=await pm.resolveExtensionSources([extensions]);
   const entries=resolved.extensions.map((x)=>x.path);assert.equal(entries.length,1);assert.ok(entries[0].endsWith('index.ts'));
   let loaded=await loadExtensions(entries,base); const checkoutErrors=loaded.errors;
   if(loaded.errors.length){ const installed=await import('D:/nvm4w/nodejs/node_modules/@earendil-works/pi-coding-agent/dist/core/extensions/loader.js'); loaded=await installed.loadExtensions(entries,base); }
   assert.deepEqual(loaded.errors,[]);assert.equal(loaded.extensions.length,1);
   const tools=loaded.extensions[0].tools;const tool=tools.get('fixture_resource');assert.ok(tool);const definition=tool.definition??tool;
   const output=await definition.execute('fixture',{},undefined,undefined,{});assert.equal(output.content[0].text,'relative-import-ok:relative-asset-ok');
   const row:Record<string,unknown>={kind,suite,checkoutErrors,piLoader:checkoutErrors.length?'installed':'checkout',piSkills:skillResult.skills.map((s)=>({name:s.name,path:s.filePath})),entries,toolResult:output,realSkill:fs.realpathSync(path.join(skills,'fixture-resource')),realExtension:fs.realpathSync(path.join(extensions,'fixture-extension'))};
   try{const ompSkills=await import('E:/Developer/myhub/oh-my-pi/packages/coding-agent/src/extensibility/skills.ts');const r=await ompSkills.loadSkillsFromDir({dir:skills,source:'custom:user'});assert.equal(r.skills.length,1);row.ompSkills=r;}catch(e){row.ompSkillsError=String(e)}
   try{const omp=await import('E:/Developer/myhub/oh-my-pi/packages/coding-agent/src/extensibility/extensions/loader.ts');const r=await omp.loadExtensions(entries,base);row.ompExtensions={errors:r.errors,count:r.extensions.length};}catch(e){row.ompExtensionsError=String(e)}
   evidence.results.push(row);
  }
 }
 if(process.env.HERDR_BOT_MANUAL_SYMLINKS!=='1')evidence.symlinks='NOT ATTEMPTED: explicit manual environment switch absent';
} catch(e){evidence.error=String(e);process.exitCode=1} finally {fs.rmSync(root,{recursive:true,force:true});evidence.cleaned=!fs.existsSync(root);console.log(JSON.stringify(evidence,null,2));}
