// Executes pinned TSX and locked public-ip/is-ip code. Privileged file loading lives
// outside the closed VM; every import/effect inside is allowlisted or rejected.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import net from 'node:net';
import {createRequire} from 'node:module';
import assert from 'node:assert/strict';
const root=path.resolve(import.meta.dirname,'../../');
const source=path.join(root,'v2/.work/v1');
const realRequire=createRequire(path.join(source,'package.json'));
const ts=realRequire('typescript');
const locked=JSON.parse(fs.readFileSync(path.join(root,'package-lock.json')));
assert.equal(locked.packages['node_modules/public-ip'].version,'7.0.1');
const reject=(name)=>{throw Error(`Unmocked effect/import denied: ${name}`)};
function oracle(config){
 const calls=[],cache=new Map(),modules=new Map(); let rows, promise;
 const sandbox=vm.createContext({console:{log(){},error(){},warn(){}},process:{hrtime:{bigint:()=>1000000000n}},Date:class extends Date{static now(){return 1000}},setTimeout:()=>reject('timer'),fetch:()=>reject('network')},{codeGeneration:{strings:false,wasm:false}});
 const component=(type)=>Object.assign((props)=>({type,props}),{label:type});
 const List=Object.assign(component('List'),{Item:component('List.Item'),EmptyView:component('List.EmptyView')});
 const Action=Object.assign(component('Action'),{CopyToClipboard:component('Action.CopyToClipboard')});
 const ActionPanel=Object.assign(component('ActionPanel'),{Section:component('ActionPanel.Section')});
 const api={List,Action,ActionPanel,Icon:new Proxy({},{get:(_,key)=>'Icon.'+String(key)}),getPreferenceValues:()=>config.preferences,Cache:class{constructor({namespace}){this.namespace=namespace}get(k){calls.push(['cache.get',k]);return cache.get(this.namespace+':'+k)}set(k,v){calls.push(['cache.set',k,v]);cache.set(this.namespace+':'+k,v)}},Clipboard:{copy:async(text)=>calls.push(['copy',text])},closeMainWindow:async()=>calls.push(['hide']),showHUD:async(text)=>calls.push(['hud',text]),updateCommandMetadata:async(data)=>calls.push(['metadata',data]),captureException:()=>calls.push(['exception']),environment:{launchType:config.background?'background':'user'},LaunchType:{UserInitiated:'user'},openCommandPreferences:async()=>calls.push(['settings.command']),openExtensionPreferences:async()=>calls.push(['settings.extension'])};
 const react={useState:(value)=>[value,()=>reject('unexpected setState')],useMemo:(fn)=>fn()};
 const jsx=(type,props)=>typeof type==='function'?type(props):({type,props});
 class CancelError extends Error{}
 const got={get(url,options){calls.push(['https',url,options]);const f=options.dnsLookupIpVersion;let response=config.public?.[f];if(Array.isArray(response))response=response.shift();const promise=response instanceof Error?Promise.reject(response):Promise.resolve({body:response??(f===4?'198.51.100.4':'2001:db8::4')});promise.cancel=()=>{};return promise;}};
 const mocks={'@raycast/api':api,'react':{...react,default:react},'react/jsx-runtime':{jsx,jsxs:jsx,Fragment:'Fragment'},'@raycast/utils':{useCachedPromise(fn,args=[]){if(rows===undefined){promise=fn(...args);}return {data:rows,isLoading:false};}},'os':{networkInterfaces:()=>{calls.push(['interfaces']);return config.interfaces??{};}},'net':{isIP:net.isIP},'axios':async(options)=>{calls.push(['geo',options]);if(config.failure)throw Error('SYNTHETIC_URL_TOKEN_MUST_NOT_LEAK');return {data:config.geo};},'got':{__esModule:true,default:got,CancelError},'node:util':{promisify:()=>()=>reject('DNS')},'node:dgram':{createSocket:()=>reject('UDP')},'dns-socket':()=>reject('DNS'),'node:vm':{Script:class{constructor(){}runInNewContext(context){context.returnValue=context.function_();}}}};
 const packages=new Set(['public-ip','is-ip','ip-regex','super-regex','function-timeout','time-span','convert-hrtime','clone-regexp','is-regexp']);
 function load(filename){
  if(modules.has(filename))return modules.get(filename).exports;
  const m={exports:{}};modules.set(filename,m);
  const text=fs.readFileSync(filename,'utf8');
  const code=ts.transpileModule(text,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS,jsx:ts.JsxEmit.ReactJSX,esModuleInterop:true},fileName:filename}).outputText;
  const localRequire=(name)=>{
   if(Object.hasOwn(mocks,name))return mocks[name];
   if(name.startsWith('.')){let f=path.resolve(path.dirname(filename),name);if(!fs.existsSync(f)){f=['.ts','.tsx','.js'].map(x=>f+x).find(fs.existsSync);}if(!f||!f.startsWith(source+path.sep))return reject(name);return load(f);}
   if(!packages.has(name))return reject(name);
   return load(realRequire.resolve(name));
  };
  const wrapped=new vm.Script('(function(require,module,exports){'+code+'\n})',{filename}).runInContext(sandbox,{timeout:2000});
  wrapped(localRequire,m,m.exports);return m.exports;
 }
 return {calls,load,async render(command,args={}){
  const fn=load(path.join(source,'src',command+'.tsx')).default;
  if(command==='copy-ip'){await fn({arguments:args});return {calls};}
  fn({arguments:args});rows=await promise;
  const tree=fn({arguments:args});
  return {rows:rows??[],tree:JSON.parse(JSON.stringify(tree)),calls};
 }};
}
const preferences={language:'en',coordinatesFormat:'latLon',showIPv6:false};
const geo={status:'success',query:'203.0.113.1',country:'Test',regionName:'Region',city:'City',district:'',zip:'',lat:0,lon:-2,timezone:'Etc/UTC',as:'AS0 Test',isp:'Fixture ISP',org:'Fixture Org'};
const interfaces={lo:[{family:'IPv4',address:'127.0.0.1',internal:true}],en0:[{family:'IPv4',address:'192.0.2.7',internal:false},{family:'IPv6',address:'2001:db8::7',internal:false}]};
const cases=[];
for(const query of ['', 'localhost','192.0.2.1',' 192.0.2.1 ','999.1.1.1','01.2.3.4','::1','2001:db8::1','fe80::1%en0','fe80::1%a_b','fe80::1%a.b','fe80::1%a-b','fe80::1%a~b','fe80::1%a:b','https://example.test/path','http://example.test','https://[::1]/x','例え.テスト','example.test?x=&lang=ru','x.test:8080','1.2.3','http://localhost','HTTPS://a.test/x','https://a.test/http://b.test','\ufeffx.test\ufeff','\u0085x.test\u0085']){
 const o=oracle({preferences,geo,interfaces});cases.push({kind:'query',input:query,preferences,...await o.render('query-ip-geolocation',{ipAddress:query})});
}
for(const change of [{district:false,zip:0,lat:0,lon:null,as:'AS123'},{district:null,zip:null,lat:1e-6,lon:1e21},{district:'0',zip:false,lat:-0,lon:1.25},{as:'',country:null,city:0}]){
 const value={...geo,...change};const o=oracle({preferences:{...preferences,coordinatesFormat:'lonLat'},geo:value});cases.push({kind:'format',input:'x.test',preferences:{...preferences,coordinatesFormat:'lonLat'},geo:value,...await o.render('query-ip-geolocation',{ipAddress:'x.test'})});
}
for(const showIPv6 of [false,true]){
 for(const fail of [false,true]){
  const value=fail?{status:'fail'}:geo;const o=oracle({preferences:{...preferences,showIPv6},geo:value,interfaces});cases.push({kind:'my',preferences:{...preferences,showIPv6},geo:value,interfaces,...await o.render('my-ip-geolocation')});
 }
}
for(const [ipType,ipVersion] of [['',''],['Public','IPv6'],['Public',''],['','IPv6'],['public','ipv6']]){
 const o=oracle({preferences,geo,interfaces});cases.push({kind:'copy',args:{ipType,ipVersion},...await o.render('copy-ip',{ipType,ipVersion})});
}
for(const [name,aliases] of Object.entries({internal:[{family:'IPv4',address:'10.0.0.2',internal:true},{family:'IPv4',address:'172.16.0.1',internal:false}],linklocal:[{family:'IPv6',address:'fe80::4',internal:false}],empty:[],multiple:[{family:'IPv4',address:'127.0.0.2',internal:false},{family:'IPv4',address:'192.0.2.9',internal:false}]})){
 const o=oracle({preferences,geo,interfaces:{[name]:aliases}});cases.push({kind:'copyinterfaces',interfaces:{[name]:aliases},args:{ipType:'',ipVersion:name==='linklocal'?'IPv6':''},...await o.render('copy-ip',{ipType:'',ipVersion:name==='linklocal'?'IPv6':''})});
}
{
 const entries=[['10',[{family:'IPv4',address:'10.0.0.10',internal:false}]],['2',[{family:'IPv4',address:'10.0.0.2',internal:false}]],['name',[{family:'IPv4',address:'192.0.2.1',internal:false}]]];const interfaces=Object.fromEntries(entries);const o=oracle({preferences,geo,interfaces});cases.push({kind:'copyinterfaces',interfaceOrder:entries.map(e=>e[0]),interfaces,args:{ipType:'',ipVersion:''},...await o.render('copy-ip',{ipType:'',ipVersion:''})});
}
for(const public4 of [['invalid','198.51.100.9'],[new Error('synthetic'),new Error('synthetic')],[' 198.51.100.8\n'],['999.1.1.1','invalid']]){
 const responses=public4.map(v=>v instanceof Error?null:v);const o=oracle({preferences,geo,interfaces,public:{4:public4}});cases.push({kind:'public',responses,...await o.render('copy-ip',{ipType:'Public',ipVersion:''})});
}
{
 const o=oracle({preferences,geo,interfaces});await o.render('copy-ip',{ipType:'Public',ipVersion:'IPv6'});await o.render('copy-ip',{});cases.push({kind:'persistent-args',calls:o.calls});
 const writes=o.calls.filter(c=>c[0]==='copy');assert.deepEqual(writes,[['copy','2001:db8::4'],['copy','2001:db8::4']]);assert(o.calls.some(c=>c[0]==='cache.get'&&c[1]==='IpVersion'));
}
{
 const o=oracle({preferences,geo,interfaces,background:true});await o.render('copy-ip',{});assert(!o.calls.some(c=>['hide','copy','hud'].includes(c[0])));cases.push({kind:'background',calls:o.calls});
}
// Explicitly verify the VM denies an unmocked OS/import capability.
assert.throws(()=>oracle({preferences}).load(path.join(source,'node_modules/axios/index.js')),/denied|Cannot use|Unexpected|outside/);
fs.writeFileSync(path.join(root,'v2/qa/oracle-results.json'),JSON.stringify({source:'48b1cd483b6e8fbf2d8ae8394ffdb11b4e8f659c',publicIp:'7.0.1',cases},null,2)+'\n');
console.log(`source oracle: ${cases.length} complete TSX/locked-library outputs; closed imports/effects`);
