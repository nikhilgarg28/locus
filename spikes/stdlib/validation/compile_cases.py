"""Rust representation/boundary probes, not Locus rejection tests."""
from pathlib import Path
import subprocess,os,json,shutil
root=Path(__file__).resolve().parents[1]
tool=Path('/workspace/locus-review/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin')
env=dict(os.environ)
if not shutil.which('cargo') and tool.exists():
    env['PATH']=str(tool)+':'+env.get('PATH','')
subprocess.run(['cargo','build','--offline'],cwd=root,env=env,check=True)
libs=list((root/'target').rglob('liblocus_stdlib_spike*.rlib'))
lib=max(libs,key=lambda p:p.stat().st_mtime)
cases={
 'copy_permission':('E0382','use locus_stdlib_spike::permission::Region; fn main(){let r=Region::<u32>::allocate(1);let a=r;let b=r;drop((a,b));}'),
 'use_after_free':('E0382','use locus_stdlib_spike::permission::Region; fn main(){let r=Region::<u32>::allocate(1);drop(r);let _=r.pointer();}'),
 'two_mutable_loans':('E0499','use locus_stdlib_spike::permission::Region; fn main(){let mut r=Region::<u32>::allocate(1);let p=r.pointer();r.write(p,0,1);let a=r.borrow_mut(p,0);let b=r.borrow_mut(p,0);*a+=*b;}'),
 'write_during_shared_loan':('E0502','use locus_stdlib_spike::permission::Region; fn main(){let mut r=Region::<u32>::allocate(1);let p=r.pointer();r.write(p,0,1);let a=r.read(p,0);let b=r.take(p,0);println!("{} {}",a,b);}'),
 'rc_is_not_send':('E0277','use locus_stdlib_spike::sharing::PRc; fn main(){let r=PRc::new(1_u32);std::thread::spawn(move||drop(r));}'),
 'arc_of_cell_is_not_send':('E0277','use locus_stdlib_spike::{sharing::PArc,interior::PCell}; fn main(){let r=PArc::new(PCell::new(1_u32));std::thread::spawn(move||drop(r));}'),
 'mutex_guard_not_send':('E0277','use locus_stdlib_spike::interior::PMutex; fn main(){let m=PMutex::new(1_u32);let g=m.lock().unwrap_or_else(|e|e.into_inner());std::thread::scope(|s|{s.spawn(move||drop(g));});}'),
 'native_handoff_reference_escape':('lifetime','use locus_stdlib_spike::wrappers::WVec; fn main(){let mut w=WVec::from_std(vec![1_u32]);let r=w.with_native(|v| &mut v[0]);*r=2;}'),
 'wrong_element_permission':('E0308','use locus_stdlib_spike::permission::Region; fn main(){let mut a=Region::<u32>::allocate(1);let b=Region::<u64>::allocate(1);a.write(b.pointer(),0,1);}'),
}
directory=root/'validation'/'compile_cases';directory.mkdir(exist_ok=True)
results=[]
for name,(expected,source) in cases.items():
    path=directory/(name+'.rs');path.write_text(source+'\n')
    p=subprocess.run(['rustc','--edition=2024',str(path),'--extern',f'locus_stdlib_spike={lib}',
        '-L',f'dependency={lib.parent}','--emit=metadata','-o',str(directory/(name+'.rmeta'))],env=env,capture_output=True,text=True)
    assert p.returncode!=0 and expected in p.stderr,(name,p.stderr)
    (directory/(name+'.stderr')).write_text(p.stderr)
    results.append({'case':name,'expected':expected,'rejected':True})
(root/'validation'/'compile_results.json').write_text(json.dumps(results,indent=2)+'\n')
print(json.dumps({'rust_compile_rejections':len(results),'all_expected':True}))
