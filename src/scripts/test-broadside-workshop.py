#!/usr/bin/env python3
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('workshop',Path(__file__).with_name('build-broadside-workshop.py'))
workshop=importlib.util.module_from_spec(spec);spec.loader.exec_module(workshop)

class WorkshopTests(unittest.TestCase):
    def source(self,root):
        src=root/'source';src.mkdir()
        files={}
        for name in workshop.FILES:
            data=('synthetic '+name).encode();(src/name).write_bytes(data)
            files[name]=workshop.digest(data)
        manifest={'private_reference':True,'files':files,'flags':[[1,2,3],[4,5,6]],
                  'spawns':[[7,8,9],[10,11,12]],'reference_bases':[{'test':'unchanged'}]}
        (src/'map.json').write_text(json.dumps(manifest))
        return src,manifest

    def test_exact_payload_and_runtime_metadata_preservation(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);src,before=self.source(root);out=root/'copy'
            original=(src/'map.json').read_bytes()
            workshop.build(src,out)
            after=json.loads((out/'map.json').read_text())
            for k,v in before.items():self.assertEqual(after[k],v)
            for name in workshop.FILES:self.assertEqual((src/name).read_bytes(),(out/name).read_bytes())
            self.assertEqual((src/'map.json').read_bytes(),original)
            self.assertEqual(after['workshop_baseline']['changes'],[])
            with self.assertRaises(ValueError):workshop.build(src,out)

    def test_corruption_rejected_before_output_created(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);src,_=self.source(root);out=root/'copy'
            (src/'collision.bin').write_bytes(b'corrupt')
            with self.assertRaises(ValueError):workshop.build(src,out)
            self.assertFalse(out.exists())

if __name__=='__main__':unittest.main()
