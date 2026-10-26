from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def block(text,name):
 start=text.index(name);pos=text.index('{',start);depth=0
 for i in range(pos,len(text)):
  if text[i]=='{':depth+=1
  if text[i]=='}':
   depth-=1
   if depth==0:return text[start:i+1]
 raise ValueError(name)
b=(ROOT/'qa/host/src-tauri/src/native_core/extensions/registry.rs').read_text();a=(ROOT/'qa/host-after/src-tauri/src/native_core/extensions/registry.rs').read_text()
for fn in ['fn preferences_path(', 'fn retained_preferences_path(', 'fn retain_preferences(']:assert block(a,fn).replace('        resource_path: path.clone(),\n','')==block(b,fn),'Canonical preferences changed'
(ROOT/'qa/host-alias.rs').write_text(block(a,'pub(super) fn verified_alias_from(')+'\n')
for label,text in [('before',b),('after',a)]:
 (ROOT/f'qa/host-icon-{label}.rs').write_text(block(text,'fn fallback')+'\n'+block(text,'fn icon_path(')+'\npub fn call(e:&Installed)->Option<String>{icon_path(e)}\n')
print('Actual extracted Rust functions saved; canonical preference/retention functions byte-identical')
