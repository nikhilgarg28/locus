//! expect: runtime=E0596 check=E0596
//! Only the wrapper's own methods change the vector: a standard method that
//! takes `&mut self` is not reached through the shared `Deref`.
use ::locus::locus;
locus! {
    use ::locus::Vec;
    fn run() -> usize {
        let mut v: Vec<u32> = Vec::new();
        let _ = v.push(1);
        v.truncate(0);
        v.iter().count()
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
