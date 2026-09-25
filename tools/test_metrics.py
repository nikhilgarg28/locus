"""Status cannot present stale counts, incomplete logs, or hand-edited output."""
import json
import pathlib
import tempfile
import unittest
from unittest import mock
import metrics
import content

class Metrics(unittest.TestCase):
    def test_status_publisher_leaves_manuals_and_roadmap_bytes_unchanged(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp)
            (root/'docs/data').mkdir(parents=True)
            manual=root/'docs/manual.md';manual.write_text('Unusual spacing retained.\n')
            roadmap=root/'docs/roadmap.md';roadmap.write_text('Task order retained.\n')
            data={'docs':[{'id':'generated-status','source':'docs/generated-status.md','body':['STALE']},
                          {'id':'manual','source':'docs/manual.md','body':['rewritten']}],
                  'projects':[{'id':'p','name':'Project','source':'docs/roadmap.md'}],
                  'tasks':[], 'measurements':{'source_fingerprint':'old'}}
            with mock.patch.object(metrics,'ROOT',root):metrics.store_status(data)
            self.assertEqual(manual.read_text(),'Unusual spacing retained.\n')
            self.assertEqual(roadmap.read_text(),'Task order retained.\n')
            self.assertIn('STALE',(root/'docs/generated-status.md').read_text())
            self.assertEqual(json.loads((root/'docs/data/state.json').read_text())['measurements'],data['measurements'])

    def test_only_completed_successful_test_logs_contribute_counts(self):
        tally='test result: ok. 7 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 1s\n'
        with self.assertRaises(ValueError):metrics.test_counts(tally,'fast')
        with self.assertRaises(ValueError):metrics.test_counts(tally+'LOCUS TEST RUN COMPLETE: extended\n','fast')
        full=tally+tally+'LOCUS TEST RUN COMPLETE: fast\n'
        self.assertEqual(metrics.test_counts(full,'fast'),{'passed':14,'ignored':4,'targets':2})
        with self.assertRaises(ValueError):metrics.test_counts(full+'error: test failed\n','fast')

    def test_stale_status_hides_counts_and_preserves_raw_record(self):
        record={'source_fingerprint':'old','fast_tests':{'passed':999}}
        original=json.dumps(record)
        with mock.patch.object(metrics,'render',return_value=['measured counts']) as render:
            stale=metrics.expected_body(record,'new')
            self.assertIn('STALE','\n'.join(stale));self.assertNotIn('999','\n'.join(stale))
            render.assert_not_called()
            self.assertEqual(metrics.expected_body(record,'old'),['measured counts'])
            self.assertEqual(json.dumps(record),original)

    def test_fingerprint_tracks_code_tests_policy_and_normative_docs_not_generated_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp)
            for name in ['tests/corpus/target/program.lc','docs/diagnostics/L0001.md','examples/demo.lc','editors/highlight/locus.js','src/a.rs','tests/b.rs','library/c.lc','tools/policy.json','website/assets/site.js','docs/roadmap/project.md','Cargo.toml','Cargo.lock']:
                p=root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text('original')
            data={'docs':[{'id':'language','group':'Now','body':['rule']},{'id':'generated-status','body':['count']}, {'id':'readme','group':'Now','body':['checked example']} ]}
            def save():
                (root/'atlas.html').write_text('<script type="application/json" id="atlas-data">\n'+json.dumps(data)+'\n</script>')
            save();first=metrics.fingerprint(root)
            data['docs'][1]['body']=['different count'];save()
            self.assertEqual(first,metrics.fingerprint(root))
            data['docs'][0]['body']=['different rule'];save()
            self.assertNotEqual(first,metrics.fingerprint(root))
            previous=metrics.fingerprint(root)
            (root/'tools/policy.json').write_text('changed policy')
            self.assertNotEqual(previous,metrics.fingerprint(root))
            for path in ('tests/corpus/target/program.lc','docs/diagnostics/L0001.md','examples/demo.lc','editors/highlight/locus.js','website/assets/site.js','docs/roadmap/project.md'):
                previous=metrics.fingerprint(root);(root/path).write_text('changed input')
                self.assertNotEqual(previous,metrics.fingerprint(root))
            previous=metrics.fingerprint(root);data['docs'][2]['body']=['changed example'];save()
            self.assertNotEqual(previous,metrics.fingerprint(root))

    def test_markdown_fingerprint_ignores_receipts_but_tracks_current_manual(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp)
            (root/'docs/data').mkdir(parents=True)
            (root/'docs/data/state.json').write_text('{"meta": {}}')
            manual=root/'docs/manual.md'
            metadata={'id':'language','group':'Now','route':'manual.html'}
            manual.write_text(content.markdown(metadata,'# Manual\n\nCurrent rule.'))
            generated=root/'docs/generated-status.md'
            generated.write_text(content.markdown({'id':'generated-status','group':'Now'},'# Generated status\n\nOld count.'))
            first=metrics.fingerprint(root)
            (root/'docs/data/state.json').write_text('{"meta": {"rev": 8}}')
            generated.write_text(content.markdown({'id':'generated-status','group':'Now'},'# Generated status\n\nNew count.'))
            self.assertEqual(first,metrics.fingerprint(root))
            manual.write_text(content.markdown(metadata,'# Manual\n\nChanged rule.'))
            self.assertNotEqual(first,metrics.fingerprint(root))

    def test_source_changes_during_gate_are_rejected_before_collecting(self):
        with mock.patch.object(metrics,'fingerprint',return_value='new'):
            with self.assertRaisesRegex(ValueError,'source changed'):
                metrics.collect('nonexistent','nonexistent',1,'old')

if __name__=='__main__':unittest.main()
