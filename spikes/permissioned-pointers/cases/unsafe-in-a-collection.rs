// The collections forbid `unsafe`, as this module does.
#![forbid(unsafe_code)]

pub fn f(p: *const u32) -> u32 {
    unsafe { *p }
}
