//! expect: runtime=E0277 check=E0277
//! `Bool: IntoLogic<Rational>` does not hold: there is no
//! `impl FromLogic<Bool> for Rational`. rustc says so.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    impl FromLogic<Int> for Rational {
        logic fn from(t: Int) -> Rational { Rational { num: t, den: 1 } }
    }

    fn widened<T: IntoLogic<Rational>>(t: T) -> u32 { 2 }

    fn run(b: Bool) -> u32 {
        widened(b)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
