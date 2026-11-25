//! expect: runtime=error check=error
//! A hand-written implementation of `Model` inside `locus!`, in a function's
//! body, where lowering lowers no item. The trait's members have names with
//! a prefix that is reserved for what lowering writes, so the source cannot
//! write them: lowering refuses the identifier.
use ::locus::locus;
locus! {
    fn run() -> u32 {
        struct Mine;
        unsafe impl ::locus::Model for Mine {
            type Snap = ::locus::Nat;
            type Logic = ::locus::Nat;
            fn __logic_model(s: ::locus::Nat) -> ::locus::Nat { s }
            fn __logic_valid(s: ::locus::Nat) -> ::locus::Prop { ::locus::True }
        }
        1
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
