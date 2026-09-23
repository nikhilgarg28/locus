#!/usr/bin/env python3
"""Raw benchmark records on an append-only Git data branch; robust comparisons."""
import argparse
import datetime
import hashlib
import json
import math
import os
import pathlib
import platform
import statistics
import subprocess
import sys

BRANCH = 'refs/heads/locus-bench-data'
WINDOW = 10
STALE_COMMITS = 5
K = 6


def git(*args, input=None, check=True):
    result = subprocess.run(['git', *args], input=input, capture_output=True)
    if check and result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace'))
    return result.stdout.decode().strip()


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False)+'\n').encode()


def digest(value):
    return hashlib.sha256(value).hexdigest()


def median_mad(values):
    middle = statistics.median(values)
    return middle, 1.4826 * statistics.median(abs(x-middle) for x in values)


def comparison(current, previous, k=K):
    """Flag a slowdown beyond k pooled robust dispersions, never a percentage."""
    if len(current) < 3 or len(previous) < 3:
        return {'status':'insufficient_samples'}
    now, spread_now = median_mad(current)
    old, spread_old = median_mad(previous)
    pooled = math.sqrt((spread_now**2 + spread_old**2) / 2)
    return {'status':'slowdown' if now-old > k*pooled else 'stable',
            'median_ns':now, 'baseline_median_ns':old, 'pooled_dispersion_ns':pooled, 'k':k}


def records(ref=BRANCH):
    if not git('rev-parse','--verify',ref,check=False):
        return []
    names = git('ls-tree','-r','--name-only',ref).splitlines()
    result=[]
    for name in names:
        if name.startswith('records/') and name.endswith('.json'):
            raw=git('show',f'{ref}:{name}')+'\n'
            if digest(raw.encode()) != pathlib.PurePosixPath(name).stem:
                raise RuntimeError(f'benchmark content hash mismatch: {name}')
            result.append(json.loads(raw))
    return sorted(result,key=lambda item:item['recorded_at'])


def append_record(record):
    raw=canonical(record); identity=digest(raw)
    blob=git('hash-object','-w','--stdin',input=raw)
    old=git('rev-parse','--verify',BRANCH,check=False)
    existing={}
    if old:
        for line in git('ls-tree',f'{old}:records').splitlines():
            metadata,name=line.split('\t',1); existing[name]=metadata
    name=f'{identity}.json'
    if name in existing: return identity
    existing[name]=f'100644 blob {blob}'
    tree=git('mktree',input=(''.join(f'{meta}\t{name}\n' for name,meta in sorted(existing.items()))).encode())
    roots={}
    if old:
        for line in git('ls-tree',old).splitlines():
            metadata,name=line.split('\t',1); roots[name]=metadata
    roots['records']=f'040000 tree {tree}'
    root=git('mktree',input=(''.join(f'{meta}\t{name}\n' for name,meta in sorted(roots.items()))).encode())
    args=['-c','user.name=Locus benchmark recorder','-c','user.email=locus-bench@localhost','commit-tree',root]
    if old: args += ['-p',old]
    commit=git(*args,input=f'Append benchmark {identity}\n'.encode())
    git('update-ref',BRANCH,commit,old or '0'*40)
    return identity


def machine_class():
    model=platform.processor()
    if platform.system()=='Darwin':
        result=subprocess.run(['sysctl','-n','machdep.cpu.brand_string'],capture_output=True,text=True)
        if result.returncode==0:model=result.stdout.strip()
    elif pathlib.Path('/proc/cpuinfo').exists():
        for line in pathlib.Path('/proc/cpuinfo').read_text().splitlines():
            if line.startswith('model name'):model=line.split(':',1)[1].strip();break
    return {'system':platform.system(),'release':platform.release(),'architecture':platform.machine(),
            'processor':model,'class_override':os.environ.get('LOCUS_BENCH_MACHINE_CLASS','')}


def gate_history(history, root=None):
    """The gate requires the current target corpus, not an unrelated fresh run."""
    root=root or pathlib.Path(git('rev-parse','--show-toplevel'))
    paths=sorted((root/'tests/corpus/target').rglob('*.lc'))
    workloads=[{'path':str(path.relative_to(root)),'sha256':digest(path.read_bytes())} for path in paths]
    machine=machine_class()
    toolchain=subprocess.check_output(['rustc','-Vv'],text=True).strip()
    return [record for record in history
            if record['pins']['workloads']==workloads and record['pins']['libraries']==[]
            and record['pins']['machine_class']==machine
            and record['pins']['compiler_build']['toolchain']==toolchain
            and record['pins']['compiler_build']['profile']=='release'
            and record['pins']['configuration']=={'check_moves':True,'previews':[]}]


