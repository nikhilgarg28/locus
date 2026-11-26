// Type-checking skeleton only. A compiler-internal marker's type is captured
// before the call; using the marker later neither reads nor borrows the parent.
// Logical loan identity, proof validity and data-escape checks are NOT encoded
// by this Rust marker. Production erasure removes the marker/proof expressions.
use std::marker::PhantomData;
struct FinalToken<S>(PhantomData<fn() -> S>);
struct Proof;
trait Snapshot {
    type Snap;
}
impl Snapshot for [i32; 2] {
    type Snap = (i32, i32);
}
fn future_token<T: Snapshot>(_: &T) -> FinalToken<T::Snap> {
    FinalToken(PhantomData)
}
fn proof_about_final<S>(_: &FinalToken<S>) -> Proof {
    Proof
}
fn main() {
    let mut values = [1, 2];
    let r = &mut values;
    let token = future_token(&*r); // generated before acquisition, not a snapshot
    let x = &mut r[0];
    let _proof = proof_about_final(&token);
    *x = 3;
    assert_eq!(values, [3, 2]);
}
