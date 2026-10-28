//~ question: Is `prove!` written inside `stringify!` only tokens?
//~ spec: compat.names.macros-expand
//~ rfc: 0001

fn shown() -> &'static str {
    stringify!(::__locus::proof!([&x] [] x > 0))
}

//~ run: shown() => "prove!(x > 0)"
fn main() { println!("shown() = {:?}", shown()); }
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// Option (c): lowering rewrote the invocation inside stringify!'s input.
// expect: check=ok run=ok
