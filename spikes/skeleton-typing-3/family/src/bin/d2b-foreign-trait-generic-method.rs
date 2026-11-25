//! expect: runtime=E0277 check=E0277
//! Required case 2, a foreign trait's generic method: `Extend::extend<I>`.
//! The implementation cannot add `I: Model`, so a value of the method's own
//! type parameter cannot be read. A stated limit of the design.
use ::locus::locus;
locus! {
    struct Bag { items: Vec<u32> }
    impl Extend<u32> for Bag {
        fn extend<I: IntoIterator<Item = u32>>(&mut self, iter: I) {
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
