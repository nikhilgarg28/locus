//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Nat", "locus::Nat", "locus::Nat", "locus::Seq<locus::BoundedU32>")
//! P4: a runtime place as an argument. To a parameter declared with a
//! runtime type it is the place itself; to one of a logical type, its
//! model; to one whose type is a type parameter the program writes
//! `model(x)` or `snap(x)` (see p4b, and probes2/q2.rs for an encoding in
//! which nothing is written).
use ::locus::locus;
locus! {
    struct Counter { hits: u32 }
    logic fn hits_of(c: &Counter) -> Nat { c.hits }
    logic fn double(n: Nat) -> Nat { n + n }
    logic fn same<T: Logical>(t: T) -> T { t }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str, &'static str) {
        let c = Counter { hits: 3 };
        let r = &c;
        let n = 4u32;
        let v = vec![1u32];
        let itself = logic! { hits_of(r) };
        let its_model = logic! { double(n) };
        let written_model = logic! { same(model(n)) };
        let written_snap = logic! { same(snap(v)) };
        (type_of(&itself), type_of(&its_model), type_of(&written_model), type_of(&written_snap))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
