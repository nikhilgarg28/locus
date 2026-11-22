//! expect: runtime=ok check=ok
//! prints: forged => ("locus::Nat", "locus::Seq<locus::Nat>", "locus::Nat")
//! The same with `unsafe impl`: rustc accepts it. A forged model, and a
//! forged one made through a macro of this crate's own. The lowered
//! library's generic function then reads `Mine` as the forger said. The
//! driver's listing is what shows these two were not written by lowering.
struct Mine {
    #[allow(dead_code)]
    n: u32,
}
unsafe impl locus::Model for Mine {
    type Snap = locus::Nat;
    type Logic = locus::Seq<locus::Nat>;
    fn __logic_model(_: locus::Nat) -> locus::Seq<locus::Nat> {
        locus::stub()
    }
    fn __logic_valid(_: locus::Nat) -> locus::Prop {
        locus::True
    }
}
/// A macro of the forger's own, named as Locus's is.
macro_rules! locus {
    ($t:ident) => {
        unsafe impl locus::Model for $t {
            type Snap = locus::Nat;
            type Logic = locus::Nat;
            fn __logic_model(s: locus::Nat) -> locus::Nat {
                s
            }
            fn __logic_valid(_: locus::Nat) -> locus::Prop {
                locus::True
            }
        }
    };
}
struct Other;
locus!(Other);
fn type_of<T>(_: &T) -> &'static str {
    std::any::type_name::<T>()
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    let forged = (
        type_of(&two_lib::keep(&Mine { n: 1 })),
        type_of(&two_lib::read(&Mine { n: 1 })),
        type_of(&two_lib::keep(&Other)),
    );
    println!("forged => {forged:?}");
}
