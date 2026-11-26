use locus_stdlib_spike::permission::Region; fn main(){let mut a=Region::<u32>::allocate(1);let b=Region::<u64>::allocate(1);a.write(b.pointer(),0,1);}
