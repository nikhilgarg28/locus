// expect: E0277. A closure has no model: a generic that asks for one refuses it, and one that does not takes it.
include!("../lib.rs");
fn apply<F: Fn(u32) -> u32>(f: F) -> u32 { f(1) }
fn apply_read<F: Fn(u32) -> u32 + Model>(f: F) -> u32 { f(1) }
fn main() { let k = 2; apply(|x| x + k); apply_read(|x| x + k); }
