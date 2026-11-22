//! expect: runtime=ok check=ok
//! prints: run() => 1
//! t17 with `#![spike(logic_members_in_trait)]`.
use ::locus::locus;

locus! {
    #![spike(logic_members_in_trait)]
    trait Named {
        logic fn tag() -> Nat { 0 }
    }
}

locus! {
    #![spike(logic_members_in_trait)]
    struct Thing;

    impl Named for Thing {}

    fn run() -> u32 {
        let t = logic! { Thing::tag() };
        1
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
