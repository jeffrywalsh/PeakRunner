#!/usr/bin/env python3
"""Small source-format regressions; no original game assets required."""
import importlib.util
from pathlib import Path
import unittest

import numpy as np

spec = importlib.util.spec_from_file_location('converter', Path(__file__).with_name('convert-raindance.py'))
converter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(converter)


class ConversionTests(unittest.TestCase):
    def test_mission_nesting_comments_and_quoted_braces(self):
        result = converter.objects('''// ignored new Fake() { }
            new SimGroup(Team1) {
                new Item(Flag) { position = "1 2 3"; note = "} // literal"; };
            };''')
        self.assertEqual(len(result), 2)
        self.assertEqual(result[1]['parents'], ['Team1'])
        self.assertEqual(result[1]['fields']['note'], '} // literal')

    def test_expressions_are_not_evaluated(self):
        with self.assertRaises(ValueError):
            converter.objects('new Item(X) { position = execute("bad"); };')

    def test_unclosed_object_rejected(self):
        with self.assertRaises(ValueError):
            converter.objects('new SimGroup(X) {')

    def test_coordinate_basis(self):
        np.testing.assert_allclose(converter.point([10, 20, 30]), [1034, 30, 1044])
        np.testing.assert_allclose(converter.direction([1, 2, 3]), [1, 3, 2])

    def test_torque_rotation_and_scale(self):
        matrix, offset = converter.transform(dict(rotation='0 0 1 90', scale='2 3 4', position='1 2 3'))
        np.testing.assert_allclose(matrix @ [1, 0, 0], [0, -2, 0], atol=1e-12)
        np.testing.assert_allclose(offset, [1, 2, 3])


if __name__ == '__main__':
    unittest.main()
