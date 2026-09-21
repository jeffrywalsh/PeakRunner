"""Private end-to-end edit proof: rebuild without native assets, edit one poster."""
import argparse
import contextlib
import hashlib
import io
import json
from pathlib import Path
import shutil
import tempfile
import numpy as np
from stonehenge import build


def verify(pack):
    manifest=json.loads((pack/'map.json').read_text())
    for name,expected in manifest['files'].items():
        if hashlib.sha256((pack/name).read_bytes()).hexdigest()!=expected:
            raise ValueError('Installed asset hash mismatch: '+name)
    with tempfile.TemporaryDirectory(prefix='stonehenge-proof-') as temp:
        root=Path(temp);editable=root/'editable'
        shutil.copytree(pack/'editable',editable)
        with contextlib.redirect_stdout(io.StringIO()):
            build(editable,root/'unchanged',editable)
        for name in manifest['files']:
            if (pack/name).read_bytes()!=(root/'unchanged'/name).read_bytes():
                raise ValueError('Editable rebuild differs: '+name)
        settings=json.loads((editable/'poster.json').read_text());settings['size']*=0.8
        (editable/'poster.json').write_text(json.dumps(settings))
        with contextlib.redirect_stdout(io.StringIO()):
            build(editable,root/'edited',editable)
        for name in manifest['files']:
            same=(pack/name).read_bytes()==(root/'edited'/name).read_bytes()
            if same != (name!='vertices.bin'):
                raise ValueError('Poster edit touched unexpected payload: '+name)
        before=np.frombuffer((pack/'vertices.bin').read_bytes(),dtype='<f4').reshape(-1,12)
        after=np.frombuffer((root/'edited'/'vertices.bin').read_bytes(),dtype='<f4').reshape(-1,12)
        changed=np.any(before!=after,axis=1)
        if changed.sum()!=6 or not np.array_equal(before[:,3:],after[:,3:]):
            raise ValueError('Poster edit changed more than its six corner positions')
    return {'all_runtime_payloads_rebuild_identically':True,
            'native_files_needed_for_editable_rebuild':False,
            'poster_edit_changed_corners':6,'collision_terrain_textures_audio_unchanged':True}


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('pack',type=Path)
    args=p.parse_args()
    report=verify(args.pack)
    print(json.dumps(report,indent=2))
