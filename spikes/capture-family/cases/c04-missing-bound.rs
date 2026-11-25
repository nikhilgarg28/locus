// expect: E0277. The message is the library's, and rustc suggests the bound itself.
include!("../lib.rs");
fn g<T>(x: &T) { let _ = capture(x); }
fn main() { g(&1u32); }
