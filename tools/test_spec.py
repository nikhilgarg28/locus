#!/usr/bin/env python3
"""Adversarial tests of the rule and documentation-example gates."""
import copy
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

import spec


def atlas(body=None):
    return {'docs': [
        {'id': 'language', 'group': 'Now', 'body': body or [
            '# Language', '<!-- spec: 1.1:1 legality-rule -->',
            'Runtime control requires a physical boolean.']},
        {'id': 'kernel-contract', 'group': 'Now', 'body': [
            '# Kernel', '<!-- spec: 2.1:1 informative -->', 'Historical context.']},
    ], 'tasks': [{'n': 91, 'status': 'todo'}, {'n': 92, 'status': 'done'}]}


class SpecGateTests(unittest.TestCase):
    def scratch_gate(self, data, citation='1.1:1'):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root/'tests').mkdir()
            (root/'tests/check.rs').write_text('#[test]\n#[doc = "spec: '+citation+'"]\nfn condition() { assert!(true); }\n')
            path = root/'atlas.html'
            path.write_text('<script type="application/json" id="atlas-data">\n'+json.dumps(data)+'\n</script>')
            return subprocess.run(['python3', str(spec.ROOT/'tools/spec.py'), 'check', '--root', str(root), '--atlas', str(path)], capture_output=True, text=True)

    def test_valid_gate_counts_explicit_rules_and_focused_tests(self):
        result = self.scratch_gate(atlas())
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('normative=1', result.stdout)
        self.assertIn('focused_tests=1', result.stdout)

    def test_removed_paragraph_is_a_dangling_citation(self):
        data = atlas()
        data['docs'][0]['body'] = ['# Language']
        result = self.scratch_gate(data)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('dangling citation', result.stderr)

    def test_uncovered_operative_paragraph_fails(self):
        data = atlas()
        data['docs'][0]['body'] += ['', '<!-- spec: 1.1:2 dynamic-semantics -->', 'An ordinary argument still runs.']
        result = self.scratch_gate(data)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('1.1:2', result.stderr)
        self.assertIn('no focused test', result.stderr)

    def test_informative_only_assertion_fails(self):
        result = self.scratch_gate(atlas(), '2.1:1')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('only informative', result.stderr)

    def test_category_is_local_and_omission_is_informative(self):
        data = atlas()
        data['docs'][0]['body'] += ['', '<!-- spec: 1.1:2 -->', 'This is background.']
        self.assertEqual(spec.inventory(data)[1].category, 'informative')
        self.assertEqual(self.scratch_gate(data).returncode, 0)

    def test_duplicate_missing_and_unknown_category_annotations_fail(self):
        for body, expected in [
            (['No annotation.'], 'no stable spec ID'),
            (['<!-- spec: 1.1:1 alleged -->', 'A rule.'], 'unknown category'),
            (['<!-- spec: 1.1:1 syntax -->', 'A rule.', '', '<!-- spec: 1.1:1 syntax -->', 'Another.'], 'duplicate spec ID'),
        ]:
            with self.assertRaisesRegex(ValueError, expected):
                spec.inventory(atlas(body))

    def test_broad_corpus_does_not_count_as_focused(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = root/'tests/corpus/accept/long.lc'
            path.parent.mkdir(parents=True)
            path.write_text('fn x()->u8{0}\n'+'// still part of the file\n'*38+'//~ spec: 1.1:1\n')
            uses = spec.citations(root)
            self.assertFalse(uses[0].focused)
            with self.assertRaisesRegex(ValueError, 'no focused test'):
                spec.validate(spec.inventory(atlas()), uses)
            path.write_text('fn x()->u8{0}\n//~ spec: 1.1:1\n')
            self.assertTrue(spec.citations(root)[0].focused)

    def test_rust_citation_must_belong_to_one_test(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root/'tests').mkdir()
            (root/'tests/unfocused.rs').write_text('#[doc = "spec: 1.1:1"]\nfn helper() {}\n')
            with self.assertRaisesRegex(ValueError, 'attached after'):
                spec.citations(root)

    def test_now_fences_declare_modes_vision_is_exempt(self):
        data = atlas(['~~~locus check', 'fn f()->u8{1}', '~~~'])
        self.assertEqual(spec.fences(data)[0].mode, 'check')
        data['docs'][0]['body'][0] = '~~~locus'
        with self.assertRaisesRegex(ValueError, 'needs LANGUAGE'):
            spec.fences(data)
        data['docs'][0]['group'] = 'Vision'
        self.assertEqual(spec.fences(data), [])

    def test_prose_requires_reason_run_requires_value_reject_requires_code(self):
        for opening, code, expected in [
            ('~~~text prose', 'X := Y', 'needs a reason'),
            ('~~~locus run', 'fn f()->u8{1}', 'no expected run value'),
            ('~~~locus reject', 'fn f()->u8{true}', 'needs diagnostic codes'),
        ]:
            with self.assertRaisesRegex(ValueError, expected):
                spec.fences(atlas([opening, code, '~~~']))

    def test_known_markers_require_existing_open_tasks(self):
        self.assertEqual(spec.validate_known(atlas(), [('bug.lc', 'LOC-91', 'fold fails')]), 1)
        for name, reason, expected in [('LOC-92','fixed','closed'),('LOC-93','missing','does not exist'),('LOC-91','','no reason')]:
            with self.assertRaisesRegex(ValueError, expected):
                spec.validate_known(atlas(), [('bug.lc', name, reason)])

    def test_renderer_emits_real_anchor_and_understands_fence_modes(self):
        node = shutil.which('node')
        self.assertIsNotNone(node, 'Node is required to test the Atlas renderer')
        html = (spec.ROOT/'atlas.html').read_text()
        renderer = html[html.index('const SEP ='):html.index('// The lines of the section a heading opens:')]
        prelude = '''const DB={meta:{taskPrefix:'LOC'},tasks:[]};
const esc=s=>s.replace(/&/g,'&amp;').replace(/</g,'&lt;');
const slug=s=>s; const docById=()=>true;
'''
        test = '''const rendered=renderMarkdown('<!-- spec: 1.21:2 legality-rule -->\\n\\nA rule.\\n\\n~~~locus check\\nfn f()->u8{1}\\n~~~',{docId:'language'}).html;
if(!rendered.includes('id="spec-1.21:2"')||!rendered.includes('#/doc/language/spec-1.21:2')||!rendered.includes('<pre>')||rendered.includes('~~~'))throw Error(rendered);
'''
        result = subprocess.run([node, '-e', prelude+renderer+test], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("r[2].startsWith('spec-') ? r[2] : 'h-' + r[2]", html)

if __name__ == '__main__':
    unittest.main()
