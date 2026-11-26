use locus_stdlib_spike::permission::Region; fn main(){let r=Region::<u32>::allocate(1);drop(r);let _=r.pointer();}
