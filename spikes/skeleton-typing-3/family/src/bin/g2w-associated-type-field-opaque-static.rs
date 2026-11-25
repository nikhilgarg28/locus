//! expect: runtime=ok check=ok
//! `litmus/models/snapshot-with-associated-type-field.accept.lc`, the other
//! way: the field marked `#[opaque]` and no bound on `kept`. The snapshot of
//! the field is `Opaque<F::Out>`, which keeps whatever lifetimes `F::Out`
//! has, so rustc asks for `F::Out: 'static` in its place. (g2w writes it.)
use ::locus::locus;
locus! {
    trait Family { type Out; }
    struct Holder<F: Family> { id: u32, #[opaque] item: F::Out }
    fn kept<F: Family>(h: Holder<F>) -> Holder<F> where F::Out: 'static {
        let before = logic! { snap(h) };
        let moved = h;
        let same: @(snap(moved) === before) = _;
        moved
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
