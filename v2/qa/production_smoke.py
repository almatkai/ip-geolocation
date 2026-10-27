import pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).parent))
from pipes import Peer,result
root=pathlib.Path(__file__).resolve().parents[1]
p=Peer((root/'.work/extracted/bin/ip-geolocation').resolve())
v=result(p.open('invalid',args={'ipAddress':'localhost'}))
assert v['root']['children'][0]['props']['title']=='Invalid Query'
assert not p.reverse
p.dispose('invalid');p.close()
print('EXTRACTED ACTUAL PRODUCTION executable: invalid input, zero network/host effects passed')
