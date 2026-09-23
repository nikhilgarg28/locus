#!/usr/bin/env python3
"""Generate Atlas status exclusively from successful gate output and raw samples."""
import argparse
import hashlib
import json
import pathlib
import re
import sys
import datetime
import atlas
import bench
import spec

ROOT=pathlib.Path(__file__).resolve().parent.parent


def fingerprint(root=ROOT):
    h=hashlib.sha256()
    paths=[]
    for directory in ('src','tests','library','tools','examples','editors','docs'):
        paths.extend(path for path in (root/directory).rglob('*') if path.is_file() and not set(path.parts) & {'__pycache__','node_modules','.git'})
    paths += [root/name for name in ('Cargo.toml','Cargo.lock','build.rs') if (root/name).exists()]
    for path in sorted(paths):
        h.update(str(path.relative_to(root)).encode()+b'\0'+path.read_bytes()+b'\0')
    data=spec.load(root/'atlas.html')
    for doc in data['docs']:
        if doc['id'] != 'generated-status' and (doc.get('group') == 'Now' or doc['id'] in ('language','kernel-contract','architecture','formal-core')):
            h.update(json.dumps(doc['body'],sort_keys=True).encode())
    return h.hexdigest()


def test_counts(text, mode):
    found=re.findall(r'^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',text,re.M)
    if f'LOCUS TEST RUN COMPLETE: {mode}' not in text.splitlines() or not found or re.search(r'^test result: FAILED|^error: test failed',text,re.M):
        raise ValueError('a complete successful test log is required')
    return {'passed':sum(int(a) for a,_,_ in found),'ignored':sum(int(c) for _,_,c in found),'targets':len(found)}


def enum_names(name):
    source=(ROOT/'src/kernel/term.rs').read_text()
    body=re.search(r'pub enum '+name+r'\s*\{(.*?)^\}',source,re.S|re.M).group(1)
    return re.findall(r'^    ([A-Z]\w*)\b',body,re.M)


def expected_body(record, current_fingerprint):
    if not record or record['source_fingerprint'] != current_fingerprint:
        return ['# Generated status', '', '**STALE — current measurements are unavailable.**', '',
                'Run `tools/check.sh --extended` to measure this source and regenerate status. Previous raw records remain in the benchmark data branch; obsolete counts are not displayed as current results.']
    return render(record)


def invalidate():
    text, match, data = atlas.load()
    record = data.get('measurements')
    current = fingerprint()
    if record and record['source_fingerprint'] == current:
        return
    doc = next((d for d in data['docs'] if d['id'] == 'generated-status'), None)
    body = expected_body(record, current)
    if doc is None:
        data['docs'].append({'id':'generated-status','title':'Generated status','group':'Now','body':body,'created':atlas.now()})
    elif doc['body'] != body:
        doc['body'] = body; doc['updated'] = atlas.now()
    else:
        return
    atlas.store(text, match, data)


def collect(fast_log,extended_log,seconds,expected_fingerprint):
    if expected_fingerprint != fingerprint():
        raise ValueError("source changed during the gate; rerun against one stable checkout")
    data=spec.load(ROOT/'atlas.html')
    trace=spec.validate(spec.inventory(data),spec.citations(ROOT))
    manifest=json.loads((ROOT/'tools/trusted-base.json').read_text())
    trusted=[]
    for entry in manifest['files']:
        trusted.append({**entry,'physical_lines':len((ROOT/entry['path']).read_text().splitlines())})
    history=bench.gate_history(bench.records())
    if not history:raise ValueError('benchmark samples must be recorded before status generation')
    latest=history[-1]
    tiers={};hits={}
    for sample in latest['samples']:
        if sample['sample']!=0:continue
        if sample['pass']=='search':
            counts={}
            for proof in sample['obligations']:counts[proof['tier']]=counts.get(proof['tier'],0)+1
            tiers[sample['workload']]=counts
        else:hits[sample['workload']]=sample['store_hits']
    return {'schema_version':1,'generated_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'source_fingerprint':fingerprint(),'fast_seconds':seconds,'fast_limit_seconds':120,
            'fast_tests':test_counts(pathlib.Path(fast_log).read_text(),'fast'),
            'extended_tests':test_counts(pathlib.Path(extended_log).read_text(),'extended'),
            'corpus_files':len(list((ROOT/'tests/corpus').rglob('*.lc'))),
            'proof_tiers':tiers,'stored_proof_hits':hits,'kernel_rules':enum_names('Proof'),
            'kernel_axioms':enum_names('Axiom'),'trusted_base':trusted,'specification':trace,
            'benchmark':bench.summary(history),'benchmark_record_sha256':bench.digest(bench.canonical(latest))}


