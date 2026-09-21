"""Fresh, bounded reader for T2 DIF 44 / interior 0 high-detail surfaces.

Format research: OpenMBU engine/source/interior/interiorIO.cpp and interior.h.
No prior PeakRunner map tools used. Source coordinates remain Torque Z-up.
The unparsed collision/LOD tail is preserved, NOT claimed decoded.
"""
import argparse
import base64
import hashlib
import io
import json
import math
from pathlib import Path
import struct
import zlib
from PIL import Image


class InteriorError(ValueError):
    pass


class Reader:
    def __init__(self, data):
        self.data = data
        self.offset = 0

    def raw(self, size):
        if size < 0 or size > len(self.data) - self.offset:
            raise InteriorError(f'Truncated DIF at {self.offset}: requested {size}')
        out = self.data[self.offset:self.offset + size]
        self.offset += size
        return out

    def values(self, fmt):
        return list(struct.unpack('<' + fmt, self.raw(struct.calcsize('<' + fmt))))

    def one(self, fmt):
        return self.values(fmt)[0]

    def count(self, stride=1, packed=False):
        count = self.one('I')
        alternate = bool(count & 0x80000000)
        if alternate:
            if not packed:
                raise InteriorError('Unexpected compressed vector')
            count &= 0x7fffffff
            self.one('B')
        if count > 1_000_000 or count * stride > len(self.data) - self.offset:
            raise InteriorError(f'Invalid vector size {count} at {self.offset}')
        return count, alternate

    def array(self, fmt):
        count, _ = self.count(struct.calcsize('<' + fmt))
        return [self.values(fmt) for _ in range(count)]

    def indices(self, fmt, alternate_fmt=None):
        count, alt = self.count(packed=True)
        return self.values(str(count) + (alternate_fmt if alt and alternate_fmt else fmt))

    def png(self):
        start = self.offset
        if self.raw(8) != b'\x89PNG\r\n\x1a\n':
            raise InteriorError(f'Missing lightmap PNG at {start}')
        while True:
            length = struct.unpack('>I', self.raw(4))[0]
            kind = self.raw(4)
            payload = self.raw(length)
            crc = struct.unpack('>I', self.raw(4))[0]
            if zlib.crc32(kind + payload) != crc:
                raise InteriorError('Lightmap CRC mismatch')
            if kind == b'IEND':
                break
        data = self.data[start:self.offset]
        with Image.open(io.BytesIO(data)) as im:
            if any(n < 1 or n > 1024 or n & (n-1) for n in im.size):
                raise InteriorError(f'Unsupported lightmap dimensions {im.size}')
            im.load()
        return data


def decode(data):
    r = Reader(data)
    if r.one('I') != 44 or r.one('B') != 0:
        raise InteriorError('Only DIF resource 44 without preview supported')
    detail_count = r.one('I')
    if not 1 <= detail_count <= 16 or r.one('I') != 0:
        raise InteriorError('Only version-0 highest-detail interior supported')
    result = {'schema': 1, 'detail_count': detail_count,
              'detail_level': r.one('I'), 'min_pixels': r.one('I'),
              'bounds': r.values('6f'), 'sphere': r.values('4f'),
              'alarm_state': r.one('B'), 'light_state_entries': r.one('I')}
    result['normals'] = r.array('3f')
    result['planes'] = r.array('Hf')
    result['points'] = r.array('3f')
    result['visibility'] = r.indices('B')
    result['texgen'] = r.array('8f')
    result['bsp_nodes'] = r.array('HHH')
    result['solid_leaves'] = r.array('IH')
    if r.one('B') != 1:
        raise InteriorError('Unsupported material list version')
    count, _ = r.count()
    result['materials'] = [r.raw(r.one('B')).decode('latin-1') for _ in range(count)]
    result['windings'] = r.indices('I', 'H')
    result['winding_groups'] = r.array('II')
    result['zones'] = r.array('HHIHH')
    result['zone_surfaces'] = r.indices('H')
    result['zone_portals'] = r.indices('H')
    result['portals'] = r.array('HHIHH')
    count, _ = r.count(38)
    surfaces = []
    for _ in range(count):
        start, n, plane, material, texgen, flags, fan = r.values('IBHHIBI')
        word, u, v = r.values('Hff')
        light_count, light_start, ox, oy, sx, sy = r.values('HIBBBB')
        if word >> 13 > 5:
            raise InteriorError('Invalid lightmap coordinate encoding')
        surfaces.append(dict(start=start, count=n, plane=plane, material=material,
                             texgen=texgen, flags=flags, fan_mask=fan,
                             light_word=word, light_offset=[u,v], light_count=light_count,
                             light_start=light_start, light_rect=[ox,oy,sx,sy]))
    result['surfaces'] = surfaces
    result['normal_lightmaps'] = r.indices('B')
    result['alarm_lightmaps'] = r.indices('B')
    result['null_surfaces'] = r.array('IHBB')
    count, _ = r.count()
    result['lightmaps'] = []
    for _ in range(count):
        result['lightmaps'].append({'png': base64.b64encode(r.png()).decode('ascii'),
                                    'keep': r.one('B')})
    result['decoded_bytes'] = r.offset
    result['opaque_tail_base64'] = base64.b64encode(data[r.offset:]).decode('ascii')
    result['source_sha256'] = hashlib.sha256(data).hexdigest()
    validate(result)
    return result


