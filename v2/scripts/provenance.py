#!/usr/bin/env python3
import pathlib,json,subprocess,hashlib
root=pathlib.Path(__file__).resolve().parents[1];repo=root.parent
sha=lambda b:hashlib.sha256(b).hexdigest()
source='48b1cd483b6e8fbf2d8ae8394ffdb11b4e8f659c';host='ea100cee3f69711de12485ba061950c5fc65258f';sdk='e955911f0bf5b558f9d3eb5183df0adc95c22af2'
files=subprocess.check_output(['git','ls-tree','-r','--name-only',source],cwd=repo,text=True).splitlines()
source_files={f:sha(subprocess.check_output(['git','show',source+':'+f],cwd=repo))for f in files}
fixtures={str(p.relative_to(root)):sha(p.read_bytes())for folder in ['qa/host','qa/sdk','qa/upstream','qa/node-v24']for p in sorted((root/folder).rglob('*'))if p.is_file()}
(root/'docs/provenance.json').write_text(json.dumps({'ownRepository':'https://github.com/almatkai/ip-geolocation.git','ownCommit':source,'branch':'feat/native-v2-rust','hostCommit':host,'sdkGit':'https://github.com/almatkai/tezbar-extension-sdk-rust.git','sdkCommit':sdk,'sdkVersion':'0.1.1','upstreamCommit':'6734d85992290066fd31a7f5bee3cca3d3545a1a','nodeBehaviorFixture':'nodejs/node v24.0.0; native APIs compared, not executed','geolocationContract':{'url':'http://ip-api.com/json/{encodedQuery}?lang={sourceLanguage}&fields=585727','transport':'original fixed HTTP, no auth/cookies/proxy/redirect; separate from public-IP TLS','evidence':'docs/provider-evidence.md','qa':'actual production transport via cfg(test)-only loopback routing; no live provider API'},'sourceFilesSha256':source_files,'immutableFixtureSha256':fixtures},indent=2)+'\n')
