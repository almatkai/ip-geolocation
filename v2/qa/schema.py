import json,copy,pathlib
from jsonschema import Draft202012Validator,FormatChecker
root=pathlib.Path(__file__).resolve().parents[1]
schema=json.loads((root/'qa/host/schemas/extension-v3.schema.json').read_text());Draft202012Validator.check_schema(schema);validator=Draft202012Validator(schema,format_checker=FormatChecker())
manifest=json.loads((root/'extension.json').read_text());validator.validate(manifest)
negative=[]
for key,value in [('schemaVersion',2),('runtime','javascript'),('execution','embedded'),('protocolVersion',2),('id','com.tezbar.ip-geolocation'),('repository','not a URI'),('capabilities',['unsafe']),('artifacts',{'x':{}}),('extra',True),('files',{'../escape':'a'*64})]:
 m=copy.deepcopy(manifest);m[key]=value;assert list(validator.iter_errors(m)),key;negative.append(key)
for field,value in [('refreshMs',499),('timeoutMs',300001),('concurrency','parallel')]:
 m=copy.deepcopy(manifest);m['commands'][0][field]=value;assert list(validator.iter_errors(m)),field;negative.append('command.'+field)
assert manifest['artifacts']=={};assert {c['name']for c in manifest['commands']}=={'query-ip-geolocation','my-ip-geolocation','copy-ip'}
assert all(c['refreshMs'] is None for c in manifest['commands']);assert all(v['state']=='not-run'for v in manifest['verification'])
print('Actual schema valid; '+str(len(negative))+' negative families rejected')
