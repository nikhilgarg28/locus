//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Nat", "locus::Nat", "locus::Seq<locus::Nat>", "locus::Nat")
//! Required case 1: a generic function returns a snapshot (and a model),
//! and it is used where the concrete type is wanted.
use ::locus::locus;
locus! {
    fn keep<T: Model>(x: &T) -> Snap<T> { logic! { snap(x) } }
    fn read<T: Model>(x: &T) -> T::Logic { logic! { *x } }
    logic fn takes_nat(n: Nat) -> Nat { n }
    logic fn takes_seq(xs: Seq<Nat>) -> Nat { xs.len() }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str, &'static str) {
        let k = keep(&5u32);
        let five = 5u64;
        let direct = logic! { snap(five) };
        let boxed = keep(&Box::new([1u8, 2, 3]));
        let a = logic! { takes_nat(k) + takes_nat(direct) + takes_seq(boxed) };
        let same = logic! { k === direct };
        (type_of(&k), type_of(&read(&7usize)), type_of(&boxed), type_of(&a))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
