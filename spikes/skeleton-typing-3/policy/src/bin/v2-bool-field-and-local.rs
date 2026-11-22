//! expect: runtime=ok check=ok
//! prints: run() => 7
//! A runtime `bool` field and a runtime `bool` local used in contracts.
//! Each is a `Bool` where a proposition is wanted: accepted by default,
//! and written `=== true` under the strict variant.
use ::locus::locus;
locus! {
    struct Gate { open: bool, width: u32 }
    fn pass(g: &Gate, n: u32, allowed: @(g.open), fits: @(g.open && n <= g.width)) -> u32 { n }
    fn run() -> u32 {
        let g = Gate { open: true, width: 9 };
        let ready = g.open;
        let checked: @(ready ==> g.open) = _;
        let both: @(!ready || g.width > 0) = _;
        pass(&g, 7, _, _)
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
