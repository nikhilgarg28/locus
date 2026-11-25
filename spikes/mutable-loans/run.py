#!/usr/bin/env python3
"""Reproduce the Rust, process, compile-boundary and SMT experiments."""
import argparse
import json
import os
from pathlib import Path
import resource
import shutil
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parent
TARGET = ROOT/'target'
RESULTS = ROOT/'results'
TARGET.mkdir(exist_ok=True)
RESULTS.mkdir(exist_ok=True)
parser = argparse.ArgumentParser()
parser.add_argument('--rustc', default=shutil.which('rustc') or
                    '/workspace/locus-review/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc')
parser.add_argument('--asan', action='store_true', help='Also run the Rust tests and scenarios under ASan')
args = parser.parse_args()
records = []

def no_core():
    resource.setrlimit(resource.RLIMIT_CORE, (0,0))

def run(name, command, expected=0, tokens=(), forbidden=(), env=None, timeout=60):
    proc = subprocess.run([str(x) for x in command], cwd=ROOT, capture_output=True,
                          text=True, env=env, timeout=timeout, preexec_fn=no_core)
    text = proc.stdout + proc.stderr
    (RESULTS/(name+'.log')).write_text(text)
    passed = proc.returncode == expected and all(t in text for t in tokens) and all(t not in text for t in forbidden)
    records.append(dict(name=name, command=[str(x) for x in command], code=proc.returncode,
                        expected_code=expected, passed=passed))
    print(f'{name}: {"PASS" if passed else "FAIL"} (exit {proc.returncode})')
    if not passed:
        print(text)
        (RESULTS/'runs.json').write_text(json.dumps(records,indent=2)+'\n')
        raise SystemExit(1)
    return proc

run('rustc-version',[args.rustc,'--version','--verbose'])
scenarios = {
    'success': (0,['SUCCESS','OWNER_DROP_VALID=true'],[]),
    'forget_reference': (0,['NO_REPAIR_DROP_NEEDED','OWNER_DROP_VALID=true'],[]),
    'forget_repair_guard': (0,['COUNTEREXAMPLE_FORGET','OWNER_DROP_VALID=false'],[]),
    'panic_acquire_changed': (-signal.SIGABRT,['ABORT_ACQUISITION'],['OWNER_DROP']),
    'panic_acquire_unchanged': (-signal.SIGABRT,['ABORT_ACQUISITION'],['OWNER_DROP']),
    'callee_cleanup': (-signal.SIGABRT,['CALLEE_LOCAL_DROP\nABORT_ACQUISITION'],['OWNER_DROP']),
    'panic_after_return': (0,['CAUGHT_AFTER_RETURN','OWNER_DROP_VALID=true'],['ABORT_ACQUISITION']),
    'panic_argument_outside': (0,['CAUGHT_ARGUMENT','OWNER_DROP_VALID=true'],['ABORT_ACQUISITION']),
    'panic_argument_broad_guard': (-signal.SIGABRT,['ABORT_ACQUISITION'],['OWNER_DROP']),
    'result_drop': (0,['CAUGHT_RESULT_DROP','OWNER_DROP_VALID=true'],['ABORT_ACQUISITION']),
    'deferred_closure': (0,['COUNTEREXAMPLE_DEFERRED_CLOSURE','OWNER_DROP_VALID=false'],['ABORT_ACQUISITION']),
    'future_cancellation': (0,['COUNTEREXAMPLE_CANCELLATION','OWNER_DROP_VALID=false'],['ABORT_ACQUISITION']),
    'two_phase': (0,['TWO_PHASE_METHOD_SHIM'],[]),
    'named_acquisition': (0,['NAMED_ACQUISITION','OWNER_DROP_VALID=true'],[]),
    'named_argument_panic': (0,['CAUGHT_NAMED_ARGUMENT','OWNER_DROP_VALID=true'],['ABORT_ACQUISITION']),
    'native_panic_acquire': (-signal.SIGABRT,['ABORT_ACQUISITION'],['OWNER_DROP']),
    'native_panic_after_return': (0,['CAUGHT_NATIVE_AFTER_RETURN','NATIVE_OWNER_DROP_VALID=true'],['ABORT_ACQUISITION']),
}

variants = [('debug',[],None),('optimized',['-O'],None)]
if args.asan:
    asan_env = dict(os.environ, RUSTC_BOOTSTRAP='1', ASAN_OPTIONS='detect_leaks=1:abort_on_error=1')
    variants.append(('asan',['-Zsanitizer=address','-Cdebuginfo=1','-Copt-level=1'],asan_env))
for variant, flags, env in variants:
    common = [args.rustc,'--edition=2024','-Dwarnings','-Cpanic=unwind',*flags]
    unit = TARGET/('unit-'+variant)
    exe = TARGET/('probes-'+variant)
    run('build-unit-'+variant,[*common,'--test','rust/probes.rs','-o',unit],env=env)
    run('unit-'+variant,[unit],tokens=['15 passed; 0 failed'],env=env)
    run('build-probes-'+variant,[*common,'rust/probes.rs','-o',exe],env=env)
    for name,(code,tokens,forbidden) in scenarios.items():
        run(variant+'-'+name,[exe,name],code,tokens,forbidden,env)

failures = {
    'owner_read_during_loan':'E0502',
    'parent_write_with_live_child':'E0506',
    'named_borrow_then_owner_argument':'E0502',
    'free_function_argument_order':'E0502',
    'guard_local_reference':'E0713',
    'naive_final_reads_suspended':'E0502',
}
for name,code in failures.items():
    run('reject-'+name,[args.rustc,'--edition=2024',ROOT/'rust'/'compile_fail'/(name+'.rs'),
                       '-o',TARGET/name],1,[code])
for source in sorted((ROOT/'rust'/'compile_pass').glob('*.rs')):
    exe = TARGET/source.stem
    run('accept-'+source.stem,[args.rustc,'--edition=2024',source,'-o',exe])
    run('execute-'+source.stem,[exe])

py_env = dict(os.environ, PYTHONPATH=str(ROOT/'.deps')+os.pathsep+os.environ.get('PYTHONPATH',''))
run('smt-vcs',[sys.executable,'logic/check_vcs.py'],tokens=['VC SUMMARY: 17 expected outcomes'],env=py_env)
run('emit-mir',[args.rustc,'--edition=2024','--emit=mir','rust/probes.rs','-o',TARGET/'probes.mir'])
run('emit-cost-assembly',[args.rustc,'--edition=2024','--crate-type=lib','-O','--emit=asm',
                          'rust/cost.rs','-o',TARGET/'cost.s'])
(RESULTS/'runs.json').write_text(json.dumps(records,indent=2)+'\n')
summary = {'rust_unit_tests_per_variant':15, 'runtime_scenarios_per_variant':len(scenarios),
           'variants':[v[0] for v in variants], 'rust_compile_rejections':len(failures),
           'rust_compile_acceptances':len(list((ROOT/'rust'/'compile_pass').glob('*.rs'))),
           'smt_queries':17, 'locus_programs_compiled':0, 'all_commands_matched_expectations':True}
(RESULTS/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
