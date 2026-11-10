//! Shows that the checked build notices a broken `requires`, so that the
//! pass in `collections.rs` means something.
#![cfg(checked)]

use permissioned_pointers_spike::perm;

#[test]
#[should_panic(expected = "slot is in the wrong state")]
fn take_twice() {
    let (p, mut perm) = perm::alloc::<String>(1);
    perm::write(p, &mut perm, 0, String::from("x"));
    let _a = perm::take(p, &mut perm, 0);
    let _b = perm::take(p, &mut perm, 0);
}

#[test]
#[should_panic(expected = "slot is not initialized")]
fn read_uninitialized() {
    let (p, perm) = perm::alloc::<u32>(2);
    let _ = perm::get(p, &perm, 1);
}

#[test]
#[should_panic(expected = "slot out of the block")]
fn write_past_the_end() {
    let (p, mut perm) = perm::alloc::<u32>(2);
    perm::write(p, &mut perm, 2, 5);
}

#[test]
#[should_panic(expected = "the permission is for another address")]
fn wrong_permission() {
    let (p, _perm_p) = perm::alloc::<u32>(1);
    let (_q, mut perm_q) = perm::alloc::<u32>(1);
    perm::write(p, &mut perm_q, 0, 5);
}

#[test]
#[should_panic(expected = "freeing a block that still holds values")]
fn free_with_values() {
    let (p, mut perm) = perm::alloc::<String>(1);
    perm::write(p, &mut perm, 0, String::from("x"));
    perm::free(p, perm, 1);
}

#[test]
#[should_panic(expected = "the permission holds no block")]
fn use_after_free_in_place() {
    let (p, mut perm) = perm::alloc::<u32>(1);
    perm::free_in_place(p, &mut perm, 1);
    perm::write(p, &mut perm, 0, 5);
}
