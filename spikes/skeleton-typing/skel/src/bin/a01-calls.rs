//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Nat", "locus::Nat", "locus::Nat", "locus::Int")
//! A call of a logic function, by its name and by a path; a method call on
//! a logical value; and a method call on a runtime value, which is on its
//! model. Each ghost local has the type of its logical code.
use ::locus::locus;

locus! {
    logic fn double(n: Nat) -> Nat { n + n }
    logic fn first<T>(s: Seq<T>) -> T { s[0] }
    mod geometry {
        pub logic fn area(side: Int) -> Int { side * side }
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(v: &Vec<u32>, s: Seq<Nat>, i: Int) -> (&'static str, &'static str, &'static str, &'static str) {
        let a = logic! { double(v.len()) };
        let b = logic! { s.len() };
        let c = logic! { first(s.skip(1)) };
        let d = logic! { geometry::area(i) };
        (type_of(&a), type_of(&b), type_of(&c), type_of(&d))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(&vec![1], ::locus::ghost(), ::locus::ghost()));
}
