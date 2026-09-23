import contextlib
import json
import os
import pathlib
import subprocess
import tempfile
import unittest
import bench

@contextlib.contextmanager
def repository():
    with tempfile.TemporaryDirectory(prefix='locus-bench-test-') as directory:
        old=os.getcwd();os.chdir(directory)
        try:
            subprocess.run(['git','init','-q'],check=True)
            bench.git('config','user.name','Benchmark tests');bench.git('config','user.email','bench@localhost')
            pathlib.Path('input.lc').write_text('fn run()->u8 {7}\n')
            bench.git('add','input.lc');bench.git('commit','-qm','workload')
            yield pathlib.Path(directory)
        finally:os.chdir(old)

class Bench(unittest.TestCase):
    def test_deliberate_slowdown_and_noise(self):
        self.assertEqual(bench.comparison([200,201,199,200,202],[100,101,99,102,100])['status'],'slowdown')
        self.assertEqual(bench.comparison([100,103,98,101,97],[100,101,99,102,98])['status'],'stable')
        self.assertEqual(bench.comparison([200,200,200],[100,100,100])['status'],'slowdown')
        self.assertEqual(bench.comparison([100],[90,90,90])['status'],'insufficient_samples')

    def test_data_branch_is_immutable_and_leaves_source_checkout_alone(self):
        with repository():
            head=bench.git('rev-parse','HEAD');status=bench.git('status','--porcelain')
            a={'recorded_at':'2026-01-01T00:00:00+00:00','source_commit':head,'samples':[1]}
            aid=bench.append_record(a)
            original=bench.git('show',f'{bench.BRANCH}:records/{aid}.json')
            self.assertEqual(bench.append_record(a),aid)
            b={**a,'recorded_at':'2026-01-02T00:00:00+00:00','samples':[2]}
            bid=bench.append_record(b)
            self.assertNotEqual(aid,bid)
            self.assertEqual(bench.records(),[a,b])
            self.assertEqual(bench.git('show',f'{bench.BRANCH}:records/{aid}.json'),original)
            self.assertEqual(bench.git('rev-parse','HEAD'),head)
            self.assertEqual(bench.git('status','--porcelain'),status)
            self.assertEqual(bench.git('rev-list','--count',bench.BRANCH),'2')

    def test_epoch_pins_roles_library_order_and_canonical_workload_names(self):
        with repository() as root:
            for name in ('a.lc','b.lc'):pathlib.Path(name).write_text('logic fn x()->Int {0}')
            raw={'schema_version':1,'workloads':['input.lc'],'libraries':['a.lc','b.lc'],
                 'samples':[{'workload':'input.lc','pass':'search','total_ns':100} for _ in range(3)]}
            first=bench.enrich(raw);bench.append_record(first)
            absolute={**raw,'workloads':[str(root/'input.lc')],
                      'samples':[{**s,'workload':str(root/'input.lc')} for s in raw['samples']]}
            same=bench.enrich(absolute)
            self.assertEqual(first['epoch'],same['epoch'])
            self.assertEqual(same['comparisons']['input.lc']['status'],'stable')
            swapped={**raw,'workloads':['a.lc'],'libraries':['input.lc','b.lc'],'samples':[]}
            self.assertNotEqual(first['epoch'],bench.enrich(swapped)['epoch'])
            reversed_libs={**raw,'libraries':['b.lc','a.lc']}
            self.assertNotEqual(first['epoch'],bench.enrich(reversed_libs)['epoch'])
            changed_options={**raw,'configuration':{'check_moves':False}}
            self.assertNotEqual(first['epoch'],bench.enrich(changed_options)['epoch'])

    def test_unrelated_recent_record_cannot_refresh_required_target_series(self):
        with repository() as root:
            target=root/'tests/corpus/target';target.mkdir(parents=True)
            (target/'program.lc').write_text('fn run()->u8 {0}')
            build={'toolchain':subprocess.check_output(['rustc','-Vv'],text=True).strip(),'profile':'release','source_git_blob':'a'}
            raw={'schema_version':1,'workloads':['tests/corpus/target/program.lc'],'libraries':[], 'samples':[],
                 'compiler_build':build,'configuration':{'check_moves':True,'previews':[]}}
            target_record=bench.enrich(raw)
            other=bench.enrich({**raw,'workloads':['input.lc']})
            self.assertEqual(bench.gate_history([target_record,other]),[target_record])
            (target/'program.lc').write_text('fn run()->u8 {1}')
            self.assertEqual(bench.gate_history([target_record,other]),[])
            other_build=bench.enrich({**raw,'compiler_build':{**build,'toolchain':'different'}})
            self.assertEqual(bench.gate_history([other_build]),[])

    def test_epoch_changes_on_pinned_input_and_stale_series_fails(self):
        with repository():
            raw={'schema_version':1,'workloads':['input.lc'],'libraries':[], 'samples':[]}
            first=bench.enrich(raw)
            self.assertEqual(first['epoch'],bench.enrich(raw)['epoch'])
            pathlib.Path('input.lc').write_text('fn run()->u8 {8}\n')
            self.assertNotEqual(first['epoch'],bench.enrich(raw)['epoch'])
            altered=dict(first['pins']);altered['compiler_build']={'toolchain':'different toolchain'}
            self.assertNotEqual(first['epoch'],bench.digest(bench.canonical(altered)))
            altered=dict(first['pins']);altered['machine_class']={'architecture':'different'}
            self.assertNotEqual(first['epoch'],bench.digest(bench.canonical(altered)))
            for i in range(bench.STALE_COMMITS+1):
                pathlib.Path('input.lc').write_text(str(i));bench.git('add','input.lc');bench.git('commit','-qm',f'change{i}')
            self.assertEqual(bench.freshness([first])['status'],'stale')
            self.assertEqual(bench.freshness([])['status'],'missing')

if __name__=='__main__':unittest.main()
