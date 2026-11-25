use locus_stdlib_spike::permission::Region; fn main(){let mut r=Region::<u32>::allocate(1);let p=r.pointer();r.write(p,0,1);let a=r.read(p,0);let b=r.take(p,0);println!("{} {}",a,b);}
