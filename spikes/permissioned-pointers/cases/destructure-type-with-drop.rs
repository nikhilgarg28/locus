// A type with a destructor cannot be taken apart.
use permissioned_pointers_spike::perm::{self, Perm, Ptr};

pub struct B {
    ptr: Ptr<u32>,
    perm: Perm<u32>,
}

impl Drop for B {
    fn drop(&mut self) {
        let _ = perm::take(self.ptr, &mut self.perm, 0);
        perm::free_in_place(self.ptr, &mut self.perm, 1);
    }
}

pub fn into_inner(b: B) -> u32 {
    let B { ptr, mut perm } = b;
    let v = perm::take(ptr, &mut perm, 0);
    perm::free(ptr, perm, 1);
    v
}
