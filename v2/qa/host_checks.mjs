import fs from 'node:fs';import path from 'node:path';import vm from 'node:vm';import assert from 'node:assert/strict';import{createRequire}from'node:module';
const root=path.resolve(import.meta.dirname,'..');const ts=createRequire(path.join(root,'.work/v1/package.json'))('typescript');
function block(source,marker){const start=source.indexOf(marker);assert(start>=0);let pos=source.indexOf('{',start),depth=0;for(let i=pos;i<source.length;i++){if(source[i]==='{')depth++;if(source[i]==='}'&&!--depth)return source.slice(start,i+1);}throw Error('missing block');}
const name='src/renderer/ExtensionRuntimeView.tsx';
for(const [dir,expected]of[['host',[]],['host-after',['dispose','back']]]){
 const s=fs.readFileSync(path.join(root,'qa',dir,name),'utf8');const start=s.lastIndexOf("if (result.mode === 'no-view')");const code=block(s.slice(start),"if (result.mode === 'no-view')");
 const calls=[];const context=vm.createContext({result:{mode:'no-view'},setState:()=>{},state:{sessionId:'synthetic'},currentSession:{current:'owner'},window:{tezbar:{extensionDisposeSession:async v=>{assert.deepEqual(v.sessionId,'synthetic');calls.push('dispose');}}},onBack:()=>calls.push('back')},{codeGeneration:{strings:false,wasm:false}});
 await new vm.Script('(async()=>{'+code+'})()').runInContext(context,{timeout:1000});assert.deepEqual(calls,expected);
}
// Contracts are from actual router/platform extracts, not a universal ACK shim.
const router=fs.readFileSync(path.join(root,'qa/host/src-tauri/src/native_core/extensions/process_host.rs'),'utf8');
const platform=fs.readFileSync(path.join(root,'qa/host/src-tauri/src/native_core/platform.rs'),'utf8');
assert(router.includes('"host.window.visibility"=>self.ctx.request("core:platform:visibility"'));
assert(block(platform,'"core:platform:visibility" =>').includes('Ok(Value::Null)'));
assert(block(platform,'"core:platform:open-settings" =>').includes('Ok(json!({"ok":true}))'));
console.log('Actual renderer no-view callback before/after; actual visibility/settings router contracts passed');

const clipboard=fs.readFileSync(path.join(root,'qa/host/src-tauri/src/native_core/launcher/clipboard.rs'),'utf8');
assert(block(clipboard,'"clipboard:write" =>').includes('Ok(json!({"ok":true}))'));
assert(block(clipboard,'"clipboard:read" =>').includes('Ok(json!(text))'));
assert(router.includes('self.ctx.request("clipboard:write",vec![json!(text)])'));
