//! expect: runtime=reject check=reject
//! P4: a runtime place given to a parameter whose type is a type
//! parameter, with nothing written. The model should win; in this encoding
//! rustc finds the place itself as good, and asks.
use ::locus::locus;
locus! {
    logic fn same<T: Logical>(t: T) -> T { t }
    fn run() -> u32 {
        let n = 4u32;
        let ambiguous = logic! { same(n) };
        1
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
