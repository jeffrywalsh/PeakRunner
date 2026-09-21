"""Read-only mission inventory for the private collection; no legacy importer."""
import json
from pathlib import Path
import re
from collections import Counter

root=Path(__file__).resolve().parents[2]
source=root/'local-assets/tribes-map-catalog'
rows=[]
for path in sorted(source.rglob('*.mis')):
    text=path.read_text(errors='replace')
    refs={field:sorted(set(re.findall(r'\b'+field+r'\s*=\s*"([^"\n]+)"',text)))
          for field in ('terrainFile','interiorFile','shapeName','dataBlock')}
    kinds=Counter(re.findall(r'\bnew\s+(\w+)\s*\(',text))
    rows.append({'mission':str(path.relative_to(source)), 'objects':dict(kinds),
        'references':refs,'status':'inventoried, not converted or validated'})
output=root/'local-assets/broadside-clone/mission-inventory.json'
if output.exists(): raise ValueError('Refusing inventory overwrite')
output.write_text(json.dumps({'source':str(source),'missions':rows},indent=2)+'\n')
print(f'{len(rows)} mission files; '+str(output))
print(dict(Counter(Path(row['mission']).parts[1] for row in rows)))
