use locus_stdlib_spike::permission::Region; fn main(){let mut r=Region::<u32>::allocate(1);let p=r.pointer();r.write(p,0,1);let a=r.borrow_mut(p,0);let b=r.borrow_mut(p,0);*a+=*b;}
