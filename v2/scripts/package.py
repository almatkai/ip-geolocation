#!/usr/bin/env python3
import pathlib,json,hashlib,zipfile,shutil,os
root=pathlib.Path(__file__).resolve().parents[1]
artifacts=root/'artifacts';artifacts.mkdir(exist_ok=True)
stage=root/'.work/package';stage.mkdir(parents=True,exist_ok=True)
shutil.copytree(root/'resources',stage/'resources',dirs_exist_ok=True)
shutil.copytree(root/'docs/notices/cargo',stage/'resources/notices/cargo',dirs_exist_ok=True)
for name,source in [('LICENSE',root.parent/'LICENSE'),('NOTICE',root/'NOTICE')]:shutil.copyfile(source,stage/name)
for name,source in [('SDK-LICENSE',root/'qa/sdk/LICENSE'),('SDK-NOTICE',root/'qa/sdk/NOTICE'),('UPSTREAM-LICENSE',root/'qa/upstream/LICENSE'),('LIBUV-LICENSE',root/'qa/node-v24/deps/uv/LICENSE')]:shutil.copyfile(source,stage/'resources'/name)
(stage/'bin').mkdir(exist_ok=True);executable=stage/'bin/ip-geolocation';shutil.copyfile(pathlib.Path(os.environ.get('CARGO_TARGET_DIR',str(root/'.work/final-target')))/'release/ip-geolocation',executable);executable.chmod(0o755)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
manifest=json.loads((root/'extension.json').read_text());manifest['files']={str(p.relative_to(stage)):sha(p)for p in sorted(stage.rglob('*'))if p.is_file() and p.name!='extension.json'}
(stage/'extension.json').write_text(json.dumps(manifest,indent=2)+'\n')
archive=artifacts/'ip-geolocation-aarch64-apple-darwin-dev.zip'
with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED,compresslevel=9)as z:
 for p in sorted(stage.rglob('*')):
  if p.is_file():
   name=str(p.relative_to(stage));info=zipfile.ZipInfo(name,date_time=(2026,1,1,0,0,0));info.create_system=3;info.external_attr=((0o100755 if name.startswith('bin/')else 0o100644)<<16);info.compress_type=zipfile.ZIP_DEFLATED;z.writestr(info,p.read_bytes())
extract=root/'.work/extracted';extract.mkdir(exist_ok=True)
with zipfile.ZipFile(archive)as z:
 assert set(z.namelist())==set(manifest['files'])|{'extension.json'}
 for name,digest in manifest['files'].items():assert hashlib.sha256(z.read(name)).hexdigest()==digest
 z.extractall(extract)
(extract/'bin/ip-geolocation').chmod(0o755)
report={'developmentOnly':True,'installableRelease':False,'sourceCommit':'48b1cd483b6e8fbf2d8ae8394ffdb11b4e8f659c','target':'aarch64-apple-darwin','archive':str(archive.relative_to(root)),'archiveSha256':sha(archive),'executableSha256':sha(executable),'extractedExecutableSha256':sha(extract/'bin/ip-geolocation'),'productionSourcesSha256':{str(p.relative_to(root)):sha(p) for p in sorted((root/'src').glob('*.rs')) if p.name!='geo_tests.rs'},'cargoLockSha256':sha(root/'Cargo.lock'),'freshCargoHome':os.environ.get('CARGO_HOME','unspecified'),'freshTargetDir':os.environ.get('CARGO_TARGET_DIR','unspecified'),'inventory':manifest['files'],'artifacts':{},'note':'No release URL, catalog pins or installation; fresh HTTPS git SDK production build. Extracted smoke must use invalid input only.'}
(root/'docs/package-report.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({k:v for k,v in report.items()if k!='inventory'},indent=2))
