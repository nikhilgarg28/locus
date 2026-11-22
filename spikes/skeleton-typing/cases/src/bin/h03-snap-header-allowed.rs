//! expect: runtime=ok check=ok
//! prints: <Snap<bool> as Describe>::Width is locus::Nat
//! prints: <Snap<&mut Vec<String>> as Describe>::Width is locus::Nat
//! prints: from a proof of 2 == 2: written for a proof of 1 == 2
//! h01 and h02 with the rejection turned off, to show why the draft has the
//! rule. rustc selects the implementation written for `Snap<u32>` at every
//! snapshot type, and the one written for a proof of `1 == 2` at every proof.
use ::locus::locus;

locus! {
    #![spike(allow_ghost_headers)]

    trait Describe: Logical {
        type Width: Logical;
    }

    impl Describe for Snap<u32> {
        type Width = Nat;
    }

    struct Absurd(&'static str);

    impl From<Option<@(1 == 2)>> for Absurd {
        fn from(_: Option<@(1 == 2)>) -> Absurd { Absurd("written for a proof of 1 == 2") }
    }

    fn from_truth() -> &'static str {
        let fine: @(2 == 2) = _;
        Absurd::from(Some(fine)).0
    }
}

fn main() {
    use std::any::type_name;
    println!("expansion: {}", ::locus::__expansion!());
    println!(
        "<Snap<bool> as Describe>::Width is {}",
        type_name::<<::locus::Snap<bool> as Describe>::Width>()
    );
    println!(
        "<Snap<&mut Vec<String>> as Describe>::Width is {}",
        type_name::<<::locus::Snap<&mut Vec<String>> as Describe>::Width>()
    );
    println!("from a proof of 2 == 2: {}", from_truth());
}
