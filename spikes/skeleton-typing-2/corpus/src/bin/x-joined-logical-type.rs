//! Spike case for question 2, not a litmus example: `x-split-logical-type.rs`
//! with its three invocations joined into one.
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

    impl Point {
        logic fn origin() -> Point {
            Point { x: 0, y: 0 }
        }
    }

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
