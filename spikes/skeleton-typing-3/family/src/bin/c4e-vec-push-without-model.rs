//! expect: runtime=E0277 check=E0277
//! An element type with no model: the wrapper's `push` asks for `T: Model`,
//! and rustc reports that bound. (With no `DerefMut` the standard `push` could
//! not be reached either.)
use ::locus::locus;
locus! {
    use ::locus::Vec;
    fn run() -> usize {
        let k = 1u32;
        let mut closures = Vec::new();
        closures.push(move |x: u32| x + k);
        closures.iter().count()
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
