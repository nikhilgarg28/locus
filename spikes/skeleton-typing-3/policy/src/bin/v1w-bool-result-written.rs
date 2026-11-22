//! expect: runtime=ok check=ok
//! prints: run() => (true, false, true)
//! The same with every `Bool` written as a proposition, `r === true`: what
//! the strict variant (feature p3-v2) asks for. `decide` needs nothing.
use ::locus::locus;
locus! {
    fn is_zero_iff(x: u32) -> (r: bool, @(r === true <==> x == 0)) { (x == 0, _) }
    fn is_zero_implications(x: u32) -> (r: bool, @((r === true ==> x == 0) && (x == 0 ==> r === true))) { (x == 0, _) }
    fn is_zero_decide(x: u32) -> (r: bool, @(r === decide(x == 0))) { (x == 0, _) }
    fn run() -> (bool, bool, bool) {
        (is_zero_iff(0).0, is_zero_implications(3).0, is_zero_decide(0).0)
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
