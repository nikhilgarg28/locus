use locus_stdlib_spike::{sharing::PArc,interior::PCell}; fn main(){let r=PArc::new(PCell::new(1_u32));std::thread::spawn(move||drop(r));}
