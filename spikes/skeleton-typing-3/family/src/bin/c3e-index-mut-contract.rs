//! expect: runtime=E0053 check=E0053
//! `IndexMut` cannot carry the contract of `get_mut`: the trait fixes the
//! method's signature, so a proof in the result is not the trait's method.
use ::locus::locus;
locus! {
    use ::locus::Vec;
    struct Row { cells: Vec<u32> }
    impl std::ops::Index<usize> for Row {
        type Output = u32;
        fn index(&self, i: usize) -> &u32 { &self.cells[i] }
    }
    impl std::ops::IndexMut<usize> for Row {
        fn index_mut(&mut self, i: usize)
            -> (r: &mut u32, @(snap(final!(self).cells).elements() === snap(self.cells).elements().set(i, snap(final!(r)))))
        {
            (&mut self.cells[i], _)
        }
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
