//~ question: Is `prove!` written inside `stringify!` only tokens?
//~ spec: compat.names.macros-expand
//~ rfc: 0001

fn shown() -> &'static str {
    stringify!(prove!(x > 0))
}

//~ run: shown() => "prove!(x > 0)"
fn main() { println!("shown() = {:?}", shown()); }
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// expect: check=ok run=ok
