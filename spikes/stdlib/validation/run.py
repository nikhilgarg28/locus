"""Reproduce executable checks; none of these commands typechecks Locus source."""
from pathlib import Path
import json, os, re, shutil, subprocess, sys, time

root = Path(__file__).resolve().parents[1]
env = dict(os.environ)
fallback = Path('/workspace/locus-review/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin')
if not shutil.which('cargo') and fallback.exists():
    env['PATH'] = str(fallback) + ':' + env.get('PATH', '')
logs = root / 'validation' / 'logs'
logs.mkdir(exist_ok=True)
version = subprocess.check_output(['rustc', '--version'], env=env, text=True).strip()
commands = [
    ('format', ['cargo', 'fmt', '--all', '--', '--check']),
    ('debug_audit', ['cargo', 'test', '--offline']),
    ('debug_erased_shadows', ['cargo', 'test', '--offline', '--no-default-features']),
    ('release_audit', ['cargo', 'test', '--offline', '--release']),
    ('compile_rejections', [sys.executable, 'validation/compile_cases.py']),
]
results = []
for name, cmd in commands:
    start = time.monotonic()
    p = subprocess.run(cmd, cwd=root, env=env, capture_output=True, text=True)
    output = p.stdout + p.stderr
    (logs / (name + '.txt')).write_text(output)
    result = dict(name=name, command=cmd, exit_code=p.returncode,
        seconds=round(time.monotonic()-start, 3),
        tests_passed=sum(map(int, re.findall(r'test result: ok\. (\d+) passed', output))))
    results.append(result)
    print(json.dumps(result), flush=True)
    if p.returncode: print(output, file=sys.stderr)
(root / 'validation' / 'results.json').write_text(json.dumps(dict(rustc=version, runs=results), indent=2)+'\n')
sys.exit(any(r['exit_code'] for r in results))
