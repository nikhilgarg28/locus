//! expect: runtime=E0277 check=E0277
//! Plain Rust, written by hand: what r02b's arrangement costs. With the
//! trait's argument written as a stub in a bound, a logical value meets
//! `IntoLogic<Int>` only if the marker has it at the stub. The library's
//! blanket conversion gives the marker `IntoLogic<U>` for a `U` that is
//! `FromLogic` of the marker, and the stub of `Int` is `FromLogic` of the
//! stub of `Nat`.
use ::locus::{__locus_Int, IntoLogic, LogicalData};

fn to_int<T: IntoLogic<__locus_Int>>(_t: T) -> u32 {
    1
}

fn run(n: LogicalData) -> u32 {
    // `n` is a `Nat` in the source.
    to_int(n)
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(LogicalData));
}
