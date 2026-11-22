//! expect: runtime=ok check=ok
//! prints: run() => (3, 9, "locus::Nat", "d4_reference_snapshot_after_borrow::Holder_Snap<locus::Nat, locus::Seq<locus::Nat>>")
//! Required case 4: a snapshot taken through a `&mut`, and of a struct that
//! holds one, is used after the borrow has ended and the place is written.
//! In generic code too.
use ::locus::locus;
locus! {
    struct Holder<'a> { r: &'a mut u32, all: &'a [u32] }
    logic fn takes_nat(n: Nat) -> Nat { n }
    fn through<T: Model>(r: &mut T, next: T) -> Snap<T> {
        let old = logic! { snap(r) };
        *r = next;
        old
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (u32, u32, &'static str, &'static str) {
        let mut a = 1u32;
        let r = &mut a;
        let old = logic! { snap(r) };
        *r = 2;
        a = 3;
        let still = logic! { takes_nat(old) };
        let mut b = 7u32;
        let list = vec![1u32, 2];
        let h = Holder { r: &mut b, all: &list };
        let held = logic! { snap(h) };
        *h.r = 8;
        b = 9;
        drop(list);
        let after = logic! { takes_nat(held.r) + held.all.len() };
        let mut c = 1u8;
        let generic = through(&mut c, 2);
        c = 3;
        let _ = logic! { takes_nat(generic) };
        (a, b, type_of(&still), type_of(&held))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
