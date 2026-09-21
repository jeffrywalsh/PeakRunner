import unittest
from mission import parse, MissionError

class MissionTests(unittest.TestCase):
    def test_hierarchy_literals_comments_arrays(self):
        result=parse('''// leading comment
new SimGroup(Team1) { label="brace } // not a comment";
/* block */ new InteriorInstance() { position="1 2 3"; texture[0]="a.png"; scale=1; }; };''')
        a,b=result['objects']
        self.assertEqual(b['parent'],a['id'])
        self.assertEqual(b['fields']['position'],'1 2 3')
        self.assertEqual(b['fields']['texture[0]'],'a.png')
        self.assertEqual(a['fields']['label'],'brace } // not a comment')
    def test_executable_syntax_not_accepted(self):
        for text in ['exec("evil.cs");','new X() { value=run(); };',
                     'new X() { value="a" @ "b"; };','new X() { value="a"; value="b"; };',
                     'new X() { value="a";']:
            with self.assertRaises(MissionError):parse(text)
    def test_external_script_flagged_not_executed(self):
        result=parse('//--- OBJECT WRITE BEGIN ---\nnew X() {};\n//--- OBJECT WRITE END ---\nexec("evil.cs");')
        self.assertEqual(len(result['objects']),1)
        self.assertTrue(result['warnings'])
    def test_deep_nesting_rejected(self):
        with self.assertRaises(MissionError):parse('new X() {'*66+'};'*66)

if __name__=='__main__':unittest.main()
