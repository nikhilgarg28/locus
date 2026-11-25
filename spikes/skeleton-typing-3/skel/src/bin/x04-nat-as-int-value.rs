//! expect: runtime=ok check=ok
//! prints: run() => "locus::Int"
//! The same where the `Nat` is not a place but the result of a call: there
//! is no stub around it to say that an `Int` may be wanted.
use ::locus::locus;

locus! {
    logic fn negate(i: Int) -> Int { 0 - i }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(v: &Vec<u32>) -> &'static str {
        let i = logic! { negate(v.len()) };
        type_of(&i)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(&vec![1]));
}
