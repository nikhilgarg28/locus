// Compare caller code generation with an unknown Rust callee that may unwind.
// These are assembly observations, not a benchmark or a universal cost claim.
unsafe extern "Rust" {
    fn acquisition(p: &mut i32) -> &mut i32;
}
struct Guard(bool);
impl Drop for Guard {
    fn drop(&mut self) {
        if self.0 && std::thread::panicking() {
            std::process::abort();
        }
    }
}
#[unsafe(no_mangle)]
pub fn unguarded(p: &mut i32) -> &mut i32 {
    unsafe { acquisition(p) }
}
#[unsafe(no_mangle)]
pub fn guarded(p: &mut i32) -> &mut i32 {
    let mut guard = Guard(true);
    let out = unsafe { acquisition(p) };
    guard.0 = false;
    out
}
