import tempfile
from pathlib import Path
import unittest
from terrain_file import decode,encode,save_editable,load_editable,TerrainError,SAMPLES


class TerrainTests(unittest.TestCase):
    def fixture(self):
        return {'version':3,'heights':[i%65536 for i in range(SAMPLES)],
            'material_flags':bytes(range(256))*256,
            'material_names':['grass','','rock','','','','',''],
            'alpha':{'0':bytes([45])*SAMPLES,'2':bytes([210])*SAMPLES},
            'scripts':{'texture_editor':b'exec("DO NOT RUN");\x00\xff','height_editor':b'raw\x92'}}
    def test_exact_native_and_editable_roundtrip(self):
        original=encode(self.fixture())
        self.assertEqual(encode(decode(original)),original)
        with tempfile.TemporaryDirectory() as root:
            path=Path(root)/'scene';save_editable(decode(original),path)
            self.assertEqual(encode(load_editable(path)),original)
            with self.assertRaises(TerrainError):save_editable(self.fixture(),path)
    def test_reject_truncation_version_and_trailing_bytes(self):
        data=encode(self.fixture())
        for bad in (b'',data[:100],data[:-1],b'\x07'+data[1:],data+b'extra'):
            with self.assertRaises(TerrainError):decode(bad)
    def test_height_edit_changes_exact_sample_only(self):
        original=self.fixture(); raw=encode(original)
        changed=decode(raw);changed['heights'][42]=1234
        encoded=encode(changed)
        self.assertEqual(encoded[:85],raw[:85]);self.assertEqual(encoded[87:],raw[87:])
        self.assertEqual(decode(encoded)['heights'][42],1234)

if __name__=='__main__':unittest.main()
