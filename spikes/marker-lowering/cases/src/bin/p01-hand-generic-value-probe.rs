//! expect: runtime=E0283 check=E0283
//! Plain Rust, written by hand: why a name that logical code calls is probed
//! by `use name as _;`. RFC 0001 probes a stub as a value and writes every
//! type argument as the marker. With a type for each logical type there is
//! no marker to write for `T: OrderSpec`, and rustc will not leave it open.
use ::locus::{Bool, Prop, probe, stub};

trait OrderSpec {
    fn le(a: Self, b: Self) -> Bool
    where
        Self: Sized;
}

fn is_max<T: OrderSpec>(_m: T, _a: T, _b: T) -> Prop {
    stub()
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    probe(is_max);
}
