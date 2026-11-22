//! expect: runtime=E0276 check=E0276
//! As d2b with the bound written on the implementation's method: rustc
//! refuses a requirement that the trait does not have.
use ::locus::locus;
locus! {
    struct Bag { items: Vec<u32> }
    impl Extend<u32> for Bag {
        fn extend<I: IntoIterator<Item = u32> + Model>(&mut self, iter: I) {
            let before = logic! { snap(iter) };
            for x in iter { self.items.push(x); }
        }
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
