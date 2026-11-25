//! expect: runtime=ok check=ok
//! prints: run() => "locus::Nat"
//! "A logic function declared on the runtime type itself is found first,
//! then those of its model's type" (RFC 0002). The skeleton takes a method
//! call's receiver to its model before rustc looks for the method.
use ::locus::locus;

locus! {
    struct Stack { items: Vec<u32>, limit: u32 }
    impl Stack {
        logic fn room(self) -> Nat { self.limit }
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(s: &Stack) -> &'static str {
        let by_path = logic! { Stack::room(*s) };
        let by_method = logic! { s.room() };
        type_of(&by_method)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(&Stack { items: vec![], limit: 3 }));
}
