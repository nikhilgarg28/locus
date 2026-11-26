use locus_stdlib_spike::permission::Region; fn main(){let r=Region::<u32>::allocate(1);let a=r;let b=r;drop((a,b));}
