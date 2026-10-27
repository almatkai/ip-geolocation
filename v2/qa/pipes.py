#!/usr/bin/env python3
"""Real JSONL pipes, fixture services, synthetic private paths, no OS/network effects."""
import json,subprocess,selectors,time,pathlib,os,sys,tempfile,uuid
ROOT=pathlib.Path(__file__).resolve().parents[1]
class Peer:
 def __init__(self,exe,profile='p'):
  self.work=pathlib.Path(tempfile.mkdtemp(prefix='pipes-',dir=ROOT/'.work'))
  self.p=subprocess.Popen([str(exe)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env={**os.environ,'HOME':str(self.work/'home'),'TMPDIR':str(self.work)},bufsize=0)
  self.buffer=b'';self.events=[];self.reverse=[];self.seq=0;self.replies={};self.hello=self.read();assert self.hello['type']=='hello'
  self.request('initialize',{'protocol':1,'extensionId':'com.tezbar.native.ip-geolocation','platform':'macOS','packageDir':str(ROOT),'dataDir':str(self.work/profile),'tempDir':str(self.work/'temp'),'grantedCapabilities':['clipboard.write','window.visibility','settings.open']})
 def send(self,v):self.p.stdin.write((json.dumps(v)+'\n').encode());self.p.stdin.flush()
 def read(self,timeout=4):
  end=time.monotonic()+timeout
  while b'\n' not in self.buffer:
   remaining=end-time.monotonic();assert remaining>0,'JSONL timeout'
   with selectors.DefaultSelector() as s:
    s.register(self.p.stdout,selectors.EVENT_READ);assert s.select(remaining),'JSONL timeout'
   b=os.read(self.p.stdout.fileno(),65536);assert b,'unexpected EOF';self.buffer+=b;assert len(self.buffer)<16*1024*1024
  line,self.buffer=self.buffer.split(b'\n',1);return json.loads(line)
 def start(self,method,params):
  self.seq+=1;i='h:'+str(uuid.uuid4());self.send({'type':'request','id':i,'method':method,'params':params});return i
 def wait(self,i,handler=None):
  if i in self.replies:return self.replies.pop(i)
  while True:
   v=self.read()
   if v['type']=='request':
    self.reverse.append(v)
    result=handler(v) if handler else (None if v['method']=='host.window.visibility' else {'ok':True})
    if result is not HOLD:self.send({'type':'response','id':v['id'],'result':result})
   elif v['type']=='response':
    if v['id']==i:return v
    self.replies[v['id']]=v
   else:self.events.append(v)
 def request(self,method,params,handler=None):return self.wait(self.start(method,params),handler)
 def open(self,s,command='query-ip-geolocation',args=None,prefs=None,handler=None):return self.request('session.open',{'sessionId':s,'commandName':command,'argumentValues':args or {},'preferences':prefs or {}},handler)
 def call(self,s,operation,payload=None,handler=None):return self.request('session.call',{'sessionId':s,'operation':'extension:'+operation,'payload':payload or {}},handler)
 def dispose(self,s):return self.request('session.dispose',{'sessionId':s})
 def close(self):
  self.p.stdin.close();self.p.wait(timeout=5);assert self.p.returncode==0;assert self.p.stderr.read()==b''
HOLD=object()
def result(v):assert 'result' in v,v;return v['result']
def action(v,title):return next(a['id'] for a in v['actions'] if a['title']==title)
def run(exe):
 p=Peer(exe)
 q=result(p.open('q',args={'ipAddress':'a.test'}));old=action(q,'Copy IP');assert q['root']['children'][0]['props']['subtitle']=='a.test'
 b=result(p.call('q','search-text-changed',{'searchText':'b.test'}));assert b['root']['children'][0]['props']['subtitle']=='b.test'
 assert 'error' in p.call('q','invoke-action',{'actionId':old})
 a=result(p.call('q','search-text-changed',{'searchText':'a.test'}));assert action(a,'Copy IP')!=old
 assert 'error' in p.call('q','invoke-action',{'actionId':old})
 # Reader-order latest search cancellation and old rendered A IDs remain invalid.
 slow=p.start('session.call',{'sessionId':'q','operation':'extension:search-text-changed','payload':{'searchText':'slow.example'}})
 latest=p.start('session.call',{'sessionId':'q','operation':'extension:search-text-changed','payload':{'searchText':'latest.test'}})
 current=result(p.wait(latest));assert current['root']['children'][0]['props']['subtitle']=='latest.test';assert 'error' in p.wait(slow)
 assert 'error' in p.call('q','invoke-action',{'actionId':action(a,'Copy IP')})
 # Ordinary failed lookup exposes manual retry; no hidden revalidation/polling.
 failed=result(p.call('q','search-text-changed',{'searchText':'fail.example'}));assert action(failed,'Retry Lookup');assert result(p.call('q','invoke-action',{'actionId':action(failed,'Retry Lookup')}))['mode']=='view'
 current=result(p.call('q','search-text-changed',{'searchText':'latest.test'}));copy=action(current,'Copy IP')
 recovery=result(p.call('q','invoke-action',{'actionId':copy},lambda r: True))
 retry=action(recovery,'Retry Exact Copy');before=len(p.reverse)
 done=result(p.call('q','invoke-action',{'actionId':retry}));assert done['mode']=='no-view'
 assert [r['method'] for r in p.reverse[before:]]==['host.clipboard.write','host.window.visibility'];assert p.reverse[before]['params']=={'text':'latest.test'}
 assert 'error' in p.call('q','invoke-action',{'actionId':retry});p.dispose('q')
 # All commands successful with the same adapter, interfaces and HTTP injected.
 m=result(p.open('m','my-ip-geolocation',prefs={'showIPv6':True,'coordinatesFormat':'lonLat'}));assert len(m['root']['children'])==10;p.dispose('m')
 before=len(p.reverse);c=result(p.open('c','copy-ip',{'ipType':'Public','ipVersion':'IPv6'}));assert c['mode']=='no-view';assert [r['method'] for r in p.reverse[before:]]==['host.window.visibility','host.clipboard.write'];assert p.reverse[-1]['params']=={'text':'2001:db8::4'};p.dispose('c')
 # Args hotkey persistence is profile-owned, including original capitalized IpVersion key.
 result(p.open('persist','copy-ip'));assert p.reverse[-1]['params']=={'text':'2001:db8::4'};p.dispose('persist')
 # Cancel irreversible write; refresh exposes exact-byte retry, late ACK grants nothing.
 q=result(p.open('cancel',args={'ipAddress':'cancel.test'}));rid=p.start('session.call',{'sessionId':'cancel','operation':'extension:invoke-action','payload':{'actionId':action(q,'Copy IP')}})
 reverse=p.read();assert reverse['method']=='host.clipboard.write';p.send({'type':'cancel','requestId':rid,'reason':'synthetic cancel'});assert 'error' in p.wait(rid)
 p.send({'type':'response','id':reverse['id'],'result':{'ok':True}})
 view=result(p.call('cancel','refresh-session'));first_retry=action(view,'Retry Exact Copy');view=result(p.call('cancel','refresh-session'));assert action(view,'Retry Exact Copy')!=first_retry;assert 'error' in p.call('cancel','invoke-action',{'actionId':first_retry});p.dispose('cancel')
 # Drop guard + SDK failed-open cleanup; no dispose required after canceled opens.
 for n in range(140):
  if n%2==0:
   rid=p.start('session.open',{'sessionId':'drop-'+str(n),'commandName':'copy-ip','argumentValues':{},'preferences':{}})
   hide=p.read();assert hide['method']=='host.window.visibility';p.send({'type':'response','id':hide['id'],'result':None})
   write=p.read();assert write['method']=='host.clipboard.write'
   p.send({'type':'cancel','requestId':rid,'reason':'synthetic'});assert 'error' in p.wait(rid)
   p.send({'type':'response','id':write['id'],'result':{'ok':True}})
  else:
   rid=p.start('session.open',{'sessionId':'drop-'+str(n),'commandName':'query-ip-geolocation','argumentValues':{'ipAddress':'slow.example'},'preferences':{}})
   time.sleep(.002);p.send({'type':'cancel','requestId':rid,'reason':'synthetic'});assert 'error' in p.wait(rid)
 result(p.open('after',args={'ipAddress':'after.test'}));p.dispose('after')
 # Session capacity and cleanup. Opens are sequential to avoid handler-cap saturation.
 for n in range(128):result(p.open('cap-'+str(n)))
 assert 'error' in p.open('over')
 for n in range(128):p.dispose('cap-'+str(n))
 # EOF with active lookup must exit without leaking tasks.
 p.start('session.open',{'sessionId':'eof','commandName':'query-ip-geolocation','argumentValues':{'ipAddress':'slow.example'}});p.close()
 # Independent process/profile contains no persisted Args from previous profile.
 fresh=Peer(exe,'other');result(fresh.open('fresh','copy-ip'));assert fresh.reverse[-1]['params']=={'text':'192.0.2.7'};fresh.dispose('fresh');fresh.close()
 print('JSONL: all commands, input/latest/A-B-A, unknown ACK/exact retry, cancel/late ACK, 140 canceled opens, capacity, profile isolation, dispose, EOF passed')
if __name__=='__main__':run(pathlib.Path(sys.argv[1]).resolve())
