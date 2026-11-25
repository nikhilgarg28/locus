//! expect: runtime=reject check=reject
//! A deliberate type error in logical code: a field that the logical
//! struct does not have, on a ghost local and on the result of a call.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    struct Rational { num: Int, den: Int }
    logic fn twice(r: Rational) -> Rational { Rational { num: r.num * 2, den: r.den } }

    fn run(r: Rational) -> u32 {
        let on_a_local = logic! { r.nom };
        let on_a_call = logic! { twice(r).denominator };
        1
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