def validate(d):
    def index(i, values, what):
        if not 0 <= i < len(values):
            raise InteriorError(f'Invalid {what} index {i}')
    for values in d['points'] + d['normals'] + d['texgen']:
        if not all(math.isfinite(v) for v in values):
            raise InteriorError('Nonfinite geometry')
    for normal, distance in d['planes']:
        index(normal, d['normals'], 'normal')
        if not math.isfinite(distance):
            raise InteriorError('Nonfinite plane')
    for i in d['windings']:
        index(i, d['points'], 'point')
    if len(d['normal_lightmaps']) != len(d['surfaces']):
        raise InteriorError('Surface/lightmap count mismatch')
    for i, s in enumerate(d['surfaces']):
        index(s['plane'] & 0x7fff, d['planes'], 'plane')
        index(s['material'], d['materials'], 'material')
        index(s['texgen'], d['texgen'], 'texgen')
        lm = d['normal_lightmaps'][i]
        if lm != 255:
            index(lm, d['lightmaps'], 'lightmap')
        if s['count'] < 3 or s['start'] + s['count'] > len(d['windings']):
            raise InteriorError('Invalid polygon winding span')


def triangles(d):
    """Yield source triangle strips, NOT fans, with explicit corner UVs."""
    validate(d)
    axes = [(0,1), (0,2), (1,0), (1,2), (2,0), (2,1)]
    for surface_id, s in enumerate(d['surfaces']):
        normal_id, _ = d['planes'][s['plane'] & 0x7fff]
        sign = -1 if s['plane'] & 0x8000 else 1
        normal = [v * sign for v in d['normals'][normal_id]]
        uv = d['texgen'][s['texgen']]
        word = s['light_word']
        ax, ay = axes[word >> 13]
        corners = []
        for pi in d['windings'][s['start']:s['start'] + s['count']]:
            p = d['points'][pi]
            corners.append({'position': p, 'normal': normal,
                            'uv': [sum(p[j]*uv[j+k] for j in range(3))+uv[k+3] for k in (0,4)],
                            'light_uv': [p[ax]*2**(-((word >> 6)&63))+s['light_offset'][0],
                                         p[ay]*2**(-(word&63))+s['light_offset'][1]]})
        for i in range(len(corners)-2):
            ids = (i,i+1,i+2) if i % 2 == 0 else (i+1,i,i+2)
            yield {'surface': surface_id, 'material': s['material'],
                   'lightmap': d['normal_lightmaps'][surface_id],
                   'corners': [corners[j] for j in ids]}


def exterior_fragments(d, polygon):
    """Clip a local-space polygon to BSP exterior zone 0, excluding solids.

Used to remove terrain intruding into source interior rooms. No guessed boxes.
"""
    result=[]
    pending=[(0, polygon, frozenset())]
    while pending:
        index, points, visited = pending.pop()
        if len(points)<3: continue
        if index & 0x8000:
            if not index & 0x4000 and index & 0x3fff == 0:
                result.append(points)
            continue
        if index in visited or index >= len(d['bsp_nodes']):
            raise InteriorError('Invalid BSP graph')
        if len(pending)+len(result)>10000:
            raise InteriorError('BSP fragment limit exceeded')
        plane,front,back=d['bsp_nodes'][index]
        ni,distance=d['planes'][plane&0x7fff]
        sign=-1 if plane&0x8000 else 1
        normal=d['normals'][ni]
        distances=[sign*(sum(p[i]*normal[i] for i in range(3))+distance) for p in points]
        if min(distances)>=-1e-7:
            pending.append((front,points,visited|{index}));continue
        if max(distances)<=1e-7:
            pending.append((back,points,visited|{index}));continue
        for child,side in ((front,1),(back,-1)):
            clipped=[]
            for i,p in enumerate(points):
                q=points[(i+1)%len(points)];a=distances[i]*side;b=distances[(i+1)%len(points)]*side
                if a>=0: clipped.append(p)
                if (a<0) != (b<0):
                    t=a/(a-b);clipped.append([p[k]+(q[k]-p[k])*t for k in range(3)])
            pending.append((child,clipped,visited|{index}))
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    private = (Path(__file__).resolve().parents[2]/'local-assets').resolve()
    if not args.output.resolve().is_relative_to(private):
        parser.error('Output must stay in local-assets')
    if args.output.exists():
        parser.error('Refusing overwrite')
    d = decode(args.source.read_bytes())
    args.output.mkdir(parents=True)
    (args.output/'interior.json').write_text(json.dumps(d, allow_nan=False)+'\n')
    for i, lm in enumerate(d['lightmaps']):
        (args.output/f'lightmap-{i}.png').write_bytes(base64.b64decode(lm['png']))
    ts = list(triangles(d))
    with (args.output/'triangles.jsonl').open('w') as stream:
        for triangle in ts:
            stream.write(json.dumps(triangle, allow_nan=False)+'\n')
    print(json.dumps({'source': str(args.source), 'points': len(d['points']),
                      'surfaces': len(d['surfaces']), 'triangles': len(ts),
                      'materials': d['materials'], 'lightmaps': len(d['lightmaps']),
                      'decoded_bytes': d['decoded_bytes'],
                      'tail_status': 'preserved but not interpreted'}, indent=2))
