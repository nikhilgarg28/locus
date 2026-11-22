//! expect: runtime=reject check=reject
//! A deliberate type error in logical code: `+` on a logical type that
//! does not implement `AddLogic`, and `<` on one with no order.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    fn run(r: Rational) -> u32 {
        let sum = logic! { r + r };
        let less = prove!(r < r);
        1
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
