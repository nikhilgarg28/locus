//! expect: runtime=annotate check=annotate
//! A ghost local passed only to a generic function, as in g06, but the
//! bound has exactly one implementation in the program. By the draft, every
//! use is generic, so the program writes the type.
use ::locus::locus;

locus! {
    trait Monoid: Logical {
        logic fn unit() -> Self;
    }
    impl Monoid for Int {
        logic fn unit() -> Int { 0 }
    }

    fn only<M: Monoid>(m: M) -> &'static str { std::any::type_name::<M>() }

    fn run(v: &Vec<u32>) -> &'static str {
        let before = logic! { v.len() };
        only(before)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
