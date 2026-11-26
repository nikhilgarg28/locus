use locus_stdlib_spike::sharing::PRc; fn main(){let r=PRc::new(1_u32);std::thread::spawn(move||drop(r));}
