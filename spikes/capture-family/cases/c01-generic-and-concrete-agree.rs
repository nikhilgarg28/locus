// expect: ok. Generic and concrete code agree; references and `Box` are transparent; no borrow is kept.
include!("../lib.rs");
fn keep<T: Model>(x: &T) -> T::Logic { capture(x) }
fn keep_snap<T: Model>(x: &T) -> T::Snap { snap(x) }
fn main() {
    takes_nat(keep(&5u32));                 // a generic result used where the concrete type is wanted
    takes_nat(keep_snap(&5u64));
    let b = Box::new(7u32); takes_nat(capture(&b));
    let xs: &[u32] = &[1, 2, 3]; takes_seq_nat(capture(&xs)); takes_nat(capture(&xs).len());
    let mut a = 1u32;
    let r = &mut a;
    let old = snap(&r);                     // snapshot through a `&mut`
    *r = 2;
    a = 3;                                  // the borrow has ended; `old` is still used below
    takes_nat(old);
    println!("{} {}", name(&keep(&5u32)), a);
}
