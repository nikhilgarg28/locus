//! expect: runtime=ok check=ok
//! prints: run() => (true, false, true)
//! A runtime function that returns `bool`, whose contract ties the result
//! to a proposition. Three spellings, with the default's conversion of a
//! `Bool` in force: an iff connective, two implications, and a library
//! function from `Prop` to `Bool` (`decide`).
use ::locus::locus;
locus! {
    fn is_zero_iff(x: u32) -> (r: bool, @(r <==> x == 0)) { (x == 0, _) }
    fn is_zero_implications(x: u32) -> (r: bool, @((r ==> x == 0) && (x == 0 ==> r))) { (x == 0, _) }
    fn is_zero_decide(x: u32) -> (r: bool, @(r === decide(x == 0))) { (x == 0, _) }
    fn run() -> (bool, bool, bool) {
        (is_zero_iff(0).0, is_zero_implications(3).0, is_zero_decide(0).0)
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
