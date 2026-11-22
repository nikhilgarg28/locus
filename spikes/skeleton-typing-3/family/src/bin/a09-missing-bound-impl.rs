//! expect: runtime=E0277 check=E0277
//! The same in an `impl`: a contract reads a field whose type names `T`.
use ::locus::locus;
locus! {
    struct Shelf<T> { items: Vec<T>, limit: usize }
    impl<T> Shelf<T> {
        fn count(&self) -> (n: usize, @(n == self.items.len())) { (self.items.len(), _) }
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
