//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Nat", "locus::BoundedU8")
//! "Where a snapshot is expected, it stays a snapshot: as the argument of
//! a parameter of type `Snap<T>`, under an annotation of that type" (RFC
//! 0002). The skeleton reads a place as its model, whatever is expected.
use ::locus::locus;

locus! {
    logic fn length(s: Snap<Vec<u32>>) -> Nat { s.len() }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(v: &Vec<u32>, b: u8) -> (&'static str, &'static str) {
        let n = logic! { length(*v) };
        let s: Snap<u8> = logic! { b };
        (type_of(&n), type_of(&s))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(&vec![1], 2));
}
