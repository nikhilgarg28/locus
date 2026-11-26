use locus_stdlib_spike::interior::PMutex; fn main(){let m=PMutex::new(1_u32);let g=m.lock().unwrap_or_else(|e|e.into_inner());std::thread::scope(|s|{s.spawn(move||drop(g));});}
