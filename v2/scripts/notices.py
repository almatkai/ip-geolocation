#!/usr/bin/env python3
"""Copy actual license texts, with package/source/hash attribution; no synthesized grants."""
import json,subprocess,os,pathlib,hashlib,shutil
root=pathlib.Path(__file__).resolve().parents[1]
records=[]
def copy_notices(kind,name,version,directory,license_value,source):
 directory=pathlib.Path(directory);dest=root/'docs/notices'/kind/(name.replace('/','_')+'-'+version);files=[]
 for path in sorted(directory.iterdir()):
  if path.is_file() and path.name.lower().startswith(('license','licence','copying','notice','copyright')):
   dest.mkdir(parents=True,exist_ok=True);out=dest/path.name;shutil.copyfile(path,out);files.append({'path':str(out.relative_to(root)),'sha256':hashlib.sha256(out.read_bytes()).hexdigest()})
 records.append({'kind':kind,'name':name,'version':version,'declaredLicense':license_value,'source':source,'texts':files,'missingText':not bool(files)})
metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1'],cwd=root,env={**os.environ,'CARGO_HOME':str(root/'.work/cargo-home')}))
for package in metadata['packages']:
 if package['name']=='tezbar-ip-geolocation':continue
 copy_notices('cargo',package['name'],package['version'],pathlib.Path(package['manifest_path']).parent,package['license'],package['source'])
lock=json.loads((root.parent/'package-lock.json').read_text())
for path,package in lock['packages'].items():
 if not path:continue
 directory=root/'.work/v1'/path
 if directory.is_dir():
  manifest=json.loads((directory/'package.json').read_text())
  copy_notices('npm-qa',manifest['name'],manifest['version'],directory,manifest.get('license'),package.get('resolved'))
for info in (root/'.work/python/lib').glob('python*/site-packages/*.dist-info'):
 name=info.name.rsplit('.dist-info',1)[0];files=[]
 for directory in [info,info/'licenses']:
  if directory.is_dir():copy_notices('python-qa',name,'qa',directory,'See package metadata','PyPI fixture tooling')
(root/'docs/dependency-notices.json').write_text(json.dumps(records,indent=2)+'\n')
print('Authentic notice records:',len(records),'missing text:',sum(r['missingText'] for r in records))
