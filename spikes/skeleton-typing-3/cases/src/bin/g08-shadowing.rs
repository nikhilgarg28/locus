//! expect: runtime=ok check=ok
//! prints: run(5) => (12, 12, 1, 7, 6, 11, 15)
//! Shadowing. In each function, does `locus!` find, from the tokens alone,
//! whether runtime code mentions the ghost local, and not another binding
//! of the same name?
use ::locus::locus;

locus! {
    fn takes_nat(n: Nat) -> u32 { 1 }

    // A runtime local of the same name, later in the block. Only `prove!`
    // mentions the ghost `x`: it has no type for rustc.
    fn later(n: u32) -> u32 {
        let x = logic! { n };
        let p = prove!(x == n);
        let x = n + 1;
        x * 2
    }

    // The same in an inner block, and the ghost local read again after it.
    fn inner(n: u32) -> u32 {
        let x = logic! { n };
        let doubled = {
            let x = n + 1;
            x * 2
        };
        let p = prove!(x == n);
        doubled
    }

    // The other way round: a ghost local shadows a runtime one, and runtime
    // code passes it on. Its type is inferred.
    fn ghost_over_runtime(n: u32) -> u32 {
        let x = n;
        let x = logic! { x };
        takes_nat(x)
    }

    // A closure's parameter of the same name.
    fn closure_parameter(n: u32) -> u32 {
        let x = logic! { n };
        let add = |x: u32| x + 2;
        let p = prove!(x == n);
        add(n)
    }

    // A binding of the same name in a `match` arm and in an `if let`.
    fn patterns(n: u32) -> u32 {
        let x = logic! { n };
        let from_match = match Some(n) {
            Some(x) => x,
            None => 0,
        };
        let from_if = if let Some(x) = Some(1u32) { x } else { 0 };
        let p = prove!(x == n);
        from_match + from_if
    }

    // A struct pattern with a field of the same name: `x:` is a label, not
    // a binding, and the arm passes the ghost `x` to runtime code.
    struct Point { x: u32 }
    fn field_label(n: u32, pt: Point) -> u32 {
        let x = logic! { n };
        match pt {
            Point { x: here } => here + takes_nat(x),
        }
    }

    // A binding of the same name by `for` and by `while let`.
    fn loops(n: u32) -> u32 {
        let x = logic! { n };
        let mut total = 0;
        for x in 0..n {
            total += x;
        }
        let mut left = Some(n);
        while let Some(x) = left {
            total += x;
            left = None;
        }
        let p = prove!(x == n);
        total
    }

    fn run(n: u32) -> (u32, u32, u32, u32, u32, u32, u32) {
        (later(n), inner(n), ghost_over_runtime(n), closure_parameter(n), patterns(n), field_label(n, Point { x: 10 }), loops(n))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(5) => {:?}", run(5));
}
