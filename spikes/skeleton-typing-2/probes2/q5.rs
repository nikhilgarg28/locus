// Probe q5: making rustc's message for an omitted conversion say what to
// write. (a) An operator with ONE implementation for its left operand is
// E0308 on the right operand, and no library text is shown; with a second,
// dummy implementation it is E0277, with the library's text. (b) A literal
// argument whose type is fixed by a bound.
#![allow(dead_code, unused_variables)]
fn stub<T>() -> T { panic!() }
#[derive(Clone, Copy)] pub struct Nat;
#[derive(Clone, Copy)] pub struct Int;
#[doc(hidden)] #[derive(Clone, Copy)] pub struct NoSuchOperand;
#[diagnostic::on_unimplemented(
    message = "`+` is not defined on a `{Self}` and a `{Rhs}` in logical code",
    label = "this is a `{Self}`, and the other side is a `{Rhs}`",
    note = "a `Nat` is added to an `Int` after it is made one: write `as Int` after the `Nat`")]
pub trait AddLogic<Rhs = Self> { type Output; }
impl AddLogic for Int { type Output = Int; }
impl AddLogic for Nat { type Output = Nat; }
#[cfg(dummy)] impl AddLogic<NoSuchOperand> for Int { type Output = Int; }
#[cfg(dummy)] impl AddLogic<NoSuchOperand> for Nat { type Output = Nat; }
pub fn add<A: AddLogic<B>, B>(_: A, _: B) -> A::Output { stub() }

#[diagnostic::on_unimplemented(
    message = "an integer literal with nothing beside it is an `Int`, and a `{P}` is wanted here",
    label = "this literal is an `Int`",
    note = "write the literal's type after it: `0 as Nat`")]
pub trait LiteralGivenAs<P> {}
impl LiteralGivenAs<Int> for Int {}
pub fn literal_argument<P>(_: u128) -> P where Int: LiteralGivenAs<P> { stub() }
fn double(n: Nat) -> Nat { n }
fn same<T>(t: T) -> T { t }
fn takes_nat(_: Nat) {}
fn user(n: Nat, i: Int) {
    let _ = || {
        #[cfg(op)] let s = add(i, n);
        #[cfg(lit)] let d = double(literal_argument(3));
        let x = same(literal_argument(0));
        #[cfg(fwd)] takes_nat(x);
    };
}
fn main() {}