def enrich(raw):
    root=pathlib.Path(git('rev-parse','--show-toplevel'))
    def path_name(name):
        path=pathlib.Path(name).resolve()
        return str(path.relative_to(root)) if path.is_relative_to(root) else str(path)
    def pin(name):
        return {'path':name,'sha256':digest((root/name).read_bytes())}
    # Roles and library order affect elaboration. Equivalent absolute/relative
    # spellings must still join the same series and workload keys.
    raw={**raw,'workloads':[path_name(name) for name in raw['workloads']],
         'libraries':[path_name(name) for name in raw['libraries']],
         'samples':[{**sample,'workload':path_name(sample['workload'])} for sample in raw['samples']]}
    workloads=[pin(name) for name in raw['workloads']]
    libraries=[pin(name) for name in raw['libraries']]
    build=raw.get('compiler_build',{})
    settings={key:value for key,value in build.items() if key!='source_git_blob'}
    pins={'workloads':workloads,'libraries':libraries,'machine_class':machine_class(),
          'compiler_build':settings,'schema_version':raw['schema_version'],'configuration':raw.get('configuration',{})}
    record={**raw,'pins':pins,'epoch':digest(canonical(pins)),
            'recorded_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'source_commit':git('rev-parse','HEAD'),'dirty':bool(git('status','--porcelain')),
            'compiler_sha256':digest(pathlib.Path(raw['executable']).read_bytes()) if raw.get('executable') else None}
    history=root/'target/gate-history.jsonl'
    record['fast_suite_seconds']=int(os.environ['LOCUS_FAST_SECONDS']) if 'LOCUS_FAST_SECONDS' in os.environ else (json.loads(history.read_text().splitlines()[-1])['fast_seconds'] if history.exists() and history.read_text().strip() else None)
    prior=[r for r in records() if r['epoch']==record['epoch']][-WINDOW:]
    flags={}
    for name in raw['workloads']:
        current=[s['total_ns'] for s in raw['samples'] if s['workload']==name and s['pass']=='search']
        previous=[s['total_ns'] for r in prior for s in r['samples'] if s['workload']==name and s['pass']=='search']
        flags[name]=comparison(current,previous)
    record['comparisons']=flags
    return record


def freshness(history, head=None):
    if not history: return {'status':'missing','commits':None}
    head=head or git('rev-parse','HEAD')
    latest=history[-1]
    distance=git('rev-list','--count',f"{latest['source_commit']}..{head}")
    count=int(distance)
    ancestor=subprocess.run(['git','merge-base','--is-ancestor',latest['source_commit'],head],capture_output=True).returncode==0
    if not ancestor:return {'status':'diverged','commits':count}
    return {'status':'stale' if count>STALE_COMMITS else 'current','commits':count}


def summary(history):
    if not history: return {'status':'awaiting_records','workloads':[]}
    latest=history[-1]; epoch=latest['epoch']
    series=[r for r in history if r['epoch']==epoch]
    workloads=[]; ratios=[]
    for name in latest['workloads']:
        medians=[]
        for record in series:
            values=[s['total_ns'] for s in record['samples'] if s['workload']==name and s['pass']=='search']
            if values:medians.append(statistics.median(values))
        if not medians:continue
        ratios.append(medians[-1]/max(1,medians[0]))
        low,high=min(medians),max(medians)
        spark=''.join('▁▂▃▄▅▆▇█'[round(7*(v-low)/(high-low)) if high>low else 0] for v in medians[-30:])
        workloads.append({'name':name,'medians_ns':medians,'sparkline':spark,'comparison':latest['comparisons'][name]})
    start=datetime.datetime.fromisoformat(series[0]['recorded_at']); end=datetime.datetime.fromisoformat(series[-1]['recorded_at'])
    return {'epoch':epoch,'records':len(series),'observed_days':(end-start).total_seconds()/86400,
            'two_week_window_complete':(end-start).total_seconds()>=14*86400,
            'headline_index':100*math.exp(sum(math.log(r) for r in ratios)/len(ratios)) if ratios else None,
            'freshness':freshness(history),'workloads':workloads}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command',choices=['record','summary','check'])
    args=parser.parse_args()
    if args.command=='record':
        record=enrich(json.load(sys.stdin)); identity=append_record(record)
        print(json.dumps({'record_sha256':identity,'branch':BRANCH, 'record':record},sort_keys=True))
    elif args.command=='summary':print(json.dumps(summary(records()),sort_keys=True))
    else:
        state=freshness(gate_history(records())); print(json.dumps(state,sort_keys=True))
        if state['status']!='current':sys.exit(1)


if __name__=='__main__':
    main()
