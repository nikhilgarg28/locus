"""Check future crate paths and module wiring, not Locus parsing or proofs."""
from pathlib import Path
import re,tomllib
root=Path(__file__).resolve().parent
count=0
for crate in ('stdlib','mutable-loans'):
    directory=root/crate
    data=tomllib.loads((directory/'Cargo.toml').read_text())
    lib=directory/data['lib']['path']
    assert lib.is_file(),lib
    modules=re.findall(r'^pub mod (\w+);$',lib.read_text(),re.M)
    for name in modules:
        assert (lib.parent/(name+'.lc')).is_file(),name
    known=set(modules)
    for source in lib.parent.glob('*.lc'):
        for name in re.findall(r'crate::(\w+)',source.read_text()):
            assert name in known,(source,name)
    for target in data.get('bin',[]):
        assert (directory/target['path']).is_file(),target
    count+=len(modules)
print(f'2 future crate layouts; {count} declared modules; no Locus compilation performed')
