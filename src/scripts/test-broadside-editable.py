#!/usr/bin/env python3
"""Synthetic exchange tests, no proprietary fixtures required."""
import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('exchange', Path(__file__).with_name('broadside-editable.py'))
exchange = importlib.util.module_from_spec(spec)
spec.loader.exec_module(exchange)


class ExchangeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / 'source'; self.source.mkdir()
        positions = [(0., 0., 0.), (1., 0., 0.), (0., 1., 0.)]
        rows = [(*p, 0., 0., 1., .25, .5, .125, .875, 0., -4.) for p in positions]
        data = {'vertices.bin': b''.join(struct.pack('<12f', *r) for r in rows),
                'collision.bin': struct.pack('<9f', *(v for p in positions for v in p)),
                'textures.rgba': bytes([128, 64, 32, 255]) * 256 * 256,
                'ambient.f32': struct.pack('<f', .5)}
        for name, value in data.items():
            (self.source / name).write_bytes(value)
        manifest = dict(private_reference=True, files={n: exchange.sha(v) for n, v in data.items()},
                        instances=[dict(name='dbase_broadside_nef.dif', first=0, count=1)])
        (self.source / 'map.json').write_text(json.dumps(manifest))
        self.edit = self.root / 'edit'
        exchange.export(self.source, self.edit)

    def test_exact_roundtrip_and_overwrite_refusal(self):
        out = self.root / 'roundtrip'
        self.assertTrue(all(exchange.import_mesh(self.edit, out).values()))
        with self.assertRaises(ValueError):
            exchange.import_mesh(self.edit, out)
        with self.assertRaises(ValueError):
            exchange.export(self.source, self.edit)

    def test_edit_updates_render_and_collision_preserves_lighting(self):
        obj = self.edit / 'fortress.obj'
        obj.write_text(obj.read_text().replace('v 0.0 0.0 0.0', 'v 0.25 0.0 0.0', 1))
        out = self.root / 'edited'
        result = exchange.import_mesh(self.edit, out)
        self.assertFalse(result['vertices.bin']); self.assertFalse(result['collision.bin'])
        self.assertTrue(result['textures.rgba'])
        self.assertEqual(struct.unpack_from('<f', (out / 'collision.bin').read_bytes())[0], .25)
        self.assertEqual((out / 'vertices.bin').read_bytes()[32:48],
                         (self.source / 'vertices.bin').read_bytes()[32:48])

    def test_reject_topology_change_before_output(self):
        obj = self.edit / 'fortress.obj'
        obj.write_text(obj.read_text().replace('g triangle_0', 'g changed'))
        out = self.root / 'bad'
        with self.assertRaises(ValueError):
            exchange.import_mesh(self.edit, out)
        self.assertFalse(out.exists())


if __name__ == '__main__':
    unittest.main()
