//! expect: runtime=ok check=ok
//! prints: run() => ("locus::BoundedU32", "locus::Seq<locus::BoundedU8>", "n1_snapshot_bounds_inserted::__locus_snap_Keep<locus::Bool>", 1)
//! Every runtime type has a snapshot (RFC 0002), and no program writes a
//! bound for it. Lowering adds `Snapshot` to the type parameters of what
//! writes `Snap<..>` or `snap(..)`: a function, a type, an `impl`, and a
//! function that only passes its parameter on to one of those.
use ::locus::locus;
locus! {
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn in_a_signature<T>(x: T, s: Snap<T>) -> &'static str { type_of(&s) }
    fn in_a_body<T>(x: T) -> &'static str { type_of(&logic! { snap(x) }) }
    fn passes_on<T>(x: Vec<T>) -> &'static str { in_a_body(x) }
    struct Keep<T> { now: T, before: Snap<T> }
    impl<T> Keep<T> {
        fn before(&self) -> &'static str { type_of(&logic! { snap(*self) }) }
    }
    fn run() -> (&'static str, &'static str, &'static str, u32) {
        let k = Keep { now: true, before: logic! { true } };
        (in_a_signature(1u32, logic! { snap(1u32) }), passes_on(vec![1u8]), k.before(), 1)
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
