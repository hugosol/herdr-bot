import { registerHooks, createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
const installed = createRequire('D:/nvm4w/nodejs/node_modules/@earendil-works/pi-coding-agent/package.json');
// Only locate existing third-party dependencies. Workspace aliases remain the checkout resolver's job.
registerHooks({resolve(specifier,context,nextResolve){
 try {return nextResolve(specifier,context);}
 catch(error){
  if(error.code!=='ERR_MODULE_NOT_FOUND'||specifier.startsWith('.')||specifier.startsWith('/')||specifier.startsWith('file:')||specifier.startsWith('@earendil-works/')) throw error;
  return {url:pathToFileURL(installed.resolve(specifier)).href,shortCircuit:true};
 }
}});
