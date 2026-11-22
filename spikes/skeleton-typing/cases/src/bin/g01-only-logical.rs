//! expect: runtime=ok check=ok
//! prints: count(&vec![4, 5, 6]) => 3
//! A ghost local that only later logical code mentions. rustc has nothing to
//! infer its type from, so by the draft it holds a marker with no logical
//! type. (In the first spike, with one typed binding, this was E0283.)
use ::locus::locus;

locus! {
    fn count(v: &Vec<u32>) -> usize {
        let before = logic! { v.len() };
        let n = v.len();
        let same: @(before == n) = _;
        let again = prove!(before == v.len());
        n
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("count(&vec![4, 5, 6]) => {:?}", count(&vec![4, 5, 6]));
}
