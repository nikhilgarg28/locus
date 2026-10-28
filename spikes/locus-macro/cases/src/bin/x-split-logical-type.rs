//! Spike case for question 2, not a litmus example: the earlier spike's case 11
//! (a logical type, a logic function in an `impl` of it, and logical code that
//! calls it), split across three `locus!` invocations in one module. A plain
//! logic function defined in one invocation and called in another is beside it.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    struct Point {
        x: Int,
        y: Int,
    }

    logic fn double(n: Nat) -> Nat {
        n + n
    }
}

locus! {
    impl Point {
        logic fn origin() -> Point {
            Point { x: 0, y: 0 }
        }
    }
}

locus! {
    fn use_them() -> u32 {
        let d = logic! { double(3) };
        let o: Point = logic! { Point::origin() };
        1
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("use_them() => {:?}", use_them());
}
