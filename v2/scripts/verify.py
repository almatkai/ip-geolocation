#!/usr/bin/env python3
import pathlib,subprocess,json,hashlib
v2=pathlib.Path(__file__).resolve().parents[1];root=v2.parent
pin='48b1cd483b6e8fbf2d8ae8394ffdb11b4e8f659c'
def git(*args):return subprocess.check_output(['git',*args],cwd=root)
assert git('rev-parse','HEAD').decode().strip()==pin
assert git('branch','--show-current').decode().strip()=='feat/native-v2-rust'
assert not git('diff','--cached','--name-only')
assert git('diff','--name-only').decode().splitlines()==['README.md']
for name in git('ls-tree','-r','--name-only',pin).decode().splitlines():
 original=git('show',pin+':'+name);current=(root/name).read_bytes()
 if name=='README.md':assert current==original+b'\nStandalone Rust V2 development implementation and review notes: [v2/README.md](v2/README.md).\n'
 else:assert current==original,name
assert (v2/'.work/v1/package-lock.json').read_bytes()==(root/'package-lock.json').read_bytes()
for record in json.loads((v2/'docs/upstream-assets.json').read_text())['files']:
 if '/assets/' in record['path']:
  p=v2/'resources'/pathlib.Path(record['path']).name;assert hashlib.sha256(p.read_bytes()).hexdigest()==record['sha256'];assert record['ownBytesIdentical']
lock=(v2/'Cargo.lock').read_text();assert 'git+https://github.com/almatkai/tezbar-extension-sdk-rust.git?rev=e955911f0bf5b558f9d3eb5183df0adc95c22af2#e955911f0bf5b558f9d3eb5183df0adc95c22af2' in lock
assert 'path =' not in (v2/'Cargo.toml').read_text().split('[dependencies]',1)[1].split('[[bin]]',1)[0]
print('HEAD/branch/index, all V1 bytes except exact README link, original PNGs, immutable HTTPS SDK lock passed')
