"""Tests for the new codec, without old tools or proprietary test fixtures."""
import json
from pathlib import Path
import struct
import tempfile
import unittest
import scene


class CodecTests(unittest.TestCase):
    def test_named_attributes_keep_float_bits(self):
        values = struct.unpack('<12f', struct.pack('<12f', -0., 1.25, 30.5,
            0., -1., 0., .125, .875, .25, .75, 7., -12.))
        result = scene.flatten(json.loads(json.dumps(scene.vertex(values))))
        self.assertEqual(struct.pack('<12f',*values),struct.pack('<12f',*result))

    def test_missing_attribute_rejected(self):
        row = scene.vertex([0.]*12)
        del row['light_uv']
        with self.assertRaises(ValueError):
            scene.flatten(row)

    def test_full_scene_reconstruction_and_real_edit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); original = root/'reference'; original.mkdir()
            values = [0.,0.,0.,0.,1.,0.,0.,0.,0.,0.,0.,-1.]
            payload = {'vertices.bin':struct.pack('<36f',*(values*3)),
                'collision.bin':struct.pack('<9f',*([0.]*9)),
                'height.bin':bytes(256*256*2), 'weights.rgba':bytes([255,0,0,0])*256*256,
                'ambient.f32':struct.pack('<f',.125),
                'textures.rgba':b''.join(bytes([45,80,100,255])*(256>>level)**2 for level in range(9))}
            for name,value in payload.items(): (original/name).write_bytes(value)
            scene.save_json(original/'map.json',{'private_reference':True,'texture_count':1,
                'files':{name:scene.sha(value) for name,value in payload.items()}})
            editable=root/'editable'; scene.decode(original,editable)
            report=scene.encode(editable,root/'compiled')
            self.assertTrue(all(report['payload_identity'].values()))
            # Compiler must use editable values, not secretly copy source bytes.
            path=editable/'surfaces.jsonl'; row=json.loads(path.read_text())
            row['corners'][0]['position'][0]=2.5
            scene.save_json(path,row)
            changed=scene.encode(editable,root/'edited')
            self.assertFalse(changed['payload_identity']['vertices.bin'])
            self.assertEqual(struct.unpack_from('<f',(root/'edited/vertices.bin').read_bytes())[0],2.5)
            with self.assertRaises(ValueError): scene.encode(editable,root/'compiled')


if __name__=='__main__': unittest.main()