def render(record):
    lines=['# Generated status','',
           'Generated from a successful complete gate. Source edits invalidate this record until the extended gate refreshes it. Counts are measurements, not promises.', '',
           f"Measured at {record['generated_at']}; source fingerprint `{record['source_fingerprint']}`.",'',
           '| Measurement | Value |','|---|---|',
           f"| Fast tests passed | {record['fast_tests']['passed']} |",
           f"| Extended tests passed | {record['extended_tests']['passed']} |",
           f"| Corpus files | {record['corpus_files']} |",
           f"| Fast suite / advisory target | {record['fast_seconds']}s / {record['fast_limit_seconds']}s |",
           f"| Kernel proof constructors / axiom constructors | {len(record['kernel_rules'])} / {len(record['kernel_axioms'])} |",
           f"| Operative paragraphs / citations | {record['specification']['normative']} / {record['specification']['citations']} |",'',
           '## Target proof measurements','','| Workload | Search tiers | Locked replay hits |','|---|---|---|']
    for name,tiers in sorted(record['proof_tiers'].items()):
        lines.append(f"| {name} | {', '.join(f'{k}: {v}' for k,v in sorted(tiers.items()))} | {record['stored_proof_hits'].get(name,0)} |")
    lines += ['', '## Trusted-base source inventory','',
              'Physical source lines, including comments and blanks. Mixed files are counted whole; this conservative count is distinct from the proof-kernel size.', '',
              '| File | Lines | Scope |','|---|---|---|']
    for item in record['trusted_base']:lines.append(f"| {item['path']} | {item['physical_lines']} | {item['category']} |")
    measured=record['benchmark']
    lines += ['','## Benchmark history','',f"Epoch `{measured['epoch']}`; {measured['records']} real records; {measured['observed_days']:.2f} observed days.",
              '',f"Headline index: {measured['headline_index']:.2f} (first record = 100; lower is faster).",'',
              'The two-week observation window is complete.' if measured['two_week_window_complete'] else 'The two-week observation window is still open; no historical samples are fabricated.', '',
              '| Workload | Median history | Latest assessment |','|---|---|---|']
    for item in measured['workloads']:lines.append(f"| {item['name']} | {item['sparkline']} | {item['comparison']['status']} |")
    return lines


def publish(record):
    text,match,data=atlas.load()
    data['measurements']=record
    doc=next((doc for doc in data['docs'] if doc['id']=='generated-status'),None)
    if doc is None:
        doc={'id':'generated-status','title':'Generated status','group':'Now','body':[],'created':atlas.now()}
        data['docs'].append(doc)
    doc['body']=render(record);doc['updated']=atlas.now()
    atlas.store(text,match,data)
    path=ROOT/'target/status.json';path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(record,indent=2,sort_keys=True)+'\n')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command',choices=['collect','check','invalidate','fingerprint'])
    parser.add_argument('--source-fingerprint');parser.add_argument('--fast-log');parser.add_argument('--extended-log');parser.add_argument('--fast-seconds',type=int)
    parser.add_argument('--fresh',action='store_true',help='require a current complete measurement, rather than a clearly stale display')
    args=parser.parse_args()
    if args.command=='fingerprint':
        print(fingerprint())
    elif args.command=='invalidate':
        invalidate()
    elif args.command=='collect':
        record=collect(args.fast_log,args.extended_log,args.fast_seconds,args.source_fingerprint)
        publish(record);print(json.dumps(record,sort_keys=True))
    else:
        data=spec.load(ROOT/'atlas.html');record=data.get('measurements')
        current=fingerprint()
        if args.fresh and (not record or record['source_fingerprint']!=current):
            sys.exit('generated status is absent or stale; run tools/check.sh --extended')
        body=next(d['body'] for d in data['docs'] if d['id']=='generated-status')
        if body!=expected_body(record,current):sys.exit('generated status was edited by hand; regenerate it')
        print('generated status matches its measurements or explicitly hides stale counts')

if __name__=='__main__':main()
