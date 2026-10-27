#!/usr/bin/env python3
"""Inventory every Git-visible file, including untracked V2, except this inventory."""
import pathlib,subprocess,hashlib,sys
root=pathlib.Path(__file__).resolve().parents[2]
index=root/'v2/INDEX.sha256'
paths=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z'],cwd=root).split(b'\0')
paths=sorted({p.decode()for p in paths if p and p.decode()!='v2/INDEX.sha256'})
content=''.join(hashlib.sha256((root/p).read_bytes()).hexdigest()+'  '+p+'\n'for p in paths)
if '--check' in sys.argv:assert index.read_text()==content,'Git-visible inventory changed'
else:index.write_text(content)
print(f'Whole Git-visible inventory verified: {len(paths)} files; excludes only self, not required fixtures')
