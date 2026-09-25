use std::sync::atomic::{AtomicU8, Ordering};
static COUNT: AtomicU8 = AtomicU8::new(0);
pub fn tick() -> u8 { COUNT.fetch_add(1, Ordering::SeqCst) }
pub fn count() -> u8 { COUNT.load(Ordering::SeqCst) }
pub fn pair(value: u8) -> (u8, bool) { (value.wrapping_add(1), value != 0) }
pub fn fail() -> u8 { panic!("native failure") }
pub fn borrow(value: &u8) -> u8 { *value }
pub fn generic<T>(value: T) -> T { value }
pub async fn future() -> u8 { 4 }
pub unsafe fn dangerous() -> u8 { 4 }
pub struct Token;
impl Token { pub fn new() -> Self { Self } pub fn amount(&self) -> u8 { 4 } }
pub trait Surface { type Item; const LIMIT: u8; fn read(&self) -> u8; }
pub enum Event { One(u8), None }
pub union Raw { pub n: u8, pub b: bool }
pub type Alias = Token;
pub const LIMIT: u8 = 9;
#[macro_export]
macro_rules! identity { ($x:expr) => { $x }; }
#[cfg(feature = "extra")]
pub fn extra() -> u8 { 9 }
#[cfg(target_pointer_width = "64")]
pub fn wide() -> u8 { 64 }
#[cfg(doc)]
pub fn doc_only() -> u8 { 7 }
#[cfg(doc)]
pub fn changed(value: u8) -> u8 { value }
#[cfg(not(doc))]
pub fn changed(value: bool) -> bool { value }
#[doc(hidden)]
pub fn hidden_docs() -> u8 { 3 }
mod private { pub fn echo(value: u8) -> u8 { value } }
pub use private::echo;
pub mod api { pub use super::{echo, pair}; }

// Rust permits distinct type, value and macro namespaces with one spelling.
#[allow(non_camel_case_types)]
pub struct shared { pub value: u8 }
pub fn shared(n:u8)->u8 {n}
#[macro_export]
macro_rules! shared { ($e:expr) => {$e}; }
