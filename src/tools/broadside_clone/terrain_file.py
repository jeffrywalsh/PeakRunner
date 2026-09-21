#!/usr/bin/env python3
"""Independent TER v3 codec. Preserves editor scripts as inert bytes.

Format research: GarageGames Torque3D terrFile.cpp::_loadLegacy.
No dependency on previous PeakRunner converters/parsers.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import struct
from PIL import Image

SIDE=256
SAMPLES=SIDE*SIDE


class TerrainError(ValueError): pass


class Reader:
    def __init__(self,data): self.data=data; self.offset=0
    def read(self,size):
        if size<0 or size>len(self.data)-self.offset:
            raise TerrainError(f'Truncated terrain at byte {self.offset}; requested {size}')
        result=self.data[self.offset:self.offset+size]; self.offset+=size
        return result
    def u8(self): return self.read(1)[0]
    def u32(self): return struct.unpack('<I',self.read(4))[0]


def decode(data):
    reader=Reader(data)
    version=reader.u8()
    if version!=3: raise TerrainError(f'Unsupported TER version {version}; only 3 validated')
    heights=list(struct.unpack('<65536H',reader.read(SAMPLES*2)))
    material_flags=reader.read(SAMPLES)
    names=[reader.read(reader.u8()).decode('latin-1') for _ in range(8)]
    if not any(names): raise TerrainError('No terrain materials')
    alpha={str(i):reader.read(SAMPLES) for i,name in enumerate(names) if name}
    scripts={name:reader.read(reader.u32()) for name in ('texture_editor','height_editor')}
    if reader.offset!=len(data): raise TerrainError(f'Unexpected trailing terrain bytes: {len(data)-reader.offset}')
    return {'version':version,'heights':heights,'material_flags':material_flags,
            'material_names':names,'alpha':alpha,'scripts':scripts}


def encode(terrain):
    if terrain['version']!=3: raise TerrainError('Expected TER v3')
    result=bytearray([3])
    if len(terrain['heights'])!=SAMPLES: raise TerrainError('Invalid height count')
    result.extend(struct.pack('<65536H',*terrain['heights']))
    if len(terrain['material_flags'])!=SAMPLES: raise TerrainError('Invalid material grid')
    result.extend(terrain['material_flags'])
    names=terrain['material_names']
    if len(names)!=8: raise TerrainError('Expected eight material slots')
    for name in names:
        raw=name.encode('latin-1')
        if len(raw)>255: raise TerrainError('Material name too long')
        result.append(len(raw));result.extend(raw)
    expected={str(i) for i,name in enumerate(names) if name}
    if set(terrain['alpha'])!=expected: raise TerrainError('Alpha layers do not match material slots')
    for i,name in enumerate(names):
        if name:
            raw=terrain['alpha'][str(i)]
            if len(raw)!=SAMPLES: raise TerrainError('Invalid alpha dimensions')
            result.extend(raw)
    for key in ('texture_editor','height_editor'):
        raw=terrain['scripts'][key];result.extend(struct.pack('<I',len(raw)));result.extend(raw)
    return bytes(result)


def save_editable(terrain,output):
    if output.exists(): raise TerrainError('Refusing overwrite')
    output.mkdir(parents=True)
    data={'schema':1,'version':3,'width':SIDE,'height':SIDE,'units_per_metre':32,
        'rows':[terrain['heights'][i:i+SIDE] for i in range(0,SAMPLES,SIDE)],
        'material_names':terrain['material_names'],
        'editor_scripts_base64':{key:base64.b64encode(value).decode('ascii') for key,value in terrain['scripts'].items()},
        'script_policy':'inert roundtrip data; never execute'}
    (output/'terrain.json').write_text(json.dumps(data,indent=2)+'\n')
    Image.frombytes('L',(SIDE,SIDE),terrain['material_flags']).save(output/'material-flags.png')
    for index,alpha in terrain['alpha'].items():
        Image.frombytes('L',(SIDE,SIDE),alpha).save(output/f'alpha-{index}.png')


def load_editable(root):
    data=json.loads((root/'terrain.json').read_text())
    if (data['schema'],data['width'],data['height'],data['units_per_metre'])!=(1,SIDE,SIDE,32):
        raise TerrainError('Invalid editable terrain schema')
    if len(data['rows'])!=SIDE or any(len(row)!=SIDE for row in data['rows']):
        raise TerrainError('Invalid height grid')
    def pixels(name):
        with Image.open(root/name) as im:
            if im.mode!='L' or im.size!=(SIDE,SIDE): raise TerrainError('Expected 256x256 grayscale image')
            return im.tobytes()
    return {'version':data['version'],'heights':[v for row in data['rows'] for v in row],
        'material_names':data['material_names'],'material_flags':pixels('material-flags.png'),
        'alpha':{str(i):pixels(f'alpha-{i}.png') for i,name in enumerate(data['material_names']) if name},
        'scripts':{key:base64.b64decode(value,validate=True) for key,value in data['editor_scripts_base64'].items()}}


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source',type=Path);parser.add_argument('output',type=Path)
    args=parser.parse_args()
    private=(Path(__file__).resolve().parents[2]/'local-assets').resolve()
    if not args.output.resolve().is_relative_to(private): parser.error('Use ignored local-assets output')
    raw=args.source.read_bytes();terrain=decode(raw)
    save_editable(terrain,args.output)
    rebuilt=encode(load_editable(args.output))
    if rebuilt!=raw: raise TerrainError('Editable terrain roundtrip mismatch')
    (args.output/'reconstructed.ter').write_bytes(rebuilt)
    report={'source_sha256':hashlib.sha256(raw).hexdigest(),'exact_roundtrip':True,
        'height_samples':len(terrain['heights']),'height_range_metres':[min(terrain['heights'])/32,max(terrain['heights'])/32],
        'materials':[name for name in terrain['material_names'] if name],
        'editor_script_bytes':{name:len(value) for name,value in terrain['scripts'].items()},
        'source':str(args.source),'placement':'mission transform not applied in this local-space codec'}
    (args.output/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2))
