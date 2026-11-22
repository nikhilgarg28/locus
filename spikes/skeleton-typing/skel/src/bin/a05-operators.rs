//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Bool", "locus::Prop", "locus::Int", "locus::Erased")
//! Operators in logical code: arithmetic, comparisons, `==`, `===`, `&&`,
//! `||`, `!`, `==>`. A comparison gives a `Bool`, `===` gives a `Prop`,
//! `-` on two `Nat`s gives an `Int`, and a `Bool` stands where a
//! proposition is wanted.
use ::locus::locus;

locus! {
    logic fn ops(a: Int, b: Int, n: Nat, m: Nat, p: Bool, q: Bool) -> Prop {
        a + b * a - b / a % b == a && n + m > m ==> !(p || q) && a === b
    }
    logic fn both(p: Bool, q: Bool) -> Bool { p && !q || p != q }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(x: u32, y: u32, flag: bool) -> (&'static str, &'static str, &'static str, &'static str) {
        let b = logic! { x + 1 > y && flag };
        let p = prop!(x + 1 > x && (x == y || x != y) ==> x <= x + y);
        let d = logic! { x - y };
        let h = prove!(x === x && flag ==> x >= 0);
        let holds: @(flag || !flag) = _;
        (type_of(&b), type_of(&p), type_of(&d), type_of(&h))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(1, 2, true));
}
