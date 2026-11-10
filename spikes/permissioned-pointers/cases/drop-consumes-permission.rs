// `drop` has only `&mut self`, so it cannot give the permission to `free`.
use permissioned_pointers_spike::perm::{self, Perm, Ptr};

pub struct B {
    ptr: Ptr<u32>,
    perm: Perm<u32>,
}

impl Drop for B {
    fn drop(&mut self) {
        let _ = perm::take(self.ptr, &mut self.perm, 0);
        perm::free(self.ptr, self.perm, 1);
    }
}
