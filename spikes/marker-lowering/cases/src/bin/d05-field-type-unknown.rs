//! expect: runtime=ok check=E0425
//! A field of a logical type whose type does not exist. The marker has no
//! fields, so only the checking expansion, which names the types of the
//! fields in a probe, has the error.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    struct Rational { num: Int, den: Integer }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
